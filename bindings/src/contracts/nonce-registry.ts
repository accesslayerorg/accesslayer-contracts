// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: nonce-registry
// WASM SHA-256: 5d2919ce993fbbc9994c3d02c7c09d62e4f117b2226240b67432c3689a1ea999
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




export type DataKey = {tag: "UsedNonce", values: readonly [string, u64]};


/**
 * Payload of the `NonceConsumed` event.
 */
export interface NonceConsumed {
  nonce: u64;
  wallet: string;
}

export const NonceRegistryError = {
  /**
   * The nonce has already been consumed for this wallet.
   */
  1: {message:"NonceAlreadyUsed"}
}

export interface Client {
  /**
   * Construct and simulate a consume_nonce transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Marks `nonce` as consumed for `wallet`.
   * 
   * Returns [`NonceRegistryError::NonceAlreadyUsed`] if the nonce was already
   * consumed, and emits `NonceConsumed` otherwise. Deliberately does not
   * require authorization: submission is performed by a relayer, and the
   * signed message being verified is what authorizes the action. As a
   * result a third party can burn a nonce, which only invalidates a
   * meta-transaction that has not been submitted yet.
   */
  consume_nonce: ({wallet, nonce}: {wallet: string, nonce: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a is_nonce_used transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns whether `nonce` has been consumed for `wallet`.
   */
  is_nonce_used: ({wallet, nonce}: {wallet: string, nonce: u64}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a bulk_invalidate transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pre-emptively burns every nonce in `nonces` for `wallet`.
   * 
   * Only the wallet itself may invalidate its nonces. Nonces that are
   * already consumed are skipped rather than rejected, so the call is
   * idempotent; the returned value is the number of newly consumed nonces.
   * `NonceConsumed` is emitted once per newly consumed nonce.
   */
  bulk_invalidate: ({wallet, nonces}: {wallet: string, nonces: Array<u64>}, options?: MethodOptions) => Promise<AssembledTransaction<u32>>

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
      new ContractSpec([ "AAAAAgAAAAAAAAAAAAAAB0RhdGFLZXkAAAAAAQAAAAEAAABlTWFya3MgYCh3YWxsZXQsIG5vbmNlKWAgYXMgY29uc3VtZWQuIE5vbmNlcyBjYXJyeSBubyBvcmRlcmluZywgc28gZWFjaApwYWlyIGlzIHRyYWNrZWQgaW5kZXBlbmRlbnRseS4AAAAAAAAJVXNlZE5vbmNlAAAAAAAAAgAAABMAAAAG",
        "AAAAAAAAAbBNYXJrcyBgbm9uY2VgIGFzIGNvbnN1bWVkIGZvciBgd2FsbGV0YC4KClJldHVybnMgW2BOb25jZVJlZ2lzdHJ5RXJyb3I6Ok5vbmNlQWxyZWFkeVVzZWRgXSBpZiB0aGUgbm9uY2Ugd2FzIGFscmVhZHkKY29uc3VtZWQsIGFuZCBlbWl0cyBgTm9uY2VDb25zdW1lZGAgb3RoZXJ3aXNlLiBEZWxpYmVyYXRlbHkgZG9lcyBub3QKcmVxdWlyZSBhdXRob3JpemF0aW9uOiBzdWJtaXNzaW9uIGlzIHBlcmZvcm1lZCBieSBhIHJlbGF5ZXIsIGFuZCB0aGUKc2lnbmVkIG1lc3NhZ2UgYmVpbmcgdmVyaWZpZWQgaXMgd2hhdCBhdXRob3JpemVzIHRoZSBhY3Rpb24uIEFzIGEKcmVzdWx0IGEgdGhpcmQgcGFydHkgY2FuIGJ1cm4gYSBub25jZSwgd2hpY2ggb25seSBpbnZhbGlkYXRlcyBhCm1ldGEtdHJhbnNhY3Rpb24gdGhhdCBoYXMgbm90IGJlZW4gc3VibWl0dGVkIHlldC4AAAANY29uc3VtZV9ub25jZQAAAAAAAAIAAAAAAAAABndhbGxldAAAAAAAEwAAAAAAAAAFbm9uY2UAAAAAAAAGAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAASTm9uY2VSZWdpc3RyeUVycm9yAAA=",
        "AAAAAAAAADdSZXR1cm5zIHdoZXRoZXIgYG5vbmNlYCBoYXMgYmVlbiBjb25zdW1lZCBmb3IgYHdhbGxldGAuAAAAAA1pc19ub25jZV91c2VkAAAAAAAAAgAAAAAAAAAGd2FsbGV0AAAAAAATAAAAAAAAAAVub25jZQAAAAAAAAYAAAABAAAAAQ==",
        "AAAAAAAAAT9QcmUtZW1wdGl2ZWx5IGJ1cm5zIGV2ZXJ5IG5vbmNlIGluIGBub25jZXNgIGZvciBgd2FsbGV0YC4KCk9ubHkgdGhlIHdhbGxldCBpdHNlbGYgbWF5IGludmFsaWRhdGUgaXRzIG5vbmNlcy4gTm9uY2VzIHRoYXQgYXJlCmFscmVhZHkgY29uc3VtZWQgYXJlIHNraXBwZWQgcmF0aGVyIHRoYW4gcmVqZWN0ZWQsIHNvIHRoZSBjYWxsIGlzCmlkZW1wb3RlbnQ7IHRoZSByZXR1cm5lZCB2YWx1ZSBpcyB0aGUgbnVtYmVyIG9mIG5ld2x5IGNvbnN1bWVkIG5vbmNlcy4KYE5vbmNlQ29uc3VtZWRgIGlzIGVtaXR0ZWQgb25jZSBwZXIgbmV3bHkgY29uc3VtZWQgbm9uY2UuAAAAAA9idWxrX2ludmFsaWRhdGUAAAAAAgAAAAAAAAAGd2FsbGV0AAAAAAATAAAAAAAAAAZub25jZXMAAAAAA+oAAAAGAAAAAQAAAAQ=",
        "AAAAAQAAACVQYXlsb2FkIG9mIHRoZSBgTm9uY2VDb25zdW1lZGAgZXZlbnQuAAAAAAAAAAAAAA1Ob25jZUNvbnN1bWVkAAAAAAAAAgAAAAAAAAAFbm9uY2UAAAAAAAAGAAAAAAAAAAZ3YWxsZXQAAAAAABM=",
        "AAAABAAAAAAAAAAAAAAAEk5vbmNlUmVnaXN0cnlFcnJvcgAAAAAAAQAAADRUaGUgbm9uY2UgaGFzIGFscmVhZHkgYmVlbiBjb25zdW1lZCBmb3IgdGhpcyB3YWxsZXQuAAAAEE5vbmNlQWxyZWFkeVVzZWQAAAAB" ]),
      options
    )
  }
  public readonly fromJSON = {
    consume_nonce: this.txFromJSON<Result<void>>,
        is_nonce_used: this.txFromJSON<boolean>,
        bulk_invalidate: this.txFromJSON<u32>
  }
}