use super::harness::{scenario_seed, Harness, KEY_PRICE};

#[test]
fn trading_fees_and_royalties_reconcile() {
    let mut h = Harness::new(scenario_seed("trading_fees_and_royalties"));
    let creator = h.step("register creator", |h| h.register_creator("fees"));

    h.step("configure royalties and prefund reserve", |h| {
        h.set_royalty(&creator, 500, 500);
        h.prefund_royalty(&creator, 220_000);
    });

    let buy_count = h.random_quantity(4, 6) as usize;
    for index in 0..buy_count {
        let actors = h.shuffled_actors();
        let buyer = &actors[index % actors.len()];
        h.step(&format!("buy {index}"), |h| h.buy_one(&creator, buyer));

        if index % 2 == 1 {
            let sellers = h.shuffled_actors();
            let seller = sellers
                .iter()
                .find(|actor| h.keys.get_key_balance(&creator, actor) > 0)
                .cloned();
            if let Some(seller) = seller {
                h.step(&format!("sell {index}"), |h| h.sell_one(&creator, &seller));
            }
        }
    }

    h.step("keeper publishes latest price", |h| {
        h.sync_keeper(&creator, KEY_PRICE)
    });
    h.step("reconcile fee liabilities", |h| {
        h.settle_fees(&creator);
        assert_eq!(h.liabilities.protocol_fee, 0);
        assert_eq!(h.liabilities.creator_fee, 0);
        assert_eq!(h.liabilities.royalty, 0);
    });

    h.print_resources();
}
