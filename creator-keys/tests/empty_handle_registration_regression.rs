//! Regression test for creator registration rejection with an empty string handle (#301).
//!
//! Confirms that the shortest possible invalid input (empty string) is caught by the
//! handle validation guard and that no creator state is written as a result.

use creator_keys::{ContractError, CreatorKeysContract, CreatorKeysContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_register_creator_rejects_empty_handle() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let result = client.try_register_creator(&creator, &String::from_str(&env, ""));

    assert_eq!(result, Err(Ok(ContractError::HandleTooShort)));
    assert!(!client.is_creator_registered(&creator));
}
