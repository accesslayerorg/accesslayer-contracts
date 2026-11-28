// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: creator-penalty
// Interface SHA-256: f0b5d94543a65cc492618fc354303c4626ade17a978d4eea2959b6d3188aadfe
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




export const Errors = {
  1: {message:"AlreadyInitialized"},
  2: {message:"Unauthorized"},
  3: {message:"InvalidAmount"},
  4: {message:"NotFound"},
  5: {message:"AlreadySlashed"}
}


export interface Violation {
  amount: i128;
  creator: string;
  reason: string;
  resolved: boolean;
}

export interface Client {
  /**
   * Construct and simulate a slash transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  slash: ({creator, amount, reason}: {creator: string, amount: i128, reason: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a resolve transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  resolve: ({creator}: {creator: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  initialize: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_violation transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_violation: ({creator}: {creator: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Violation>>>

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
      new ContractSpec([ "AAAAAAAAAAAAAAAFc2xhc2gAAAAAAAADAAAAAAAAAAdjcmVhdG9yAAAAABMAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAGcmVhc29uAAAAAAAQAAAAAQAAA+kAAAPtAAAAAAAAAAM=",
        "AAAAAAAAAAAAAAAHcmVzb2x2ZQAAAAABAAAAAAAAAAdjcmVhdG9yAAAAABMAAAABAAAD6QAAA+0AAAAAAAAAAw==",
        "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAABQAAAAAAAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAABAAAAAAAAAAxVbmF1dGhvcml6ZWQAAAACAAAAAAAAAA1JbnZhbGlkQW1vdW50AAAAAAAAAwAAAAAAAAAITm90Rm91bmQAAAAEAAAAAAAAAA5BbHJlYWR5U2xhc2hlZAAAAAAABQ==",
        "AAAAAAAAAAAAAAAKaW5pdGlhbGl6ZQAAAAAAAQAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAQAAA+kAAAPtAAAAAAAAAAM=",
        "AAAAAQAAAAAAAAAAAAAACVZpb2xhdGlvbgAAAAAAAAQAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAHY3JlYXRvcgAAAAATAAAAAAAAAAZyZWFzb24AAAAAABAAAAAAAAAACHJlc29sdmVkAAAAAQ==",
        "AAAAAAAAAAAAAAANZ2V0X3Zpb2xhdGlvbgAAAAAAAAEAAAAAAAAAB2NyZWF0b3IAAAAAEwAAAAEAAAPpAAAH0AAAAAlWaW9sYXRpb24AAAAAAAAD" ]),
      options
    )
  }
  public readonly fromJSON = {
    slash: this.txFromJSON<Result<void>>,
        resolve: this.txFromJSON<Result<void>>,
        initialize: this.txFromJSON<Result<void>>,
        get_violation: this.txFromJSON<Result<Violation>>
  }
}