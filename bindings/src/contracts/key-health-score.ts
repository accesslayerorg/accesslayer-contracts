// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: key-health-score
// Interface SHA-256: d897b994c57521fb6f3b629aca5db36064492d90e5057a31aa61f8583f432f70
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




export type Feature = {tag: "Borrowing", values: void} | {tag: "InsuranceCover", values: void};


export interface HealthWeights {
  age: u32;
  concentration: u32;
  reserve_coverage: u32;
  trade_activity: u32;
}


export interface HealthBreakdown {
  activity_score: u32;
  activity_value: i128;
  age_score: u32;
  age_value: u64;
  concentration_score: u32;
  concentration_value: u32;
  reserve_score: u32;
  reserve_value: i128;
  total_score: u32;
}

export const HealthScoreError = {
  1: {message:"AlreadyInitialized"},
  2: {message:"Unauthorized"},
  3: {message:"InvalidWeights"},
  4: {message:"InsufficientHealth"},
  5: {message:"ArithmeticOverflow"}
}

export interface Client {
  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  initialize: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a health_score transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  health_score: ({key_id}: {key_id: string}, options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a min_health_for transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  min_health_for: ({feature}: {feature: Feature}, options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a update_key_data transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  update_key_data: ({admin, key_id, reserve, top_holder_share_bps, volume_7d, created_at}: {admin: string, key_id: string, reserve: i128, top_holder_share_bps: u32, volume_7d: i128, created_at: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a health_breakdown transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  health_breakdown: ({key_id}: {key_id: string}, options?: MethodOptions) => Promise<AssembledTransaction<HealthBreakdown>>

  /**
   * Construct and simulate a set_health_weights transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_health_weights: ({admin, weights}: {admin: string, weights: HealthWeights}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a set_min_health_for transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_min_health_for: ({admin, feature, min_score}: {admin: string, feature: Feature, min_score: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a assert_health_for_feature transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  assert_health_for_feature: ({key_id, feature}: {key_id: string, feature: Feature}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

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
      new ContractSpec([ "AAAAAgAAAAAAAAAAAAAAB0ZlYXR1cmUAAAAAAgAAAAAAAAAAAAAACUJvcnJvd2luZwAAAAAAAAAAAAAAAAAADkluc3VyYW5jZUNvdmVyAAA=",
        "AAAAAAAAAAAAAAAKaW5pdGlhbGl6ZQAAAAAAAQAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAQSGVhbHRoU2NvcmVFcnJvcg==",
        "AAAAAAAAAAAAAAAMaGVhbHRoX3Njb3JlAAAAAQAAAAAAAAAGa2V5X2lkAAAAAAATAAAAAQAAAAQ=",
        "AAAAAAAAAAAAAAAObWluX2hlYWx0aF9mb3IAAAAAAAEAAAAAAAAAB2ZlYXR1cmUAAAAH0AAAAAdGZWF0dXJlAAAAAAEAAAAE",
        "AAAAAAAAAAAAAAAPdXBkYXRlX2tleV9kYXRhAAAAAAYAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAAGa2V5X2lkAAAAAAATAAAAAAAAAAdyZXNlcnZlAAAAAAsAAAAAAAAAFHRvcF9ob2xkZXJfc2hhcmVfYnBzAAAABAAAAAAAAAAJdm9sdW1lXzdkAAAAAAAACwAAAAAAAAAKY3JlYXRlZF9hdAAAAAAABgAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAEEhlYWx0aFNjb3JlRXJyb3I=",
        "AAAAAQAAAAAAAAAAAAAADUhlYWx0aFdlaWdodHMAAAAAAAAEAAAAAAAAAANhZ2UAAAAABAAAAAAAAAANY29uY2VudHJhdGlvbgAAAAAAAAQAAAAAAAAAEHJlc2VydmVfY292ZXJhZ2UAAAAEAAAAAAAAAA50cmFkZV9hY3Rpdml0eQAAAAAABA==",
        "AAAAAAAAAAAAAAAQaGVhbHRoX2JyZWFrZG93bgAAAAEAAAAAAAAABmtleV9pZAAAAAAAEwAAAAEAAAfQAAAAD0hlYWx0aEJyZWFrZG93bgA=",
        "AAAAAQAAAAAAAAAAAAAAD0hlYWx0aEJyZWFrZG93bgAAAAAJAAAAAAAAAA5hY3Rpdml0eV9zY29yZQAAAAAABAAAAAAAAAAOYWN0aXZpdHlfdmFsdWUAAAAAAAsAAAAAAAAACWFnZV9zY29yZQAAAAAAAAQAAAAAAAAACWFnZV92YWx1ZQAAAAAAAAYAAAAAAAAAE2NvbmNlbnRyYXRpb25fc2NvcmUAAAAABAAAAAAAAAATY29uY2VudHJhdGlvbl92YWx1ZQAAAAAEAAAAAAAAAA1yZXNlcnZlX3Njb3JlAAAAAAAABAAAAAAAAAANcmVzZXJ2ZV92YWx1ZQAAAAAAAAsAAAAAAAAAC3RvdGFsX3Njb3JlAAAAAAQ=",
        "AAAAAAAAAAAAAAASc2V0X2hlYWx0aF93ZWlnaHRzAAAAAAACAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAAB3dlaWdodHMAAAAH0AAAAA1IZWFsdGhXZWlnaHRzAAAAAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAQSGVhbHRoU2NvcmVFcnJvcg==",
        "AAAAAAAAAAAAAAASc2V0X21pbl9oZWFsdGhfZm9yAAAAAAADAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAAB2ZlYXR1cmUAAAAH0AAAAAdGZWF0dXJlAAAAAAAAAAAJbWluX3Njb3JlAAAAAAAABAAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAEEhlYWx0aFNjb3JlRXJyb3I=",
        "AAAABAAAAAAAAAAAAAAAEEhlYWx0aFNjb3JlRXJyb3IAAAAFAAAAAAAAABJBbHJlYWR5SW5pdGlhbGl6ZWQAAAAAAAEAAAAAAAAADFVuYXV0aG9yaXplZAAAAAIAAAAAAAAADkludmFsaWRXZWlnaHRzAAAAAAADAAAAAAAAABJJbnN1ZmZpY2llbnRIZWFsdGgAAAAAAAQAAAAAAAAAEkFyaXRobWV0aWNPdmVyZmxvdwAAAAAABQ==",
        "AAAAAAAAAAAAAAAZYXNzZXJ0X2hlYWx0aF9mb3JfZmVhdHVyZQAAAAAAAAIAAAAAAAAABmtleV9pZAAAAAAAEwAAAAAAAAAHZmVhdHVyZQAAAAfQAAAAB0ZlYXR1cmUAAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAQSGVhbHRoU2NvcmVFcnJvcg==" ]),
      options
    )
  }
  public readonly fromJSON = {
    initialize: this.txFromJSON<Result<void>>,
        health_score: this.txFromJSON<u32>,
        min_health_for: this.txFromJSON<u32>,
        update_key_data: this.txFromJSON<Result<void>>,
        health_breakdown: this.txFromJSON<HealthBreakdown>,
        set_health_weights: this.txFromJSON<Result<void>>,
        set_min_health_for: this.txFromJSON<Result<void>>,
        assert_health_for_feature: this.txFromJSON<Result<void>>
  }
}