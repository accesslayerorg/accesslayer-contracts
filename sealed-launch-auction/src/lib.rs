#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Bytes, Env, Vec};

pub const MAX_BIDS: u32 = 32;
pub const MAX_COMMITMENT_BYTES: u32 = 256;
pub const SETTLEMENT_PAGE_SIZE: u32 = 16;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    Unauthorized = 1,
    NotFound = 2,
    Phase = 3,
    InvalidCommitment = 4,
    AlreadySettled = 5,
    TooManyItems = 6,
    InvalidCursor = 7,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bid {
    pub bidder: Address,
    pub commitment: Bytes,
    pub deposit: i128,
    pub price: i128,
    pub quantity: u32,
    pub revealed: bool,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Auction {
    pub creator: Address,
    pub key_id: Address,
    pub supply: u32,
    pub min_price: i128,
    pub bid_end: u32,
    pub reveal_end: u32,
    pub settled: bool,
    pub clearing_price: i128,
    pub filled: u32,
}
#[contracttype]
enum Key {
    Admin,
    Next,
    Auction(u32),
    Bids(u32),
    Settlement(u32),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementPage {
    pub cursor: u32,
    pub price: i128,
    pub filled: u32,
    pub complete: bool,
}
#[contract]
pub struct SealedLaunchAuction;
#[contractimpl]
impl SealedLaunchAuction {
    pub fn create_sealed_auction(
        e: Env,
        creator: Address,
        key_id: Address,
        supply: u32,
        min_price: i128,
        bid_ledgers: u32,
        reveal_ledgers: u32,
    ) -> Result<u32, Error> {
        creator.require_auth();
        if supply == 0 || min_price <= 0 {
            return Err(Error::Phase);
        }
        let id = e
            .storage()
            .persistent()
            .get::<Key, u32>(&Key::Next)
            .unwrap_or(0)
            + 1;
        e.storage().persistent().set(&Key::Next, &id);
        let now = e.ledger().sequence();
        e.storage().persistent().set(
            &Key::Auction(id),
            &Auction {
                creator,
                key_id,
                supply,
                min_price,
                bid_end: now + bid_ledgers,
                reveal_end: now + bid_ledgers + reveal_ledgers,
                settled: false,
                clearing_price: 0,
                filled: 0,
            },
        );
        e.storage()
            .persistent()
            .set(&Key::Bids(id), &Vec::<Bid>::new(&e));
        Ok(id)
    }
    pub fn bid(
        e: Env,
        bidder: Address,
        auction_id: u32,
        commitment: Bytes,
        deposit: i128,
    ) -> Result<(), Error> {
        bidder.require_auth();
        let a: Auction = e
            .storage()
            .persistent()
            .get(&Key::Auction(auction_id))
            .ok_or(Error::NotFound)?;
        if e.ledger().sequence() >= a.bid_end || deposit <= 0 {
            return Err(Error::Phase);
        }
        let mut bids = e
            .storage()
            .persistent()
            .get::<Key, Vec<Bid>>(&Key::Bids(auction_id))
            .unwrap_or_else(|| Vec::new(&e));
        if bids.len() >= MAX_BIDS || commitment.len() > MAX_COMMITMENT_BYTES {
            return Err(Error::TooManyItems);
        }
        bids.push_back(Bid {
            bidder,
            commitment,
            deposit,
            price: 0,
            quantity: 0,
            revealed: false,
        });
        e.storage().persistent().set(&Key::Bids(auction_id), &bids);
        Ok(())
    }
    pub fn reveal(
        e: Env,
        bidder: Address,
        auction_id: u32,
        price: i128,
        quantity: u32,
        salt: Bytes,
    ) -> Result<(), Error> {
        bidder.require_auth();
        let a: Auction = e
            .storage()
            .persistent()
            .get(&Key::Auction(auction_id))
            .ok_or(Error::NotFound)?;
        if e.ledger().sequence() < a.bid_end || e.ledger().sequence() >= a.reveal_end {
            return Err(Error::Phase);
        }
        let mut bids = e
            .storage()
            .persistent()
            .get::<Key, Vec<Bid>>(&Key::Bids(auction_id))
            .ok_or(Error::NotFound)?;
        if bids.len() > MAX_BIDS {
            return Err(Error::TooManyItems);
        }
        for i in 0..bids.len() {
            let mut b = bids.get(i).unwrap();
            if b.bidder == bidder && !b.revealed {
                reveal_bid(&e, &mut b, price, quantity, salt)?;
                bids.set(i, b);
                break;
            }
        }
        e.storage().persistent().set(&Key::Bids(auction_id), &bids);
        Ok(())
    }

    /// Reveals by zero-based submission index, including in legacy auctions.
    pub fn reveal_at(
        e: Env,
        bidder: Address,
        auction_id: u32,
        index: u32,
        price: i128,
        quantity: u32,
        salt: Bytes,
    ) -> Result<(), Error> {
        bidder.require_auth();
        let storage = e.storage().persistent();
        let auction: Auction = storage
            .get(&Key::Auction(auction_id))
            .ok_or(Error::NotFound)?;
        let ledger = e.ledger().sequence();
        if ledger < auction.bid_end || ledger >= auction.reveal_end {
            return Err(Error::Phase);
        }
        let key = Key::Bids(auction_id);
        let mut bids: Vec<Bid> = storage.get(&key).ok_or(Error::NotFound)?;
        let mut bid = bids.get(index).ok_or(Error::NotFound)?;
        if bid.bidder != bidder || bid.revealed {
            return Err(Error::InvalidCommitment);
        }
        reveal_bid(&e, &mut bid, price, quantity, salt)?;
        bids.set(index, bid);
        storage.set(&key, &bids);
        Ok(())
    }
    pub fn settle(e: Env, auction_id: u32) -> Result<i128, Error> {
        Ok(settle_page(&e, auction_id, 0, MAX_BIDS, true)?.price)
    }

    /// Pass zero to start, then pass each returned cursor until complete.
    pub fn settle_page(e: Env, auction_id: u32, cursor: u32) -> Result<SettlementPage, Error> {
        settle_page(&e, auction_id, cursor, SETTLEMENT_PAGE_SIZE, false)
    }
}

fn reveal_bid(
    env: &Env,
    bid: &mut Bid,
    price: i128,
    quantity: u32,
    mut salt: Bytes,
) -> Result<(), Error> {
    if salt.len().checked_add(20) != Some(bid.commitment.len()) {
        return Err(Error::InvalidCommitment);
    }
    salt.append(&Bytes::from_slice(env, &price.to_be_bytes()));
    salt.append(&Bytes::from_slice(env, &quantity.to_be_bytes()));
    if salt != bid.commitment {
        return Err(Error::InvalidCommitment);
    }
    bid.price = price;
    bid.quantity = quantity;
    bid.revealed = true;
    Ok(())
}

fn settle_page(
    e: &Env,
    auction_id: u32,
    cursor: u32,
    limit: u32,
    atomic: bool,
) -> Result<SettlementPage, Error> {
    let mut a: Auction = e
        .storage()
        .persistent()
        .get(&Key::Auction(auction_id))
        .ok_or(Error::NotFound)?;
    if a.settled {
        return Err(Error::AlreadySettled);
    }
    if e.ledger().sequence() < a.reveal_end {
        return Err(Error::Phase);
    }
    let bids = e
        .storage()
        .persistent()
        .get::<Key, Vec<Bid>>(&Key::Bids(auction_id))
        .unwrap_or_else(|| Vec::new(e));
    if atomic && bids.len() > limit {
        return Err(Error::TooManyItems);
    }
    let progress_key = Key::Settlement(auction_id);
    let mut page = e
        .storage()
        .persistent()
        .get::<_, SettlementPage>(&progress_key)
        .unwrap_or(SettlementPage {
            cursor: 0,
            price: 0,
            filled: 0,
            complete: false,
        });
    if cursor != page.cursor {
        return Err(Error::InvalidCursor);
    }
    let end = cursor.saturating_add(limit).min(bids.len());
    for index in cursor..end {
        let bid = bids.get(index).ok_or(Error::NotFound)?;
        if bid.revealed && bid.price >= a.min_price && page.filled < a.supply {
            page.price = page.price.max(bid.price);
            page.filled += bid.quantity.min(a.supply - page.filled);
        }
    }
    page.cursor = end;
    page.complete = end == bids.len();
    if page.complete {
        a.settled = true;
        a.clearing_price = page.price;
        a.filled = page.filled;
        e.storage().persistent().set(&Key::Auction(auction_id), &a);
        e.storage().persistent().remove(&progress_key);
        e.events().publish(
            (soroban_sdk::symbol_short!("settled"), auction_id),
            (page.price, page.filled),
        );
    } else {
        e.storage().persistent().set(&progress_key, &page);
    }
    Ok(page)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};

    #[test]
    fn settles_at_bid_limit_across_pages_with_stable_cursor() {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register(SealedLaunchAuction, ());
        let client = SealedLaunchAuctionClient::new(&env, &contract);
        let bidder = Address::generate(&env);
        let now = env.ledger().sequence();
        let auction = client.create_sealed_auction(&bidder, &bidder, &MAX_BIDS, &1, &10, &10);
        let salt = Bytes::from_array(&env, &[0; (MAX_COMMITMENT_BYTES - 20) as usize]);
        let mut commitment = salt.clone();
        commitment.append(&Bytes::from_slice(&env, &10i128.to_be_bytes()));
        commitment.append(&Bytes::from_slice(&env, &1u32.to_be_bytes()));
        for _ in 0..MAX_BIDS {
            env.cost_estimate().budget().reset_default();
            client.bid(&bidder, &auction, &commitment, &10);
        }
        assert_eq!(
            client.try_bid(&bidder, &auction, &commitment, &10),
            Err(Ok(Error::TooManyItems))
        );
        env.ledger().set_sequence_number(now + 10);
        for _ in 0..MAX_BIDS - 1 {
            env.cost_estimate().budget().reset_default();
            client.reveal(&bidder, &auction, &10, &1, &salt);
        }
        client.reveal_at(&bidder, &auction, &(MAX_BIDS - 1), &10, &1, &salt);
        env.ledger().set_sequence_number(now + 20);
        env.cost_estimate().budget().reset_default();
        let first = client.settle_page(&auction, &0);
        assert_eq!(first.cursor, SETTLEMENT_PAGE_SIZE);
        assert_eq!(first.filled, SETTLEMENT_PAGE_SIZE);
        assert!(!first.complete);
        assert_eq!(
            client.try_settle_page(&auction, &0),
            Err(Ok(Error::InvalidCursor))
        );
        env.cost_estimate().budget().reset_default();
        let last = client.settle_page(&auction, &first.cursor);
        assert!(last.complete);
        assert_eq!(last.cursor, MAX_BIDS);
        assert_eq!(last.filled, MAX_BIDS);
        assert_eq!(last.price, 10);
        assert_eq!(
            client.try_settle_page(&auction, &last.cursor),
            Err(Ok(Error::AlreadySettled))
        );
    }
}
