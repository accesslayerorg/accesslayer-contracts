// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: safe-mode-contract
// WASM SHA-256: 1d77242f23bb3fa25309fb387f85a6581836a92363ad1a62a7b379c38480d278
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




export enum Mode {
  Normal = 0,
  SafeMode = 1,
  Paused = 2,
}

export const Errors = {
  1: {message:"NotInitialized"},
  2: {message:"Unauthorized"},
  3: {message:"SafeMode"},
  4: {message:"Paused"},
  5: {message:"Timelock"}
}

export enum Action {
  Risk = 0,
  Exit = 1,
  Admin = 2,
}

export interface Client {
  /**
   * Construct and simulate a mode transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  mode: (options?: MethodOptions) => Promise<AssembledTransaction<Result<Mode>>>

  /**
   * Construct and simulate a set_mode transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_mode: ({admin, mode, reason}: {admin: string, mode: Mode, reason: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  initialize: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a require_mode transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  require_mode: ({action}: {action: Action}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a activate_normal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  activate_normal: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

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
      new ContractSpec([ "AAAAAAAAAAAAAAAEbW9kZQAAAAAAAAABAAAD6QAAB9AAAAAETW9kZQAAAAM=",
        "AAAAAwAAAAAAAAAAAAAABE1vZGUAAAADAAAAAAAAAAZOb3JtYWwAAAAAAAAAAAAAAAAACFNhZmVNb2RlAAAAAQAAAAAAAAAGUGF1c2VkAAAAAAAC",
        "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAABQAAAAAAAAAOTm90SW5pdGlhbGl6ZWQAAAAAAAEAAAAAAAAADFVuYXV0aG9yaXplZAAAAAIAAAAAAAAACFNhZmVNb2RlAAAAAwAAAAAAAAAGUGF1c2VkAAAAAAAEAAAAAAAAAAhUaW1lbG9jawAAAAU=",
        "AAAAAAAAAAAAAAAIc2V0X21vZGUAAAADAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAABG1vZGUAAAfQAAAABE1vZGUAAAAAAAAABnJlYXNvbgAAAAAAEAAAAAEAAAPpAAAD7QAAAAAAAAAD",
        "AAAAAwAAAAAAAAAAAAAABkFjdGlvbgAAAAAAAwAAAAAAAAAEUmlzawAAAAAAAAAAAAAABEV4aXQAAAABAAAAAAAAAAVBZG1pbgAAAAAAAAI=",
        "AAAAAAAAAAAAAAAKaW5pdGlhbGl6ZQAAAAAAAQAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAQAAA+kAAAPtAAAAAAAAAAM=",
        "AAAAAAAAAAAAAAAMcmVxdWlyZV9tb2RlAAAAAQAAAAAAAAAGYWN0aW9uAAAAAAfQAAAABkFjdGlvbgAAAAAAAQAAA+kAAAPtAAAAAAAAAAM=",
        "AAAAAAAAAAAAAAAPYWN0aXZhdGVfbm9ybWFsAAAAAAEAAAAAAAAABWFkbWluAAAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAAD" ]),
      options
    )
  }
  public readonly fromJSON = {
    mode: this.txFromJSON<Result<Mode>>,
        set_mode: this.txFromJSON<Result<void>>,
        initialize: this.txFromJSON<Result<void>>,
        require_mode: this.txFromJSON<Result<void>>,
        activate_normal: this.txFromJSON<Result<void>>
  }
}