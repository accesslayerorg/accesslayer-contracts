#!/usr/bin/env python3
"""Generate deterministic integer-token bonding curve reference vectors."""
from decimal import Decimal, getcontext
import argparse
import csv
import random
from pathlib import Path

getcontext().prec = 80
ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "tools/reference/vectors.csv"
HEADER = ["preset", "base", "slope", "supply", "quantity", "fee_bps", "buy", "sell", "fee"]


def curve_price(preset, base, slope, supply, quantity):
    exponent = {"flat": 0, "linear": 1, "quadratic": 2}[preset]
    total = Decimal(0)
    for step in range(supply, supply + quantity):
        total += Decimal(base) + Decimal(slope) * (Decimal(step) ** exponent if exponent else 0)
    return int(total)


def row(preset, base, slope, supply, quantity, bps):
    buy = curve_price(preset, base, slope, supply, quantity)
    # Selling the just-bought quantity at post-buy supply retraces these steps.
    sell = curve_price(preset, base, slope, supply, quantity)
    fee = int(
        (Decimal(buy) * Decimal(bps) / Decimal(10_000)).to_integral_value(
            rounding="ROUND_FLOOR"
        )
    )
    return [preset, base, slope, supply, quantity, bps, buy, sell, fee]


def generate(seed):
    vectors = []
    for preset in ("flat", "linear", "quadratic"):
        for supply, quantity, base, slope, bps in [
            (0, 1, 1, 0, 0), (1, 1, 1, 1, 1), (2, 2, 10, 3, 9999),
            (100, 10, 1_000_000, 17, 100), (10_000, 100, 123_456, 7, 2500),
        ]:
            vectors.append(row(preset, base, slope, supply, quantity, bps))
    rng = random.Random(seed)
    for _ in range(300):
        preset = rng.choice(("flat", "linear", "quadratic"))
        supply = rng.choice([rng.randrange(0, 20), rng.randrange(100, 100_000)])
        quantity = rng.randint(1, min(100, supply + 1))
        vectors.append(
            row(
                preset,
                rng.randint(1, 10**8),
                rng.randint(0, 10**4),
                supply,
                quantity,
                rng.choice([0, 1, 100, 1000, 9999, 10000]),
            )
        )
    return vectors


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--seed", type=int, default=1325)
    parser.add_argument("--check", action="store_true", help="fail if committed vectors differ")
    args = parser.parse_args()
    import io
    buffer = io.StringIO(newline="")
    writer = csv.writer(buffer, lineterminator="\n")
    writer.writerow(HEADER)
    writer.writerows(generate(args.seed))
    content = buffer.getvalue()
    if args.check:
        if not OUT.exists() or OUT.read_text(encoding="utf-8") != content:
            raise SystemExit("reference vectors are stale; run python tools/reference/generate_vectors.py")
    else:
        OUT.parent.mkdir(parents=True, exist_ok=True)
        OUT.write_text(content, encoding="utf-8", newline="")


if __name__ == "__main__":
    main()
