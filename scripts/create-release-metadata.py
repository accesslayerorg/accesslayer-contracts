#!/usr/bin/env python3
import argparse
import hashlib
import json
import pathlib
import re
import sys


def main():
    parser = argparse.ArgumentParser(description="Generate release WASM verification metadata")
    parser.add_argument("tag")
    parser.add_argument("commit")
    parser.add_argument("build_info", type=pathlib.Path)
    parser.add_argument("artifact_directory", type=pathlib.Path)
    parser.add_argument("output", type=pathlib.Path)
    args = parser.parse_args()

    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._/-]*", args.tag):
        parser.error("invalid release tag")
    if not re.fullmatch(r"[0-9a-f]{40}", args.commit):
        parser.error("commit must be a full lowercase Git SHA")
    info = json.loads(args.build_info.read_text())
    if info.get("source_commit") != args.commit:
        parser.error("build metadata commit does not match the release commit")
    if not info.get("artifacts"):
        parser.error("build metadata contains no WASM artifacts")

    entries = []
    for artifact in info["artifacts"]:
        filename = artifact.get("file")
        identifier = artifact.get("identifier")
        expected_hash = artifact.get("sha256")
        if not isinstance(filename, str) or pathlib.Path(filename).name != filename:
            parser.error("build metadata contains an unsafe artifact filename")
        if not isinstance(identifier, str) or not identifier:
            parser.error("build metadata contains a missing contract/WASM identifier")
        if not isinstance(expected_hash, str) or not re.fullmatch(r"[0-9a-f]{64}", expected_hash):
            parser.error(f"build metadata contains an invalid SHA-256 for {filename}")
        path = args.artifact_directory / filename
        if not path.is_file():
            parser.error(f"build artifact is missing: {filename}")
        actual_hash = hashlib.sha256(path.read_bytes()).hexdigest()
        if actual_hash != expected_hash:
            parser.error(f"build artifact hash changed after building: {filename}")
        entries.append({
            "tag": args.tag,
            "commit": args.commit,
            "contract_wasm_identifier": identifier,
            "wasm_sha256": actual_hash,
            "rust_toolchain": info.get("rust_toolchain"),
            "soroban_cli_version": info.get("soroban_cli_version"),
        })

    if any(not entry["rust_toolchain"] or not entry["soroban_cli_version"] for entry in entries):
        parser.error("build metadata is missing pinned tool versions")
    args.output.write_text(json.dumps({"schema_version": 1, "releases": entries}, indent=2) + "\n")
    print(f"Wrote {len(entries)} release artifact record(s) to {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
