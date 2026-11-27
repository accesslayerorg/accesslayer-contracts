#![no_main]

use creator_keys::{CreatorKeysContract, CreatorKeysContractClient, RegisterCreatorParams};
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String,
};

const ACCOUNT_COUNT: usize = 4;
const KEY_PRICE: i128 = 1_000;

fn assert_trading_invariants(
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
    accounts: &[Address; ACCOUNT_COUNT],
) {
    let balances: u32 = accounts
        .iter()
        .map(|account| client.get_key_balance(creator, account))
        .sum();
    assert_eq!(
        client.get_total_key_supply(creator),
        balances,
        "creator supply must equal the sum of all fuzz-account balances"
    );
}

fuzz_target!(|input: &[u8]| {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreatorKeysContract, ());
    let client = CreatorKeysContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.set_protocol_admin(&admin, &admin);
    client.set_key_price(&admin, &KEY_PRICE);
    client.set_curve_slope(&admin, &0);
    client.set_fee_config(&admin, &9_000, &1_000);

    let creator = Address::generate(&env);
    client.register_creator(
        &RegisterCreatorParams {
            creator: creator.clone(),
            handle: String::from_str(&env, "fuzz-creator"),
        },
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    let accounts = std::array::from_fn(|_| Address::generate(&env));

    for step in input.chunks_exact(4).take(64) {
        let operation = step[0] % 5;
        let actor = usize::from(step[1]) % ACCOUNT_COUNT;
        let recipient = usize::from(step[2]) % ACCOUNT_COUNT;

        match operation {
            0 => {
                let quote = client.get_buy_quote(&creator);
                let _ = client.try_buy_key(&creator, &accounts[actor], &quote.total_amount, &None);
            }
            1 => {
                let _ = client.try_sell_key(&creator, &accounts[actor], &None);
            }
            2 => {
                let amount = u32::from(step[3] % 4) + 1;
                let _ = client.try_transfer_keys(
                    &creator,
                    &accounts[actor],
                    &accounts[recipient],
                    &amount,
                );
            }
            3 => {
                let amount = i128::from(step[3]) + 1;
                let _ = client.try_distribute_dividend(&creator, &accounts[actor], &amount);
            }
            _ => {
                let _ = client.try_claim_dividend(&creator, &accounts[actor]);
            }
        }

        env.ledger().with_mut(|ledger| {
            ledger.sequence_number = ledger.sequence_number.saturating_add(1);
            ledger.timestamp = ledger.timestamp.saturating_add(1);
        });
        assert_trading_invariants(&client, &creator, &accounts);
    }
});
