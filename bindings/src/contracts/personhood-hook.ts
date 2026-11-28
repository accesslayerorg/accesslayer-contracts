// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: personhood-hook
// Interface SHA-256: 160ed8c66ab9f58fe45661a4a4e8a9abcb8b04b383d78d9a0f8b1bbd3678b25a
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




export const PersonhoodError = {
  1: {message:"AlreadyInitialized"},
  2: {message:"Unauthorized"},
  3: {message:"InvalidAttestation"},
  4: {message:"WalletAlreadyLinked"},
  5: {message:"MaxWalletsReached"},
  6: {message:"CooldownActive"},
  7: {message:"UnverifiedWallet"},
  8: {message:"CapExceeded"},
  9: {message:"AttestationExpired"},
  10: {message:"ArithmeticOverflow"}
}


export interface PersonAttestationData {
  person_id: Buffer;
  valid_until: u64;
  wallet: string;
}

export interface Client {
  /**
   * Construct and simulate a person_of transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  person_of: ({wallet}: {wallet: string}, options?: MethodOptions) => Promise<AssembledTransaction<Option<Buffer>>>

  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  initialize: ({admin, max_wallets_per_person, unlink_cooldown}: {admin: string, max_wallets_per_person: u32, unlink_cooldown: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a link_person transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  link_person: ({wallet, person_id, valid_until, signature}: {wallet: string, person_id: Buffer, valid_until: u64, signature: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a unlink_person transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  unlink_person: ({wallet}: {wallet: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a record_purchase transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  record_purchase: ({wallet, key_id, amount, per_person_cap}: {wallet: string, key_id: string, amount: i128, per_person_cap: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a set_unverified_cap transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_unverified_cap: ({creator, key_id, cap}: {creator: string, key_id: string, cap: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_person_purchases transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_person_purchases: ({person_id, key_id}: {person_id: Buffer, key_id: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a set_personhood_provider transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_personhood_provider: ({admin, attestor}: {admin: string, attestor: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a set_key_requires_personhood transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_key_requires_personhood: ({creator, key_id, required}: {creator: string, key_id: string, required: boolean}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

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
      new ContractSpec([ "AAAAAAAAAAAAAAAJcGVyc29uX29mAAAAAAAAAQAAAAAAAAAGd2FsbGV0AAAAAAATAAAAAQAAA+gAAAPuAAAAIA==",
        "AAAAAAAAAAAAAAAKaW5pdGlhbGl6ZQAAAAAAAwAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAAAAABZtYXhfd2FsbGV0c19wZXJfcGVyc29uAAAAAAAEAAAAAAAAAA91bmxpbmtfY29vbGRvd24AAAAABgAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAD1BlcnNvbmhvb2RFcnJvcgA=",
        "AAAAAAAAAAAAAAALbGlua19wZXJzb24AAAAABAAAAAAAAAAGd2FsbGV0AAAAAAATAAAAAAAAAAlwZXJzb25faWQAAAAAAAPuAAAAIAAAAAAAAAALdmFsaWRfdW50aWwAAAAABgAAAAAAAAAJc2lnbmF0dXJlAAAAAAAADgAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAD1BlcnNvbmhvb2RFcnJvcgA=",
        "AAAAAAAAAAAAAAANdW5saW5rX3BlcnNvbgAAAAAAAAEAAAAAAAAABndhbGxldAAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAD1BlcnNvbmhvb2RFcnJvcgA=",
        "AAAAAAAAAAAAAAAPcmVjb3JkX3B1cmNoYXNlAAAAAAQAAAAAAAAABndhbGxldAAAAAAAEwAAAAAAAAAGa2V5X2lkAAAAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAADnBlcl9wZXJzb25fY2FwAAAAAAALAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAPUGVyc29uaG9vZEVycm9yAA==",
        "AAAABAAAAAAAAAAAAAAAD1BlcnNvbmhvb2RFcnJvcgAAAAAKAAAAAAAAABJBbHJlYWR5SW5pdGlhbGl6ZWQAAAAAAAEAAAAAAAAADFVuYXV0aG9yaXplZAAAAAIAAAAAAAAAEkludmFsaWRBdHRlc3RhdGlvbgAAAAAAAwAAAAAAAAATV2FsbGV0QWxyZWFkeUxpbmtlZAAAAAAEAAAAAAAAABFNYXhXYWxsZXRzUmVhY2hlZAAAAAAAAAUAAAAAAAAADkNvb2xkb3duQWN0aXZlAAAAAAAGAAAAAAAAABBVbnZlcmlmaWVkV2FsbGV0AAAABwAAAAAAAAALQ2FwRXhjZWVkZWQAAAAACAAAAAAAAAASQXR0ZXN0YXRpb25FeHBpcmVkAAAAAAAJAAAAAAAAABJBcml0aG1ldGljT3ZlcmZsb3cAAAAAAAo=",
        "AAAAAAAAAAAAAAASc2V0X3VudmVyaWZpZWRfY2FwAAAAAAADAAAAAAAAAAdjcmVhdG9yAAAAABMAAAAAAAAABmtleV9pZAAAAAAAEwAAAAAAAAADY2FwAAAAAAsAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA9QZXJzb25ob29kRXJyb3IA",
        "AAAAAAAAAAAAAAAUZ2V0X3BlcnNvbl9wdXJjaGFzZXMAAAACAAAAAAAAAAlwZXJzb25faWQAAAAAAAPuAAAAIAAAAAAAAAAGa2V5X2lkAAAAAAATAAAAAQAAAAs=",
        "AAAAAAAAAAAAAAAXc2V0X3BlcnNvbmhvb2RfcHJvdmlkZXIAAAAAAgAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAAAAAAhhdHRlc3RvcgAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA9QZXJzb25ob29kRXJyb3IA",
        "AAAAAQAAAAAAAAAAAAAAFVBlcnNvbkF0dGVzdGF0aW9uRGF0YQAAAAAAAAMAAAAAAAAACXBlcnNvbl9pZAAAAAAAA+4AAAAgAAAAAAAAAAt2YWxpZF91bnRpbAAAAAAGAAAAAAAAAAZ3YWxsZXQAAAAAABM=",
        "AAAAAAAAAAAAAAAbc2V0X2tleV9yZXF1aXJlc19wZXJzb25ob29kAAAAAAMAAAAAAAAAB2NyZWF0b3IAAAAAEwAAAAAAAAAGa2V5X2lkAAAAAAATAAAAAAAAAAhyZXF1aXJlZAAAAAEAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA9QZXJzb25ob29kRXJyb3IA" ]),
      options
    )
  }
  public readonly fromJSON = {
    person_of: this.txFromJSON<Option<Buffer>>,
        initialize: this.txFromJSON<Result<void>>,
        link_person: this.txFromJSON<Result<void>>,
        unlink_person: this.txFromJSON<Result<void>>,
        record_purchase: this.txFromJSON<Result<void>>,
        set_unverified_cap: this.txFromJSON<Result<void>>,
        get_person_purchases: this.txFromJSON<i128>,
        set_personhood_provider: this.txFromJSON<Result<void>>,
        set_key_requires_personhood: this.txFromJSON<Result<void>>
  }
}