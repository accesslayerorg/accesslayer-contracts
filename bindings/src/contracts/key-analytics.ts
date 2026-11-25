// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: key-analytics
// WASM SHA-256: 97c17f25160600076e7a06ad14c2f155dc72ea97eda9ac87bdc9c5d3dd533c2f
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




/**
 * Storage keys used by the key analytics contract.
 */
export type DataKey = {tag: "CreatorKeys", values: void} | {tag: "Snapshot", values: readonly [string, u64]};


/**
 * A complete historical analytics snapshot record stored in persistent storage.
 */
export interface Snapshot {
  /**
 * Calendar day index derived as `ledger_timestamp / 86400`.
 */
date: u64;
  /**
 * Distinct wallet holder count at call time.
 */
holder_count: u32;
  /**
 * Key identifier (creator address).
 */
key_id: string;
  /**
 * Circulating market capitalization (`price * circulating_supply`).
 */
market_cap: i128;
  /**
 * Spot buy price at call time (in XLM stroops).
 */
price: i128;
  /**
 * Cumulative gross trade volume up to this call (in XLM stroops).
 */
volume: i128;
}


/**
 * Event payload emitted when a daily analytics snapshot is recorded.
 * 
 * Topics emitted: `(Symbol("snp_take"), key_id: Address, date: u64)`.
 */
export interface SnapshotTaken {
  /**
 * Distinct wallet holder count at call time.
 */
holder_count: u32;
  /**
 * Circulating market capitalization (`price * circulating_supply`).
 */
market_cap: i128;
  /**
 * Spot buy price at call time (in XLM stroops).
 */
price: i128;
  /**
 * Cumulative gross trade volume up to this call (in XLM stroops).
 */
volume: i128;
}


/**
 * Live market stats view returned by `creator-keys::get_key_market_stats`.
 */
export interface KeyMarketStatsView {
  buy_price: i128;
  creator: string;
  holder_count: u32;
  sell_price: i128;
  supply: u32;
  volume: i128;
}

/**
 * Errors returned by the key analytics snapshot contract.
 */
export const AnalyticsSnapshotError = {
  /**
   * A snapshot for this key and date already exists.
   */
  1: {message:"SnapshotExists"},
  /**
   * No snapshot was found for this key and date.
   */
  2: {message:"SnapshotNotFound"},
  /**
   * `from_date` is strictly greater than `to_date`.
   */
  3: {message:"InvalidRange"},
  /**
   * Requested date range span exceeds [`MAX_SNAPSHOT_RANGE_DAYS`].
   */
  4: {message:"RangeTooLarge"},
  /**
   * Contract is already initialized.
   */
  5: {message:"AlreadyInitialized"},
  /**
   * Contract has not been initialized with the creator-keys address.
   */
  6: {message:"NotInitialized"},
  /**
   * Cross-contract metric fetch failed or metric source returned an error.
   */
  7: {message:"MetricFetchFailed"},
  /**
   * Arithmetic overflow occurred while computing market capitalization.
   */
  8: {message:"ArithmeticOverflow"}
}

