# @accesslayer/contracts-bindings

Generated TypeScript bindings for the AccessLayer Soroban contracts. Each
contract gets a typed client generated from its WASM interface, plus typed
error enums and typed event decoders. Every contract module is versioned
together as one package, and [manifest.json](./manifest.json) records the exact
WASM hash each module was generated from.

> Do not edit `src/contracts/`, `src/errors.ts`, `src/events.ts`, `src/index.ts`
> or `manifest.json` by hand. They are produced by
> [`scripts/generate-bindings.sh`](../scripts/generate-bindings.sh) and CI fails
> when they drift from the contracts.

## Install

```bash
npm install @accesslayer/contracts-bindings
```

## Use a typed client

```ts
import { holderInsurancePool, Networks } from "@accesslayer/contracts-bindings";

const client = new holderInsurancePool.Client({
  contractId: process.env.POOL_CONTRACT_ID!,
  networkPassphrase: Networks.TESTNET,
  rpcUrl: "https://soroban-testnet.stellar.org",
});

const tx = await client.buy_cover({ ... });
await tx.signAndSend();
```

Each contract module is exported as a namespace from the package root, so
several contracts can be used side by side without name clashes
(`holderInsurancePool.Client`, `multisigAdmin.Client`, ...).

## Decode events

```ts
import { decodeHolderInsurancePoolEvent } from "@accesslayer/contracts-bindings";

for (const event of response.events) {
  const decoded = decodeHolderInsurancePoolEvent(event);
  if (decoded === null) continue;
  console.log(decoded.name, decoded.data); // typed payload
}
```

Decoders are generated from the contracts' event definitions (the documented
`Event shape` blocks and the `env.events().publish` call sites) and read payload
types from the contract spec. An event whose payload type cannot be resolved
statically is typed `unknown`; the name and topics are still decoded.

## Map error codes to enums

```ts
import { contractErrorEnums, describeContractError } from "@accesslayer/contracts-bindings";

// contractErrorEnums[contract][enumName][code] -> { message: "VariantName" }
const error = describeContractError("multisig-admin", "MultisigError", 3);
if (error) console.log(error.message);
```

## Pin a version

The package version describes a snapshot of **all** contract interfaces. To pin:

1. Pin the package version in `package.json` (`"@accesslayer/contracts-bindings": "0.1.0"`).
2. Check `manifest.json` for the `wasmSha256` of the contracts you call and
   verify it matches the hash of the deployed contract (`stellar contract
   info hash` or the deployment record).
3. Re-pin only after reviewing the generated diff for the contracts you use.

`manifest.json` also lists contracts that were skipped during generation (with a
reason), so a contract missing from the package is explicit rather than silent.

## Regenerate

```bash
./scripts/generate-bindings.sh            # build all WASM + regenerate
npm run build                             # type-check and emit dist/
```

See [docs/typescript-bindings.md](../docs/typescript-bindings.md) for the full
pipeline, CI checks and the release process.
