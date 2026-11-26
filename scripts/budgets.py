#!/usr/bin/env python3
import argparse
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BUDGETS_PATH = ROOT / "budgets.toml"


def parse_value(value):
    value = value.strip()
    if len(value) >= 2 and value[0] == value[-1] and value[0] in ("\"", "'"):
        return value[1:-1]
    try:
        return int(value.replace("_", ""))
    except ValueError:
        return value


def parse_toml(text):
    data = {}
    table = data
    for raw_line in text.splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("[") and line.endswith("]"):
            table = data
            for part in line[1:-1].strip().split("."):
                table = table.setdefault(part.strip().strip("\"").strip("'"), {})
            continue
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        table[key.strip().strip("\"").strip("'")] = parse_value(value)
    return data


def load_budgets(path=BUDGETS_PATH):
    return parse_toml(path.read_text(encoding="utf-8"))


def contract_budgets(data):
    return data["wasm"]["contracts"]


def resource_limits(data):
    return data["resources"]["limits"]


def wasm_artifact(target, package):
    return ROOT / "target" / target / "release" / f"{package.replace('-', '_')}.wasm"


def command_target(_args):
    print(load_budgets()["wasm"]["target"])
    return 0


def command_packages(_args):
    for package in contract_budgets(load_budgets()):
        print(package)
    return 0


def command_sizes(_args):
    data = load_budgets()
    target = data["wasm"]["target"]
    failed = False
    for package, budget in contract_budgets(data).items():
        artifact = wasm_artifact(target, package)
        if not artifact.is_file():
            print(f"missing wasm artifact: {artifact}", file=sys.stderr)
            failed = True
            continue
        actual = artifact.stat().st_size
        print(f"WASM_SIZE\t{package}\t{actual}\t{budget}")
        if actual > budget:
            print(
                f"wasm budget exceeded: {package} {actual} > {budget}",
                file=sys.stderr,
            )
            failed = True
    return 1 if failed else 0


def parse_report(path, marker, fields):
    values = {}
    if not path:
        return values
    file_path = Path(path)
    if not file_path.is_file():
        return values
    for raw_line in file_path.read_text(encoding="utf-8", errors="replace").splitlines():
        parts = raw_line.strip().split("\t")
        if len(parts) != len(fields) + 1 or parts[0] != marker:
            continue
        name = parts[1]
        values[name] = dict(zip(fields, (int(part) for part in parts[2:])))
    return values


def load_usage(wasm_report, resource_report):
    usage = {}
    for name, values in parse_report(
        wasm_report, "WASM_SIZE", ("actual", "budget")
    ).items():
        usage[f"wasm/{name}"] = values
    for name, values in parse_report(
        resource_report,
        "RESOURCE_USAGE",
        ("cpu_insns", "mem_bytes", "cpu_budget", "mem_budget"),
    ).items():
        usage[f"{name}/cpu_insns"] = values
        usage[f"{name}/mem_bytes"] = values
    return usage


def flatten_limits(data):
    flattened = {}
    for package, budget in contract_budgets(data).items():
        flattened[f"wasm/{package}"] = int(budget)
    for entrypoint, limits in resource_limits(data).items():
        for dimension in ("cpu_insns", "mem_bytes"):
            flattened[f"{entrypoint}/{dimension}"] = int(limits[dimension])
    return flattened


def load_base_budgets(base_sha):
    if not base_sha or set(base_sha) == {"0"}:
        return None
    try:
        text = subprocess.check_output(
            ["git", "show", f"{base_sha}:budgets.toml"],
            cwd=ROOT,
            stderr=subprocess.DEVNULL,
        ).decode("utf-8")
    except (subprocess.CalledProcessError, UnicodeDecodeError):
        return None
    return parse_toml(text)


def budgets_changed(base_sha):
    if not base_sha or set(base_sha) == {"0"}:
        return True
    try:
        output = subprocess.check_output(
            ["git", "diff", "--name-only", base_sha, "--", "budgets.toml"],
            cwd=ROOT,
            stderr=subprocess.DEVNULL,
        ).decode("utf-8")
    except subprocess.CalledProcessError:
        return True
    return "budgets.toml" in output.splitlines()


