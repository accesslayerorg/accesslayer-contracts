#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Bytes, Env, Vec};
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    Unauthorized = 1,
    NotFound = 2,
    Phase = 3,
    InvalidCommitment = 4,
    AlreadySettled = 5,
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
        for i in 0..bids.len() {
            let mut b = bids.get(i).unwrap();
            if b.bidder == bidder && !b.revealed {
                let mut payload = Bytes::new(&e);
                payload.append(&salt);
                payload.append(&Bytes::from_slice(&e, &price.to_be_bytes()));
                payload.append(&Bytes::from_slice(&e, &quantity.to_be_bytes()));
                if payload != b.commitment {
                    return Err(Error::InvalidCommitment);
                }
                b.price = price;
                b.quantity = quantity;
                b.revealed = true;
                bids.set(i, b);
                break;
            }
        }
        e.storage().persistent().set(&Key::Bids(auction_id), &bids);
        Ok(())
    }
    pub fn settle(e: Env, auction_id: u32) -> Result<i128, Error> {
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
            .unwrap_or_else(|| Vec::new(&e));
        let mut price = 0;
        let mut filled = 0;
        for b in bids.iter() {
            if b.revealed && b.price >= a.min_price && filled < a.supply {
                price = price.max(b.price);
                filled = filled.saturating_add(b.quantity.min(a.supply - filled));
            }
        }
        a.settled = true;
        a.clearing_price = price;
        a.filled = filled;
        e.storage().persistent().set(&Key::Auction(auction_id), &a);
        e.events().publish(
            (soroban_sdk::symbol_short!("settled"), auction_id),
            (price, filled),
        );
        Ok(price)
    }
}
