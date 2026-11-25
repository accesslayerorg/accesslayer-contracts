#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, BytesN, Env};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    Unauthorized = 1,
    InvalidAmount = 2,
    CapExceeded = 3,
    NoTrigger = 4,
    Expired = 5,
    AlreadyClaimed = 6,
    ReserveViolation = 7,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cover {
    pub id: u64,
    pub holder: Address,
    pub key_id: Address,
    pub amount: i128,
    pub premium: i128,
    pub expiry: u32,
    pub claimed: bool,
}
#[contracttype]
#[allow(clippy::enum_variant_names)]
enum Key {
    Admin,
    Pool,
    NextId,
    Cover(u64),
    KeyTotal(Address),
    Trigger(Address),
}
const RESERVE_BPS: u32 = 2_000;
#[contract]
pub struct HolderInsurancePool;
#[contractimpl]
impl HolderInsurancePool {
    pub fn initialize(e: Env, admin: Address) -> Result<(), Error> {
        if e.storage().instance().has(&Key::Admin) {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        e.storage().instance().set(&Key::Admin, &admin);
        e.storage().persistent().set(&Key::Pool, &0i128);
        e.storage().persistent().set(&Key::NextId, &0u64);
        Ok(())
    }
    pub fn fund(e: Env, from: Address, amount: i128) -> Result<(), Error> {
        from.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let p = e
            .storage()
            .persistent()
            .get::<Key, i128>(&Key::Pool)
            .unwrap_or(0)
            .checked_add(amount)
            .ok_or(Error::InvalidAmount)?;
        e.storage().persistent().set(&Key::Pool, &p);
        Ok(())
    }
    pub fn buy_cover(
        e: Env,
        holder: Address,
        key_id: Address,
        amount: i128,
        duration: u32,
        premium_rate_bps: u32,
        max_cover_ratio_bps: u32,
    ) -> Result<u64, Error> {
        holder.require_auth();
        if amount <= 0 || duration == 0 {
            return Err(Error::InvalidAmount);
        }
        let pool = e
            .storage()
            .persistent()
            .get::<Key, i128>(&Key::Pool)
            .unwrap_or(0);
        let total = e
            .storage()
            .persistent()
            .get::<Key, i128>(&Key::KeyTotal(key_id.clone()))
            .unwrap_or(0);
        if total.checked_add(amount).ok_or(Error::InvalidAmount)?
            > pool.saturating_mul(max_cover_ratio_bps as i128) / 10_000
        {
            return Err(Error::CapExceeded);
        }
        let premium = amount
            .checked_mul(premium_rate_bps as i128)
            .ok_or(Error::InvalidAmount)?
            / 10_000;
        let id = e
            .storage()
            .persistent()
            .get::<Key, u64>(&Key::NextId)
            .unwrap_or(0)
            + 1;
        e.storage().persistent().set(&Key::NextId, &id);
        e.storage()
            .persistent()
            .set(&Key::KeyTotal(key_id.clone()), &(total + amount));
        e.storage().persistent().set(
            &Key::Cover(id),
            &Cover {
                id,
                holder,
                key_id,
                amount,
                premium,
                expiry: e.ledger().sequence().saturating_add(duration),
                claimed: false,
            },
        );
        Ok(id)
    }
    pub fn declare_trigger(
        e: Env,
        admin: Address,
        key_id: Address,
        trigger_type: u32,
        evidence_hash: BytesN<32>,
    ) -> Result<(), Error> {
        let owner: Address = e
            .storage()
            .instance()
            .get(&Key::Admin)
            .ok_or(Error::Unauthorized)?;
        if owner != admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        e.storage().persistent().set(
            &Key::Trigger(key_id.clone()),
            &(trigger_type, evidence_hash),
        );
        e.events().publish(
            (soroban_sdk::symbol_short!("trigger"), key_id),
            trigger_type,
        );
        Ok(())
    }
    pub fn claim(e: Env, holder: Address, cover_id: u64) -> Result<i128, Error> {
        holder.require_auth();
        let mut c: Cover = e
            .storage()
            .persistent()
            .get(&Key::Cover(cover_id))
            .ok_or(Error::NoTrigger)?;
        if c.holder != holder {
            return Err(Error::Unauthorized);
        }
        if c.claimed {
            return Err(Error::AlreadyClaimed);
        }
        if e.ledger().sequence() >= c.expiry {
            return Err(Error::Expired);
        }
        if !e
            .storage()
            .persistent()
            .has(&Key::Trigger(c.key_id.clone()))
        {
            return Err(Error::NoTrigger);
        }
        let pool = e
            .storage()
            .persistent()
            .get::<Key, i128>(&Key::Pool)
            .unwrap_or(0);
        let payout = c.amount.min(pool);
        let remaining = pool - payout;
        e.storage().persistent().set(&Key::Pool, &remaining);
        c.claimed = true;
        e.storage().persistent().set(&Key::Cover(cover_id), &c);
        e.events()
            .publish((soroban_sdk::symbol_short!("claim"), holder), payout);
        Ok(payout)
    }
    pub fn withdraw_premium_surplus(e: Env, admin: Address, amount: i128) -> Result<(), Error> {
        let owner: Address = e
            .storage()
            .instance()
            .get(&Key::Admin)
            .ok_or(Error::Unauthorized)?;
        if owner != admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        let pool = e
            .storage()
            .persistent()
            .get::<Key, i128>(&Key::Pool)
            .unwrap_or(0);
        let total = e
            .storage()
            .persistent()
            .get::<Key, i128>(&Key::KeyTotal(admin.clone()))
            .unwrap_or(0);
        if amount < 0 || pool - amount < total.saturating_mul(RESERVE_BPS as i128) / 10_000 {
            return Err(Error::ReserveViolation);
        }
        e.storage().persistent().set(&Key::Pool, &(pool - amount));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};

    fn setup() -> (Env, Address, HolderInsurancePoolClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(HolderInsurancePool, ());
        let client = HolderInsurancePoolClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, contract_id, client, admin)
    }

