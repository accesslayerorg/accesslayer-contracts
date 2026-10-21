#![cfg(test)]

use crate::{
    events::{
        AirdropClaimedEvent, AirdropCreatedEvent, AIRDROP_CLAIMED_EVENT_NAME,
        AIRDROP_CREATED_EVENT_NAME,
    },
    AirdropAllocation, ContractError, HolderRewardsContract, HolderRewardsContractClient,
    MAX_AIRDROP_HOLDERS,
};
use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, Events},
    token::{StellarAssetClient, TokenClient},
    Address, Env, IntoVal, Symbol, TryFromVal, Val, Vec,
};

// ── Mock creator-keys contract ────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum MockKey {
    Registry(Address),
    Balance(Address, Address),
}

/// Minimal stand-in for the creator-keys contract exposing the read-only
/// interface `holder-rewards` depends on.
#[contract]
pub struct MockCreatorKeys;

#[contractimpl]
impl MockCreatorKeys {
    pub fn set_holder_registry(env: Env, key_id: Address, holders: Vec<Address>) {
        env.storage()
            .instance()
            .set(&MockKey::Registry(key_id), &holders);
    }

    pub fn set_key_balance(env: Env, creator: Address, wallet: Address, balance: u32) {
        env.storage()
            .instance()
            .set(&MockKey::Balance(creator, wallet), &balance);
    }

    pub fn get_holder_registry(env: Env, key_id: Address) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&MockKey::Registry(key_id))
            .unwrap_or_else(|| Vec::new(&env))
    }

    pub fn get_key_balance(env: Env, creator: Address, wallet: Address) -> u32 {
        env.storage()
            .instance()
            .get(&MockKey::Balance(creator, wallet))
            .unwrap_or(0)
    }
}

// ── Setup helpers ─────────────────────────────────────────────────────────────

const REWARD_TOTAL: i128 = 1_000;

fn setup(
    env: &Env,
) -> (
    HolderRewardsContractClient<'_>,
    MockCreatorKeysClient<'_>,
    Address,
    Address,
) {
    let id = env.register(HolderRewardsContract, ());
    let client = HolderRewardsContractClient::new(env, &id);
    let mock_id = env.register(MockCreatorKeys, ());
    let mock = MockCreatorKeysClient::new(env, &mock_id);
    let admin = Address::generate(env);
    client.init(&admin, &mock_id);
    (client, mock, admin, id)
}

/// Register a reward token, mint `mint_amount` to `funder`, and return the
/// token contract address.
fn setup_token(env: &Env, funder: &Address, mint_amount: i128) -> Address {
    let sac = env.register_stellar_asset_contract_v2(Address::generate(env));
    let token_addr = sac.address();
    StellarAssetClient::new(env, &token_addr).mint(funder, &mint_amount);
    token_addr
}

/// Register `key_id` with holders `[(wallet, balance), ...]` in the mock.
fn setup_holders(
    env: &Env,
    mock: &MockCreatorKeysClient,
    key_id: &Address,
    holders: &[(Address, u32)],
) {
    let mut registry = Vec::new(env);
    for (wallet, balance) in holders {
        registry.push_back(wallet.clone());
        mock.set_key_balance(key_id, wallet, balance);
    }
    mock.set_holder_registry(key_id, &registry);
}

fn get_events<T>(env: &Env, topic_symbol: &str) -> Vec<T>
where
    T: IntoVal<Env, Val> + TryFromVal<Env, Val>,
{
    let mut matching = Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        if let Some(first) = topics.get(0) {
            if Symbol::try_from_val(env, &first) == Ok(Symbol::new(env, topic_symbol)) {
                if let Ok(event) = T::try_from_val(env, &data) {
                    matching.push_back(event);
                }
            }
        }
    }
    matching
}

// ── Pro-rata allocation ───────────────────────────────────────────────────────

#[test]
fn test_pro_rata_allocation_computed_from_snapshot_balances() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, mock, _, _) = setup(&env);

    let key_id = Address::generate(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    setup_holders(
        &env,
        &mock,
        &key_id,
        &[(alice.clone(), 30), (bob.clone(), 70)],
    );

    let funder = key_id.clone(); // the key creator funds the airdrop
    let token = setup_token(&env, &funder, REWARD_TOTAL);
    let airdrop_id = client.create_airdrop(&key_id, &funder, &token, &REWARD_TOTAL);

    // Alice holds 30/100 keys → 30% of 1_000; Bob holds 70/100 → 70%.
    let alice_alloc = client.get_airdrop_allocation(&airdrop_id, &alice);
    assert_eq!(
        alice_alloc,
        AirdropAllocation {
            claimable: 300,
            claimed: 0
        }
    );
    let bob_alloc = client.get_airdrop_allocation(&airdrop_id, &bob);
    assert_eq!(
        bob_alloc,
        AirdropAllocation {
            claimable: 700,
            claimed: 0
        }
    );
}

