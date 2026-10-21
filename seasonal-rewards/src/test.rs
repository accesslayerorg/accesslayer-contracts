#![cfg(test)]

use crate::{
    CampaignCreated, Criteria, Criterion, RewardClaimed, SeasonalError, SeasonalRewardsContract,
    SeasonalRewardsContractClient, WalletQualified, WalletStats, CAMPAIGN_CREATED_EVENT,
    REWARD_CLAIMED_EVENT, WALLET_QUALIFIED_EVENT,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, IntoVal, Symbol, TryFromVal, Val, Vec,
};

const DAY: u64 = 86_400;

fn setup() -> (Env, SeasonalRewardsContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SeasonalRewardsContract, ());
    let client = SeasonalRewardsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

fn stats(volume: i128, trade_count: u32, hold_seconds: u64) -> WalletStats {
    WalletStats {
        volume,
        trade_count,
        hold_seconds,
    }
}

fn get_events<T>(env: &Env, topic: Symbol) -> Vec<T>
where
    T: IntoVal<Env, Val> + TryFromVal<Env, Val>,
{
    let mut matching = Vec::new(env);
    for (_, topics, data) in env.events().all().iter() {
        if let Some(first) = topics.get(0) {
            if Symbol::try_from_val(env, &first) == Ok(topic.clone()) {
                if let Ok(ev) = T::try_from_val(env, &data) {
                    matching.push_back(ev);
                }
            }
        }
    }
    matching
}

#[test]
fn create_campaign_by_admin_succeeds() {
    let (env, client, _admin) = setup();
    env.ledger().set_timestamp(1_000);

    let key_id = Address::generate(&env);
    let id = client.create_campaign(
        &key_id,
        &1_000,
        &2_000,
        &1_000_000i128,
        &Criteria {
            kind: Criterion::Volume,
            threshold: 500_000,
        },
    );
    assert_eq!(id, 1);

    let c = client.get_campaign(&id);
    assert_eq!(c.reward_pool, 1_000_000);
    assert_eq!(c.total_score, 0);
    assert_eq!(c.total_claimed, 0);
}

#[test]
fn create_campaign_rejects_invalid_window() {
    let (env, client, _admin) = setup();
    env.ledger().set_timestamp(1_000);

    let key_id = Address::generate(&env);
    let res = client.try_create_campaign(
        &key_id,
        &2_000,
        &1_500,
        &1_000_000,
        &Criteria {
            kind: Criterion::Volume,
            threshold: 500_000,
        },
    );
    assert_eq!(res.unwrap_err().unwrap(), SeasonalError::InvalidTimeWindow);
}

#[test]
fn qualify_min_volume_accepts_above_and_rejects_below() {
    let (env, client, _admin) = setup();
    env.ledger().set_timestamp(1_000);
    let key_id = Address::generate(&env);
    let id = client.create_campaign(
        &key_id,
        &1_000,
        &2_000,
        &1_000_000,
        &Criteria {
            kind: Criterion::Volume,
            threshold: 500_000,
        },
    );

    let above = Address::generate(&env);
    let below = Address::generate(&env);
    assert!(client.qualify_wallet(&id, &above, &stats(600_000, 0, 0)));
    assert!(!client.qualify_wallet(&id, &below, &stats(400_000, 0, 0)));
}

#[test]
fn qualify_min_trades_accepts_above_and_rejects_below() {
    let (env, client, _admin) = setup();
    env.ledger().set_timestamp(1_000);
    let key_id = Address::generate(&env);
    let id = client.create_campaign(
        &key_id,
        &1_000,
        &2_000,
        &1_000_000,
        &Criteria {
            kind: Criterion::Trades,
            threshold: 10,
        },
    );

    let above = Address::generate(&env);
    let below = Address::generate(&env);
    assert!(client.qualify_wallet(&id, &above, &stats(0, 12, 0)));
    assert!(!client.qualify_wallet(&id, &below, &stats(0, 8, 0)));
}

#[test]
fn qualify_min_hold_days_accepts_above_and_rejects_below() {
    let (env, client, _admin) = setup();
    env.ledger().set_timestamp(1_000);
    let key_id = Address::generate(&env);
    let id = client.create_campaign(
        &key_id,
        &1_000,
        &2_000,
        &1_000_000,
        &Criteria {
            kind: Criterion::HoldDays,
            threshold: 7,
        },
    );

    let above = Address::generate(&env);
    let below = Address::generate(&env);
    assert!(client.qualify_wallet(&id, &above, &stats(0, 0, 8 * DAY)));
    assert!(!client.qualify_wallet(&id, &below, &stats(0, 0, 3 * DAY)));
}

