use crate::events::{
    bootstrap_ended_topics, bootstrap_started_topics, BootstrapEndedEvent, BootstrapStartedEvent,
};
use soroban_sdk::{contracttype, Address, Env};

/// TTL policy for bootstrap configuration entries (persistent storage).
/// `BOOTSTRAP_TTL_THRESHOLD`: extend when remaining ledgers fall below this.
/// `BOOTSTRAP_TTL_EXTEND_TO`: extend the entry's lifetime to this many ledgers.
pub const BOOTSTRAP_TTL_THRESHOLD: u32 = 17_280;
pub const BOOTSTRAP_TTL_EXTEND_TO: u32 = 518_400;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapConfig {
    pub start_price: i128,
    pub end_price: i128,
    pub start_time: u64,
    pub duration_seconds: u64,
}

#[contracttype]
pub enum BootstrapDataKey {
    Config(Address),
}

pub fn set_bootstrap_config(
    env: &Env,
    key_id: &Address,
    start_price: i128,
    end_price: i128,
    duration_seconds: u64,
) {
    let current_time = env.ledger().timestamp();

    let config = BootstrapConfig {
        start_price,
        end_price,
        start_time: current_time,
        duration_seconds,
    };

    let storage_key = BootstrapDataKey::Config(key_id.clone());
    env.storage().persistent().set(&storage_key, &config);
    env.storage().persistent().extend_ttl(
        &storage_key,
        BOOTSTRAP_TTL_THRESHOLD,
        BOOTSTRAP_TTL_EXTEND_TO,
    );

    env.events().publish(
        bootstrap_started_topics(key_id),
        BootstrapStartedEvent {
            key_id: key_id.clone(),
            start_price,
            end_price,
            start_time: current_time,
            duration_seconds,
            ledger: env.ledger().sequence(),
        },
    );
}

pub fn get_bootstrap_price_internal(env: &Env, key_id: &Address) -> Option<i128> {
    let storage_key = BootstrapDataKey::Config(key_id.clone());
    if !env.storage().persistent().has(&storage_key) {
        return None;
    }

    let config: BootstrapConfig = env.storage().persistent().get(&storage_key).unwrap();
    let current_time = env.ledger().timestamp();
    let expiration_time = config.start_time.saturating_add(config.duration_seconds);

    if current_time >= expiration_time {
        env.storage().persistent().remove(&storage_key);

        env.events().publish(
            bootstrap_ended_topics(key_id),
            BootstrapEndedEvent {
                key_id: key_id.clone(),
                final_price: config.end_price,
                timestamp: current_time,
                ledger: env.ledger().sequence(),
            },
        );

        return None;
    }

    // Extend TTL on every access so an active bootstrap is never archived mid-window.
    env.storage().persistent().extend_ttl(
        &storage_key,
        BOOTSTRAP_TTL_THRESHOLD,
        BOOTSTRAP_TTL_EXTEND_TO,
    );

    let elapsed = current_time - config.start_time;
    let total_duration = config.duration_seconds;
    let price_diff = config.start_price - config.end_price;

    let reduction = (price_diff * elapsed as i128) / total_duration as i128;
    Some(config.start_price - reduction)
}
