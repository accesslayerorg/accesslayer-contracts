use super::harness::{scenario_seed, Harness};
use creator_keys::ContractError;

#[test]
fn pause_and_recovery() {
    let mut h = Harness::new(scenario_seed("pause_and_recovery"));
    let creator = h.step("register creator", |h| h.register_creator("recovery"));
    let buyer = h.actors[0].clone();

    h.step("seed initial position", |h| h.buy_one(&creator, &buyer));
    h.step("pause protocol", |h| h.keys.pause(&h.admin).unwrap());

    h.step("buy blocked while paused", |h| {
        let quote = h.keys.get_buy_quote(&creator).unwrap();
        h.assert_contract_error(
            h.keys
                .try_buy_key(&creator, &h.actors[1], &quote.total_amount, &None),
            ContractError::ProtocolPaused,
        );
    });

    h.step("unpause protocol", |h| h.keys.unpause(&h.admin).unwrap());
    h.step("buy resumes after unpause", |h| {
        h.buy_one(&creator, &h.actors[1])
    });

    let first_governor = h.governors[0].clone();
    let second_governor = h.governors[1].clone();
    h.step("global pause by two governors", |h| {
        h.keys.global_pause(&first_governor).unwrap();
        h.keys.global_pause(&second_governor).unwrap();
    });

    h.step("sell blocked while globally halted", |h| {
        h.assert_contract_error(
            h.keys.try_sell_key(&creator, &buyer, &None),
            ContractError::GlobalTradingHalted,
        );
    });

    h.step("global resume by two governors", |h| {
        h.keys.global_resume(&first_governor).unwrap();
        h.keys.global_resume(&second_governor).unwrap();
    });

    h.step("sell resumes after global resume", |h| {
        h.sell_one(&creator, &buyer);
    });
    h.step("settle accumulated fees", |h| h.settle_fees(&creator));

    h.print_resources();
}
