#![cfg(test)]

use crate::{
    dynamic_fees::VolumeBracket, ContractError, CreatorKeysContract, CreatorKeysContractClient,
    FeeDiscountTier, RegisterCreatorParams, SwapDirection,
};
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    Address, Env, IntoVal, String, Symbol,
};

fn setup() -> (Env, CreatorKeysContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &100i128);
    client.set_curve_slope(&admin, &100i128);
    client.set_circuit_breaker_threshold(&admin, &10_000);
    (env, client, admin)
}

fn register_creator(
    env: &Env,
    client: &CreatorKeysContractClient,
    creator: &Address,
    handle: &str,
) {
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(env, handle),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
}

#[test]
fn split_waits_24_hours_and_scales_supply_balances_and_price() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let holder = Address::generate(&env);
    register_creator(&env, &client, &creator, "splitkey");
    client.buy_key(&creator, &holder, &10_000, &None);
    client.buy_key(&creator, &holder, &10_000, &None);
    let price_before = client.get_price_at_supply(&creator, &2);

    let proposal_id = client.propose_split(&creator, &creator, &2);
    let (_, proposed_topics, proposed_data) = env.events().all().last().unwrap();
    let proposed_event: Symbol = proposed_topics.get(0).unwrap().into_val(&env);
    assert_eq!(proposed_event, Symbol::new(&env, "SplitProposed"));
    let proposed_key: Address = proposed_topics.get(1).unwrap().into_val(&env);
    let proposed_payload: (u32, u32, u32, u64) = proposed_data.into_val(&env);
    assert_eq!(proposed_key, creator);
    assert_eq!(
        proposed_payload,
        (proposal_id, 2, 2, env.ledger().timestamp() + 24 * 60 * 60)
    );
    assert_eq!(
        client.try_execute_split(&proposal_id),
        Err(Ok(ContractError::TimelockNotElapsed))
    );
    env.ledger()
        .with_mut(|ledger| ledger.timestamp += 24 * 60 * 60);
    assert_eq!(client.execute_split(&proposal_id), 4);
    let (_, topics, data) = env.events().all().last().unwrap();
    let event: Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(event, Symbol::new(&env, "SplitExecuted"));
    let event_key: Address = topics.get(1).unwrap().into_val(&env);
    let payload: (u32, u32, u32) = data.into_val(&env);
    assert_eq!(event_key, creator);
    assert_eq!(payload, (proposal_id, 2, 4));
    assert_eq!(client.get_key_balance(&creator, &holder), 4);
    assert_eq!(client.get_creator_supply(&creator), 4);
    assert_eq!(client.get_price_at_supply(&creator, &4), price_before / 2);
}

#[test]
fn wallet_discount_uses_rolling_volume_and_is_applied_to_swap_fee() {
    let (env, client, admin) = setup();
    let creator = Address::generate(&env);
    let trader = Address::generate(&env);
    let treasury = Address::generate(&env);
    register_creator(&env, &client, &creator, "discount");
    client.set_protocol_fee(&admin, &Some(1_000), &treasury);
    client.set_fee_config(&admin, &10_000, &0);
    client.set_fee_config(&admin, &9_000, &1_000);
    let volume_brackets = soroban_sdk::Vec::from_array(
        &env,
        [VolumeBracket {
            min_volume: 0,
            fee_bps: 1_000,
        }],
    );
    client.set_volume_brackets(&admin, &volume_brackets);
    let tiers = soroban_sdk::Vec::from_array(
        &env,
        [
            FeeDiscountTier {
                volume_threshold: 0,
                discount_bps: 0,
            },
            FeeDiscountTier {
                volume_threshold: 100,
                discount_bps: 1_000,
            },
            FeeDiscountTier {
                volume_threshold: 200,
                discount_bps: 2_500,
            },
        ],
    );
    client.set_discount_tiers(&admin, &tiers);
    assert_eq!(client.get_fee_discount(&trader), 0);
    assert_eq!(
        client.try_set_discount_tiers(&trader, &tiers),
        Err(Ok(ContractError::Unauthorized))
    );

    client.swap(&creator, &trader, &1, &SwapDirection::Buy, &0);
    assert_eq!(client.get_fee_discount(&trader), 1_000);
    assert_eq!(client.get_wallet_rolling_volume(&trader), 100);

    let before_fee = client.get_treasury_balance();
    client.swap(&creator, &trader, &1, &SwapDirection::Buy, &0);
    let discounted_fee = client.get_treasury_balance() - before_fee;
    assert_eq!(discounted_fee, 36);

    env.ledger()
        .with_mut(|ledger| ledger.timestamp += 30 * 24 * 60 * 60 + 3_600);
    assert_eq!(client.get_wallet_rolling_volume(&trader), 0);
    assert_eq!(client.get_fee_discount(&trader), 0);
}

