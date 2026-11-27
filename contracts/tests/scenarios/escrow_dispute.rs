use super::harness::{scenario_seed, Harness};
use soroban_sdk::{String, Symbol};

#[test]
fn escrow_dispute_and_redemption() {
    let mut h = Harness::new(scenario_seed("escrow_dispute_and_redemption"));
    let creator = h.step("register creator", |h| h.register_creator("wind-down"));

    let buy_count = h.random_quantity(3, 5) as usize;
    for index in 0..buy_count {
        let actors = h.shuffled_actors();
        let buyer = &actors[index % actors.len()];
        h.step(&format!("buy {index}"), |h| h.buy_one(&creator, buyer));
    }

    let supply = h.keys.get_total_key_supply(&creator);
    let buyback_price = 1_200i128;
    let buyback_escrow = i128::from(supply) * buyback_price;
    let dispute_reserve = 4_000i128;

    h.step("fund mock escrow", |h| {
        h.deposit_escrow(&creator, buyback_escrow + dispute_reserve);
        h.note_buyback_liability(buyback_escrow);
        h.note_dispute_liability(dispute_reserve);
    });

    h.step("deprecate key with escrow", |h| {
        h.keys.deprecate_key(
            &creator,
            &creator,
            &buyback_price,
            &buyback_escrow,
            &String::from_str(h.env, "orderly wind down"),
            &None,
        );
    });

    let claimant = h
        .actors
        .iter()
        .find(|actor| h.keys.get_key_balance(&creator, actor) > 0)
        .cloned()
        .unwrap();

    h.step("holder raises dispute", |h| {
        h.escrow
            .raise_dispute(&claimant, &creator, &Symbol::new(h.env, "payout"));
        assert!(h.escrow.is_disputed(&creator));
    });

    h.step("arbitrator resolves dispute", |h| {
        h.resolve_dispute(&creator, &claimant, 1_500);
        assert!(!h.escrow.is_disputed(&creator));
    });

    let actors = h.actors.clone();
    h.step("redeem all keys", |h| {
        for actor in &actors {
            if h.keys.get_key_balance(&creator, actor) > 0 {
                let payout = h.keys.redeem(&creator, actor);
                h.release_buyback(&creator, actor, payout);
            }
        }
        assert_eq!(h.keys.get_total_key_supply(&creator), 0);
        assert_eq!(h.buyback_liability, 0);
    });

    h.step("settle accumulated fees", |h| h.settle_fees(&creator));
    h.print_resources();
}
