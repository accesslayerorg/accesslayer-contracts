use crate::ContractError;
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CurveMigrationProposal {
    pub new_slope: i128,
    pub proposed_at_ledger: u32,
    pub timelock_ledgers: u32,
    pub is_executed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeySubscription {
    pub min_keys_required: u32,
    pub expires_at_ledger: u32,
    pub is_active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PaymentAssetConfig {
    pub asset: Address,
    pub oracle_rate_xlm_scaled: i128, // 1 Asset = X XLM (scaled by 10^7)
    pub max_slippage_bps: u32,
    pub is_enabled: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum EmdevelopaDataKey {
    CurveMigration(Address),
    Subscription(Address, Address), // (creator, subscriber)
    PaymentAsset(Address),
    PaymentAssetList,
    VolumeWindow(Address),
    DiscountTiers,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VolumeEntry {
    pub amount: i128,
    pub ledger: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DiscountTiers {
    pub bronze_threshold: i128,
    pub silver_threshold: i128,
    pub gold_threshold: i128,
    pub bronze_bps: u32,
    pub silver_bps: u32,
    pub gold_bps: u32,
}

pub const CURVE_MIGRATION_PROPOSED_EVENT: Symbol = symbol_short!("mig_prop");
pub const CURVE_MIGRATION_EXECUTED_EVENT: Symbol = symbol_short!("mig_exec");
pub const SUBSCRIPTION_GRANTED_EVENT: Symbol = symbol_short!("sub_grant");
pub const SUBSCRIPTION_RENEWED_EVENT: Symbol = symbol_short!("sub_renew");
pub const ATOMIC_SWAP_EXECUTED_EVENT: Symbol = symbol_short!("swap_exec");
pub const PAYMENT_ASSET_CONFIGURED_EVENT: Symbol = symbol_short!("asset_cfg");
pub const VOLUME_DISCOUNT_TIER_CHANGED_EVENT: Symbol = symbol_short!("vd_tier");

pub const VOLUME_WINDOW_LEDGERS: u32 = 518_400; // ~30 days at 5s ledgers

/// #941: Propose bonding curve parameters upgrade with timelock.
pub fn propose_curve_migration(
    env: &Env,
    creator: &Address,
    new_slope: i128,
    timelock_ledgers: u32,
) -> Result<(), ContractError> {
    creator.require_auth();
    if new_slope <= 0 {
        return Err(ContractError::InvalidFeeConfig);
    }

    let proposal = CurveMigrationProposal {
        new_slope,
        proposed_at_ledger: env.ledger().sequence(),
        timelock_ledgers,
        is_executed: false,
    };

    env.storage().instance().set(
        &EmdevelopaDataKey::CurveMigration(creator.clone()),
        &proposal,
    );

    env.events().publish(
        (CURVE_MIGRATION_PROPOSED_EVENT, creator.clone()),
        (new_slope, proposal.proposed_at_ledger + timelock_ledgers),
    );

    Ok(())
}

/// Execute approved curve migration after timelock expires with admin authorization.
pub fn execute_curve_migration(
    env: &Env,
    creator: &Address,
    admin: &Address,
) -> Result<i128, ContractError> {
    admin.require_auth();

    let migration_key = EmdevelopaDataKey::CurveMigration(creator.clone());
    let mut proposal: CurveMigrationProposal = env
        .storage()
        .instance()
        .get(&migration_key)
        .ok_or(ContractError::Unauthorized)?;

    if proposal.is_executed {
        return Err(ContractError::AlreadyRegistered);
    }

    let current_ledger = env.ledger().sequence();
    if current_ledger < proposal.proposed_at_ledger + proposal.timelock_ledgers {
        return Err(ContractError::AllocationLocked);
    }

    proposal.is_executed = true;
    env.storage().instance().set(&migration_key, &proposal);

    env.events().publish(
        (CURVE_MIGRATION_EXECUTED_EVENT, creator.clone()),
        proposal.new_slope,
    );

    Ok(proposal.new_slope)
}

/// #942: Grant key subscription for gated access based on minimum key balance.
pub fn subscribe_key_access(
    env: &Env,
    creator: &Address,
    subscriber: &Address,
    duration_ledgers: u32,
    min_keys: u32,
    subscriber_balance: u32,
) -> Result<u32, ContractError> {
    subscriber.require_auth();
    if subscriber_balance < min_keys {
        return Err(ContractError::InsufficientBalance);
    }

    let sub_key = EmdevelopaDataKey::Subscription(creator.clone(), subscriber.clone());
    let expires_at = env.ledger().sequence() + duration_ledgers;

    let sub = KeySubscription {
        min_keys_required: min_keys,
        expires_at_ledger: expires_at,
        is_active: true,
    };

    env.storage().instance().set(&sub_key, &sub);

    env.events().publish(
        (
            SUBSCRIPTION_GRANTED_EVENT,
            creator.clone(),
            subscriber.clone(),
        ),
        expires_at,
    );

    Ok(expires_at)
}

/// Check if user has active subscription and meets balance requirement.
pub fn is_subscribed(
    env: &Env,
    creator: &Address,
    subscriber: &Address,
    subscriber_balance: u32,
) -> bool {
    let sub_key = EmdevelopaDataKey::Subscription(creator.clone(), subscriber.clone());
    if let Some(sub) = env.storage().instance().get::<_, KeySubscription>(&sub_key) {
        if sub.is_active
            && env.ledger().sequence() <= sub.expires_at_ledger
            && subscriber_balance >= sub.min_keys_required
        {
            return true;
        }
    }
    false
}

/// #945: Execute cross-key atomic swap between two parties.
// Eight parameters, over clippy's limit of seven. Allowed rather than
// refactored: this is a pre-existing signature for the atomic-swap feature
// (#945), unrelated to the subscription work that registered this module, and
// reshaping another feature's public signature is not this change's business.
#[allow(clippy::too_many_arguments)]
pub fn execute_atomic_swap(
    env: &Env,
    party_a: &Address,
    key_a: &Address,
    amount_a: u32,
    party_b: &Address,
    key_b: &Address,
    amount_b: u32,
    fee_bps: u32,
) -> Result<(u32, u32), ContractError> {
    party_a.require_auth();
    party_b.require_auth();

    if amount_a == 0 || amount_b == 0 {
        return Err(ContractError::ZeroTransferAmount);
    }
    if key_a == key_b {
        return Err(ContractError::SelfTransfer);
    }

    let fee_a = ((amount_a as u64 * fee_bps as u64) / 10_000) as u32;
    let fee_b = ((amount_b as u64 * fee_bps as u64) / 10_000) as u32;

    let net_a = amount_a.saturating_sub(fee_a);
    let net_b = amount_b.saturating_sub(fee_b);

    env.events().publish(
        (ATOMIC_SWAP_EXECUTED_EVENT, party_a.clone(), party_b.clone()),
        (key_a.clone(), key_b.clone(), net_a, net_b),
    );

    Ok((net_a, net_b))
}

/// #932: Configure whitelisted payment asset with oracle rate.
pub fn configure_payment_asset(
    env: &Env,
    admin: &Address,
    asset: &Address,
    oracle_rate_xlm_scaled: i128,
    max_slippage_bps: u32,
    is_enabled: bool,
) -> Result<(), ContractError> {
    admin.require_auth();
    if oracle_rate_xlm_scaled <= 0 {
        return Err(ContractError::InvalidFeeConfig);
    }

    let config = PaymentAssetConfig {
        asset: asset.clone(),
        oracle_rate_xlm_scaled,
        max_slippage_bps,
        is_enabled,
    };

    env.storage()
        .instance()
        .set(&EmdevelopaDataKey::PaymentAsset(asset.clone()), &config);

    env.events().publish(
        (PAYMENT_ASSET_CONFIGURED_EVENT, asset.clone()),
        (oracle_rate_xlm_scaled, is_enabled),
    );

    Ok(())
}

/// Convert asset amount to XLM equivalent using oracle rate.
pub fn convert_payment_asset_to_xlm(
    env: &Env,
    asset: &Address,
    asset_amount: i128,
) -> Result<i128, ContractError> {
    let config: PaymentAssetConfig = env
        .storage()
        .instance()
        .get(&EmdevelopaDataKey::PaymentAsset(asset.clone()))
        .ok_or(ContractError::Unauthorized)?;

    if !config.is_enabled {
        return Err(ContractError::Unauthorized);
    }

    let xlm_equivalent = (asset_amount * config.oracle_rate_xlm_scaled) / 10_000_000;
    Ok(xlm_equivalent)
}

/// Configure discount tier thresholds and bps values (admin only).
pub fn configure_discount_tiers(
    env: &Env,
    admin: &Address,
    bronze_threshold: i128,
    silver_threshold: i128,
    gold_threshold: i128,
    bronze_bps: u32,
    silver_bps: u32,
    gold_bps: u32,
) -> Result<(), ContractError> {
    admin.require_auth();

    if bronze_threshold <= 0
        || silver_threshold <= bronze_threshold
        || gold_threshold <= silver_threshold
    {
        return Err(ContractError::InvalidFeeConfig);
    }
    if bronze_bps > 10_000 || silver_bps > 10_000 || gold_bps > 10_000 {
        return Err(ContractError::InvalidFeeConfig);
    }
    if bronze_bps > silver_bps || silver_bps > gold_bps {
        return Err(ContractError::InvalidFeeConfig);
    }

    let tiers = DiscountTiers {
        bronze_threshold,
        silver_threshold,
        gold_threshold,
        bronze_bps,
        silver_bps,
        gold_bps,
    };

    env.storage()
        .instance()
        .set(&EmdevelopaDataKey::DiscountTiers, &tiers);

    Ok(())
}

/// Record trading volume for a wallet, pruning entries older than the rolling window.
pub fn record_volume(env: &Env, wallet: &Address, amount: i128) -> Result<(), ContractError> {
    if amount <= 0 {
        return Err(ContractError::InvalidFeeConfig);
    }

    let key = EmdevelopaDataKey::VolumeWindow(wallet.clone());
    let mut entries: soroban_sdk::Vec<VolumeEntry> = env
        .storage()
        .instance()
        .get(&key)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));

    let current_ledger = env.ledger().sequence();
    let cutoff = current_ledger.saturating_sub(VOLUME_WINDOW_LEDGERS);

    let mut pruned: soroban_sdk::Vec<VolumeEntry> = soroban_sdk::Vec::new(env);
    for entry in entries.iter() {
        if entry.ledger >= cutoff {
            pruned.push_back(entry);
        }
    }

    pruned.push_back(VolumeEntry {
        amount,
        ledger: current_ledger,
    });

    env.storage().instance().set(&key, &pruned);
    Ok(())
}

/// Compute the total 30-day rolling volume for a wallet.
pub fn get_rolling_volume(env: &Env, wallet: &Address) -> i128 {
    let key = EmdevelopaDataKey::VolumeWindow(wallet.clone());
    let entries: soroban_sdk::Vec<VolumeEntry> = env
        .storage()
        .instance()
        .get(&key)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));

    let current_ledger = env.ledger().sequence();
    let cutoff = current_ledger.saturating_sub(VOLUME_WINDOW_LEDGERS);

    let mut total: i128 = 0;
    for entry in entries.iter() {
        if entry.ledger >= cutoff {
            total = total.saturating_add(entry.amount);
        }
    }
    total
}

/// Determine the tier symbol and discount bps for a given volume.
fn tier_for_volume(tiers: &DiscountTiers, volume: i128) -> (Symbol, u32) {
    if volume >= tiers.gold_threshold {
        (symbol_short!("gold"), tiers.gold_bps)
    } else if volume >= tiers.silver_threshold {
        (symbol_short!("silver"), tiers.silver_bps)
    } else if volume >= tiers.bronze_threshold {
        (symbol_short!("bronze"), tiers.bronze_bps)
    } else {
        (symbol_short!("none"), 0)
    }
}

/// #943: Return the current discount tier and bps for a wallet.
pub fn get_discount(env: &Env, wallet: &Address) -> (Symbol, u32) {
    let tiers: DiscountTiers = match env
        .storage()
        .instance()
        .get(&EmdevelopaDataKey::DiscountTiers)
    {
        Some(t) => t,
        None => return (symbol_short!("none"), 0),
    };

    let volume = get_rolling_volume(env, wallet);
    tier_for_volume(&tiers, volume)
}

/// Record volume and emit a tier-change event if the wallet's tier changed.
pub fn record_volume_and_emit_tier(
    env: &Env,
    wallet: &Address,
    amount: i128,
) -> Result<(), ContractError> {
    let (old_tier, _) = get_discount(env, wallet);
    record_volume(env, wallet, amount)?;
    let (new_tier, new_bps) = get_discount(env, wallet);

    if old_tier != new_tier {
        env.events().publish(
            (VOLUME_DISCOUNT_TIER_CHANGED_EVENT, wallet.clone()),
            (old_tier, new_tier, new_bps),
        );
    }
    Ok(())
}

/// Apply the wallet's discount to a fee amount, returning the discounted fee.
pub fn apply_discount_to_fee(env: &Env, wallet: &Address, fee: i128) -> i128 {
    if fee <= 0 {
        return fee;
    }
    let (_, discount_bps) = get_discount(env, wallet);
    if discount_bps == 0 {
        return fee;
    }
    let discount = (fee * discount_bps as i128) / 10_000;
    fee.saturating_sub(discount)
}

/// Compute the bonding-curve fee for a trade, applying the wallet's discount.
pub fn compute_bonding_curve_fee(
    env: &Env,
    wallet: &Address,
    trade_amount: i128,
    base_fee_bps: u32,
) -> Result<i128, ContractError> {
    if trade_amount <= 0 {
        return Err(ContractError::ZeroTransferAmount);
    }
    if base_fee_bps > 10_000 {
        return Err(ContractError::InvalidFeeConfig);
    }
    let gross_fee = (trade_amount * base_fee_bps as i128) / 10_000;
    Ok(apply_discount_to_fee(env, wallet, gross_fee))
}
