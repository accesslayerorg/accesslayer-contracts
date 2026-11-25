use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env,
};

const T0: u64 = 1_000_000;

fn setup(env: &Env) -> (KeyDiscoveryRegistryClient<'_>, Address) {
    env.mock_all_auths();
    env.ledger().with_mut(|l| {
        l.timestamp = T0;
    });
    let contract_id = env.register(KeyDiscoveryRegistry, ());
    let client = KeyDiscoveryRegistryClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialise(&admin);
    (client, admin)
}

fn advance(env: &Env, seconds: u64) {
    env.ledger().with_mut(|l| {
        l.timestamp += seconds;
    });
}

// --- Acceptance: keys registered correctly on factory deployment ---

#[test]
fn registers_keys_with_metadata_and_count() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let creator = Address::generate(&env);
    let key_a = Address::generate(&env);
    let key_b = Address::generate(&env);

    client.register_key(&admin, &key_a, &creator);
    advance(&env, 10);
    client.register_key(&admin, &key_b, &creator);

    assert_eq!(client.get_registered_count(), 2);
    let info = client.get_key_info(&key_a).unwrap();
    assert_eq!(info.key_id, key_a);
    assert_eq!(info.creator, creator);
    assert_eq!(info.deploy_timestamp, T0);
    let info_b = client.get_key_info(&key_b).unwrap();
    assert_eq!(info_b.deploy_timestamp, T0 + 10);
}

#[test]
fn register_requires_authorised_caller() {
    let env = Env::default();
    let (client, _admin) = setup(&env);
    let stranger = Address::generate(&env);
    let creator = Address::generate(&env);
    let key = Address::generate(&env);
    assert_eq!(
        client.try_register_key(&stranger, &key, &creator),
        Err(Ok(DiscoveryError::Unauthorized))
    );
}

#[test]
fn authorised_registrar_can_register() {
    let env = Env::default();
    let (client, _admin) = setup(&env);
    let registrar = Address::generate(&env);
    let creator = Address::generate(&env);
    let key = Address::generate(&env);
    client.set_registrar(&registrar, &true);
    client.register_key(&registrar, &key, &creator);
    assert_eq!(client.get_registered_count(), 1);
}

#[test]
fn duplicate_registration_is_rejected() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let creator = Address::generate(&env);
    let key = Address::generate(&env);
    client.register_key(&admin, &key, &creator);
    assert_eq!(
        client.try_register_key(&admin, &key, &creator),
        Err(Ok(DiscoveryError::AlreadyRegistered))
    );
}

// --- Acceptance: 24h volume computed correctly, excluding older trades ---

#[test]
fn volume_24h_excludes_trades_older_than_window() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let creator = Address::generate(&env);
    let key = Address::generate(&env);
    client.register_key(&admin, &key, &creator);

    client.record_volume(&admin, &key, &100);
    // Still inside the 24h window.
    advance(&env, VOLUME_WINDOW_24H - 1);
    assert_eq!(client.get_volume_24h(&key), 100);

    // One second past the window: the original trade is now outside 24h.
    advance(&env, 2);
    assert_eq!(client.get_volume_24h(&key), 0);
}

#[test]
fn volume_24h_sums_recent_and_drops_old() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let creator = Address::generate(&env);
    let key = Address::generate(&env);
    client.register_key(&admin, &key, &creator);

    client.record_volume(&admin, &key, &100);
    advance(&env, VOLUME_WINDOW_24H - 100);
    client.record_volume(&admin, &key, &50);
    // Both trades are within 24h of "now".
    assert_eq!(client.get_volume_24h(&key), 150);

    // Advance past the first trade's window but keep the second.
    advance(&env, 200);
    assert_eq!(client.get_volume_24h(&key), 50);
}

#[test]
fn record_volume_guards() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let stranger = Address::generate(&env);
    let creator = Address::generate(&env);
    let key = Address::generate(&env);

    // Unregistered key.
    assert_eq!(
        client.try_record_volume(&admin, &key, &10),
        Err(Ok(DiscoveryError::KeyNotRegistered))
    );

    client.register_key(&admin, &key, &creator);

    // Non-positive amount.
    assert_eq!(
        client.try_record_volume(&admin, &key, &0),
        Err(Ok(DiscoveryError::NotPositiveAmount))
    );
    assert_eq!(
        client.try_record_volume(&admin, &key, &-5),
        Err(Ok(DiscoveryError::NotPositiveAmount))
    );

    // Unauthorised caller.
    assert_eq!(
        client.try_record_volume(&stranger, &key, &10),
        Err(Ok(DiscoveryError::Unauthorized))
    );
}

#[test]
fn authorised_recorder_can_record() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let recorder = Address::generate(&env);
    let creator = Address::generate(&env);
    let key = Address::generate(&env);
    client.register_key(&admin, &key, &creator);
    client.set_recorder(&recorder, &true);
    client.record_volume(&recorder, &key, &42);
    assert_eq!(client.get_volume_24h(&key), 42);
}

// --- Acceptance: get_trending returns correct top N in descending order ---

