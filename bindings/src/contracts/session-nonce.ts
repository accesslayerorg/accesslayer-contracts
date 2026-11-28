// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: session-nonce
// Interface SHA-256: e5d32cc1e810752e612eefb364617d203723aaf100315692293a6befa151dd88
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




export type DataKey = {tag: "Nonce", values: readonly [string]};


/**
 * Payload of the `ActionExecuted` event.
 * 
 * Event shape:
 * - topics: `(Symbol("ActionExecuted"), wallet)`
 * - data: `ActionExecuted`
 */
export interface ActionExecuted {
  /**
 * The nonce that was consumed by this action.
 */
nonce: u64;
  /**
 * Wallet whose nonce was consumed.
 */
wallet: string;
}

export const SessionNonceError = {
  /**
   * The supplied nonce is not the wallet's current nonce: it was already
   * consumed (replay) or is ahead of the counter (out of order).
   */
  1: {message:"NonceAlreadyUsed"},
  /**
   * The wallet's nonce counter cannot be incremented further.
   */
  2: {message:"NonceOverflow"}
}

export interface Client {
  /**
   * Construct and simulate a get_nonce transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the current (next unused) nonce for `wallet`. Starts at `0`.
   */
  get_nonce: ({wallet}: {wallet: string}, options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a consume_nonce transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Validates and consumes `nonce` for `wallet`.
   * 
   * `nonce` must equal [`get_nonce`](Self::get_nonce); on success the
   * stored nonce is incremented, `ActionExecuted` is emitted with the
   * consumed nonce, and the new current nonce is returned. Any other value
   * is rejected with [`SessionNonceError::NonceAlreadyUsed`] and leaves
   * state untouched. Requires the wallet's authorization so a third party
   * cannot advance (and thereby invalidate) another wallet's nonce.
   */
  consume_nonce: ({wallet, nonce}: {wallet: string, nonce: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>

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
      new ContractSpec([ "AAAAAAAAAERSZXR1cm5zIHRoZSBjdXJyZW50IChuZXh0IHVudXNlZCkgbm9uY2UgZm9yIGB3YWxsZXRgLiBTdGFydHMgYXQgYDBgLgAAAAlnZXRfbm9uY2UAAAAAAAABAAAAAAAAAAZ3YWxsZXQAAAAAABMAAAABAAAABg==",
        "AAAAAgAAAAAAAAAAAAAAB0RhdGFLZXkAAAAAAQAAAAEAAAAxTmV4dCB1bnVzZWQgbm9uY2UgZm9yIGEgd2FsbGV0LiBBYnNlbnQgbWVhbnMgYDBgLgAAAAAAAAVOb25jZQAAAAAAAAEAAAAT",
        "AAAAAAAAAcJWYWxpZGF0ZXMgYW5kIGNvbnN1bWVzIGBub25jZWAgZm9yIGB3YWxsZXRgLgoKYG5vbmNlYCBtdXN0IGVxdWFsIFtgZ2V0X25vbmNlYF0oU2VsZjo6Z2V0X25vbmNlKTsgb24gc3VjY2VzcyB0aGUKc3RvcmVkIG5vbmNlIGlzIGluY3JlbWVudGVkLCBgQWN0aW9uRXhlY3V0ZWRgIGlzIGVtaXR0ZWQgd2l0aCB0aGUKY29uc3VtZWQgbm9uY2UsIGFuZCB0aGUgbmV3IGN1cnJlbnQgbm9uY2UgaXMgcmV0dXJuZWQuIEFueSBvdGhlciB2YWx1ZQppcyByZWplY3RlZCB3aXRoIFtgU2Vzc2lvbk5vbmNlRXJyb3I6Ok5vbmNlQWxyZWFkeVVzZWRgXSBhbmQgbGVhdmVzCnN0YXRlIHVudG91Y2hlZC4gUmVxdWlyZXMgdGhlIHdhbGxldCdzIGF1dGhvcml6YXRpb24gc28gYSB0aGlyZCBwYXJ0eQpjYW5ub3QgYWR2YW5jZSAoYW5kIHRoZXJlYnkgaW52YWxpZGF0ZSkgYW5vdGhlciB3YWxsZXQncyBub25jZS4AAAAAAA1jb25zdW1lX25vbmNlAAAAAAAAAgAAAAAAAAAGd2FsbGV0AAAAAAATAAAAAAAAAAVub25jZQAAAAAAAAYAAAABAAAD6QAAAAYAAAfQAAAAEVNlc3Npb25Ob25jZUVycm9yAAAA",
        "AAAAAQAAAHxQYXlsb2FkIG9mIHRoZSBgQWN0aW9uRXhlY3V0ZWRgIGV2ZW50LgoKRXZlbnQgc2hhcGU6Ci0gdG9waWNzOiBgKFN5bWJvbCgiQWN0aW9uRXhlY3V0ZWQiKSwgd2FsbGV0KWAKLSBkYXRhOiBgQWN0aW9uRXhlY3V0ZWRgAAAAAAAAAA5BY3Rpb25FeGVjdXRlZAAAAAAAAgAAACtUaGUgbm9uY2UgdGhhdCB3YXMgY29uc3VtZWQgYnkgdGhpcyBhY3Rpb24uAAAAAAVub25jZQAAAAAAAAYAAAAgV2FsbGV0IHdob3NlIG5vbmNlIHdhcyBjb25zdW1lZC4AAAAGd2FsbGV0AAAAAAAT",
        "AAAABAAAAAAAAAAAAAAAEVNlc3Npb25Ob25jZUVycm9yAAAAAAAAAgAAAIFUaGUgc3VwcGxpZWQgbm9uY2UgaXMgbm90IHRoZSB3YWxsZXQncyBjdXJyZW50IG5vbmNlOiBpdCB3YXMgYWxyZWFkeQpjb25zdW1lZCAocmVwbGF5KSBvciBpcyBhaGVhZCBvZiB0aGUgY291bnRlciAob3V0IG9mIG9yZGVyKS4AAAAAAAAQTm9uY2VBbHJlYWR5VXNlZAAAAAEAAAA5VGhlIHdhbGxldCdzIG5vbmNlIGNvdW50ZXIgY2Fubm90IGJlIGluY3JlbWVudGVkIGZ1cnRoZXIuAAAAAAAADU5vbmNlT3ZlcmZsb3cAAAAAAAAC" ]),
      options
    )
  }
  public readonly fromJSON = {
    get_nonce: this.txFromJSON<u64>,
        consume_nonce: this.txFromJSON<Result<u64>>
  }
}