#[test]
fn secondary_sale_routes_royalty_and_claims_proceeds() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    register_creator(&env, &client, &creator, "royalty");
    client.buy_key(&creator, &seller, &10_000, &None);
    client.set_royalty_rate(&creator, &creator, &500);
    assert_eq!(client.get_royalty_rate(&creator), 500);

    let sale = client.secondary_market_buy(&creator, &buyer, &seller, &1, &20_000);
    assert_eq!(sale.royalty_amount, 1_000);
    assert_eq!(sale.seller_proceeds, 19_000);
    let (_, topics, data) = env.events().all().last().unwrap();
    let event: Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(event, Symbol::new(&env, "RoyaltyPaid"));
    let event_key: Address = topics.get(1).unwrap().into_val(&env);
    let event_buyer: Address = topics.get(2).unwrap().into_val(&env);
    let event_seller: Address = topics.get(3).unwrap().into_val(&env);
    assert_eq!(event_key, creator);
    assert_eq!(event_buyer, buyer);
    assert_eq!(event_seller, seller);
    let paid: i128 = data.into_val(&env);
    assert_eq!(paid, 1_000);

    assert_eq!(client.get_key_balance(&creator, &seller), 0);
    assert_eq!(client.get_key_balance(&creator, &buyer), 1);
    assert_eq!(client.claim_royalties(&creator, &creator), 1_000);
    assert_eq!(client.claim_secondary_proceeds(&creator, &seller), 19_000);
}

#[test]
fn merge_requires_66_percent_and_migrates_balances() {
    let (env, client, _admin) = setup();
    let key_a = Address::generate(&env);
    let key_b = Address::generate(&env);
    let holder_a = Address::generate(&env);
    let holder_b = Address::generate(&env);
    register_creator(&env, &client, &key_a, "mergea");
    register_creator(&env, &client, &key_b, "mergeb");
    client.buy_key(&key_a, &holder_a, &10_000, &None);
    client.buy_key(&key_b, &holder_b, &10_000, &None);
    client.buy_key(&key_b, &holder_b, &10_000, &None);

    let proposal_id = client.propose_merge(&key_a, &key_b, &key_a, &key_b, &10_000, &10_000);
    let (_, topics, data) = env.events().all().last().unwrap();
    let event: Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(event, Symbol::new(&env, "MergeProposed"));
    let event_proposal_id: u32 = topics.get(1).unwrap().into_val(&env);
    let proposal_payload: (u32, u32, u64) = data.into_val(&env);
    assert_eq!(event_proposal_id, proposal_id);
    assert_eq!(proposal_payload, (10_000, 10_000, 3));
    client.vote_merge(&proposal_id, &holder_a, &true);
    let (_, topics, data) = env.events().all().last().unwrap();
    let event: Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(event, Symbol::new(&env, "MergeVoteCast"));
    let vote_proposal_id: u32 = topics.get(1).unwrap().into_val(&env);
    let vote_payload: (bool, u64, u64, u64) = data.into_val(&env);
    assert_eq!(vote_proposal_id, proposal_id);
    assert_eq!(vote_payload, (true, 1, 1, 0));
    assert_eq!(
        client.try_execute_merge(&proposal_id),
        Err(Ok(ContractError::Unauthorized))
    );
    client.vote_merge(&proposal_id, &holder_b, &true);
    assert_eq!(client.execute_merge(&proposal_id), 3);
    let (_, topics, data) = env.events().all().last().unwrap();
    let event: Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(event, Symbol::new(&env, "MergeExecuted"));
    let event_proposal_id: u32 = topics.get(1).unwrap().into_val(&env);
    let payload: (u32, u32, u32) = data.into_val(&env);
    assert_eq!(event_proposal_id, proposal_id);
    assert_eq!(payload, (10_000, 10_000, 3));
    assert_eq!(client.get_key_balance(&key_a, &holder_a), 1);
    assert_eq!(client.get_key_balance(&key_a, &holder_b), 2);
    assert_eq!(client.get_creator_supply(&key_a), 3);
    assert_eq!(client.get_creator_supply(&key_b), 0);
    assert_eq!(
        client.try_buy_key(&key_b, &holder_a, &10_000, &None),
        Err(Ok(ContractError::KeyDeprecated))
    );
}
