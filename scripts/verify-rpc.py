#!/usr/bin/env python3
import argparse
import base64
import binascii
import json
import struct
import sys
import urllib.error
import urllib.request
from urllib.parse import urlsplit


class VerificationError(Exception):
    pass


def validate_rpc_url(rpc_url):
    try:
        parsed_url = urlsplit(rpc_url)
    except ValueError as error:
        raise VerificationError(f"invalid RPC URL: {error}") from error
    if parsed_url.scheme not in ("http", "https") or not parsed_url.netloc:
        raise VerificationError("invalid RPC URL: expected an HTTP or HTTPS endpoint")


def decode_contract_id(contract_id):
    if len(contract_id) != 56 or not contract_id.startswith("C"):
        raise VerificationError("invalid contract ID: expected a Stellar C... contract address")
    try:
        decoded = base64.b32decode(contract_id + "=" * ((8 - len(contract_id) % 8) % 8))
    except (binascii.Error, ValueError) as error:
        raise VerificationError("invalid contract ID: malformed StrKey") from error
    if len(decoded) != 35 or decoded[0] != 16:
        raise VerificationError("invalid contract ID: wrong StrKey version or length")
    expected_checksum = struct.pack("<H", binascii.crc_hqx(decoded[:33], 0))
    if decoded[33:] != expected_checksum:
        raise VerificationError("invalid contract ID: checksum mismatch")
    return decoded[1:33]


def contract_instance_ledger_key(contract_id):
    contract_hash = decode_contract_id(contract_id)
    return struct.pack(">i", 6) + struct.pack(">i", 1) + contract_hash + \
        struct.pack(">i", 20) + struct.pack(">i", 1)


def _read_int32(data, offset):
    if offset + 4 > len(data):
        raise VerificationError("RPC returned truncated contract-instance XDR")
    return struct.unpack_from(">i", data, offset)[0], offset + 4


def wasm_hash_from_ledger_entry(encoded_xdr, contract_id):
    try:
        data = base64.b64decode(encoded_xdr, validate=True)
    except (binascii.Error, ValueError) as error:
        raise VerificationError("RPC returned invalid base64 XDR") from error

    offset = 0
    ledger_type, offset = _read_int32(data, offset)
    if ledger_type != 6:
        raise VerificationError("RPC returned a ledger entry that is not contract data")

    entry_ext, offset = _read_int32(data, offset)
    if entry_ext != 0:
        raise VerificationError(f"unsupported contract-data extension version: {entry_ext}")

    address_type, offset = _read_int32(data, offset)
    if address_type != 1 or offset + 32 > len(data):
        raise VerificationError("RPC returned a malformed contract address")
    contract_hash = data[offset:offset + 32]
    offset += 32
    if contract_hash != decode_contract_id(contract_id):
        raise VerificationError("RPC returned ledger data for a different contract ID")

    instance_key, offset = _read_int32(data, offset)
    if instance_key != 20:
        raise VerificationError("RPC returned contract data, not the contract instance entry")
    durability, offset = _read_int32(data, offset)
    if durability != 1:
        raise VerificationError("RPC returned a non-persistent contract instance entry")
    value_type, offset = _read_int32(data, offset)
    if value_type != 19:
        raise VerificationError("RPC returned a ledger entry without a contract instance")

    instance_ext, offset = _read_int32(data, offset)
    if instance_ext != 0:
        raise VerificationError(f"unsupported contract-instance extension version: {instance_ext}")
    executable_type, offset = _read_int32(data, offset)
    if executable_type != 0:
        raise VerificationError("contract instance is not backed by a WASM executable")
    if offset + 32 > len(data):
        raise VerificationError("RPC returned a truncated WASM hash")
    wasm_hash = data[offset:offset + 32].hex()
    offset += 32
    has_storage, offset = _read_int32(data, offset)
    if has_storage not in (0, 1):
        raise VerificationError("RPC returned an invalid contract-instance storage flag")
    if has_storage == 1 and offset + 4 > len(data):
        raise VerificationError("RPC returned truncated contract-instance storage")
    if has_storage == 0 and offset != len(data):
        raise VerificationError("RPC returned trailing data after the contract instance")
    return wasm_hash


def fetch_wasm_hash(contract_id, rpc_url):
    validate_rpc_url(rpc_url)
    ledger_key = base64.b64encode(contract_instance_ledger_key(contract_id)).decode("ascii")
    request_body = json.dumps({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getLedgerEntries",
        "params": {"keys": [ledger_key]},
    }).encode("utf-8")
    try:
        request = urllib.request.Request(
            rpc_url,
            data=request_body,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(request, timeout=30) as response:
            payload = json.loads(response.read())
    except urllib.error.HTTPError as error:
        raise VerificationError(f"RPC HTTP error: {error.code} {error.reason}") from error
    except urllib.error.URLError as error:
        raise VerificationError(f"RPC transport error: {error.reason}") from error
    except (TimeoutError, json.JSONDecodeError, UnicodeDecodeError, OSError, ValueError) as error:
        raise VerificationError(f"RPC request or response failed: {error}") from error

    if not isinstance(payload, dict):
        raise VerificationError("RPC returned a malformed JSON-RPC response")
    if "error" in payload:
        raise VerificationError(f"RPC error: {payload['error']}")
    if payload.get("jsonrpc") != "2.0" or payload.get("id") != 1:
        raise VerificationError("RPC returned a response with an unexpected JSON-RPC version or ID")
    result = payload.get("result")
    entries = result.get("entries") if isinstance(result, dict) else None
    if not isinstance(entries, list):
        raise VerificationError("RPC response is missing the ledger entries list")
    if len(entries) != 1:
        raise VerificationError("contract instance was not found on the configured RPC network")
    entry = entries[0]
    if not isinstance(entry, dict) or not isinstance(entry.get("xdr"), str):
        raise VerificationError("RPC response contains a malformed ledger entry")
    if entry.get("key") != ledger_key:
        raise VerificationError("RPC returned an entry for a different ledger key")
    return wasm_hash_from_ledger_entry(entry["xdr"], contract_id)


def main():
    parser = argparse.ArgumentParser(description="Retrieve a deployed contract WASM hash from Stellar RPC")
    parser.add_argument("--validate-only", action="store_true")
    parser.add_argument("contract_id")
    parser.add_argument("rpc_url", nargs="?")
    args = parser.parse_args()
    try:
        decode_contract_id(args.contract_id)
        if args.validate_only:
            if args.rpc_url:
                validate_rpc_url(args.rpc_url)
            return 0
        if not args.rpc_url:
            raise VerificationError("an RPC URL is required")
        print(fetch_wasm_hash(args.contract_id, args.rpc_url))
        return 0
    except VerificationError as error:
        print(f"Error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
