/**
 * Buys through the generated bindings, then decodes the event that call emits.
 *
 * The call is `record_order` with `side: OrderSide.Buy` on `key-liquidity-depth`,
 * which is the buy-side order-book path in this workspace that is fully typed
 * today (see the README for why `creator-keys.buy_key` is not used yet).
 *
 * Run against a local network:
 *   stellar network start local
 *   DEPTH_CONTRACT_ID=<id> npm start
 */
import { Keypair, rpc, xdr } from "@stellar/stellar-sdk";
import {
  decodeEvent,
  decodeKeyLiquidityDepthEvent,
  describeContractError,
  keyLiquidityDepth,
} from "@accesslayer/contracts-bindings";

const RPC_URL = process.env.RPC_URL ?? "http://localhost:8000/soroban/rpc";
const NETWORK_PASSPHRASE =
  process.env.NETWORK_PASSPHRASE ?? "Standalone Network ; February 2017";
const CONTRACT_ID = process.env.DEPTH_CONTRACT_ID;
const ALLOW_HTTP = RPC_URL.startsWith("http://");

// Any address can be tracked in the order book; a fresh one keeps the example
// self-contained. Override it to inspect depth for an existing creator key.
const KEY_ID = process.env.KEY_ID ?? Keypair.random().publicKey();

function required(name: string, value: string | undefined): string {
  if (value === undefined || value === "") {
    throw new Error(`missing ${name}. See the README for the local-network setup.`);
  }
  return value;
}

async function fundWithFriendbot(publicKey: string): Promise<void> {
  const friendbot = new URL("/friendbot", RPC_URL);
  friendbot.searchParams.set("addr", publicKey);
  const response = await fetch(friendbot);
  if (!response.ok) {
    throw new Error(
      `friendbot failed (${response.status}). Pass SECRET_KEY for an already funded account.`,
    );
  }
}

async function main(): Promise<void> {
  required("DEPTH_CONTRACT_ID", CONTRACT_ID);
  const server = new rpc.Server(RPC_URL, { allowHttp: ALLOW_HTTP });

  const signer = process.env.SECRET_KEY
    ? Keypair.fromSecret(process.env.SECRET_KEY)
    : Keypair.random();
  if (process.env.SECRET_KEY === undefined) {
    console.log(`Funding ${signer.publicKey()} from the local friendbot...`);
    await fundWithFriendbot(signer.publicKey());
  }

  const client = new keyLiquidityDepth.Client({
    contractId: CONTRACT_ID!,
    networkPassphrase: NETWORK_PASSPHRASE,
    rpcUrl: RPC_URL,
    allowHttp: ALLOW_HTTP,
    publicKey: signer.publicKey(),
    signTransaction: signer,
  });

  const { sequence: startLedger } = await server.getLatestLedger();

  console.log(`Recording a buy-side order on ${CONTRACT_ID}...`);
  const assembled = await client.record_order({
    key_id: KEY_ID,
    price: 10_000_000n,
    amount: 3n,
    side: keyLiquidityDepth.OrderSide.Buy,
  });
  const sent = await assembled.signAndSend();

  if (sent.result.isErr()) {
    const code = Number(sent.result.unwrapErr());
    const described = describeContractError("key-liquidity-depth", "LiquidityDepthError", code);
    throw new Error(
      `record_order failed: ${described?.message ?? "unknown error"} (code ${code})`,
    );
  }
  console.log(`Order accepted (tx ${sent.sendTransactionResponse?.hash ?? "simulated"}).`);

  console.log("Decoding the events this contract emitted...");
  const { events } = await server.getEvents({
    startLedger,
    filters: [{ type: "contract", contractIds: [CONTRACT_ID!] }],
  });

  for (const event of events) {
    const decoded = decodeKeyLiquidityDepthEvent({
      topics: event.topic as unknown as xdr.ScVal[],
      value: event.value,
    });
    if (decoded !== null) {
      console.log(`  ${decoded.name}`, decoded.data);
      continue;
    }
    const generic = decodeEvent({
      topics: event.topic as unknown as xdr.ScVal[],
      value: event.value,
    });
    console.log("  unrecognised event", generic);
  }
}

main().catch((error: unknown) => {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
});
