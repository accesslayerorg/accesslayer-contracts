// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: holder-insurance-pool
// Interface SHA-256: 6b54bd48a955072dc6fa37a97b1c1e14fb8a2aa1e6c9833f92ae789cc66169ff
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





export interface Cover {
  amount: i128;
  claimed: boolean;
  expiry: u32;
  holder: string;
  id: u64;
  key_id: string;
  premium: i128;
}

export const Errors = {
  1: {message:"Unauthorized"},
  2: {message:"InvalidAmount"},
  3: {message:"CapExceeded"},
  4: {message:"NoTrigger"},
  5: {message:"Expired"},
  6: {message:"AlreadyClaimed"},
  7: {message:"ReserveViolation"}
}

export interface Client {
  /**
   * Construct and simulate a fund transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  fund: ({from, amount}: {from: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a claim transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  claim: ({holder, cover_id}: {holder: string, cover_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a buy_cover transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  buy_cover: ({holder, key_id, amount, duration, premium_rate_bps, max_cover_ratio_bps}: {holder: string, key_id: string, amount: i128, duration: u32, premium_rate_bps: u32, max_cover_ratio_bps: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>

  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  initialize: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a declare_trigger transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  declare_trigger: ({admin, key_id, trigger_type, evidence_hash}: {admin: string, key_id: string, trigger_type: u32, evidence_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a withdraw_premium_surplus transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  withdraw_premium_surplus: ({admin, amount}: {admin: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

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
      new ContractSpec([ "AAAAAAAAAAAAAAAEZnVuZAAAAAIAAAAAAAAABGZyb20AAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAABAAAD6QAAA+0AAAAAAAAAAw==",
        "AAAAAAAAAAAAAAAFY2xhaW0AAAAAAAACAAAAAAAAAAZob2xkZXIAAAAAABMAAAAAAAAACGNvdmVyX2lkAAAABgAAAAEAAAPpAAAACwAAAAM=",
        "AAAAAQAAAAAAAAAAAAAABUNvdmVyAAAAAAAABwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAdjbGFpbWVkAAAAAAEAAAAAAAAABmV4cGlyeQAAAAAABAAAAAAAAAAGaG9sZGVyAAAAAAATAAAAAAAAAAJpZAAAAAAABgAAAAAAAAAGa2V5X2lkAAAAAAATAAAAAAAAAAdwcmVtaXVtAAAAAAs=",
        "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAABwAAAAAAAAAMVW5hdXRob3JpemVkAAAAAQAAAAAAAAANSW52YWxpZEFtb3VudAAAAAAAAAIAAAAAAAAAC0NhcEV4Y2VlZGVkAAAAAAMAAAAAAAAACU5vVHJpZ2dlcgAAAAAAAAQAAAAAAAAAB0V4cGlyZWQAAAAABQAAAAAAAAAOQWxyZWFkeUNsYWltZWQAAAAAAAYAAAAAAAAAEFJlc2VydmVWaW9sYXRpb24AAAAH",
        "AAAAAAAAAAAAAAAJYnV5X2NvdmVyAAAAAAAABgAAAAAAAAAGaG9sZGVyAAAAAAATAAAAAAAAAAZrZXlfaWQAAAAAABMAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAIZHVyYXRpb24AAAAEAAAAAAAAABBwcmVtaXVtX3JhdGVfYnBzAAAABAAAAAAAAAATbWF4X2NvdmVyX3JhdGlvX2JwcwAAAAAEAAAAAQAAA+kAAAAGAAAAAw==",
        "AAAAAAAAAAAAAAAKaW5pdGlhbGl6ZQAAAAAAAQAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAQAAA+kAAAPtAAAAAAAAAAM=",
        "AAAAAAAAAAAAAAAPZGVjbGFyZV90cmlnZ2VyAAAAAAQAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAAGa2V5X2lkAAAAAAATAAAAAAAAAAx0cmlnZ2VyX3R5cGUAAAAEAAAAAAAAAA1ldmlkZW5jZV9oYXNoAAAAAAAD7gAAACAAAAABAAAD6QAAA+0AAAAAAAAAAw==",
        "AAAAAAAAAAAAAAAYd2l0aGRyYXdfcHJlbWl1bV9zdXJwbHVzAAAAAgAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAABAAAD6QAAA+0AAAAAAAAAAw==" ]),
      options
    )
  }
  public readonly fromJSON = {
    fund: this.txFromJSON<Result<void>>,
        claim: this.txFromJSON<Result<i128>>,
        buy_cover: this.txFromJSON<Result<u64>>,
        initialize: this.txFromJSON<Result<void>>,
        declare_trigger: this.txFromJSON<Result<void>>,
        withdraw_premium_surplus: this.txFromJSON<Result<void>>
  }
}