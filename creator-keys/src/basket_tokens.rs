use soroban_sdk::{contracttype, Address, Env, Symbol, Vec};

use crate::ContractError;

pub const MAX_CONSTITUENTS: u32 = 20;
pub const BPS_SCALE: i128 = 10_000;

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Basket {
    pub manager: Address,
    pub name_hash: u32,
    pub keys: Vec<Address>,
    pub weights_bps: Vec<u32>,
    pub management_fee_bps: u32,
    pub rebalance_limit_bps: u32,
    pub rebalance_timelock_ledgers: u32,
    pub last_rebalance_ledger: u32,
    pub total_units: i128,
}

#[derive(Clone)]
#[contracttype]
enum BasketKey {
    Basket(u32),
    Units(u32, Address),
}

pub fn create_basket(
    env: &Env,
    manager: Address,
    name_hash: u32,
    keys: Vec<Address>,
    weights_bps: Vec<u32>,
) -> Result<u32, ContractError> {
    manager.require_auth();
    if keys.is_empty() || keys.len() > MAX_CONSTITUENTS || keys.len() != weights_bps.len() {
        return Err(ContractError::InvalidBasket);
    }
    let mut sum = 0i128;
    for weight in weights_bps.iter() {
        sum += weight as i128;
    }
    if sum != BPS_SCALE {
        return Err(ContractError::InvalidWeights);
    }
    let basket_id: u32 = env
        .storage()
        .instance()
        .get(&Symbol::new(env, "basket_next"))
        .unwrap_or(1);
    let basket = Basket {
        manager,
        name_hash,
        keys,
        weights_bps,
        management_fee_bps: 0,
        rebalance_limit_bps: 1_000,
        rebalance_timelock_ledgers: 1,
        last_rebalance_ledger: env.ledger().sequence(),
        total_units: 0,
    };
    env.storage()
        .instance()
        .set(&BasketKey::Basket(basket_id), &basket);
    env.storage()
        .instance()
        .set(&Symbol::new(env, "basket_next"), &(basket_id + 1));
    env.events()
        .publish((Symbol::new(env, "basket_created"), basket_id), ());
    Ok(basket_id)
}

pub fn mint(
    env: &Env,
    depositor: Address,
    basket_id: u32,
    units: i128,
) -> Result<(), ContractError> {
    depositor.require_auth();
    if units <= 0 {
        return Err(ContractError::NotPositiveAmount);
    }
    let mut basket: Basket = env
        .storage()
        .instance()
        .get(&BasketKey::Basket(basket_id))
        .ok_or(ContractError::BasketNotFound)?;
    for i in 0..basket.keys.len() {
        let key = basket.keys.get(i).unwrap();
        let amount = units
            .checked_mul(basket.weights_bps.get(i).unwrap() as i128)
            .ok_or(ContractError::Overflow)?
            / BPS_SCALE;
        if amount <= 0 {
            continue;
        }
        let balance_key = crate::DataKey::KeyBalance(key, depositor.clone());
        let balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        if balance < amount as u32 {
            return Err(ContractError::InsufficientSupply);
        }
    }
    for i in 0..basket.keys.len() {
        let key = basket.keys.get(i).unwrap();
        let amount = units * basket.weights_bps.get(i).unwrap() as i128 / BPS_SCALE;
        let balance_key = crate::DataKey::KeyBalance(key, depositor.clone());
        let balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        env.storage()
            .persistent()
            .set(&balance_key, &(balance - amount as u32));
    }
    let units_key = BasketKey::Units(basket_id, depositor.clone());
    let old: i128 = env.storage().instance().get(&units_key).unwrap_or(0);
    env.storage().instance().set(&units_key, &(old + units));
    basket.total_units = basket
        .total_units
        .checked_add(units)
        .ok_or(ContractError::Overflow)?;
    env.storage()
        .instance()
        .set(&BasketKey::Basket(basket_id), &basket);
    env.events().publish(
        (Symbol::new(env, "basket_minted"), basket_id),
        (depositor, units),
    );
    Ok(())
}

pub fn redeem(
    env: &Env,
    holder: Address,
    basket_id: u32,
    units: i128,
) -> Result<(), ContractError> {
    holder.require_auth();
    if units <= 0 {
        return Err(ContractError::NotPositiveAmount);
    }
    let units_key = BasketKey::Units(basket_id, holder.clone());
    let held: i128 = env.storage().instance().get(&units_key).unwrap_or(0);
    if held < units {
        return Err(ContractError::InsufficientSupply);
    }
    let mut basket: Basket = env
        .storage()
        .instance()
        .get(&BasketKey::Basket(basket_id))
        .ok_or(ContractError::BasketNotFound)?;
    for i in 0..basket.keys.len() {
        let key = basket.keys.get(i).unwrap();
        let amount = units * basket.weights_bps.get(i).unwrap() as i128 / BPS_SCALE;
        let balance_key = crate::DataKey::KeyBalance(key, holder.clone());
        let balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        env.storage()
            .persistent()
            .set(&balance_key, &(balance.saturating_add(amount as u32)));
    }
    env.storage().instance().set(&units_key, &(held - units));
    basket.total_units = basket.total_units.saturating_sub(units);
    env.storage()
        .instance()
        .set(&BasketKey::Basket(basket_id), &basket);
    env.events().publish(
        (Symbol::new(env, "basket_redeemed"), basket_id),
        (holder, units),
    );
    Ok(())
}
