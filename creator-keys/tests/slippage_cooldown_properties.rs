//! Property-based tests for the slippage and cooldown guards (#835).
//!
//! Each guard is exercised through the same predicate the contract uses, over
//! 10 000 randomised inputs. Any violation prints the reproducing seed before
//! failing.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_ledger_sequence, set_pricing_and_fees,
    test_env_with_auths,
};
use creator_keys::{check_buy_slippage, check_sell_slippage, check_trade_cooldown, ContractError};
use soroban_sdk::{testutils::Address as _, Address};

const ITERATIONS: u32 = 10_000;
const SEED: u64 = 0x835_2026_0924;

/// Deterministic xorshift generator so a violation is reproducible from the seed.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9e37_79b9_7f4a_7c15
            } else {
                seed
            },
        }
    }

    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn upto(&mut self, max: u64) -> u64 {
        if max == 0 {
            0
        } else {
            self.next() % (max + 1)
        }
    }
}

#[test]
fn buy_slippage_rejects_if_and_only_if_price_exceeds_max() {
    let mut rng = Rng::new(SEED);

    for i in 0..ITERATIONS {
        let execution_price = rng.upto(2_000_000) as i128;
        let max_price = rng.upto(2_000_000) as i128;

        let result = check_buy_slippage(execution_price, Some(max_price));
        let should_reject = execution_price > max_price;

        if should_reject != result.is_err() {
            std::println!(
                "buy slippage invariant violated: seed={:#x} iteration={} execution_price={} max_price={}",
                SEED,
                i,
                execution_price,
                max_price
            );
        }

        assert_eq!(
            result.is_err(),
            should_reject,
            "buy slippage invariant (seed {:#x}, iteration {})",
            SEED,
            i
        );
        if should_reject {
            assert_eq!(result, Err(ContractError::SlippageExceeded));
        }

        // No bound configured: the guard never rejects.
        assert_eq!(check_buy_slippage(execution_price, None), Ok(()));
    }
}

#[test]
fn sell_slippage_rejects_if_and_only_if_proceeds_fall_below_min() {
    let mut rng = Rng::new(SEED ^ 0x5e11);

    for i in 0..ITERATIONS {
        let execution_proceeds = rng.upto(2_000_000) as i128;
        let min_proceeds = rng.upto(2_000_000) as i128;

        let result = check_sell_slippage(execution_proceeds, Some(min_proceeds));
        let should_reject = execution_proceeds < min_proceeds;

        if should_reject != result.is_err() {
            std::println!(
                "sell slippage invariant violated: seed={:#x} iteration={} execution_proceeds={} min_proceeds={}",
                SEED ^ 0x5e11,
                i,
                execution_proceeds,
                min_proceeds
            );
        }

        assert_eq!(
            result.is_err(),
            should_reject,
            "sell slippage invariant (seed {:#x}, iteration {})",
            SEED ^ 0x5e11,
            i
        );
        if should_reject {
            assert_eq!(result, Err(ContractError::SlippageExceeded));
        }

        assert_eq!(check_sell_slippage(execution_proceeds, None), Ok(()));
    }
}

#[test]
fn cooldown_rejects_if_and_only_if_elapsed_is_below_the_window() {
    let mut rng = Rng::new(SEED ^ 0xc001);

    for i in 0..ITERATIONS {
        let current_ledger = rng.upto(5_000_000) as u32;
        let last_buy_ledger = rng.upto(current_ledger as u64) as u32;
        let cooldown = rng.upto(10_000) as u32;

        let result = check_trade_cooldown(current_ledger, last_buy_ledger, cooldown);
        let elapsed = current_ledger - last_buy_ledger;
        let should_reject = cooldown > 0 && elapsed < cooldown;

        if should_reject != result.is_err() {
            std::println!(
                "cooldown invariant violated: seed={:#x} iteration={} last_buy_ledger={} current_ledger={} cooldown={}",
                SEED ^ 0xc001,
                i,
                last_buy_ledger,
                current_ledger,
                cooldown
            );
        }

        assert_eq!(
            result.is_err(),
            should_reject,
            "cooldown invariant (seed {:#x}, iteration {})",
            SEED ^ 0xc001,
            i
        );
        if should_reject {
            assert_eq!(result, Err(ContractError::CooldownActive));
        }
    }
}

#[test]
fn buy_above_max_price_reverts_with_slippage_exceeded() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 100, 9_000, 1_000);
    let creator = register_test_creator(&env, &client, "slip");
    let buyer = Address::generate(&env);

    let quote = client.get_buy_quote(&creator);
    let result = client.try_buy_key(
        &creator,
        &buyer,
        &quote.total_amount,
        &Some(quote.price - 1),
    );
    assert_eq!(result, Err(Ok(ContractError::SlippageExceeded)));
}

#[test]
fn buy_inside_the_cooldown_reverts_with_cooldown_active() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);
    set_pricing_and_fees(&env, &client, 100, 9_000, 1_000);
    let creator = register_test_creator(&env, &client, "cool");
    let buyer = Address::generate(&env);

    client.set_buy_cooldown(&creator, &5u32);
    set_ledger_sequence(&env, 10);

    let quote = client.get_buy_quote(&creator);
    client.buy_key(&creator, &buyer, &quote.total_amount, &None);

    set_ledger_sequence(&env, 12);
    let result = client.try_buy_key(&creator, &buyer, &quote.total_amount, &None);
    assert_eq!(result, Err(Ok(ContractError::CooldownActive)));

    set_ledger_sequence(&env, 15);
    client.buy_key(&creator, &buyer, &quote.total_amount, &None);
}
