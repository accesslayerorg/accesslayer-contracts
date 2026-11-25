// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: subscription-contract
// WASM SHA-256: 49863f273c7734b4a2539b4ee840383286a9dfcc52c205d9f74745dcdb1302be
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




export type DataKey = {tag: "Subscription", values: readonly [string, u64]};


/**
 * Subscription data storing expiry timestamp.
 */
export interface Subscription {
  expiry: u64;
}

export const ContractError = {
  1: {message:"NotPositiveDuration"},
  2: {message:"SubscriptionNotFound"},
  3: {message:"ArithmeticOverflow"}
}


/**
 * Information about a subscription including active status and days remaining.
 */
export interface SubscriptionInfo {
  active: boolean;
  days_remaining: u64;
  expiry: u64;
}


/**
 * Event data payload emitted when a subscription is created.
 */
export interface SubscriptionCreated {
  expiry: u64;
  key_id: u64;
  wallet: string;
}


/**
 * Event data payload emitted when a subscription expires.
 */
export interface SubscriptionExpired {
  expiry: u64;
  key_id: u64;
  wallet: string;
}


/**
 * Event data payload emitted when a subscription is renewed.
 */
export interface SubscriptionRenewed {
  key_id: u64;
  new_expiry: u64;
  old_expiry: u64;
  wallet: string;
}

