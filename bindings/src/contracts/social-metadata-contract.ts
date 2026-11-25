// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: social-metadata-contract
// Interface SHA-256: f41dac852c8bb0991399f1b4a48053970f6964d939cedce79a8162844c82e48d
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
  3: {message:"NameTooLong"},
  4: {message:"MissingKey"}
}


export interface SocialMetadata {
  avatar_cid: string;
  bio: string;
  discord: string;
  display_name: string;
  twitter: string;
  website: string;
}

export interface Client {
  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  initialize: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a register_key transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  register_key: ({admin, key, creator}: {admin: string, key: string, creator: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_social_metadata transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_social_metadata: ({key}: {key: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<SocialMetadata>>>

  /**
   * Construct and simulate a set_social_metadata transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_social_metadata: ({key, data}: {key: string, data: SocialMetadata}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

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
      new ContractSpec([ "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAABAAAAAAAAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAABAAAAAAAAAAxVbmF1dGhvcml6ZWQAAAACAAAAAAAAAAtOYW1lVG9vTG9uZwAAAAADAAAAAAAAAApNaXNzaW5nS2V5AAAAAAAE",
        "AAAAAAAAAAAAAAAKaW5pdGlhbGl6ZQAAAAAAAQAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAQAAA+kAAAPtAAAAAAAAAAM=",
        "AAAAAAAAAAAAAAAMcmVnaXN0ZXJfa2V5AAAAAwAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAAAAAANrZXkAAAAAEwAAAAAAAAAHY3JlYXRvcgAAAAATAAAAAQAAA+kAAAPtAAAAAAAAAAM=",
        "AAAAAQAAAAAAAAAAAAAADlNvY2lhbE1ldGFkYXRhAAAAAAAGAAAAAAAAAAphdmF0YXJfY2lkAAAAAAAQAAAAAAAAAANiaW8AAAAAEAAAAAAAAAAHZGlzY29yZAAAAAAQAAAAAAAAAAxkaXNwbGF5X25hbWUAAAAQAAAAAAAAAAd0d2l0dGVyAAAAABAAAAAAAAAAB3dlYnNpdGUAAAAAEA==",
        "AAAAAAAAAAAAAAATZ2V0X3NvY2lhbF9tZXRhZGF0YQAAAAABAAAAAAAAAANrZXkAAAAAEwAAAAEAAAPpAAAH0AAAAA5Tb2NpYWxNZXRhZGF0YQAAAAAAAw==",
        "AAAAAAAAAAAAAAATc2V0X3NvY2lhbF9tZXRhZGF0YQAAAAACAAAAAAAAAANrZXkAAAAAEwAAAAAAAAAEZGF0YQAAB9AAAAAOU29jaWFsTWV0YWRhdGEAAAAAAAEAAAPpAAAD7QAAAAAAAAAD" ]),
      options
    )
  }
  public readonly fromJSON = {
    initialize: this.txFromJSON<Result<void>>,
        register_key: this.txFromJSON<Result<void>>,
        get_social_metadata: this.txFromJSON<Result<SocialMetadata>>,
        set_social_metadata: this.txFromJSON<Result<void>>
  }
}