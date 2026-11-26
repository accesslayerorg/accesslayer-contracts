import base64
import binascii
import importlib.util
import json
import pathlib
import struct
import sys
import threading
import unittest
from http.server import BaseHTTPRequestHandler, HTTPServer


MODULE_PATH = pathlib.Path(__file__).parents[1] / "scripts" / "verify-rpc.py"
SPEC = importlib.util.spec_from_file_location("verify_rpc", MODULE_PATH)
verify_rpc = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = verify_rpc
SPEC.loader.exec_module(verify_rpc)


def make_contract_id(payload):
    versioned = b"\x10" + payload
    checksum = struct.pack("<H", binascii.crc_hqx(versioned, 0))
    return base64.b32encode(versioned + checksum).decode("ascii").rstrip("=")


def instance_xdr(contract_hash, wasm_hash):
    values = [
        6,  # LedgerEntryData.CONTRACT_DATA
        0,  # ContractDataEntry extension
        1,  # SCAddress.CONTRACT
    ]
    return (
        b"".join(struct.pack(">i", value) for value in values)
        + contract_hash
        + struct.pack(">i", 20)  # SCV_LEDGER_KEY_CONTRACT_INSTANCE
        + struct.pack(">i", 1)  # ContractDataDurability.PERSISTENT
        + struct.pack(">i", 19)  # SCV_CONTRACT_INSTANCE
        + struct.pack(">i", 0)  # SCContractInstance has no extension
        + struct.pack(">i", 0)  # CONTRACT_EXECUTABLE_WASM
        + wasm_hash
        + struct.pack(">i", 0)  # no instance storage
    )


class RpcHandler(BaseHTTPRequestHandler):
    response_payload = {}

    def do_POST(self):
        self.rfile.read(int(self.headers["Content-Length"]))
        payload = json.dumps(self.response_payload).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, *_):
        pass


class VerifyRpcTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = HTTPServer(("127.0.0.1", 0), RpcHandler)
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        cls.rpc_url = f"http://127.0.0.1:{cls.server.server_port}"

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.thread.join()
        cls.server.server_close()

    def setUp(self):
        self.contract_hash = bytes(range(32))
        self.contract_id = make_contract_id(self.contract_hash)
        self.wasm_hash = bytes(reversed(range(32)))
        ledger_key = base64.b64encode(
            verify_rpc.contract_instance_ledger_key(self.contract_id)
        ).decode()
        RpcHandler.response_payload = {
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "entries": [{
                    "key": ledger_key,
                    "xdr": base64.b64encode(
                        instance_xdr(self.contract_hash, self.wasm_hash)
                    ).decode(),
                }]
            },
        }

    def test_contract_id_checksum_and_key(self):
        self.assertEqual(
            verify_rpc.decode_contract_id(self.contract_id),
            self.contract_hash,
        )
        self.assertEqual(
            len(verify_rpc.contract_instance_ledger_key(self.contract_id)),
            48,
        )
        with self.assertRaisesRegex(verify_rpc.VerificationError, "checksum"):
            verify_rpc.decode_contract_id(self.contract_id[:-1] + "A")

    def test_retrieves_wasm_hash_from_rpc_instance(self):
        self.assertEqual(
            verify_rpc.fetch_wasm_hash(self.contract_id, self.rpc_url),
            self.wasm_hash.hex(),
        )

    def test_rpc_errors_are_not_hash_mismatches(self):
        RpcHandler.response_payload = {"jsonrpc": "2.0", "id": 1, "error": {"code": -1}}
        with self.assertRaisesRegex(verify_rpc.VerificationError, "RPC error"):
            verify_rpc.fetch_wasm_hash(self.contract_id, self.rpc_url)

    def test_missing_contract_is_verification_error(self):
        RpcHandler.response_payload = {
            "jsonrpc": "2.0",
            "id": 1,
            "result": {"entries": []},
        }
        with self.assertRaisesRegex(verify_rpc.VerificationError, "not found"):
            verify_rpc.fetch_wasm_hash(self.contract_id, self.rpc_url)

    def test_invalid_rpc_url_is_verification_error(self):
        with self.assertRaisesRegex(verify_rpc.VerificationError, "invalid RPC URL"):
            verify_rpc.fetch_wasm_hash(self.contract_id, "not-a-url")

    def test_mismatched_contract_in_xdr_is_rejected(self):
        other_hash = bytes([255]) * 32
        RpcHandler.response_payload["result"]["entries"][0]["xdr"] = base64.b64encode(
            instance_xdr(other_hash, self.wasm_hash)
        ).decode()
        with self.assertRaisesRegex(verify_rpc.VerificationError, "different contract"):
            verify_rpc.fetch_wasm_hash(self.contract_id, self.rpc_url)

    def test_truncated_instance_xdr_is_rejected(self):
        RpcHandler.response_payload["result"]["entries"][0]["xdr"] = base64.b64encode(
            instance_xdr(self.contract_hash, self.wasm_hash)[:-4]
        ).decode()
        with self.assertRaisesRegex(verify_rpc.VerificationError, "truncated"):
            verify_rpc.fetch_wasm_hash(self.contract_id, self.rpc_url)


if __name__ == "__main__":
    unittest.main()