export interface Client {
  /**
   * Construct and simulate a subscribe transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Creates a new subscription for a wallet to access a creator key's gated content.
   * Locks subscription fee (in a real implementation, this would handle payment).
   * Sets expiry timestamp based on current ledger time + duration_days.
   */
  subscribe: ({wallet, key_id, duration_days}: {wallet: string, key_id: u64, duration_days: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>

  /**
   * Construct and simulate a get_subscription transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns subscription information including active status, expiry, and days remaining.
   */
  get_subscription: ({wallet, key_id}: {wallet: string, key_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<SubscriptionInfo>>

  /**
   * Construct and simulate a check_subscription transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Checks if a wallet has an active subscription to a creator key.
   * Returns false if subscription doesn't exist or has expired.
   * Intended to be called by gated content contracts for access control.
   */
  check_subscription: ({wallet, key_id}: {wallet: string, key_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a renew_subscription transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Renews an existing subscription, extending the expiry by the same duration
   * from the current expiry (not from current time).
   */
  renew_subscription: ({wallet, key_id, duration_days}: {wallet: string, key_id: u64, duration_days: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>

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
      new ContractSpec([ "AAAAAAAAAOJDcmVhdGVzIGEgbmV3IHN1YnNjcmlwdGlvbiBmb3IgYSB3YWxsZXQgdG8gYWNjZXNzIGEgY3JlYXRvciBrZXkncyBnYXRlZCBjb250ZW50LgpMb2NrcyBzdWJzY3JpcHRpb24gZmVlIChpbiBhIHJlYWwgaW1wbGVtZW50YXRpb24sIHRoaXMgd291bGQgaGFuZGxlIHBheW1lbnQpLgpTZXRzIGV4cGlyeSB0aW1lc3RhbXAgYmFzZWQgb24gY3VycmVudCBsZWRnZXIgdGltZSArIGR1cmF0aW9uX2RheXMuAAAAAAAJc3Vic2NyaWJlAAAAAAAAAwAAAAAAAAAGd2FsbGV0AAAAAAATAAAAAAAAAAZrZXlfaWQAAAAAAAYAAAAAAAAADWR1cmF0aW9uX2RheXMAAAAAAAAGAAAAAQAAA+kAAAAGAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAgAAAAAAAAAAAAAAB0RhdGFLZXkAAAAAAQAAAAEAAAAAAAAADFN1YnNjcmlwdGlvbgAAAAIAAAATAAAABg==",
        "AAAAAQAAACtTdWJzY3JpcHRpb24gZGF0YSBzdG9yaW5nIGV4cGlyeSB0aW1lc3RhbXAuAAAAAAAAAAAMU3Vic2NyaXB0aW9uAAAAAQAAAAAAAAAGZXhwaXJ5AAAAAAAG",
        "AAAABAAAAAAAAAAAAAAADUNvbnRyYWN0RXJyb3IAAAAAAAADAAAAAAAAABNOb3RQb3NpdGl2ZUR1cmF0aW9uAAAAAAEAAAAAAAAAFFN1YnNjcmlwdGlvbk5vdEZvdW5kAAAAAgAAAAAAAAASQXJpdGhtZXRpY092ZXJmbG93AAAAAAAD",
        "AAAAAAAAAFVSZXR1cm5zIHN1YnNjcmlwdGlvbiBpbmZvcm1hdGlvbiBpbmNsdWRpbmcgYWN0aXZlIHN0YXR1cywgZXhwaXJ5LCBhbmQgZGF5cyByZW1haW5pbmcuAAAAAAAAEGdldF9zdWJzY3JpcHRpb24AAAACAAAAAAAAAAZ3YWxsZXQAAAAAABMAAAAAAAAABmtleV9pZAAAAAAABgAAAAEAAAfQAAAAEFN1YnNjcmlwdGlvbkluZm8=",
        "AAAAAAAAAMBDaGVja3MgaWYgYSB3YWxsZXQgaGFzIGFuIGFjdGl2ZSBzdWJzY3JpcHRpb24gdG8gYSBjcmVhdG9yIGtleS4KUmV0dXJucyBmYWxzZSBpZiBzdWJzY3JpcHRpb24gZG9lc24ndCBleGlzdCBvciBoYXMgZXhwaXJlZC4KSW50ZW5kZWQgdG8gYmUgY2FsbGVkIGJ5IGdhdGVkIGNvbnRlbnQgY29udHJhY3RzIGZvciBhY2Nlc3MgY29udHJvbC4AAAASY2hlY2tfc3Vic2NyaXB0aW9uAAAAAAACAAAAAAAAAAZ3YWxsZXQAAAAAABMAAAAAAAAABmtleV9pZAAAAAAABgAAAAEAAAAB",
        "AAAAAAAAAHtSZW5ld3MgYW4gZXhpc3Rpbmcgc3Vic2NyaXB0aW9uLCBleHRlbmRpbmcgdGhlIGV4cGlyeSBieSB0aGUgc2FtZSBkdXJhdGlvbgpmcm9tIHRoZSBjdXJyZW50IGV4cGlyeSAobm90IGZyb20gY3VycmVudCB0aW1lKS4AAAAAEnJlbmV3X3N1YnNjcmlwdGlvbgAAAAAAAwAAAAAAAAAGd2FsbGV0AAAAAAATAAAAAAAAAAZrZXlfaWQAAAAAAAYAAAAAAAAADWR1cmF0aW9uX2RheXMAAAAAAAAGAAAAAQAAA+kAAAAGAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAQAAAExJbmZvcm1hdGlvbiBhYm91dCBhIHN1YnNjcmlwdGlvbiBpbmNsdWRpbmcgYWN0aXZlIHN0YXR1cyBhbmQgZGF5cyByZW1haW5pbmcuAAAAAAAAABBTdWJzY3JpcHRpb25JbmZvAAAAAwAAAAAAAAAGYWN0aXZlAAAAAAABAAAAAAAAAA5kYXlzX3JlbWFpbmluZwAAAAAABgAAAAAAAAAGZXhwaXJ5AAAAAAAG",
        "AAAAAQAAADpFdmVudCBkYXRhIHBheWxvYWQgZW1pdHRlZCB3aGVuIGEgc3Vic2NyaXB0aW9uIGlzIGNyZWF0ZWQuAAAAAAAAAAAAE1N1YnNjcmlwdGlvbkNyZWF0ZWQAAAAAAwAAAAAAAAAGZXhwaXJ5AAAAAAAGAAAAAAAAAAZrZXlfaWQAAAAAAAYAAAAAAAAABndhbGxldAAAAAAAEw==",
        "AAAAAQAAADdFdmVudCBkYXRhIHBheWxvYWQgZW1pdHRlZCB3aGVuIGEgc3Vic2NyaXB0aW9uIGV4cGlyZXMuAAAAAAAAAAATU3Vic2NyaXB0aW9uRXhwaXJlZAAAAAADAAAAAAAAAAZleHBpcnkAAAAAAAYAAAAAAAAABmtleV9pZAAAAAAABgAAAAAAAAAGd2FsbGV0AAAAAAAT",
        "AAAAAQAAADpFdmVudCBkYXRhIHBheWxvYWQgZW1pdHRlZCB3aGVuIGEgc3Vic2NyaXB0aW9uIGlzIHJlbmV3ZWQuAAAAAAAAAAAAE1N1YnNjcmlwdGlvblJlbmV3ZWQAAAAABAAAAAAAAAAGa2V5X2lkAAAAAAAGAAAAAAAAAApuZXdfZXhwaXJ5AAAAAAAGAAAAAAAAAApvbGRfZXhwaXJ5AAAAAAAGAAAAAAAAAAZ3YWxsZXQAAAAAABM=" ]),
      options
    )
  }
  public readonly fromJSON = {
    subscribe: this.txFromJSON<Result<u64>>,
        get_subscription: this.txFromJSON<SubscriptionInfo>,
        check_subscription: this.txFromJSON<boolean>,
        renew_subscription: this.txFromJSON<Result<u64>>
  }
}