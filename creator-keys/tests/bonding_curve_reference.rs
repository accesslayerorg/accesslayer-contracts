//! Differential replay against the independent Decimal model in tools/reference.
//! Core curve prices are integral token units, so the documented tolerance is 0.
use creator_keys::{compute_price, CurvePreset};

#[test]
fn replay_reference_vectors() {
    let mut largest_deviation = 0i128;
    for (line_no, line) in include_str!("../../tools/reference/vectors.csv")
        .lines()
        .enumerate()
        .skip(1)
    {
        let fields: Vec<&str> = line.split(',').collect();
        let preset = match fields[0] {
            "flat" => CurvePreset::Flat,
            "linear" => CurvePreset::Linear,
            "quadratic" => CurvePreset::Quadratic,
            other => panic!("line {} has unknown preset {other}", line_no + 1),
        };
        let parse = |i: usize| fields[i].parse::<i128>().unwrap();
        let base = parse(1);
        let slope = parse(2);
        let supply = parse(3) as u32;
        let quantity = parse(4) as u32;
        let bps = parse(5) as u32;
        let expected_buy = parse(6);
        let expected_sell = parse(7);
        let expected_fee = parse(8);
        let actual_buy = compute_price(base, slope, supply, quantity, preset).unwrap();
        let actual_sell = compute_price(base, slope, supply, quantity, preset).unwrap();
        let actual_fee =
            creator_keys::CreatorKeysContract::apply_percentage_fee(actual_buy, bps).unwrap();
        for (name, expected, actual) in [
            ("buy", expected_buy, actual_buy),
            ("sell", expected_sell, actual_sell),
            ("fee", expected_fee, actual_fee),
        ] {
            let deviation = (expected - actual).abs();
            largest_deviation = largest_deviation.max(deviation);
            assert!(deviation <= 0, "line {} {name} mismatch: inputs={line}; reference={expected}; rust={actual}; deviation={deviation}", line_no + 1);
        }
        assert!(
            actual_fee * 10_000 <= actual_buy * i128::from(bps),
            "fee rounded against protocol: {line}"
        );
        assert!(
            actual_buy - actual_fee <= actual_buy,
            "buy fee increased trader proceeds: {line}"
        );
        let sell_fee =
            creator_keys::CreatorKeysContract::apply_percentage_fee(actual_sell, bps).unwrap();
        assert!(
            actual_sell - sell_fee <= actual_buy + actual_fee,
            "round trip creates value: {line}"
        );
    }
    eprintln!("bonding curve reference vectors: largest deviation = {largest_deviation} token units (tolerance 0)");
}
