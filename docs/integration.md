# Integrating with AccessLayer creator keys

This guide is for teams that want their own Soroban contract or client to read
creator-key prices, check who holds keys, and react to key transfers. It covers
the public interface, errors, events, authorization, resource limits and the
upgrade guarantees you can rely on, and walks through a working example
contract in [`examples/consumer-contract`](../examples/consumer-contract).

The example's tests run against the real `creator-keys` contract on every CI
run (`cargo test --workspace`), so the code shown here stays in step with the
protocol.

## Contents

1. [What you can build](#what-you-can-build)
2. [Public interface](#public-interface)
3. [Calling the protocol from your contract](#calling-the-protocol-from-your-contract)
4. [Receiving keys: `transfer_and_call`](#receiving-keys-transfer_and_call)
5. [Errors](#errors)
6. [Events](#events)
7. [Authorization](#authorization)
8. [Resource considerations](#resource-considerations)
9. [Client calls: simulate, sign, submit, decode](#client-calls-simulate-sign-submit-decode)
10. [Run the example from scratch](#run-the-example-from-scratch)
11. [Upgrade and versioning guarantees](#upgrade-and-versioning-guarantees)

## What you can build

A creator key is identified by the creator's address (`creator`, also called
`key_id`). Every holder balance, quote and event is scoped to that address
inside one deployed `creator-keys` contract.

The example contract does three things most integrations need:

| Need | Example entry point | Protocol call |
| --- | --- | --- |
| Gate a function on key ownership | `enter(caller)` | `get_key_balance(creator, wallet)` |
| Show or check the current price | `current_quote()` | `get_buy_quote(creator)` |
| Accept keys sent to your contract | `on_key_received(...)` | invoked by `transfer_and_call` |

## Public interface

These are the entry points an integrator needs. All amounts are `i128` in the
payment asset's smallest unit; key counts are `u32`.

### Read-only views

| Function | Returns | Notes |
| --- | --- | --- |
| `get_key_balance(creator, wallet)` | `u32` | `0` for a wallet that never held keys. Never errors. |
| `get_balance(creator, wallet)` | `u32` | Alias of `get_key_balance`. |
| `get_buy_quote(creator)` | `Result<QuoteResponse, ContractError>` | Price of the next key plus fees. Mirrors what `buy_key` charges, including a pre-launch auction price while one is active. |
| `get_sell_quote(creator, holder)` | `Result<QuoteResponse, ContractError>` | Fails with `InsufficientBalance` when `holder` has no keys. |
| `get_price_at_supply(creator, supply)` | `Result<i128, ContractError>` | Bonding-curve price at an arbitrary supply. |
| `get_supply_info(key_id)` | `Result<SupplyInfo, ContractError>` | `supply`, `cap` and `remaining`. |
| `is_paused()` | `bool` | While `true`, trading and transfers fail with `ProtocolPaused`. |
| `get_protocol_state_version()` | `u32` | Increments when protocol configuration changes. See [versioning](#upgrade-and-versioning-guarantees). |

`QuoteResponse` has four fields: `price`, `creator_fee`, `protocol_fee` and
`total_amount` (what a buyer pays).

### State-changing calls

| Function | Signer | Notes |
| --- | --- | --- |
| `buy_key(creator, buyer, payment, max_price)` | `buyer` | `payment` must cover `get_buy_quote(creator).total_amount`. `max_price: Option<i128>` caps the price you accept. |
| `sell_key(creator, seller, min_proceeds)` | `seller` | `min_proceeds: Option<i128>` is the slippage floor. |
| `transfer_keys(creator, from, to, amount)` | `from` | Moves keys without touching the bonding curve. |
| `transfer_and_call(creator, from, to, amount, data)` | `from` | Same as `transfer_keys`, then calls `to.on_key_received(...)` when `to` is a contract. `data` is at most 256 bytes. |

### Receiver interface

Defined in the `common` crate and re-exported by `creator-keys`:

```rust
pub const KEY_RECEIVED_ACK: Symbol = symbol_short!("key_ack");

#[contractclient(name = "KeyReceiverClient")]
pub trait KeyReceiver {
    fn on_key_received(env: Env, from: Address, key_id: Address, amount: u32, data: Bytes) -> Symbol;
}
```

## Calling the protocol from your contract

You do not need to depend on the `creator-keys` crate. Declare the slice of the
interface you call and let `contractclient` generate a client; your Wasm then
contains only those calls. From
[`examples/consumer-contract/src/lib.rs`](../examples/consumer-contract/src/lib.rs):

```rust
/// Wire-compatible mirror of `creator_keys::QuoteResponse`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuoteResponse {
    pub price: i128,
    pub creator_fee: i128,
    pub protocol_fee: i128,
    pub total_amount: i128,
}

/// The `creator_keys::ContractError` codes this contract branches on.
#[contracterror(export = false)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ProtocolError {
    NotRegistered = 2,
    KeyPriceNotSet = 5,
}

#[contractclient(name = "CreatorKeysClient")]
pub trait CreatorKeysInterface {
    fn get_key_balance(env: Env, creator: Address, wallet: Address) -> u32;
    fn get_buy_quote(env: Env, creator: Address) -> Result<QuoteResponse, ProtocolError>;
}
```

A `#[contracttype]` struct is encoded as a map keyed by field name, so the
mirror decodes the protocol's value as long as the field names and types match.

### Gating on key balance

```rust
pub fn enter(env: Env, caller: Address) -> Result<u32, ConsumerError> {
    caller.require_auth();
    let config = read_config(&env);
    let balance = CreatorKeysClient::new(&env, &config.keys_contract)
        .get_key_balance(&config.key_id, &caller);
    if balance < config.min_balance {
        return Err(ConsumerError::InsufficientKeys);
    }
    // ... record the entry, emit `access`
    Ok(balance)
}
```

Read the balance at the moment you need it. Keys can be sold or transferred at
any time, so a cached balance goes stale; the example's
`access_follows_the_keys_after_a_transfer` test shows access moving with the
keys.

### Reading the quote and handling protocol errors

The generated `try_*` methods return the callee's error as a value instead of
aborting your contract:

```rust
pub fn current_quote(env: Env) -> Result<QuoteResponse, ConsumerError> {
    let config = read_config(&env);
    match CreatorKeysClient::new(&env, &config.keys_contract).try_get_buy_quote(&config.key_id) {
        Ok(Ok(quote)) => Ok(quote),
        Err(Ok(ProtocolError::NotRegistered | ProtocolError::KeyPriceNotSet)) => {
            Err(ConsumerError::QuoteUnavailable)
        }
        _ => Err(ConsumerError::ProtocolCallFailed),
    }
}
```

- `Ok(Ok(value))`: the call succeeded and the value decoded.
- `Ok(Err(_))`: the call succeeded but the value did not decode into your type,
  which means your mirror has drifted from the protocol.
- `Err(Ok(e))`: the protocol returned a contract error that your mirror lists.
- `Err(Err(_))`: any other failure, including error codes you did not mirror.

Call the non-`try` method (`get_buy_quote`) instead if any protocol error should
simply abort your transaction.

## Receiving keys: `transfer_and_call`

`transfer_and_call(creator, from, to, amount, data)` runs in this order:

1. Rejects `data` longer than 256 bytes (`BatchTooLarge`) and nested calls made
   from inside another callback (`TransferAlreadyAccepted`).
2. Performs the same checks and balance move as `transfer_keys`.
3. If `to` is a contract address, calls
   `to.on_key_received(from, creator, amount, data)`.
4. If the callback returns anything other than `KEY_RECEIVED_ACK`, the whole
   transfer reverts with `TransferNotAccepted`. A panicking callback reverts it
   too.
5. Emits `transfer_called` with the SHA-256 of `data`.

When `to` is an account (a `G...` address) the callback is skipped and the call
behaves like `transfer_keys`.

The example's receiver accepts only the key it was configured for:

```rust
fn on_key_received(env: Env, from: Address, key_id: Address, amount: u32, data: Bytes) -> Symbol {
    let config = read_config(&env);
    if key_id != config.key_id || amount == 0 {
        return REJECT; // the protocol reverts the transfer
    }
    // ... store a receipt, emit `received`
    KEY_RECEIVED_ACK
}
```

Two rules for receivers:

- **Do not call back into `creator-keys` from the callback.** The protocol is
  still executing `transfer_and_call`, and Soroban rejects re-entry into a
  contract that is already on the call stack, so the transfer would revert.
  This includes read-only calls such as `get_key_balance`.
- **Treat the callback arguments as untrusted.** Anyone can call
  `on_key_received` directly with made-up values. Grant nothing of value from
  the arguments alone; confirm holdings with `get_key_balance` in a separate
  call. The example's `held_keys()` does this, and the
  `a_direct_callback_call_proves_nothing` test shows a forged callback moving no
  keys.

## Errors

Protocol failures surface as `Error(Contract, #N)`. These are the codes an
integrator is most likely to see:

| Code | Name | When |
| --- | --- | --- |
| 2 | `NotRegistered` | The creator key does not exist. |
| 3 | `Overflow` | Arithmetic overflow in a price or balance calculation. |
| 4 | `InsufficientPayment` | `payment` does not cover the price of a single key. |
| 5 | `KeyPriceNotSet` | No base price has been configured yet. |
| 9 | `InsufficientBalance` | The holder has fewer keys than requested (sells, transfers, sell quotes). |
| 15 | `ZeroAddress` | A transfer recipient is the zero address. |
| 16 | `SlippageExceeded` | The price moved past `max_price` / `min_proceeds`. |
| 17 | `ProtocolPaused` | The protocol is paused (`is_paused()` is `true`). |
| 18 | `Unauthorized` | The caller lacks the role the function requires. |
| 26 | `SelfTransfer` | `from == to`. |
| 27 | `ZeroTransferAmount` | A transfer of `0` keys. |
| 36 | `CooldownActive` | The sender is inside the creator's buy cooldown. |
| 63 | `KeyDeprecated` | The key is in its deprecation window. |
| 72 | `FrozenPosition` | The sender's own frozen keys block the transfer. |
| 122 | `TransferNotAccepted` | The receiver did not return `KEY_RECEIVED_ACK`. |
| 123 | `TransferAlreadyAccepted` | A transfer was attempted from inside a receiver callback. |
| 139 | `KeyFrozen` | The whole key is frozen. |
| 153 | `BatchTooLarge` | `transfer_and_call` data is over 256 bytes. |

The full list is the `ContractError` enum in `creator-keys/src/lib.rs`; see
also [error-codes.md](error-codes.md). Codes are only ever appended, so a
number keeps its meaning across upgrades.

The example contract has its own error space:

| Code | `ConsumerError` | When |
| --- | --- | --- |
| 1 | `InvalidConfig` | Constructor got `min_balance == 0` or `keys_contract == key_id`. |
| 2 | `InsufficientKeys` | The caller holds fewer than `min_balance` keys. |
| 3 | `QuoteUnavailable` | The protocol returned `NotRegistered` or `KeyPriceNotSet`. |
| 4 | `ProtocolCallFailed` | Any other protocol failure. |

## Events

Topic 0 is the event name. Payloads are `#[contracttype]` structs whose field
order is stable (see [event-stability-guarantees.md](event-stability-guarantees.md)).

| Event | Topics | Payload fields |
| --- | --- | --- |
| `register` | `("register", creator)` | `CreatorRegisteredEvent`: `creator`, `handle`, `supply`, `holder_count`, `creator_bps`, `protocol_bps`, `fee_recipient`, `registered_at_ledger` |
| `buy` | `("buy", creator, buyer)` | `KeysBoughtEvent`: `buyer`, `creator_id`, `quantity`, `price_paid`, `new_supply`, `ledger` |
| `sell` | `("sell", creator, seller)` | `KeysSoldEvent`: `seller`, `creator_id`, `quantity`, `proceeds`, `ledger` |
| `xfer` | `("xfer", creator, from)` | `KeysTransferredEvent`: `creator_id`, `from`, `to`, `amount`, `ledger` |
| `transfer_called` | `("transfer_called", creator, recipient)` | `TransferCalledEvent`: `creator_id`, `from`, `recipient`, `amount`, `data_hash` |

The example contract adds two of its own: `("access", caller)` with the caller's
balance as data, and `("received", from)` with the transferred amount.

Decoding in Rust (from the example's tests):

```rust
let (_, payload) = event_data(&env, &keys_id, Symbol::new(&env, TRANSFER_CALLED_EVENT_NAME)).unwrap();
let called = TransferCalledEvent::try_from_val(&env, &payload).unwrap();
assert_eq!(called.data_hash, env.crypto().sha256(&data).to_bytes());
```

Decoding from a client is shown [below](#4-decode-events).

## Authorization

- Every state-changing call needs the signature of the address whose keys or
  funds move: `buyer` for `buy_key`, `seller` for `sell_key`, `from` for
  `transfer_keys` and `transfer_and_call`.
- Views need no signature and can be simulated without submitting.
- If your contract calls `require_auth` for a user and then calls the protocol
  on that user's behalf (for example `transfer_keys` with `from = user`), the
  user signs one authorization tree that covers both invocations. Simulation
  records the tree, so clients get it from `prepareTransaction` (or the CLI)
  without building it by hand.
- A receiver callback runs inside the sender's transaction. It cannot ask the
  sender for more signatures unless the sender's authorization tree already
  includes that call.
- Admin-only functions (`set_key_price`, `set_fee_config`, `upgrade`, ...) check
  the protocol admin and fail with `Unauthorized` otherwise. See
  [authorization-model.md](authorization-model.md).

In tests, `env.mock_all_auths()` approves every `require_auth`. The example's
`gate_requires_the_callers_signature` test turns mocking off with
`env.set_auths(&[])` to show the gate rejecting an unsigned call.

## Resource considerations

- **One budget per transaction.** Soroban meters CPU, memory and ledger
  footprint for the whole transaction, including every cross-contract call and
  receiver callback. A heavy callback can push `transfer_and_call` over budget,
  which reverts the transfer. Keep callbacks to a few storage writes.
- **Callback data is capped at 256 bytes** (`MAX_KEY_RECEIVER_DATA_LEN`). Pass
  an identifier and keep larger payloads elsewhere.
- **Every cross-contract read adds footprint.** `enter` reads one balance and
  `current_quote` reads the price, fee and creator profile entries. Simulate
  first; the simulation result carries the footprint and resource fee.
- **Storage TTL.** Balances live in persistent storage and are kept alive by
  the protocol. Your own contract must extend the TTL of anything it stores; the
  example extends its instance storage and each `Entries(user)` record to about
  30 days whenever they fall below about 7 days.
- **Reads can write.** A few protocol views extend a TTL as a side effect (for
  example `get_holder_registry`). When simulating, prefer the plain balance and
  quote views listed above.

## Client calls: simulate, sign, submit, decode

The snippets use the Stellar CLI and the JavaScript SDK
(`@stellar/stellar-sdk`) against a local network. Set these once:

```sh
export NETWORK=local
export KEYS=<creator-keys contract id>
export CONSUMER=<consumer contract id>
export CREATOR=<creator address>
```

### 1. Simulate a read

CLI (read-only calls are simulated and not submitted):

```sh
stellar contract invoke --id "$KEYS" --source alice --network "$NETWORK" \
  -- get_buy_quote --creator "$CREATOR"
```

JavaScript:

```js
import {
  Address, BASE_FEE, Contract, Keypair, Networks, TransactionBuilder,
  nativeToScVal, rpc, scValToNative, xdr,
} from "@stellar/stellar-sdk";

const server = new rpc.Server("http://localhost:8000/rpc", { allowHttp: true });
const networkPassphrase = Networks.STANDALONE;
const keys = new Contract(process.env.KEYS);
const alice = Keypair.fromSecret(process.env.ALICE_SECRET); // `stellar keys show alice`

async function simulate(source, operation) {
  const account = await server.getAccount(source.publicKey());
  const tx = new TransactionBuilder(account, { fee: BASE_FEE, networkPassphrase })
    .addOperation(operation)
    .setTimeout(30)
    .build();
  const sim = await server.simulateTransaction(tx);
  if (rpc.Api.isSimulationError(sim)) throw new Error(sim.error);
  return scValToNative(sim.result.retval);
}

const quote = await simulate(
  alice,
  keys.call("get_buy_quote", new Address(process.env.CREATOR).toScVal()),
);
// { price: 100n, creator_fee: ..., protocol_fee: ..., total_amount: ... }
```

### 2. Sign and submit

CLI:

```sh
stellar contract invoke --id "$KEYS" --source alice --network "$NETWORK" \
  -- transfer_and_call --creator "$CREATOR" --from "$(stellar keys address alice)" \
     --to "$CONSUMER" --amount 1 --data 6465706f736974   # hex of "deposit"
```

JavaScript, continuing from above:

```js
async function submit(source, operation) {
  const account = await server.getAccount(source.publicKey());
  let tx = new TransactionBuilder(account, { fee: BASE_FEE, networkPassphrase })
    .addOperation(operation)
    .setTimeout(30)
    .build();
  // Simulates, then adds the footprint, resource fee and recorded auth entries.
  tx = await server.prepareTransaction(tx);
  tx.sign(source);

  const sent = await server.sendTransaction(tx);
  if (sent.status === "ERROR") throw new Error(JSON.stringify(sent.errorResult));

  let result = await server.getTransaction(sent.hash);
  while (result.status === rpc.Api.GetTransactionStatus.NOT_FOUND) {
    await new Promise((resolve) => setTimeout(resolve, 1000));
    result = await server.getTransaction(sent.hash);
  }
  if (result.status !== rpc.Api.GetTransactionStatus.SUCCESS) {
    throw new Error(`transaction failed: ${result.status}`);
  }
  return result;
}

const result = await submit(
  alice,
  keys.call(
    "transfer_and_call",
    new Address(process.env.CREATOR).toScVal(),
    new Address(alice.publicKey()).toScVal(),
    new Address(process.env.CONSUMER).toScVal(),
    nativeToScVal(1, { type: "u32" }),
    nativeToScVal(Buffer.from("deposit")),
  ),
);
```

### 3. Handle errors

A contract error appears in the simulation error text as
`Error(Contract, #N)`. Simulate before submitting and map the code:

```js
function contractErrorCode(error) {
  const match = /Error\(Contract, #(\d+)\)/.exec(String(error?.message ?? error));
  return match ? Number(match[1]) : undefined;
}

try {
  await submit(alice, keys.call("transfer_and_call", /* ... */));
} catch (error) {
  switch (contractErrorCode(error)) {
    case 9:   console.log("not enough keys"); break;
    case 122: console.log("the receiver refused the transfer"); break;
    case 17:  console.log("protocol is paused, try later"); break;
    default:  throw error;
  }
}
```

### 4. Decode events

CLI:

```sh
stellar events --network "$NETWORK" --start-ledger <ledger> --id "$KEYS" --output json
```

JavaScript:

```js
const { events } = await server.getEvents({
  startLedger: result.ledger,
  filters: [{
    type: "contract",
    contractIds: [process.env.KEYS],
    topics: [[xdr.ScVal.scvSymbol("transfer_called").toXDR("base64"), "*", "*"]],
  }],
});

for (const event of events) {
  const [name, creator, recipient] = event.topic.map(scValToNative);
  const { from, amount, data_hash } = scValToNative(event.value);
  console.log(name, creator, recipient, from, amount, Buffer.from(data_hash).toString("hex"));
}
```

`scValToNative` turns a `#[contracttype]` struct into a plain object keyed by
field name, so the fields match the payload tables in [Events](#events).

## Run the example from scratch

### Tests (no network needed)

```sh
cargo test -p consumer-contract
```

The suite deploys the real `creator-keys` contract in a test environment and
covers the gate (enough keys, too few keys, missing signature, access moving
after a transfer), quote reads and their error mapping, a successful
`transfer_and_call` with event decoding, a refused transfer that reverts, and a
forged direct callback.

### Local network

Prerequisites: Rust and the Stellar CLI (see
[local-soroban-prerequisites.md](local-soroban-prerequisites.md)) and Docker for
the local network.

```sh
# 1. Start a local network (RPC on http://localhost:8000/rpc) and fund identities.
stellar container start local
stellar network use local
stellar keys generate admin --network local --fund
stellar keys generate alice --network local --fund

# 2. Build every contract in the workspace; the CLI prints each .wasm path.
stellar contract build

# 3. Deploy and configure creator-keys (the first set_protocol_admin call
#    bootstraps the admin).
KEYS=$(stellar contract deploy --wasm <path>/creator_keys.wasm --source admin --network local)
ADMIN=$(stellar keys address admin)
stellar contract invoke --id "$KEYS" --source admin --network local \
  -- set_protocol_admin --admin "$ADMIN" --new_admin "$ADMIN"
stellar contract invoke --id "$KEYS" --source admin --network local \
  -- set_key_price --admin "$ADMIN" --price 100
stellar contract invoke --id "$KEYS" --source admin --network local \
  -- set_fee_config --admin "$ADMIN" --creator_bps 9000 --protocol_bps 1000

# 4. Register alice as a creator and buy one of her keys.
CREATOR=$(stellar keys address alice)
stellar contract invoke --id "$KEYS" --source alice --network local \
  -- register_creator --params "{\"creator\":\"$CREATOR\",\"handle\":\"alice\"}"
stellar contract invoke --id "$KEYS" --source alice --network local \
  -- get_buy_quote --creator "$CREATOR"
stellar contract invoke --id "$KEYS" --source alice --network local \
  -- buy_key --creator "$CREATOR" --buyer "$CREATOR" --payment <total_amount from the quote>

# 5. Deploy the consumer for alice's key (constructor args after `--`).
CONSUMER=$(stellar contract deploy --wasm <path>/consumer_contract.wasm --source admin --network local \
  -- --keys_contract "$KEYS" --key_id "$CREATOR" --min_balance 1)

# 6. Use it.
stellar contract invoke --id "$CONSUMER" --source alice --network local -- enter --caller "$CREATOR"
stellar contract invoke --id "$CONSUMER" --source alice --network local -- current_quote
stellar contract invoke --id "$KEYS" --source alice --network local \
  -- transfer_and_call --creator "$CREATOR" --from "$CREATOR" --to "$CONSUMER" --amount 1 --data 00
stellar contract invoke --id "$CONSUMER" --source alice --network local -- held_keys
```

`register_creator` and `buy_key` also take optional arguments (supply caps,
curve preset, `max_price`, ...); leaving them out passes `None`.

## Upgrade and versioning guarantees

What integrators can rely on:

- **The contract address does not change.** Upgrades swap the code in place
  with `update_current_contract_wasm`. Storage is keyed to the address, so
  balances, quotes and configuration carry over and the contract id in your
  integration keeps working.
- **Upgrades are announced and delayed.** `upgrade` only schedules a swap. It
  executes after the timelock delay (`get_timelock_delay()`, 2 days by default,
  at most 30 days) and only with the required multisig approvals, and never
  while the protocol is paused. Watch for scheduled actions to see an upgrade
  coming before it lands.
- **Error codes are append-only.** Existing `ContractError` numbers keep their
  meaning; new failures get new numbers. Mirroring the codes you branch on (as
  the example does) stays correct across upgrades.
- **Event shapes are stable.** Event names, topic order and payload field order
  do not change; new topics and fields are only appended. Decode by position or
  name and ignore fields you do not know. Details in
  [event-stability-guarantees.md](event-stability-guarantees.md).
- **The deployed code is verifiable.** Each release records its Wasm hash;
  compare it with the on-chain hash as described in [VERSIONING.md](VERSIONING.md).
- **Configuration changes are visible.** `get_protocol_state_version()`
  increments when protocol configuration changes, so a client can detect that
  cached prices or fee splits need refreshing.

What is not guaranteed:

- Internal storage keys and layout. Read state through the public views, never
  by reading ledger entries directly.
- Prices and fees. They follow the bonding curve and admin configuration, so
  always quote right before trading and pass `max_price` / `min_proceeds`.
- Availability. The protocol can be paused; check `is_paused()` or handle
  `ProtocolPaused` (17).
