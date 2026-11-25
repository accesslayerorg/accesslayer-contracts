// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: treasury-diversification
// Interface SHA-256: c13f603b5848b0122d85da3080b8ea14cd8ee3730fa351c6bda418a70099c392
// Regenerate with: ./scripts/generate-bindings.sh
import { Buffer } from "buffer";
import { Address } from "@stellar/stellar-sdk";
import {
  AssembledTransaction,
  Client as ContractClient,
  ClientOptions as ContractClientOptions,
  MethodOptions,
  Result,
  Spec as ContractSpec,
} from "@stellar/stellar-sdk/contract";
import type {
  u32,
  i32,
  u64,
  i64,
  u128,
  i128,
  u256,
  i256,
  Option,
  Timepoint,
  Duration,
} from "@stellar/stellar-sdk/contract";
export * from "@stellar/stellar-sdk";
export * as contract from "@stellar/stellar-sdk/contract";
export * as rpc from "@stellar/stellar-sdk/rpc";

if (typeof window !== "undefined") {
  //@ts-ignore Buffer exists
  window.Buffer = window.Buffer || Buffer;
}





export interface Target {
  asset: string;
  target_bps: u32;
}


export interface SwapCaps {
  per_period_bps: u32;
  per_swap_bps: u32;
  period_duration: u64;
}


export interface SwapRoute {
  from: string;
  path: Array<string>;
  to: string;
}


export interface AssetBalance {
  asset: string;
  balance: i128;
  drift_bps: i32;
  target_bps: u32;
}

export const DiversificationError = {
  1: {message:"AlreadyInitialized"},
  2: {message:"Unauthorized"},
  3: {message:"InvalidRoute"},
  4: {message:"InvalidSlippage"},
  5: {message:"CapExceeded"},
  6: {message:"SwapsPaused"},
  7: {message:"InvalidTargets"},
  8: {message:"TimelockActive"},
  9: {message:"ArithmeticOverflow"}
}

