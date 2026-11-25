// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: sponsored-transactions
// Interface SHA-256: 07b6615815c237a463b5c5dab866c33f9d2ca0f23415a5b472455a705c83ef92
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
  3: {message:"NonceUsed"},
  4: {message:"Expired"},
  5: {message:"InvalidFee"}
}

export interface Client {
  /**
   * Construct and simulate a is_used transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  is_used: ({user, nonce}: {user: string, nonce: u64}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a authorize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Records a relayed invocation after the user has authenticated its nonce and expiry.
   * Execution of the target call remains atomic in the caller; this contract prevents replay.
   */
  authorize: ({user, relayer, nonce, expires_at, max_fee}: {user: string, relayer: string, nonce: u64, expires_at: u64, max_fee: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  initialize: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

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
      new ContractSpec([ "AAAAAAAAAAAAAAAHaXNfdXNlZAAAAAACAAAAAAAAAAR1c2VyAAAAEwAAAAAAAAAFbm9uY2UAAAAAAAAGAAAAAQAAAAE=",
        "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAABQAAAAAAAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAABAAAAAAAAAAxVbmF1dGhvcml6ZWQAAAACAAAAAAAAAAlOb25jZVVzZWQAAAAAAAADAAAAAAAAAAdFeHBpcmVkAAAAAAQAAAAAAAAACkludmFsaWRGZWUAAAAAAAU=",
        "AAAAAAAAAK1SZWNvcmRzIGEgcmVsYXllZCBpbnZvY2F0aW9uIGFmdGVyIHRoZSB1c2VyIGhhcyBhdXRoZW50aWNhdGVkIGl0cyBub25jZSBhbmQgZXhwaXJ5LgpFeGVjdXRpb24gb2YgdGhlIHRhcmdldCBjYWxsIHJlbWFpbnMgYXRvbWljIGluIHRoZSBjYWxsZXI7IHRoaXMgY29udHJhY3QgcHJldmVudHMgcmVwbGF5LgAAAAAAAAlhdXRob3JpemUAAAAAAAAFAAAAAAAAAAR1c2VyAAAAEwAAAAAAAAAHcmVsYXllcgAAAAATAAAAAAAAAAVub25jZQAAAAAAAAYAAAAAAAAACmV4cGlyZXNfYXQAAAAAAAYAAAAAAAAAB21heF9mZWUAAAAACwAAAAEAAAPpAAAD7QAAAAAAAAAD",
        "AAAAAAAAAAAAAAAKaW5pdGlhbGl6ZQAAAAAAAQAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAQAAA+kAAAPtAAAAAAAAAAM=" ]),
      options
    )
  }
  public readonly fromJSON = {
    is_used: this.txFromJSON<boolean>,
        authorize: this.txFromJSON<Result<void>>,
        initialize: this.txFromJSON<Result<void>>
  }
}