#[test]
fn test_claim_transfers_pro_rata_share() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, mock, _, contract_id) = setup(&env);

    let key_id = Address::generate(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    setup_holders(
        &env,
        &mock,
        &key_id,
        &[(alice.clone(), 30), (bob.clone(), 70)],
    );

    let funder = key_id.clone(); // the key creator funds the airdrop
    let token = setup_token(&env, &funder, REWARD_TOTAL);
    let airdrop_id = client.create_airdrop(&key_id, &funder, &token, &REWARD_TOTAL);

    let claimed = client.claim_airdrop(&airdrop_id, &alice);
    assert_eq!(claimed, 300);

    let token_client = TokenClient::new(&env, &token);
    assert_eq!(token_client.balance(&alice), 300);
    assert_eq!(token_client.balance(&contract_id), 700);

    let claimed_bob = client.claim_airdrop(&airdrop_id, &bob);
    assert_eq!(claimed_bob, 700);
    assert_eq!(token_client.balance(&bob), 700);
    assert_eq!(token_client.balance(&contract_id), 0);
}

// ── Double claim ──────────────────────────────────────────────────────────────

#[test]
fn test_double_claim_reverts_with_already_claimed() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, mock, _, _) = setup(&env);

    let key_id = Address::generate(&env);
    let alice = Address::generate(&env);
    setup_holders(&env, &mock, &key_id, &[(alice.clone(), 100)]);

    let funder = key_id.clone(); // the key creator funds the airdrop
    let token = setup_token(&env, &funder, REWARD_TOTAL);
    let airdrop_id = client.create_airdrop(&key_id, &funder, &token, &REWARD_TOTAL);

    assert_eq!(client.claim_airdrop(&airdrop_id, &alice), 1_000);

    let result = client.try_claim_airdrop(&airdrop_id, &alice);
    let err = result.unwrap_err().unwrap();
    assert!(
        matches!(err, ContractError::AlreadyClaimed),
        "expected AlreadyClaimed, got {err:?}"
    );
}

// ── Snapshot isolation ────────────────────────────────────────────────────────

#[test]
fn test_post_snapshot_balance_changes_do_not_affect_allocation() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, mock, _, _) = setup(&env);

    let key_id = Address::generate(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    setup_holders(
        &env,
        &mock,
        &key_id,
        &[(alice.clone(), 30), (bob.clone(), 70)],
    );

    let funder = key_id.clone(); // the key creator funds the airdrop
    let token = setup_token(&env, &funder, REWARD_TOTAL);
    let airdrop_id = client.create_airdrop(&key_id, &funder, &token, &REWARD_TOTAL);

    // After the snapshot: Alice dumps all her keys, Bob doubles his, and a
    // new holder arrives — none of this may change the frozen allocation.
    mock.set_key_balance(&key_id, &alice, &0u32);
    mock.set_key_balance(&key_id, &bob, &140u32);
    let carol = Address::generate(&env);
    mock.set_key_balance(&key_id, &carol, &10u32);

    let alice_alloc = client.get_airdrop_allocation(&airdrop_id, &alice);
    assert_eq!(
        alice_alloc,
        AirdropAllocation {
            claimable: 300,
            claimed: 0
        }
    );
    let bob_alloc = client.get_airdrop_allocation(&airdrop_id, &bob);
    assert_eq!(
        bob_alloc,
        AirdropAllocation {
            claimable: 700,
            claimed: 0
        }
    );

    // Alice sold everything but can still claim her snapshot share.
    assert_eq!(client.claim_airdrop(&airdrop_id, &alice), 300);
    // Carol bought after the snapshot and gets nothing.
    let carol_alloc = client.get_airdrop_allocation(&airdrop_id, &carol);
    assert_eq!(
        carol_alloc,
        AirdropAllocation {
            claimable: 0,
            claimed: 0
        }
    );
    let result = client.try_claim_airdrop(&airdrop_id, &carol);
    let err = result.unwrap_err().unwrap();
    assert!(
        matches!(err, ContractError::NoAllocation),
        "expected NoAllocation, got {err:?}"
    );
}

// ── Allocation view ───────────────────────────────────────────────────────────

