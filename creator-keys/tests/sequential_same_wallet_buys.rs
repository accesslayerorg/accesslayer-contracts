//! Integration test for multiple sequential buys by the same wallet
//! accumulating the correct total balance.

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

const KEY_PRICE: i128 = 100;

fn setup(env: &Env) -> CreatorKeysContractClient<'_> {
    let id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(env, &id);
    let admin = Address::generate(env);
    client.set_key_price(&admin, &KEY_PRICE);
    client
}

fn register_creator(env: &Env, client: &CreatorKeysContractClient, handle: &str) -> Address {
    let creator = Address::generate(env);
    client.register_creator(
        &creator_keys::RegisterCreatorParams {
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
    creator
}

#[test]
fn test_sequential_same_wallet_buys_accumulate_balance() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup(&env);
    let creator = register_creator(&env, &client, "alice");
    let wallet = Address::generate(&env);

    let supply = client.buy_key(&creator, &wallet, &KEY_PRICE, &None);
    assert_eq!(supply, 1, "supply should be 1 after first buy");
    assert_eq!(
        client.get_key_balance(&creator, &wallet),
        1,
        "wallet balance should be 1 after first buy"
    );

    let supply = client.buy_key(&creator, &wallet, &KEY_PRICE, &None);
    assert_eq!(supply, 2, "supply should be 2 after second buy");
    assert_eq!(
        client.get_key_balance(&creator, &wallet),
        2,
        "wallet balance should be 2 after second buy"
    );

    let supply = client.buy_key(&creator, &wallet, &KEY_PRICE, &None);
    assert_eq!(supply, 3, "supply should be 3 after third buy");
    assert_eq!(
        client.get_key_balance(&creator, &wallet),
        3,
        "wallet balance should be 3 after third buy"
    );

    let supply = client.buy_key(&creator, &wallet, &KEY_PRICE, &None);
    assert_eq!(supply, 4, "supply should be 4 after fourth buy");
    assert_eq!(
        client.get_key_balance(&creator, &wallet),
        4,
        "wallet balance should be 4 after fourth buy"
    );

    let supply = client.buy_key(&creator, &wallet, &KEY_PRICE, &None);
    assert_eq!(supply, 5, "supply should be 5 after fifth buy");
    assert_eq!(
        client.get_key_balance(&creator, &wallet),
        5,
        "wallet balance should be 5 after fifth buy"
    );

    assert_eq!(
        client.get_total_key_supply(&creator),
        5,
        "creator supply should be 5 after all five buys"
    );
}
