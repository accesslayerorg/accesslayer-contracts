// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: fee-discount-contract
// Interface SHA-256: 13878dc0a50b51d04d0f460abd2b90f1b5de173c7b2a9696d3ab93c0cea0cd6a
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





export interface Tier {
  discount_bps: u32;
  threshold: i128;
}

export const Errors = {
  1: {message:"AlreadyInitialized"},
  2: {message:"Unauthorized"},
  3: {message:"InvalidTier"},
  4: {message:"Overflow"}
}


export interface Discount {
  discount_bps: u32;
  tier: u32;
}

export interface Client {
  /**
   * Construct and simulate a set_tiers transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_tiers: ({caller, tiers}: {caller: string, tiers: Array<Tier>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  initialize: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_discount transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_discount: ({wallet}: {wallet: string}, options?: MethodOptions) => Promise<AssembledTransaction<Discount>>

  /**
   * Construct and simulate a record_volume transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  record_volume: ({wallet, amount}: {wallet: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Discount>>>

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
      new ContractSpec([ "AAAAAQAAAAAAAAAAAAAABFRpZXIAAAACAAAAAAAAAAxkaXNjb3VudF9icHMAAAAEAAAAAAAAAAl0aHJlc2hvbGQAAAAAAAAL",
        "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAABAAAAAAAAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAABAAAAAAAAAAxVbmF1dGhvcml6ZWQAAAACAAAAAAAAAAtJbnZhbGlkVGllcgAAAAADAAAAAAAAAAhPdmVyZmxvdwAAAAQ=",
        "AAAAAAAAAAAAAAAJc2V0X3RpZXJzAAAAAAAAAgAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAV0aWVycwAAAAAAA+oAAAfQAAAABFRpZXIAAAABAAAD6QAAA+0AAAAAAAAAAw==",
        "AAAAAAAAAAAAAAAKaW5pdGlhbGl6ZQAAAAAAAQAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAQAAA+kAAAPtAAAAAAAAAAM=",
        "AAAAAQAAAAAAAAAAAAAACERpc2NvdW50AAAAAgAAAAAAAAAMZGlzY291bnRfYnBzAAAABAAAAAAAAAAEdGllcgAAAAQ=",
        "AAAAAAAAAAAAAAAMZ2V0X2Rpc2NvdW50AAAAAQAAAAAAAAAGd2FsbGV0AAAAAAATAAAAAQAAB9AAAAAIRGlzY291bnQ=",
        "AAAAAAAAAAAAAAANcmVjb3JkX3ZvbHVtZQAAAAAAAAIAAAAAAAAABndhbGxldAAAAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAQAAA+kAAAfQAAAACERpc2NvdW50AAAAAw==" ]),
      options
    )
  }
  public readonly fromJSON = {
    set_tiers: this.txFromJSON<Result<void>>,
        initialize: this.txFromJSON<Result<void>>,
        get_discount: this.txFromJSON<Discount>,
        record_volume: this.txFromJSON<Result<Discount>>
  }
}