#[test]
fn test_get_airdrop_allocation_tracks_claimed_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, mock, _, _) = setup(&env);

    let key_id = Address::generate(&env);
    let alice = Address::generate(&env);
    setup_holders(&env, &mock, &key_id, &[(alice.clone(), 100)]);

    let funder = key_id.clone(); // the key creator funds the airdrop
    let token = setup_token(&env, &funder, REWARD_TOTAL);
    let airdrop_id = client.create_airdrop(&key_id, &funder, &token, &REWARD_TOTAL);

    // Before claiming: full allocation claimable, nothing claimed.
    assert_eq!(
        client.get_airdrop_allocation(&airdrop_id, &alice),
        AirdropAllocation {
            claimable: 1_000,
            claimed: 0
        }
    );

    client.claim_airdrop(&airdrop_id, &alice);

    // After claiming: nothing claimable, full allocation reported as claimed.
    assert_eq!(
        client.get_airdrop_allocation(&airdrop_id, &alice),
        AirdropAllocation {
            claimable: 0,
            claimed: 1_000
        }
    );
}

#[test]
fn test_get_airdrop_allocation_returns_zero_for_unknown_wallet() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, mock, _, _) = setup(&env);

    let key_id = Address::generate(&env);
    let alice = Address::generate(&env);
    setup_holders(&env, &mock, &key_id, &[(alice.clone(), 100)]);

    let funder = key_id.clone(); // the key creator funds the airdrop
    let token = setup_token(&env, &funder, REWARD_TOTAL);
    let airdrop_id = client.create_airdrop(&key_id, &funder, &token, &REWARD_TOTAL);

    let stranger = Address::generate(&env);
    assert_eq!(
        client.get_airdrop_allocation(&airdrop_id, &stranger),
        AirdropAllocation {
            claimable: 0,
            claimed: 0
        }
    );
}

#[test]
fn test_get_airdrop_allocation_reverts_for_unknown_airdrop() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _, _) = setup(&env);

    let wallet = Address::generate(&env);
    let result = client.try_get_airdrop_allocation(&99u64, &wallet);
    let err = result.unwrap_err().unwrap();
    assert!(
        matches!(err, ContractError::AirdropNotFound),
        "expected AirdropNotFound, got {err:?}"
    );
}

// ── Events ────────────────────────────────────────────────────────────────────

#[test]
fn test_airdrop_created_event_emitted_with_correct_data() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, mock, _, _) = setup(&env);

    let key_id = Address::generate(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    setup_holders(
        &env,
        &mock,
        &key_id,
        &[(alice.clone(), 30), (bob.clone(), 70)],
    );

    let funder = key_id.clone(); // the key creator funds the airdrop
    let token = setup_token(&env, &funder, REWARD_TOTAL);
    let airdrop_id = client.create_airdrop(&key_id, &funder, &token, &REWARD_TOTAL);

    let events: Vec<AirdropCreatedEvent> = get_events(&env, AIRDROP_CREATED_EVENT_NAME);
    assert_eq!(events.len(), 1);
    let event = events.get(0).unwrap();
    assert_eq!(event.airdrop_id, airdrop_id);
    assert_eq!(event.key_id, key_id);
    assert_eq!(event.funder, funder);
    assert_eq!(event.reward_token, token);
    assert_eq!(event.total_amount, REWARD_TOTAL);
    assert_eq!(event.total_snapshot_supply, 100);
    assert_eq!(event.snapshot_ledger, env.ledger().sequence());
}

#[test]
fn test_airdrop_claimed_event_emitted_with_correct_data() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, mock, _, _) = setup(&env);

    let key_id = Address::generate(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    setup_holders(
        &env,
        &mock,
        &key_id,
        &[(alice.clone(), 30), (bob.clone(), 70)],
    );

    let funder = key_id.clone(); // the key creator funds the airdrop
    let token = setup_token(&env, &funder, REWARD_TOTAL);
    let airdrop_id = client.create_airdrop(&key_id, &funder, &token, &REWARD_TOTAL);

    client.claim_airdrop(&airdrop_id, &alice);

    let events: Vec<AirdropClaimedEvent> = get_events(&env, AIRDROP_CLAIMED_EVENT_NAME);
    assert_eq!(events.len(), 1);
    let event = events.get(0).unwrap();
    assert_eq!(event.airdrop_id, airdrop_id);
    assert_eq!(event.key_id, key_id);
    assert_eq!(event.wallet, alice);
    assert_eq!(event.amount, 300);
}

// ── Authorization and validation ──────────────────────────────────────────────

#[test]
fn test_create_airdrop_reverts_for_unauthorized_caller() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, mock, _, _) = setup(&env);

    let key_id = Address::generate(&env);
    let alice = Address::generate(&env);
    setup_holders(&env, &mock, &key_id, &[(alice.clone(), 100)]);

    let funder = key_id.clone(); // the key creator funds the airdrop
    let token = setup_token(&env, &funder, REWARD_TOTAL);
    let rando = Address::generate(&env);
    let result = client.try_create_airdrop(&key_id, &rando, &token, &REWARD_TOTAL);
    let err = result.unwrap_err().unwrap();
    assert!(
        matches!(err, ContractError::Unauthorized),
        "expected Unauthorized, got {err:?}"
    );
}

