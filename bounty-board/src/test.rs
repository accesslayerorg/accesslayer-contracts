use crate::{events, BountyBoard, BountyBoardClient, ContractError};
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Events as _, Ledger as _},
    Address, BytesN, Env, Symbol, TryFromVal, Vec as SdkVec,
};
use soroban_sdk::{token::StellarAssetClient, token::TokenClient};

const REWARD: i128 = 900;
const MAX_WINNERS: u32 = 2;
const DEADLINE_OFFSET: u32 = 10;

#[contract]
struct MockCreatorKeys;

#[contractimpl]
impl MockCreatorKeys {
    pub fn set_balance(env: Env, creator: Address, wallet: Address, balance: u32) {
        env.storage().instance().set(&(creator, wallet), &balance);
    }

    pub fn get_balance(env: Env, creator: Address, wallet: Address) -> u32 {
        env.storage()
            .instance()
            .get(&(creator, wallet))
            .unwrap_or(0)
    }
}

struct Fixture<'a> {
    client: BountyBoardClient<'a>,
    sac: StellarAssetClient<'a>,
    token: TokenClient<'a>,
    keys: MockCreatorKeysClient<'a>,
    creator: Address,
    key_id: Address,
    contract_id: Address,
}

fn setup(env: &Env, min_holder_balance: u32) -> Fixture<'_> {
    env.mock_all_auths();

    let creator = Address::generate(env);
    let key_id = Address::generate(env);
    let contract_id = env.register(BountyBoard, ());
    let client = BountyBoardClient::new(env, &contract_id);

    let token_admin = Address::generate(env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    let sac = StellarAssetClient::new(env, &token_address);
    let token = TokenClient::new(env, &token_address);

    let keys_id = env.register(MockCreatorKeys, ());
    let keys = MockCreatorKeysClient::new(env, &keys_id);

    client.init(&token_address, &keys_id, &min_holder_balance);

    Fixture {
        client,
        sac,
        token,
        keys,
        creator,
        key_id,
        contract_id,
    }
}

fn deadline(env: &Env) -> u32 {
    env.ledger().get().sequence_number + DEADLINE_OFFSET
}

fn post(f: &Fixture<'_>, reward: i128, max_winners: u32, deadline_ledger: u32) -> u64 {
    f.sac.mint(&f.creator, &reward);
    f.client.post_bounty(
        &f.creator,
        &f.key_id,
        &reward,
        &deadline_ledger,
        &max_winners,
    )
}

fn proof(env: &Env, seed: u8) -> BytesN<32> {
    let mut bytes = [0u8; 32];
    bytes[0] = seed;
    BytesN::from_array(env, &bytes)
}

fn set_ledger_sequence(env: &Env, sequence: u32) {
    let mut ledger = env.ledger().get();
    ledger.sequence_number = sequence;
    env.ledger().set(ledger);
}

fn record_event_names(env: &Env, names: &mut SdkVec<Symbol>) {
    for (_, topics, _) in env.events().all().iter() {
        let first = topics.get(0).unwrap();
        names.push_back(Symbol::try_from_val(env, &first).unwrap());
    }
}

#[test]
fn post_bounty_escrows_full_reward() {
    let env = Env::default();
    let f = setup(&env, 0);
    let deadline_ledger = deadline(&env);

    let bounty_id = post(&f, REWARD, MAX_WINNERS, deadline_ledger);
    let bounty = f.client.get_bounty(&bounty_id);

    assert_eq!(bounty.bounty_id, bounty_id);
    assert_eq!(bounty.creator, f.creator);
    assert_eq!(bounty.key_id, f.key_id);
    assert_eq!(bounty.reward, REWARD);
    assert_eq!(bounty.deadline_ledger, deadline_ledger);
    assert_eq!(bounty.max_winners, MAX_WINNERS);
    assert_eq!(bounty.winners, 0);
    assert_eq!(bounty.paid, 0);
    assert_eq!(f.token.balance(&f.creator), 0);
    assert_eq!(f.token.balance(&f.contract_id), REWARD);
}

#[test]
fn award_pays_per_winner_and_enforces_limit() {
    let env = Env::default();
    let f = setup(&env, 0);
    let bounty_id = post(&f, REWARD, MAX_WINNERS, deadline(&env));
    let first = Address::generate(&env);
    let second = Address::generate(&env);
    let third = Address::generate(&env);

    f.client.submit(&first, &bounty_id, &proof(&env, 1));
    f.client.submit(&second, &bounty_id, &proof(&env, 2));
    f.client.submit(&third, &bounty_id, &proof(&env, 3));

    let amount = REWARD / MAX_WINNERS as i128;
    assert_eq!(f.client.award(&f.creator, &bounty_id, &first), amount);
    assert_eq!(f.client.award(&f.creator, &bounty_id, &second), amount);
    assert_eq!(
        f.client.try_award(&f.creator, &bounty_id, &third),
        Err(Ok(ContractError::MaxWinnersReached))
    );

    assert_eq!(f.token.balance(&first), amount);
    assert_eq!(f.token.balance(&second), amount);
    assert_eq!(f.token.balance(&third), 0);
    assert_eq!(f.token.balance(&f.contract_id), REWARD - 2 * amount);

    let bounty = f.client.get_bounty(&bounty_id);
    assert_eq!(bounty.winners, MAX_WINNERS);
    assert_eq!(bounty.paid, 2 * amount);
    assert!(f.client.get_submission(&bounty_id, &first).awarded);
}