    fn read_pool(env: &Env, contract_id: &Address) -> i128 {
        env.as_contract(contract_id, || {
            env.storage().persistent().get(&Key::Pool).unwrap_or(0)
        })
    }

    fn read_cover(env: &Env, contract_id: &Address, cover_id: u64) -> Cover {
        env.as_contract(contract_id, || {
            env.storage()
                .persistent()
                .get(&Key::Cover(cover_id))
                .unwrap()
        })
    }

    #[test]
    fn initialize_and_fund_validate_state_and_positive_amounts() {
        let (env, contract_id, client, admin) = setup();
        let funder = Address::generate(&env);

        assert_eq!(client.try_initialize(&admin), Err(Ok(Error::Unauthorized)));
        assert_eq!(client.try_fund(&funder, &0), Err(Ok(Error::InvalidAmount)));
        assert_eq!(
            client.try_fund(&funder, &-10),
            Err(Ok(Error::InvalidAmount))
        );

        client.fund(&funder, &10_000);
        assert_eq!(read_pool(&env, &contract_id), 10_000);
    }

    #[test]
    fn buy_cover_enforces_inputs_cap_ratio_and_computes_premium() {
        let (env, contract_id, client, _) = setup();
        let funder = Address::generate(&env);
        let holder = Address::generate(&env);
        let key_id = Address::generate(&env);

        client.fund(&funder, &10_000);

        // Invalid inputs
        assert_eq!(
            client.try_buy_cover(&holder, &key_id, &0, &10, &1_000, &5_000),
            Err(Ok(Error::InvalidAmount))
        );
        assert_eq!(
            client.try_buy_cover(&holder, &key_id, &-5, &10, &1_000, &5_000),
            Err(Ok(Error::InvalidAmount))
        );
        assert_eq!(
            client.try_buy_cover(&holder, &key_id, &100, &0, &1_000, &5_000),
            Err(Ok(Error::InvalidAmount))
        );

        // Cover 1: 3_000 at 10% premium (1_000 bps) -> premium = 300
        let id1 = client.buy_cover(&holder, &key_id, &3_000, &100, &1_000, &5_000);
        assert_eq!(id1, 1);
        let c1 = read_cover(&env, &contract_id, id1);
        assert_eq!(c1.premium, 300);

        // Cover 2: 2_000 brings total to 5_000 == 50% cap (exact boundary allowed)
        let id2 = client.buy_cover(&holder, &key_id, &2_000, &100, &500, &5_000);
        assert_eq!(id2, 2);
        let c2 = read_cover(&env, &contract_id, id2);
        assert_eq!(c2.premium, 100);

        // Cover 3: 1 more exceeds the 5_000 cap
        assert_eq!(
            client.try_buy_cover(&holder, &key_id, &1, &100, &500, &5_000),
            Err(Ok(Error::CapExceeded))
        );
    }

