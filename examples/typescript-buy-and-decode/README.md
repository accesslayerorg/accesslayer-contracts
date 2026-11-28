# Buy and decode events with the generated bindings

Places a buy-side order through `@accesslayer/contracts-bindings`, then decodes
the event it emits — the workflow a client or indexer repeats in production.

The call is `key-liquidity-depth.record_order` with `side: OrderSide.Buy`, which
is the buy-side order-book entrypoint that is fully typed in this workspace
today. `creator-keys.buy_key` is the intended showcase, but that crate does not
compile on `main` yet, so it has no bindings; see
[`bindings/manifest.json`](../../bindings/manifest.json).

## Prerequisites

- Stellar CLI v22+ and Rust with the `wasm32v1-none` target
- Node.js 18+
- A local network: `stellar network start local`

## Run it

```bash
# 1. Build the bindings package this example imports.
(cd ../../bindings && npm ci && npm run build)

# 2. Build and deploy the contract to the local network.
cargo build --release --target wasm32v1-none -p key-liquidity-depth
stellar keys generate alice --network local --fund
DEPTH_CONTRACT_ID="$(
  stellar contract deploy \
    --wasm ../../target/wasm32v1-none/release/key_liquidity_depth.wasm \
    --source alice \
    --network local
)"
echo "$DEPTH_CONTRACT_ID"

# 3. Install and run the example.
npm install
DEPTH_CONTRACT_ID="$DEPTH_CONTRACT_ID" npm start
```

Expected output (hashes and volumes vary):

```
Funding G... from the local friendbot...
Recording a buy-side order on C...
Order accepted (tx 5c1f...).
Decoding the events this contract emitted...
  depth_upd {
    key_id: 'G...',
    price: 10000000n,
    amount: 3n,
    side: 0,
    volume: 3n
  }
```

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `DEPTH_CONTRACT_ID` | — (required) | Deployed `key-liquidity-depth` contract id |
| `RPC_URL` | `http://localhost:8000/soroban/rpc` | Soroban RPC endpoint |
| `NETWORK_PASSPHRASE` | `Standalone Network ; February 2017` | Passphrase of the target network |
| `SECRET_KEY` | generated + friendbot-funded | Source account for the call |
| `KEY_ID` | random address | Creator key whose order book depth is updated |

`OrderSide` is the generated enum from the bindings — `Buy = 0`, `Sell = 1` —
not a magic number written by hand, which is the point of the package.
