#![no_std]
#![cfg_attr(
    not(test),
    deny(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)
)]

use crate::ContractError;
use common::math::{checked_mul_div, RoundingDirection};
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PriceImpactConfig {
    pub max_price_impact_bps: u32,
    pub is_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ProRataDividendPool {
    pub accumulated_funds: i128,
    pub current_cycle: u32,
    pub total_distributed: i128,
}

#[derive(Clone)]
#[contracttype]
pub enum MeloballDataKey {
    PriceImpactConfig(Address),
    ProRataDividendPool(Address),
    HolderCycleClaim(Address, Address), // (creator, holder) -> cycle
    TxBuyCap(Address),
    SecondaryRoyaltyBps(Address),
}

pub const PRICE_IMPACT_CONFIG_EVENT: Symbol = symbol_short!("imp_cfg");
pub const PRICE_IMPACT_EXCEEDED_EVENT: Symbol = symbol_short!("imp_ex");
pub const PRO_RATA_FUNDED_EVENT: Symbol = symbol_short!("pr_fund");
pub const PRO_RATA_CLAIMED_EVENT: Symbol = symbol_short!("pr_claim");
pub const BUY_CAP_SET_EVENT: Symbol = symbol_short!("cap_set");
pub const SECONDARY_ROYALTY_PAID_EVENT: Symbol = symbol_short!("sec_roy");

/// #928: Configure maximum price impact threshold in basis points.
pub fn set_max_price_impact_limit(
    env: &Env,
    creator: &Address,
    max_impact_bps: u32,
) -> Result<(), ContractError> {
    creator.require_auth();
    if max_impact_bps > 5000 {
        // Maximum 50% impact cap
        return Err(ContractError::InvalidFeeConfig);
    }

    let config = PriceImpactConfig {
        max_price_impact_bps: max_impact_bps,
        is_enabled: true,
    };

    env.storage().instance().set(
        &MeloballDataKey::PriceImpactConfig(creator.clone()),
        &config,
    );

    env.events()
        .publish((PRICE_IMPACT_CONFIG_EVENT, creator.clone()), max_impact_bps);

    Ok(())
}

/// Verify trade price impact is within configured bounds.
pub fn assert_price_impact(
    env: &Env,
    creator: &Address,
    current_price: i128,
    execution_price: i128,
) -> Result<(), ContractError> {
    if let Some(config) = env
        .storage()
        .instance()
        .get::<_, PriceImpactConfig>(&MeloballDataKey::PriceImpactConfig(creator.clone()))
    {
        if config.is_enabled && current_price > 0 {
            let diff = if execution_price > current_price {
                execution_price.checked_sub(current_price)
            } else {
                current_price.checked_sub(execution_price)
            }
            .ok_or(ContractError::Overflow)?;
            let impact = checked_mul_div(env, diff, 10_000, current_price, RoundingDirection::Down)
                .map_err(|_| ContractError::Overflow)?;
            let impact_bps = u32::try_from(impact).map_err(|_| ContractError::Overflow)?;
            if impact_bps > config.max_price_impact_bps {
                env.events().publish(
                    (PRICE_IMPACT_EXCEEDED_EVENT, creator.clone()),
                    (impact_bps, config.max_price_impact_bps),
                );
                return Err(ContractError::SlippageExceeded);
            }
        }
    }
    Ok(())
}

/// #954: Fund on-chain pro-rata dividend pool with fee revenue.
pub fn fund_pro_rata_pool(env: &Env, creator: &Address, amount: i128) -> Result<(), ContractError> {
    if amount <= 0 {
        return Err(ContractError::ZeroDistributionAmount);
    }

    let key = MeloballDataKey::ProRataDividendPool(creator.clone());
    let mut pool: ProRataDividendPool =
        env.storage()
            .instance()
            .get(&key)
            .unwrap_or(ProRataDividendPool {
                accumulated_funds: 0,
                current_cycle: 1,
                total_distributed: 0,
            });

    pool.accumulated_funds = pool
        .accumulated_funds
        .checked_add(amount)
        .ok_or(ContractError::Overflow)?;
    pool.total_distributed = pool
        .total_distributed
        .checked_add(amount)
        .ok_or(ContractError::Overflow)?;

    env.storage().instance().set(&key, &pool);

    env.events().publish(
        (PRO_RATA_FUNDED_EVENT, creator.clone()),
        (pool.current_cycle, amount),
    );

    Ok(())
}

/// Claim pro-rata dividend share for current cycle.
pub fn claim_pro_rata_cycle_dividend(
    env: &Env,
    creator: &Address,
    holder: &Address,
    holder_share_bps: u32,
) -> Result<i128, ContractError> {
    holder.require_auth();

    let pool_key = MeloballDataKey::ProRataDividendPool(creator.clone());
    let claim_key = MeloballDataKey::HolderCycleClaim(creator.clone(), holder.clone());

    let pool: ProRataDividendPool = env
        .storage()
        .instance()
        .get(&pool_key)
        .ok_or(ContractError::NoDividendClaimable)?;

    let last_claimed: u32 = env.storage().instance().get(&claim_key).unwrap_or(0);
    if last_claimed >= pool.current_cycle {
        return Err(ContractError::AlreadyClaimed);
    }

    let share_amount = checked_mul_div(
        env,
        pool.accumulated_funds,
        i128::from(holder_share_bps),
        10_000,
        RoundingDirection::Down,
    )
    .map_err(|_| ContractError::Overflow)?;
    if share_amount <= 0 {
        return Err(ContractError::NoDividendClaimable);
    }

    env.storage()
        .instance()
        .set(&claim_key, &pool.current_cycle);

    env.events().publish(
        (PRO_RATA_CLAIMED_EVENT, creator.clone(), holder.clone()),
        (pool.current_cycle, share_amount),
    );

    Ok(share_amount)
}

/// #955: Set per-transaction buy cap for creator key.
pub fn set_tx_buy_cap(env: &Env, creator: &Address, cap: u32) -> Result<(), ContractError> {
    creator.require_auth();
    if cap == 0 {
        return Err(ContractError::NotPositiveAmount);
    }

    env.storage()
        .instance()
        .set(&MeloballDataKey::TxBuyCap(creator.clone()), &cap);

    env.events()
        .publish((BUY_CAP_SET_EVENT, creator.clone()), cap);

    Ok(())
}

/// Enforce per-transaction buy cap.
pub fn enforce_tx_buy_cap(
    env: &Env,
    creator: &Address,
    quantity: u32,
) -> Result<(), ContractError> {
    let cap: u32 = env
        .storage()
        .instance()
        .get(&MeloballDataKey::TxBuyCap(creator.clone()))
        .unwrap_or(u32::MAX);

    if quantity > cap {
        return Err(ContractError::SupplyCapExceeded);
    }
    Ok(())
}

/// #956: Configure secondary transfer royalty percentage in basis points.
pub fn set_secondary_transfer_royalty(
    env: &Env,
    creator: &Address,
    royalty_bps: u32,
) -> Result<(), ContractError> {
    creator.require_auth();
    if royalty_bps > 3000 {
        // Max 30% upper bound
        return Err(ContractError::InvalidFeeConfig);
    }

    env.storage().instance().set(
        &MeloballDataKey::SecondaryRoyaltyBps(creator.clone()),
        &royalty_bps,
    );

    Ok(())
}

/// Route secondary transfer royalty to creator wallet.
pub fn route_secondary_royalty(
    env: &Env,
    creator: &Address,
    sender: &Address,
    transfer_amount: i128,
) -> Result<(i128, i128), ContractError> {
    let bps: u32 = env
        .storage()
        .instance()
        .get(&MeloballDataKey::SecondaryRoyaltyBps(creator.clone()))
        .unwrap_or(0);

    if bps == 0 || transfer_amount <= 0 {
        return Ok((transfer_amount, 0));
    }

    let royalty = checked_mul_div(
        env,
        transfer_amount,
        i128::from(bps),
        10_000,
        RoundingDirection::Down,
    )
    .map_err(|_| ContractError::Overflow)?;
    let net = transfer_amount
        .checked_sub(royalty)
        .ok_or(ContractError::Overflow)?;

    env.events().publish(
        (
            SECONDARY_ROYALTY_PAID_EVENT,
            creator.clone(),
            sender.clone(),
        ),
        royalty,
    );

    Ok((net, royalty))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    #[test]
    fn maximum_dividend_claim_uses_wide_intermediate_and_checked_accumulation() {
        let env = Env::default();
        env.mock_all_auths();
        let creator = Address::generate(&env);
        let holder = Address::generate(&env);

        fund_pro_rata_pool(&env, &creator, i128::MAX).unwrap();
        assert_eq!(
            claim_pro_rata_cycle_dividend(&env, &creator, &holder, 10_000),
            Ok(i128::MAX)
        );
        assert_eq!(
            fund_pro_rata_pool(&env, &creator, 1),
            Err(ContractError::Overflow)
        );
    }

    #[test]
    fn price_impact_result_overflow_returns_contract_error() {
        let env = Env::default();
        let creator = Address::generate(&env);
        env.storage().instance().set(
            &MeloballDataKey::PriceImpactConfig(creator.clone()),
            &PriceImpactConfig {
                max_price_impact_bps: 10_000,
                is_enabled: true,
            },
        );

        assert_eq!(
            assert_price_impact(&env, &creator, 1, i128::MAX),
            Err(ContractError::Overflow)
        );
    }
}