export interface Client {
  /**
   * Construct and simulate a set_caps transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_caps: ({governor, caps}: {governor: string, caps: SwapCaps}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  initialize: ({governor, keeper, timelock_delay}: {governor: string, keeper: string, timelock_delay: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a set_routes transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_routes: ({governor, routes}: {governor: string, routes: Array<SwapRoute>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a pause_swaps transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  pause_swaps: ({governor}: {governor: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a set_targets transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_targets: ({governor, targets}: {governor: string, targets: Array<Target>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a execute_swap transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  execute_swap: ({keeper, from_asset, to_asset, amount_in, min_out}: {keeper: string, from_asset: string, to_asset: string, amount_in: i128, min_out: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a unpause_swaps transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  unpause_swaps: ({governor}: {governor: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a treasury_report transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  treasury_report: (options?: MethodOptions) => Promise<AssembledTransaction<Array<AssetBalance>>>

  /**
   * Construct and simulate a apply_pending_targets transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  apply_pending_targets: (options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions &
      Omit<ContractClientOptions, "contractId"> & {
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {@link Operation.createCustomContract}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
      }
  ): Promise<AssembledTransaction<T>> {
    return ContractClient.deploy(null, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAAAAAAAAAAAAAIc2V0X2NhcHMAAAACAAAAAAAAAAhnb3Zlcm5vcgAAABMAAAAAAAAABGNhcHMAAAfQAAAACFN3YXBDYXBzAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAURGl2ZXJzaWZpY2F0aW9uRXJyb3I=",
        "AAAAAQAAAAAAAAAAAAAABlRhcmdldAAAAAAAAgAAAAAAAAAFYXNzZXQAAAAAAAATAAAAAAAAAAp0YXJnZXRfYnBzAAAAAAAE",
        "AAAAAAAAAAAAAAAKaW5pdGlhbGl6ZQAAAAAAAwAAAAAAAAAIZ292ZXJub3IAAAATAAAAAAAAAAZrZWVwZXIAAAAAABMAAAAAAAAADnRpbWVsb2NrX2RlbGF5AAAAAAAGAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAURGl2ZXJzaWZpY2F0aW9uRXJyb3I=",
        "AAAAAAAAAAAAAAAKc2V0X3JvdXRlcwAAAAAAAgAAAAAAAAAIZ292ZXJub3IAAAATAAAAAAAAAAZyb3V0ZXMAAAAAA+oAAAfQAAAACVN3YXBSb3V0ZQAAAAAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAFERpdmVyc2lmaWNhdGlvbkVycm9y",
        "AAAAAQAAAAAAAAAAAAAACFN3YXBDYXBzAAAAAwAAAAAAAAAOcGVyX3BlcmlvZF9icHMAAAAAAAQAAAAAAAAADHBlcl9zd2FwX2JwcwAAAAQAAAAAAAAAD3BlcmlvZF9kdXJhdGlvbgAAAAAG",
        "AAAAAAAAAAAAAAALcGF1c2Vfc3dhcHMAAAAAAQAAAAAAAAAIZ292ZXJub3IAAAATAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAURGl2ZXJzaWZpY2F0aW9uRXJyb3I=",
        "AAAAAAAAAAAAAAALc2V0X3RhcmdldHMAAAAAAgAAAAAAAAAIZ292ZXJub3IAAAATAAAAAAAAAAd0YXJnZXRzAAAAA+oAAAfQAAAABlRhcmdldAAAAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAURGl2ZXJzaWZpY2F0aW9uRXJyb3I=",
        "AAAAAQAAAAAAAAAAAAAACVN3YXBSb3V0ZQAAAAAAAAMAAAAAAAAABGZyb20AAAATAAAAAAAAAARwYXRoAAAD6gAAABMAAAAAAAAAAnRvAAAAAAAT",
        "AAAAAAAAAAAAAAAMZXhlY3V0ZV9zd2FwAAAABQAAAAAAAAAGa2VlcGVyAAAAAAATAAAAAAAAAApmcm9tX2Fzc2V0AAAAAAATAAAAAAAAAAh0b19hc3NldAAAABMAAAAAAAAACWFtb3VudF9pbgAAAAAAAAsAAAAAAAAAB21pbl9vdXQAAAAACwAAAAEAAAPpAAAACwAAB9AAAAAURGl2ZXJzaWZpY2F0aW9uRXJyb3I=",
        "AAAAAAAAAAAAAAANdW5wYXVzZV9zd2FwcwAAAAAAAAEAAAAAAAAACGdvdmVybm9yAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAFERpdmVyc2lmaWNhdGlvbkVycm9y",
        "AAAAAQAAAAAAAAAAAAAADEFzc2V0QmFsYW5jZQAAAAQAAAAAAAAABWFzc2V0AAAAAAAAEwAAAAAAAAAHYmFsYW5jZQAAAAALAAAAAAAAAAlkcmlmdF9icHMAAAAAAAAFAAAAAAAAAAp0YXJnZXRfYnBzAAAAAAAE",
        "AAAAAAAAAAAAAAAPdHJlYXN1cnlfcmVwb3J0AAAAAAAAAAABAAAD6gAAB9AAAAAMQXNzZXRCYWxhbmNl",
        "AAAAAAAAAAAAAAAVYXBwbHlfcGVuZGluZ190YXJnZXRzAAAAAAAAAAAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAFERpdmVyc2lmaWNhdGlvbkVycm9y",
        "AAAABAAAAAAAAAAAAAAAFERpdmVyc2lmaWNhdGlvbkVycm9yAAAACQAAAAAAAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAABAAAAAAAAAAxVbmF1dGhvcml6ZWQAAAACAAAAAAAAAAxJbnZhbGlkUm91dGUAAAADAAAAAAAAAA9JbnZhbGlkU2xpcHBhZ2UAAAAABAAAAAAAAAALQ2FwRXhjZWVkZWQAAAAABQAAAAAAAAALU3dhcHNQYXVzZWQAAAAABgAAAAAAAAAOSW52YWxpZFRhcmdldHMAAAAAAAcAAAAAAAAADlRpbWVsb2NrQWN0aXZlAAAAAAAIAAAAAAAAABJBcml0aG1ldGljT3ZlcmZsb3cAAAAAAAk=" ]),
      options
    )
  }
  public readonly fromJSON = {
    set_caps: this.txFromJSON<Result<void>>,
        initialize: this.txFromJSON<Result<void>>,
        set_routes: this.txFromJSON<Result<void>>,
        pause_swaps: this.txFromJSON<Result<void>>,
        set_targets: this.txFromJSON<Result<void>>,
        execute_swap: this.txFromJSON<Result<i128>>,
        unpause_swaps: this.txFromJSON<Result<void>>,
        treasury_report: this.txFromJSON<Array<AssetBalance>>,
        apply_pending_targets: this.txFromJSON<Result<void>>
  }
}