#[test]
fn rejection_frees_worker_to_resubmit_but_not_two_active_submissions() {
    let env = Env::default();
    let f = setup(&env, 0);
    let bounty_id = post(&f, REWARD, MAX_WINNERS, deadline(&env));
    let worker = Address::generate(&env);
    let outsider = Address::generate(&env);

    f.client.submit(&worker, &bounty_id, &proof(&env, 1));
    assert_eq!(
        f.client.try_submit(&worker, &bounty_id, &proof(&env, 2)),
        Err(Ok(ContractError::AlreadyActiveSubmission))
    );
    assert_eq!(
        f.client
            .try_reject_submission(&outsider, &bounty_id, &worker),
        Err(Ok(ContractError::Unauthorized))
    );

    f.client.reject_submission(&f.creator, &bounty_id, &worker);
    assert!(!f.client.get_submission(&bounty_id, &worker).active);

    let replacement = proof(&env, 2);
    f.client.submit(&worker, &bounty_id, &replacement);
    let submission = f.client.get_submission(&bounty_id, &worker);
    assert!(submission.active);
    assert!(!submission.awarded);
    assert_eq!(submission.proof_hash, replacement);
}

#[test]
fn reclaim_returns_only_unpaid_rewards_after_deadline() {
    let env = Env::default();
    let f = setup(&env, 0);
    let deadline_ledger = deadline(&env);
    let bounty_id = post(&f, REWARD, MAX_WINNERS, deadline_ledger);
    let winner = Address::generate(&env);

    f.client.submit(&winner, &bounty_id, &proof(&env, 1));
    let paid = f.client.award(&f.creator, &bounty_id, &winner);

    assert_eq!(
        f.client.try_reclaim_unawarded(&f.creator, &bounty_id),
        Err(Ok(ContractError::DeadlineNotReached))
    );

    set_ledger_sequence(&env, deadline_ledger);
    assert_eq!(
        f.client.reclaim_unawarded(&f.creator, &bounty_id),
        REWARD - paid
    );
    assert_eq!(f.token.balance(&f.creator), REWARD - paid);
    assert_eq!(f.token.balance(&winner), paid);
    assert_eq!(f.token.balance(&f.contract_id), 0);
    assert!(f.client.get_bounty(&bounty_id).reclaimed);
    assert_eq!(
        f.client.try_reclaim_unawarded(&f.creator, &bounty_id),
        Err(Ok(ContractError::AlreadyReclaimed))
    );
}

#[test]
fn holder_gated_bounties_reject_non_holders() {
    let env = Env::default();
    let f = setup(&env, 5);
    let bounty_id = post(&f, REWARD, MAX_WINNERS, deadline(&env));
    let non_holder = Address::generate(&env);
    let holder = Address::generate(&env);

    f.keys.set_balance(&f.key_id, &non_holder, &4);
    assert_eq!(
        f.client
            .try_submit(&non_holder, &bounty_id, &proof(&env, 1)),
        Err(Ok(ContractError::NotAHolder))
    );

    f.keys.set_balance(&f.key_id, &holder, &5);
    f.client.submit(&holder, &bounty_id, &proof(&env, 2));
    assert!(f.client.get_submission(&bounty_id, &holder).active);
}

#[test]
fn lifecycle_events_are_emitted() {
    let env = Env::default();
    let f = setup(&env, 0);
    let deadline_ledger = deadline(&env);
    let mut names = SdkVec::new(&env);
    let bounty_id = post(&f, REWARD, MAX_WINNERS, deadline_ledger);
    record_event_names(&env, &mut names);
    let worker = Address::generate(&env);

    f.client.submit(&worker, &bounty_id, &proof(&env, 1));
    record_event_names(&env, &mut names);
    f.client.award(&f.creator, &bounty_id, &worker);
    record_event_names(&env, &mut names);
    set_ledger_sequence(&env, deadline_ledger);
    f.client.reclaim_unawarded(&f.creator, &bounty_id);
    record_event_names(&env, &mut names);

    assert!(names.contains(Symbol::new(&env, events::BOUNTY_POSTED_EVENT_NAME)));
    assert!(names.contains(Symbol::new(&env, events::SUBMISSION_MADE_EVENT_NAME)));
    assert!(names.contains(Symbol::new(&env, events::BOUNTY_AWARDED_EVENT_NAME)));
    assert!(names.contains(Symbol::new(&env, events::BOUNTY_RECLAIMED_EVENT_NAME)));
}