export interface Client {
  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Initializes the key analytics contract with the creator-keys address.
   * 
   * Can only be called once at deployment. There are no subsequent setters
   * and no administrative roles.
   * 
   * # Errors
   * - [`AnalyticsSnapshotError::AlreadyInitialized`] if already initialized.
   */
  initialize: ({creator_keys}: {creator_keys: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_snapshot transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the analytics snapshot for `key_id` at a specific `date`.
   * 
   * Extends the persistent storage TTL of the accessed entry (~30 days).
   * 
   * # Errors
   * - [`AnalyticsSnapshotError::SnapshotNotFound`] if no snapshot exists for `(key_id, date)`.
   */
  get_snapshot: ({key_id, date}: {key_id: string, date: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Snapshot>>>

  /**
   * Construct and simulate a get_creator_keys transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the configured creator-keys contract address, if initialized.
   */
  get_creator_keys: (options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>

  /**
   * Construct and simulate a get_snapshot_range transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns all recorded snapshots for `key_id` across the `[from_date, to_date]` inclusive range.
   * 
   * Missing days within the date range are omitted. Returned snapshots are strictly
   * ordered by ascending date. Extends persistent storage TTL on read.
   * 
   * # Errors
   * - [`AnalyticsSnapshotError::InvalidRange`] if `from_date > to_date`.
   * - [`AnalyticsSnapshotError::RangeTooLarge`] if `(to_date - from_date + 1) > MAX_SNAPSHOT_RANGE_DAYS`.
   */
  get_snapshot_range: ({key_id, from_date, to_date}: {key_id: string, from_date: u64, to_date: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Array<Snapshot>>>>

  /**
   * Construct and simulate a take_daily_snapshot transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Records a daily analytics snapshot for `key_id` at the current calendar day.
   * 
   * Permissionless: any caller may invoke this once per calendar day per key.
   * The exact time of day is caller-chosen, recording the spot buy price and
   * state in effect at that moment. Volume is cumulative gross trade volume.
   * Market cap is derived on-chain as `buy_price * circulating_supply`.
   * 
   * Sourced via a cross-contract view call to the immutable `creator-keys` contract.
   * The date index is derived deterministically as `ledger_timestamp / 86400`.
   * 
   * Emits a `SnapshotTaken` event with topics `(Symbol("snp_take"), key_id, date)`.
   * 
   * # Errors
   * - [`AnalyticsSnapshotError::SnapshotExists`] if a snapshot for `(key_id, date)` was already recorded.
   * - [`AnalyticsSnapshotError::NotInitialized`] if the `creator-keys` address is not configured.
   * - [`AnalyticsSnapshotError::MetricFetchFailed`] if the cross-contract query fails.
   * - [`AnalyticsSnapshotError::ArithmeticOverflow`] if `buy_price * supply` overflows `i128`.
   */
  take_daily_snapshot: ({key_id}: {key_id: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Snapshot>>>

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
      new ContractSpec([ "AAAAAgAAADBTdG9yYWdlIGtleXMgdXNlZCBieSB0aGUga2V5IGFuYWx5dGljcyBjb250cmFjdC4AAAAAAAAAB0RhdGFLZXkAAAAAAgAAAAAAAABEQ29uZmlndXJlZCBjcmVhdG9yLWtleXMgY29udHJhY3QgYWRkcmVzcyBzdG9yZWQgaW4gaW5zdGFuY2Ugc3RvcmFnZS4AAAALQ3JlYXRvcktleXMAAAAAAQAAAEJIaXN0b3JpY2FsIHNuYXBzaG90IGVudHJ5IGluIHBlcnNpc3RlbnQgc3RvcmFnZTogYChrZXlfaWQsIGRhdGUpYC4AAAAAAAhTbmFwc2hvdAAAAAIAAAATAAAABg==",
        "AAAAAAAAAP1Jbml0aWFsaXplcyB0aGUga2V5IGFuYWx5dGljcyBjb250cmFjdCB3aXRoIHRoZSBjcmVhdG9yLWtleXMgYWRkcmVzcy4KCkNhbiBvbmx5IGJlIGNhbGxlZCBvbmNlIGF0IGRlcGxveW1lbnQuIFRoZXJlIGFyZSBubyBzdWJzZXF1ZW50IHNldHRlcnMKYW5kIG5vIGFkbWluaXN0cmF0aXZlIHJvbGVzLgoKIyBFcnJvcnMKLSBbYEFuYWx5dGljc1NuYXBzaG90RXJyb3I6OkFscmVhZHlJbml0aWFsaXplZGBdIGlmIGFscmVhZHkgaW5pdGlhbGl6ZWQuAAAAAAAACmluaXRpYWxpemUAAAAAAAEAAAAAAAAADGNyZWF0b3Jfa2V5cwAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAABZBbmFseXRpY3NTbmFwc2hvdEVycm9yAAA=",
        "AAAAAQAAAE1BIGNvbXBsZXRlIGhpc3RvcmljYWwgYW5hbHl0aWNzIHNuYXBzaG90IHJlY29yZCBzdG9yZWQgaW4gcGVyc2lzdGVudCBzdG9yYWdlLgAAAAAAAAAAAAAIU25hcHNob3QAAAAGAAAAOUNhbGVuZGFyIGRheSBpbmRleCBkZXJpdmVkIGFzIGBsZWRnZXJfdGltZXN0YW1wIC8gODY0MDBgLgAAAAAAAARkYXRlAAAABgAAACpEaXN0aW5jdCB3YWxsZXQgaG9sZGVyIGNvdW50IGF0IGNhbGwgdGltZS4AAAAAAAxob2xkZXJfY291bnQAAAAEAAAAIUtleSBpZGVudGlmaWVyIChjcmVhdG9yIGFkZHJlc3MpLgAAAAAAAAZrZXlfaWQAAAAAABMAAABBQ2lyY3VsYXRpbmcgbWFya2V0IGNhcGl0YWxpemF0aW9uIChgcHJpY2UgKiBjaXJjdWxhdGluZ19zdXBwbHlgKS4AAAAAAAAKbWFya2V0X2NhcAAAAAAACwAAAC1TcG90IGJ1eSBwcmljZSBhdCBjYWxsIHRpbWUgKGluIFhMTSBzdHJvb3BzKS4AAAAAAAAFcHJpY2UAAAAAAAALAAAAP0N1bXVsYXRpdmUgZ3Jvc3MgdHJhZGUgdm9sdW1lIHVwIHRvIHRoaXMgY2FsbCAoaW4gWExNIHN0cm9vcHMpLgAAAAAGdm9sdW1lAAAAAAAL",
        "AAAAAAAAAOxSZXR1cm5zIHRoZSBhbmFseXRpY3Mgc25hcHNob3QgZm9yIGBrZXlfaWRgIGF0IGEgc3BlY2lmaWMgYGRhdGVgLgoKRXh0ZW5kcyB0aGUgcGVyc2lzdGVudCBzdG9yYWdlIFRUTCBvZiB0aGUgYWNjZXNzZWQgZW50cnkgKH4zMCBkYXlzKS4KCiMgRXJyb3JzCi0gW2BBbmFseXRpY3NTbmFwc2hvdEVycm9yOjpTbmFwc2hvdE5vdEZvdW5kYF0gaWYgbm8gc25hcHNob3QgZXhpc3RzIGZvciBgKGtleV9pZCwgZGF0ZSlgLgAAAAxnZXRfc25hcHNob3QAAAACAAAAAAAAAAZrZXlfaWQAAAAAABMAAAAAAAAABGRhdGUAAAAGAAAAAQAAA+kAAAfQAAAACFNuYXBzaG90AAAH0AAAABZBbmFseXRpY3NTbmFwc2hvdEVycm9yAAA=",
        "AAAAAQAAAIdFdmVudCBwYXlsb2FkIGVtaXR0ZWQgd2hlbiBhIGRhaWx5IGFuYWx5dGljcyBzbmFwc2hvdCBpcyByZWNvcmRlZC4KClRvcGljcyBlbWl0dGVkOiBgKFN5bWJvbCgic25wX3Rha2UiKSwga2V5X2lkOiBBZGRyZXNzLCBkYXRlOiB1NjQpYC4AAAAAAAAAAA1TbmFwc2hvdFRha2VuAAAAAAAABAAAACpEaXN0aW5jdCB3YWxsZXQgaG9sZGVyIGNvdW50IGF0IGNhbGwgdGltZS4AAAAAAAxob2xkZXJfY291bnQAAAAEAAAAQUNpcmN1bGF0aW5nIG1hcmtldCBjYXBpdGFsaXphdGlvbiAoYHByaWNlICogY2lyY3VsYXRpbmdfc3VwcGx5YCkuAAAAAAAACm1hcmtldF9jYXAAAAAAAAsAAAAtU3BvdCBidXkgcHJpY2UgYXQgY2FsbCB0aW1lIChpbiBYTE0gc3Ryb29wcykuAAAAAAAABXByaWNlAAAAAAAACwAAAD9DdW11bGF0aXZlIGdyb3NzIHRyYWRlIHZvbHVtZSB1cCB0byB0aGlzIGNhbGwgKGluIFhMTSBzdHJvb3BzKS4AAAAABnZvbHVtZQAAAAAACw==",
        "AAAAAAAAAEVSZXR1cm5zIHRoZSBjb25maWd1cmVkIGNyZWF0b3Ita2V5cyBjb250cmFjdCBhZGRyZXNzLCBpZiBpbml0aWFsaXplZC4AAAAAAAAQZ2V0X2NyZWF0b3Jfa2V5cwAAAAAAAAABAAAD6AAAABM=",
        "AAAAAAAAAadSZXR1cm5zIGFsbCByZWNvcmRlZCBzbmFwc2hvdHMgZm9yIGBrZXlfaWRgIGFjcm9zcyB0aGUgYFtmcm9tX2RhdGUsIHRvX2RhdGVdYCBpbmNsdXNpdmUgcmFuZ2UuCgpNaXNzaW5nIGRheXMgd2l0aGluIHRoZSBkYXRlIHJhbmdlIGFyZSBvbWl0dGVkLiBSZXR1cm5lZCBzbmFwc2hvdHMgYXJlIHN0cmljdGx5Cm9yZGVyZWQgYnkgYXNjZW5kaW5nIGRhdGUuIEV4dGVuZHMgcGVyc2lzdGVudCBzdG9yYWdlIFRUTCBvbiByZWFkLgoKIyBFcnJvcnMKLSBbYEFuYWx5dGljc1NuYXBzaG90RXJyb3I6OkludmFsaWRSYW5nZWBdIGlmIGBmcm9tX2RhdGUgPiB0b19kYXRlYC4KLSBbYEFuYWx5dGljc1NuYXBzaG90RXJyb3I6OlJhbmdlVG9vTGFyZ2VgXSBpZiBgKHRvX2RhdGUgLSBmcm9tX2RhdGUgKyAxKSA+IE1BWF9TTkFQU0hPVF9SQU5HRV9EQVlTYC4AAAAAEmdldF9zbmFwc2hvdF9yYW5nZQAAAAAAAwAAAAAAAAAGa2V5X2lkAAAAAAATAAAAAAAAAAlmcm9tX2RhdGUAAAAAAAAGAAAAAAAAAAd0b19kYXRlAAAAAAYAAAABAAAD6QAAA+oAAAfQAAAACFNuYXBzaG90AAAH0AAAABZBbmFseXRpY3NTbmFwc2hvdEVycm9yAAA=",
        "AAAAAAAAA9dSZWNvcmRzIGEgZGFpbHkgYW5hbHl0aWNzIHNuYXBzaG90IGZvciBga2V5X2lkYCBhdCB0aGUgY3VycmVudCBjYWxlbmRhciBkYXkuCgpQZXJtaXNzaW9ubGVzczogYW55IGNhbGxlciBtYXkgaW52b2tlIHRoaXMgb25jZSBwZXIgY2FsZW5kYXIgZGF5IHBlciBrZXkuClRoZSBleGFjdCB0aW1lIG9mIGRheSBpcyBjYWxsZXItY2hvc2VuLCByZWNvcmRpbmcgdGhlIHNwb3QgYnV5IHByaWNlIGFuZApzdGF0ZSBpbiBlZmZlY3QgYXQgdGhhdCBtb21lbnQuIFZvbHVtZSBpcyBjdW11bGF0aXZlIGdyb3NzIHRyYWRlIHZvbHVtZS4KTWFya2V0IGNhcCBpcyBkZXJpdmVkIG9uLWNoYWluIGFzIGBidXlfcHJpY2UgKiBjaXJjdWxhdGluZ19zdXBwbHlgLgoKU291cmNlZCB2aWEgYSBjcm9zcy1jb250cmFjdCB2aWV3IGNhbGwgdG8gdGhlIGltbXV0YWJsZSBgY3JlYXRvci1rZXlzYCBjb250cmFjdC4KVGhlIGRhdGUgaW5kZXggaXMgZGVyaXZlZCBkZXRlcm1pbmlzdGljYWxseSBhcyBgbGVkZ2VyX3RpbWVzdGFtcCAvIDg2NDAwYC4KCkVtaXRzIGEgYFNuYXBzaG90VGFrZW5gIGV2ZW50IHdpdGggdG9waWNzIGAoU3ltYm9sKCJzbnBfdGFrZSIpLCBrZXlfaWQsIGRhdGUpYC4KCiMgRXJyb3JzCi0gW2BBbmFseXRpY3NTbmFwc2hvdEVycm9yOjpTbmFwc2hvdEV4aXN0c2BdIGlmIGEgc25hcHNob3QgZm9yIGAoa2V5X2lkLCBkYXRlKWAgd2FzIGFscmVhZHkgcmVjb3JkZWQuCi0gW2BBbmFseXRpY3NTbmFwc2hvdEVycm9yOjpOb3RJbml0aWFsaXplZGBdIGlmIHRoZSBgY3JlYXRvci1rZXlzYCBhZGRyZXNzIGlzIG5vdCBjb25maWd1cmVkLgotIFtgQW5hbHl0aWNzU25hcHNob3RFcnJvcjo6TWV0cmljRmV0Y2hGYWlsZWRgXSBpZiB0aGUgY3Jvc3MtY29udHJhY3QgcXVlcnkgZmFpbHMuCi0gW2BBbmFseXRpY3NTbmFwc2hvdEVycm9yOjpBcml0aG1ldGljT3ZlcmZsb3dgXSBpZiBgYnV5X3ByaWNlICogc3VwcGx5YCBvdmVyZmxvd3MgYGkxMjhgLgAAAAATdGFrZV9kYWlseV9zbmFwc2hvdAAAAAABAAAAAAAAAAZrZXlfaWQAAAAAABMAAAABAAAD6QAAB9AAAAAIU25hcHNob3QAAAfQAAAAFkFuYWx5dGljc1NuYXBzaG90RXJyb3IAAA==",
        "AAAAAQAAAEhMaXZlIG1hcmtldCBzdGF0cyB2aWV3IHJldHVybmVkIGJ5IGBjcmVhdG9yLWtleXM6OmdldF9rZXlfbWFya2V0X3N0YXRzYC4AAAAAAAAAEktleU1hcmtldFN0YXRzVmlldwAAAAAABgAAAAAAAAAJYnV5X3ByaWNlAAAAAAAACwAAAAAAAAAHY3JlYXRvcgAAAAATAAAAAAAAAAxob2xkZXJfY291bnQAAAAEAAAAAAAAAApzZWxsX3ByaWNlAAAAAAALAAAAAAAAAAZzdXBwbHkAAAAAAAQAAAAAAAAABnZvbHVtZQAAAAAACw==",
        "AAAABAAAADdFcnJvcnMgcmV0dXJuZWQgYnkgdGhlIGtleSBhbmFseXRpY3Mgc25hcHNob3QgY29udHJhY3QuAAAAAAAAAAAWQW5hbHl0aWNzU25hcHNob3RFcnJvcgAAAAAACAAAADBBIHNuYXBzaG90IGZvciB0aGlzIGtleSBhbmQgZGF0ZSBhbHJlYWR5IGV4aXN0cy4AAAAOU25hcHNob3RFeGlzdHMAAAAAAAEAAAAsTm8gc25hcHNob3Qgd2FzIGZvdW5kIGZvciB0aGlzIGtleSBhbmQgZGF0ZS4AAAAQU25hcHNob3ROb3RGb3VuZAAAAAIAAAAvYGZyb21fZGF0ZWAgaXMgc3RyaWN0bHkgZ3JlYXRlciB0aGFuIGB0b19kYXRlYC4AAAAADEludmFsaWRSYW5nZQAAAAMAAAA+UmVxdWVzdGVkIGRhdGUgcmFuZ2Ugc3BhbiBleGNlZWRzIFtgTUFYX1NOQVBTSE9UX1JBTkdFX0RBWVNgXS4AAAAAAA1SYW5nZVRvb0xhcmdlAAAAAAAABAAAACBDb250cmFjdCBpcyBhbHJlYWR5IGluaXRpYWxpemVkLgAAABJBbHJlYWR5SW5pdGlhbGl6ZWQAAAAAAAUAAABAQ29udHJhY3QgaGFzIG5vdCBiZWVuIGluaXRpYWxpemVkIHdpdGggdGhlIGNyZWF0b3Ita2V5cyBhZGRyZXNzLgAAAA5Ob3RJbml0aWFsaXplZAAAAAAABgAAAEZDcm9zcy1jb250cmFjdCBtZXRyaWMgZmV0Y2ggZmFpbGVkIG9yIG1ldHJpYyBzb3VyY2UgcmV0dXJuZWQgYW4gZXJyb3IuAAAAAAARTWV0cmljRmV0Y2hGYWlsZWQAAAAAAAAHAAAAQ0FyaXRobWV0aWMgb3ZlcmZsb3cgb2NjdXJyZWQgd2hpbGUgY29tcHV0aW5nIG1hcmtldCBjYXBpdGFsaXphdGlvbi4AAAAAEkFyaXRobWV0aWNPdmVyZmxvdwAAAAAACA==" ]),
      options
    )
  }
  public readonly fromJSON = {
    initialize: this.txFromJSON<Result<void>>,
        get_snapshot: this.txFromJSON<Result<Snapshot>>,
        get_creator_keys: this.txFromJSON<Option<string>>,
        get_snapshot_range: this.txFromJSON<Result<Array<Snapshot>>>,
        take_daily_snapshot: this.txFromJSON<Result<Snapshot>>
  }
}