def format_value(metric, value):
    if value is None:
        return "-"
    if metric.startswith("wasm/") or metric.endswith("/mem_bytes"):
        return f"{value / 1024:.1f} KiB"
    if metric.endswith("/cpu_insns"):
        return f"{value / 1_000_000:.2f}M"
    return str(value)


def format_delta(metric, delta):
    if delta is None:
        return "-"
    prefix = "+" if delta > 0 else ""
    return prefix + format_value(metric, delta)


def command_report(args):
    data = load_budgets()
    limits = flatten_limits(data)
    base_data = load_base_budgets(args.base_sha)
    base_limits = flatten_limits(base_data) if base_data else None

    current_usage = load_usage(args.wasm_report, args.resource_report)
    base_usage = load_usage(args.base_wasm_report, args.base_resource_report)

    rows = []
    passed = True
    for metric, budget in limits.items():
        values = current_usage.get(metric) or {}
        base_values = base_usage.get(metric) or {}
        if metric.startswith("wasm/"):
            actual = values.get("actual")
            reported_budget = values.get("budget")
            base_actual = base_values.get("actual")
        elif metric.endswith("/cpu_insns"):
            actual = values.get("cpu_insns")
            reported_budget = values.get("cpu_budget")
            base_actual = base_values.get("cpu_insns")
        else:
            actual = values.get("mem_bytes")
            reported_budget = values.get("mem_budget")
            base_actual = base_values.get("mem_bytes")

        if base_limits is None:
            budget_delta = None
        else:
            budget_delta = budget - base_limits.get(metric, budget)
        if actual is None or base_actual is None:
            usage_delta = None
        else:
            usage_delta = actual - base_actual

        if actual is None:
            status = "MISSING"
            passed = False
        elif reported_budget is None or reported_budget != budget:
            status = "STALE BUDGET"
            passed = False
        elif actual > budget:
            status = "OVER"
            passed = False
        else:
            status = "OK"

        rows.append((metric, actual, budget, usage_delta, budget_delta, status))

    width = max(len(metric) for metric, _, _, _, _, _ in rows)
    header = (
        f"{'Metric':<{width}}  {'Current':>12}  {'Budget':>12}  "
        f"{'Δ usage vs base':>16}  {'Δ budget vs base':>18}  Status"
    )
    print(header)
    print("-" * len(header))
    for metric, actual, budget, usage_delta, budget_delta, status in rows:
        print(
            f"{metric:<{width}}  {format_value(metric, actual):>12}  "
            f"{format_value(metric, budget):>12}  "
            f"{format_delta(metric, usage_delta):>16}  "
            f"{format_delta(metric, budget_delta):>18}  {status}"
        )

    raised = [
        metric
        for metric, budget in limits.items()
        if base_limits and metric in base_limits and budget > base_limits[metric]
    ]
    if raised and not budgets_changed(args.base_sha):
        print(
            "budget increase requires budgets.toml to be changed in this pull request: "
            + ", ".join(raised),
            file=sys.stderr,
        )
        passed = False

    if not passed:
        print("Budget check failed.", file=sys.stderr)
        return 1
    print("Budget check passed.")
    return 0


def main():
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("target").set_defaults(handler=command_target)
    subparsers.add_parser("packages").set_defaults(handler=command_packages)
    subparsers.add_parser("sizes").set_defaults(handler=command_sizes)
    report_parser = subparsers.add_parser("report")
    report_parser.add_argument("--wasm-report", required=True)
    report_parser.add_argument("--resource-report", required=True)
    report_parser.add_argument("--base-sha")
    report_parser.add_argument("--base-wasm-report")
    report_parser.add_argument("--base-resource-report")
    report_parser.set_defaults(handler=command_report)
    args = parser.parse_args()
    return args.handler(args)


if __name__ == "__main__":
    sys.exit(main())
