use super::harness::{scenario_seed, Harness, KEY_PRICE};

#[test]
fn launch_to_graduation() {
    let mut h = Harness::new(scenario_seed("launch_to_graduation"));
    let creator = h.step("register creator", |h| h.register_creator("launch"));

    h.step("configure graduation milestones", |h| {
        h.configure_graduated_curve(&creator, &[(3, 1), (6, 2)]);
    });

    for round in 0..3 {
        let actors = h.shuffled_actors();
        for actor in actors.iter().take(2) {
            let label = format!("round {round} random buy");
            h.step(&label, |h| h.buy_one(&creator, actor));
        }
        h.step("keeper observes price", |h| {
            h.sync_keeper(&creator, KEY_PRICE)
        });
    }

    h.step("assert graduated exponent", |h| {
        assert_eq!(h.keys.get_curve_exponent(&creator), Some(2));
        assert_eq!(h.keys.get_total_key_supply(&creator), 6);
        assert_eq!(h.keeper.executions(), 3);
    });

    h.step("settle accumulated fees", |h| h.settle_fees(&creator));
    h.print_resources();
}