    #[test]
    fn declare_trigger_and_claim_enforce_auth_expiry_trigger_and_pool_deduction() {
        let (env, contract_id, client, admin) = setup();
        let other = Address::generate(&env);
        let funder = Address::generate(&env);
        let holder = Address::generate(&env);
        let key_id = Address::generate(&env);
        let evidence = BytesN::from_array(&env, &[7u8; 32]);

        env.ledger().set_sequence_number(10);
        client.fund(&funder, &10_000);
        let id1 = client.buy_cover(&holder, &key_id, &3_000, &20, &1_000, &5_000);
        let id2 = client.buy_cover(&holder, &key_id, &2_000, &5, &1_000, &5_000);

        // Claim before trigger fails
        assert_eq!(client.try_claim(&holder, &id1), Err(Ok(Error::NoTrigger)));

        // Non-admin cannot declare trigger
        assert_eq!(
            client.try_declare_trigger(&other, &key_id, &1, &evidence),
            Err(Ok(Error::Unauthorized))
        );
        client.declare_trigger(&admin, &key_id, &1, &evidence);

        // Wrong holder cannot claim
        assert_eq!(client.try_claim(&other, &id1), Err(Ok(Error::Unauthorized)));

        // Expired cover (sequence == expiry = 15) cannot be claimed
        env.ledger().set_sequence_number(15);
        assert_eq!(client.try_claim(&holder, &id2), Err(Ok(Error::Expired)));

        // Active cover pays out and decrements pool
        let payout = client.claim(&holder, &id1);
        assert_eq!(payout, 3_000);
        assert_eq!(read_pool(&env, &contract_id), 7_000);

        // Double claim rejected
        assert_eq!(
            client.try_claim(&holder, &id1),
            Err(Ok(Error::AlreadyClaimed))
        );
    }

    #[test]
    fn withdraw_premium_surplus_enforces_admin_and_reserve_ratio() {
        let (env, contract_id, client, admin) = setup();
        let other = Address::generate(&env);
        let funder = Address::generate(&env);
        let holder = Address::generate(&env);

        client.fund(&funder, &5_000);
        // KeyTotal(admin) = 5_000 -> required reserve at RESERVE_BPS (2_000) is 1_000
        client.buy_cover(&holder, &admin, &5_000, &50, &1_000, &10_000);

        // Non-admin rejected
        assert_eq!(
            client.try_withdraw_premium_surplus(&other, &100),
            Err(Ok(Error::Unauthorized))
        );

        // Negative amount rejected
        assert_eq!(
            client.try_withdraw_premium_surplus(&admin, &-1),
            Err(Ok(Error::ReserveViolation))
        );

        // Zero amount accepted
        client.withdraw_premium_surplus(&admin, &0);
        assert_eq!(read_pool(&env, &contract_id), 5_000);

        // Withdrawing 4_001 leaves 999 < 1_000 required reserve -> rejected
        assert_eq!(
            client.try_withdraw_premium_surplus(&admin, &4_001),
            Err(Ok(Error::ReserveViolation))
        );

        // Withdrawing exact surplus 4_000 leaves 1_000 == required reserve -> accepted
        client.withdraw_premium_surplus(&admin, &4_000);
        assert_eq!(read_pool(&env, &contract_id), 1_000);

        // Any further withdrawal violates reserve
        assert_eq!(
            client.try_withdraw_premium_surplus(&admin, &1),
            Err(Ok(Error::ReserveViolation))
        );
    }
}