#[test]
fn test_admin_can_create_airdrop_for_any_key() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, mock, admin, _) = setup(&env);

    let key_id = Address::generate(&env);
    let alice = Address::generate(&env);
    setup_holders(&env, &mock, &key_id, &[(alice.clone(), 100)]);

    let token = setup_token(&env, &admin, REWARD_TOTAL);
    let airdrop_id = client.create_airdrop(&key_id, &admin, &token, &REWARD_TOTAL);
    assert_eq!(
        client.get_airdrop_allocation(&airdrop_id, &alice),
        AirdropAllocation {
            claimable: REWARD_TOTAL,
            claimed: 0
        }
    );
}

#[test]
fn test_create_airdrop_reverts_on_non_positive_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, mock, _, _) = setup(&env);

    let key_id = Address::generate(&env);
    let alice = Address::generate(&env);
    setup_holders(&env, &mock, &key_id, &[(alice.clone(), 100)]);

    let funder = key_id.clone(); // the key creator funds the airdrop
    let token = setup_token(&env, &funder, REWARD_TOTAL);
    let result = client.try_create_airdrop(&key_id, &funder, &token, &0i128);
    let err = result.unwrap_err().unwrap();
    assert!(
        matches!(err, ContractError::NotPositiveAmount),
        "expected NotPositiveAmount, got {err:?}"
    );
}

#[test]
fn test_create_airdrop_reverts_when_key_has_no_holders() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, mock, _, _) = setup(&env);

    let key_id = Address::generate(&env);
    // Registry contains only wallets whose balance has since gone to zero.
    let ghost = Address::generate(&env);
    setup_holders(&env, &mock, &key_id, &[(ghost, 0)]);

    let funder = key_id.clone(); // the key creator funds the airdrop
    let token = setup_token(&env, &funder, REWARD_TOTAL);
    let result = client.try_create_airdrop(&key_id, &funder, &token, &REWARD_TOTAL);
    let err = result.unwrap_err().unwrap();
    assert!(
        matches!(err, ContractError::NoHolders),
        "expected NoHolders, got {err:?}"
    );
}

#[test]
fn test_create_airdrop_reverts_when_holder_limit_exceeded() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, mock, _, _) = setup(&env);

    let key_id = Address::generate(&env);
    let mut registry = Vec::new(&env);
    for _ in 0..=MAX_AIRDROP_HOLDERS {
        let wallet = Address::generate(&env);
        mock.set_key_balance(&key_id, &wallet, &1u32);
        registry.push_back(wallet);
    }
    mock.set_holder_registry(&key_id, &registry);

    let funder = key_id.clone(); // the key creator funds the airdrop
    let token = setup_token(&env, &funder, REWARD_TOTAL);
    let result = client.try_create_airdrop(&key_id, &funder, &token, &REWARD_TOTAL);
    let err = result.unwrap_err().unwrap();
    assert!(
        matches!(err, ContractError::HolderLimitExceeded),
        "expected HolderLimitExceeded, got {err:?}"
    );
}

// ── Funding ───────────────────────────────────────────────────────────────────

#[test]
fn test_create_airdrop_escrows_funder_tokens() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, mock, _, contract_id) = setup(&env);

    let key_id = Address::generate(&env);
    let alice = Address::generate(&env);
    setup_holders(&env, &mock, &key_id, &[(alice.clone(), 100)]);

    let funder = key_id.clone(); // the key creator funds the airdrop
    let token = setup_token(&env, &funder, REWARD_TOTAL);
    client.create_airdrop(&key_id, &funder, &token, &REWARD_TOTAL);

    let token_client = TokenClient::new(&env, &token);
    assert_eq!(token_client.balance(&funder), 0);
    assert_eq!(token_client.balance(&contract_id), REWARD_TOTAL);
}

#[test]
fn test_multiple_airdrops_get_distinct_ids_and_independent_snapshots() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, mock, _, _) = setup(&env);

    let key_id = Address::generate(&env);
    let alice = Address::generate(&env);
    setup_holders(&env, &mock, &key_id, &[(alice.clone(), 100)]);

    let funder = key_id.clone(); // the key creator funds the airdrop
    let token = setup_token(&env, &funder, 2 * REWARD_TOTAL);
    let first = client.create_airdrop(&key_id, &funder, &token, &REWARD_TOTAL);
    let second = client.create_airdrop(&key_id, &funder, &token, &REWARD_TOTAL);
    assert_eq!(first, 1);
    assert_eq!(second, 2);

    // Both airdrops are claimable independently.
    assert_eq!(client.claim_airdrop(&first, &alice), REWARD_TOTAL);
    assert_eq!(client.claim_airdrop(&second, &alice), REWARD_TOTAL);
}
