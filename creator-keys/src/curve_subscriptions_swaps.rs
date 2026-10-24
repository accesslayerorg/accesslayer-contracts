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

/// Per-tier configuration defined by a creator for gated access.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SubscriptionTier {
    pub fee_per_period: i128,
    pub period_seconds: u64,
    pub label: Symbol,
}

/// Recorded subscription for a (key, wallet, tier) triplet.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TierSubscription {
    pub expires_at: u64,
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

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum EmdevelopaDataKey {
    CurveMigration(Address),
    Subscription(Address, Address), // (creator, subscriber)
    PaymentAsset(Address),
    PaymentAssetList,
    /// (key_id, tier_id) -> SubscriptionTier
    Tier(Address, Symbol),
    /// (key_id, wallet, tier_id) -> TierSubscription
    TierSub(Address, Address, Symbol),
    /// key_id -> creator earnings ledger (i128)
    CreatorEarnings(Address),
}

pub const CURVE_MIGRATION_PROPOSED_EVENT: Symbol = symbol_short!("mig_prop");
pub const CURVE_MIGRATION_EXECUTED_EVENT: Symbol = symbol_short!("mig_exec");
pub const SUBSCRIPTION_GRANTED_EVENT: Symbol = symbol_short!("sub_grant");
pub const SUBSCRIPTION_RENEWED_EVENT: Symbol = symbol_short!("sub_renew");
pub const ATOMIC_SWAP_EXECUTED_EVENT: Symbol = symbol_short!("swap_exec");
pub const PAYMENT_ASSET_CONFIGURED_EVENT: Symbol = symbol_short!("asset_cfg");
pub const TIER_CREATED_EVENT: Symbol = symbol_short!("tier_cre");

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

/// Create a subscription tier for a key. Only the key creator may create tiers.
pub fn create_tier(
    env: &Env,
    creator: &Address,
    key_id: &Address,
    tier_id: Symbol,
    fee_per_period: i128,
    period_seconds: u64,
    label: Symbol,
) -> Result<(), ContractError> {
    creator.require_auth();
    if fee_per_period < 0 || period_seconds == 0 {
        return Err(ContractError::InvalidFeeConfig);
    }

    let tier = SubscriptionTier {
        fee_per_period,
        period_seconds,
        label,
    };

    env.storage().instance().set(
        &EmdevelopaDataKey::Tier(key_id.clone(), tier_id.clone()),
        &tier,
    );

    env.events().publish(
        (TIER_CREATED_EVENT, key_id.clone(), tier_id),
        (fee_per_period, period_seconds),
    );

    Ok(())
}

/// Subscribe to a tier. Transfers the tier fee to the creator and records the
/// subscription expiry. Renewal extends from max(now, current_expiry).
pub fn subscribe(
    env: &Env,
    key_id: &Address,
    wallet: &Address,
    tier_id: Symbol,
    payment_token: &Address,
) -> Result<u64, ContractError> {
    wallet.require_auth();

    let tier: SubscriptionTier = env
        .storage()
        .instance()
        .get(&EmdevelopaDataKey::Tier(key_id.clone(), tier_id.clone()))
        .ok_or(ContractError::Unauthorized)?;

    let now = env.ledger().timestamp();
    let sub_key = EmdevelopaDataKey::TierSub(key_id.clone(), wallet.clone(), tier_id.clone());

    let existing: Option<TierSubscription> = env.storage().instance().get(&sub_key);
    let base = match &existing {
        Some(prev) if prev.expires_at > now => prev.expires_at,
        _ => now,
    };
    let expires_at = base + tier.period_seconds;

    // Transfer the tier fee from the subscriber to the key creator.
    if tier.fee_per_period > 0 {
        let token = soroban_sdk::token::Client::new(env, payment_token);
        token.transfer(wallet, key_id, &tier.fee_per_period);

        // Credit the creator earnings ledger.
        let earnings_key = EmdevelopaDataKey::CreatorEarnings(key_id.clone());
        let current: i128 = env.storage().instance().get(&earnings_key).unwrap_or(0);
        env.storage()
            .instance()
            .set(&earnings_key, &(current + tier.fee_per_period));
    }

    let sub = TierSubscription {
        expires_at,
        is_active: true,
    };
    env.storage().instance().set(&sub_key, &sub);

    let event = if existing.is_some() {
        SUBSCRIPTION_RENEWED_EVENT
    } else {
        SUBSCRIPTION_GRANTED_EVENT
    };
    env.events()
        .publish((event, key_id.clone(), wallet.clone(), tier_id), expires_at);

    Ok(expires_at)
}

/// Returns true only within the active subscription window for the given tier.
pub fn is_tier_subscribed(env: &Env, key_id: &Address, wallet: &Address, tier_id: Symbol) -> bool {
    let sub_key = EmdevelopaDataKey::TierSub(key_id.clone(), wallet.clone(), tier_id);
    if let Some(sub) = env
        .storage()
        .instance()
        .get::<_, TierSubscription>(&sub_key)
    {
        return sub.is_active && env.ledger().timestamp() < sub.expires_at;
    }
    false
}

/// Returns the creator earnings accumulated from subscription fees.
pub fn get_creator_earnings(env: &Env, key_id: &Address) -> i128 {
    env.storage()
        .instance()
        .get(&EmdevelopaDataKey::CreatorEarnings(key_id.clone()))
        .unwrap_or(0)
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