#[test]
fn qualify_is_idempotent_and_does_not_double_count() {
    let (env, client, _admin) = setup();
    env.ledger().set_timestamp(1_000);
    let key_id = Address::generate(&env);
    let id = client.create_campaign(
        &key_id,
        &1_000,
        &2_000,
        &1_000_000,
        &Criteria {
            kind: Criterion::Volume,
            threshold: 100,
        },
    );

    let w = Address::generate(&env);
    assert!(client.qualify_wallet(&id, &w, &stats(500, 0, 0)));
    assert!(client.qualify_wallet(&id, &w, &stats(500, 0, 0)));
    let c = client.get_campaign(&id);
    assert_eq!(c.total_score, 500);
}

#[test]
fn claim_before_end_returns_campaign_active() {
    let (env, client, _admin) = setup();
    env.ledger().set_timestamp(1_000);
    let key_id = Address::generate(&env);
    let id = client.create_campaign(
        &key_id,
        &1_000,
        &2_000,
        &1_000_000,
        &Criteria {
            kind: Criterion::Volume,
            threshold: 100,
        },
    );

    let w = Address::generate(&env);
    client.qualify_wallet(&id, &w, &stats(500, 0, 0));

    env.ledger().set_timestamp(1_500);
    let err = client
        .try_claim_campaign_reward(&id, &w)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, SeasonalError::CampaignActive);
}

#[test]
fn pro_rata_distribution_matches_score_ratios() {
    let (env, client, _admin) = setup();
    env.ledger().set_timestamp(1_000);
    let key_id = Address::generate(&env);
    let id = client.create_campaign(
        &key_id,
        &1_000,
        &2_000,
        &600,
        &Criteria {
            kind: Criterion::Volume,
            threshold: 1,
        },
    );

    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);
    client.qualify_wallet(&id, &a, &stats(100, 0, 0));
    client.qualify_wallet(&id, &b, &stats(200, 0, 0));
    client.qualify_wallet(&id, &c, &stats(300, 0, 0));

    env.ledger().set_timestamp(3_000);
    assert_eq!(client.claim_campaign_reward(&id, &a), 100);
    assert_eq!(client.claim_campaign_reward(&id, &b), 200);
    assert_eq!(client.claim_campaign_reward(&id, &c), 300);

    let campaign = client.get_campaign(&id);
    assert_eq!(campaign.total_claimed, 600);
}

#[test]
fn claim_twice_returns_already_claimed() {
    let (env, client, _admin) = setup();
    env.ledger().set_timestamp(1_000);
    let key_id = Address::generate(&env);
    let id = client.create_campaign(
        &key_id,
        &1_000,
        &2_000,
        &1_000,
        &Criteria {
            kind: Criterion::Volume,
            threshold: 1,
        },
    );
    let w = Address::generate(&env);
    client.qualify_wallet(&id, &w, &stats(100, 0, 0));
    env.ledger().set_timestamp(3_000);
    client.claim_campaign_reward(&id, &w);
    let err = client
        .try_claim_campaign_reward(&id, &w)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, SeasonalError::AlreadyClaimed);
}

#[test]
fn unqualified_wallet_cannot_claim() {
    let (env, client, _admin) = setup();
    env.ledger().set_timestamp(1_000);
    let key_id = Address::generate(&env);
    let id = client.create_campaign(
        &key_id,
        &1_000,
        &2_000,
        &1_000,
        &Criteria {
            kind: Criterion::Volume,
            threshold: 100,
        },
    );
    let below = Address::generate(&env);
    client.qualify_wallet(&id, &below, &stats(50, 0, 0));
    env.ledger().set_timestamp(3_000);
    let err = client
        .try_claim_campaign_reward(&id, &below)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, SeasonalError::NotQualified);
}

#[test]
fn all_three_events_emitted_with_expected_data() {
    let (env, client, admin) = setup();
    env.ledger().set_timestamp(1_000);
    let key_id = Address::generate(&env);
    let id = client.create_campaign(
        &key_id,
        &1_000,
        &2_000,
        &1_000,
        &Criteria {
            kind: Criterion::Volume,
            threshold: 100,
        },
    );

    let created: Vec<CampaignCreated> = get_events(&env, CAMPAIGN_CREATED_EVENT);
    assert_eq!(created.len(), 1);
    assert_eq!(created.get(0).unwrap().campaign_id, id);
    assert_eq!(created.get(0).unwrap().admin, admin);

    let w = Address::generate(&env);
    client.qualify_wallet(&id, &w, &stats(500, 0, 0));
    let qualified: Vec<WalletQualified> = get_events(&env, WALLET_QUALIFIED_EVENT);
    assert_eq!(qualified.len(), 1);
    assert_eq!(qualified.get(0).unwrap().wallet, w);
    assert_eq!(qualified.get(0).unwrap().score, 500);

    env.ledger().set_timestamp(3_000);
    let payout = client.claim_campaign_reward(&id, &w);
    let claimed: Vec<RewardClaimed> = get_events(&env, REWARD_CLAIMED_EVENT);
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed.get(0).unwrap().wallet, w);
    assert_eq!(claimed.get(0).unwrap().amount, payout);
}