#[test]
fn trending_returns_top_n_descending() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let creator = Address::generate(&env);
    let key_a = Address::generate(&env);
    let key_b = Address::generate(&env);
    let key_c = Address::generate(&env);
    client.register_key(&admin, &key_a, &creator);
    client.register_key(&admin, &key_b, &creator);
    client.register_key(&admin, &key_c, &creator);

    client.record_volume(&admin, &key_a, &50);
    client.record_volume(&admin, &key_b, &200);
    client.record_volume(&admin, &key_c, &100);

    let trending = client.get_trending(&3);
    assert_eq!(trending.len(), 3);
    assert_eq!(trending.get(0).unwrap().key_id, key_b);
    assert_eq!(trending.get(0).unwrap().volume_24h, 200);
    assert_eq!(trending.get(1).unwrap().key_id, key_c);
    assert_eq!(trending.get(1).unwrap().volume_24h, 100);
    assert_eq!(trending.get(2).unwrap().key_id, key_a);
    assert_eq!(trending.get(2).unwrap().volume_24h, 50);

    let top2 = client.get_trending(&2);
    assert_eq!(top2.len(), 2);
    assert_eq!(top2.get(0).unwrap().key_id, key_b);
    assert_eq!(top2.get(1).unwrap().key_id, key_c);
}

#[test]
fn trending_excludes_zero_volume_and_expired() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let creator = Address::generate(&env);
    let key_a = Address::generate(&env);
    let key_b = Address::generate(&env);
    client.register_key(&admin, &key_a, &creator);
    client.register_key(&admin, &key_b, &creator);

    client.record_volume(&admin, &key_a, &30);
    // key_b never trades.

    let trending = client.get_trending(&10);
    assert_eq!(trending.len(), 1);
    assert_eq!(trending.get(0).unwrap().key_id, key_a);

    // Expire key_a's volume; nothing remains trending.
    advance(&env, VOLUME_WINDOW_24H + 1);
    assert_eq!(client.get_trending(&10).len(), 0);
}

#[test]
fn trending_limit_zero_is_empty() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let creator = Address::generate(&env);
    let key = Address::generate(&env);
    client.register_key(&admin, &key, &creator);
    client.record_volume(&admin, &key, &10);
    assert_eq!(client.get_trending(&0).len(), 0);
}

// --- Acceptance: get_new_keys returns correct keys since timestamp ---

#[test]
fn new_keys_returns_recent_newest_first() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let creator = Address::generate(&env);
    let key_a = Address::generate(&env);
    let key_b = Address::generate(&env);
    let key_c = Address::generate(&env);
    client.register_key(&admin, &key_a, &creator); // T0
    advance(&env, 10);
    client.register_key(&admin, &key_b, &creator); // T0+10
    advance(&env, 10);
    client.register_key(&admin, &key_c, &creator); // T0+20

    // Everything since T0, newest first.
    let all = client.get_new_keys(&T0, &10);
    assert_eq!(all.len(), 3);
    assert_eq!(all.get(0).unwrap(), key_c);
    assert_eq!(all.get(1).unwrap(), key_b);
    assert_eq!(all.get(2).unwrap(), key_a);

    // Only keys deployed at/after T0+10.
    let recent = client.get_new_keys(&(T0 + 10), &10);
    assert_eq!(recent.len(), 2);
    assert_eq!(recent.get(0).unwrap(), key_c);
    assert_eq!(recent.get(1).unwrap(), key_b);

    // Limit clamps the newest keys.
    let limited = client.get_new_keys(&T0, &2);
    assert_eq!(limited.len(), 2);
    assert_eq!(limited.get(0).unwrap(), key_c);
    assert_eq!(limited.get(1).unwrap(), key_b);

    // Nothing deployed yet at a future timestamp.
    assert_eq!(client.get_new_keys(&(T0 + 1000), &10).len(), 0);
    assert_eq!(client.get_new_keys(&T0, &0).len(), 0);
}

// --- Acceptance: volume pruning runs correctly beyond 48h window ---

#[test]
fn pruning_removes_records_beyond_48h() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let creator = Address::generate(&env);
    let key = Address::generate(&env);
    client.register_key(&admin, &key, &creator);

    client.record_volume(&admin, &key, &10);
    assert_eq!(client.get_volume_entries(&key).len(), 1);

    // Exactly at the 48h boundary the record is still retained.
    advance(&env, VOLUME_RETENTION_48H);
    assert!(!client.prune(&key));
    assert_eq!(client.get_volume_entries(&key).len(), 1);

    // One second past retention the record is pruned.
    advance(&env, 1);
    assert!(client.prune(&key));
    assert_eq!(client.get_volume_entries(&key).len(), 0);
}

#[test]
fn record_volume_prunes_stale_records_on_write() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    let creator = Address::generate(&env);
    let key = Address::generate(&env);
    client.register_key(&admin, &key, &creator);

    client.record_volume(&admin, &key, &10);
    advance(&env, VOLUME_RETENTION_48H + 1);
    client.record_volume(&admin, &key, &5);

    // The stale record was dropped; only the fresh one remains.
    let entries = client.get_volume_entries(&key);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries.get(0).unwrap().amount, 5);
    assert_eq!(client.get_volume_24h(&key), 5);
}

#[test]
fn initialise_is_once_only() {
    let env = Env::default();
    let (client, admin) = setup(&env);
    assert_eq!(
        client.try_initialise(&admin),
        Err(Ok(DiscoveryError::AlreadyInitialised))
    );
}
