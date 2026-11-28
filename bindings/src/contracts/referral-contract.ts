// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: referral-contract
// Interface SHA-256: 8678498646ccd7bbe18e414da18c61d798c6fd4971c75190b6e91ba62417a11c
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




export type DataKey = {tag: "FirstTradeStatus", values: readonly [string]} | {tag: "Referral", values: readonly [string]} | {tag: "RewardBalance", values: readonly [string]} | {tag: "Initialised", values: void} | {tag: "Admin", values: void} | {tag: "ReferrerVolume", values: readonly [string]} | {tag: "ReferrerTier", values: readonly [string]} | {tag: "ReferrerRewards", values: readonly [string]} | {tag: "TierConfig", values: readonly [ReferralTier]} | {tag: "PeriodPayout", values: readonly [string, u32]} | {tag: "PeriodCap", values: void} | {tag: "MaxReferralBps", values: void} | {tag: "PeriodLedgers", values: void} | {tag: "RewardEntry", values: readonly [string]} | {tag: "Treasury", values: void};


/**
 * Configuration for a referral tier.
 */
export interface TierConfig {
  reward_rate_bps: u32;
  threshold: i128;
}

/**
 * Referral tiers in ascending rank and reward rate.
 */
export enum ReferralTier {
  Bronze = 0,
  Silver = 1,
  Gold = 2,
}

export const ContractError = {
  1: {message:"AlreadyInitialised"},
  2: {message:"Unauthorized"},
  3: {message:"InvalidRate"},
  4: {message:"InvalidThreshold"},
  5: {message:"NotPositiveAmount"},
  6: {message:"ArithmeticOverflow"},
  7: {message:"NoRewardsPending"},
  8: {message:"SelfReferral"},
  9: {message:"ReferralAlreadyRegistered"},
  10: {message:"InvalidFeeAmount"},
  11: {message:"CyclicReferral"},
  12: {message:"InvalidReferralConfig"}
}


/**
 * Information about a referrer's current tier, cumulative volume, and next threshold.
 */
export interface ReferralTierInfo {
  next_threshold: Option<i128>;
  tier: ReferralTier;
  volume: i128;
}


export interface ReferralRewardEntry {
  amount: i128;
  claim_by_ledger: u32;
}


/**
 * Event data payload emitted when a referrer's tier is upgraded.
 */
export interface ReferralTierUpgraded {
  new_tier: ReferralTier;
  old_tier: ReferralTier;
  referrer: string;
  reward_rate_bps: u32;
  volume: i128;
}


/**
 * Event data payload emitted when referral rewards are accrued.
 */
export interface ReferralRewardAccrued {
  amount: i128;
  referrer: string;
  reward_rate_bps: u32;
  tier: ReferralTier;
}


/**
 * Event data payload emitted when referral rewards are claimed.
 */
export interface ReferralRewardsClaimed {
  amount: i128;
  referrer: string;
}


/**
 * Event data payload emitted when a referral reward is distributed to a chain level.
 */
export interface ReferralRewardDistributed {
  amount: i128;
  referrer: string;
  tier: u32;
}

export interface Client {
  /**
   * Construct and simulate a init transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Initializes the referral contract.
   */
  init: (options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the contract admin, if configured.
   */
  get_admin: (options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>

  /**
   * Construct and simulate a set_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Sets or updates the contract admin.
   */
  set_admin: ({new_admin}: {new_admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a on_swap_fee transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Accrue 40%, 20%, and 10% of the referral fee allocation to tiers 1-3.
   */
  on_swap_fee: ({wallet, fee_amount}: {wallet: string, fee_amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a record_trade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Records trade volume for a referrer, upgrading the tier if a threshold is crossed.
   * Emits ReferralTierUpgraded when an upgrade occurs.
   */
  record_trade: ({referrer, trade_volume}: {referrer: string, trade_volume: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<ReferralTier>>>

  /**
   * Construct and simulate a accrue_reward transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Accrues referral reward from a fee amount based on the referrer's current tier rate.
   */
  accrue_reward: ({referrer, fee_amount}: {referrer: string, fee_amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a adjust_volume transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Adjusts cumulative volume (e.g. For trade cancellation/refunds).
   * Guarantees ratchet-only behavior: Tiers NEVER downgrade regardless of volume drop.
   */
  adjust_volume: ({referrer, delta}: {referrer: string, delta: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<ReferralTierInfo>>>

  /**
   * Construct and simulate a sweep_expired transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  sweep_expired: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a claim_referral transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  claim_referral: ({referrer}: {referrer: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a referral_chain transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  referral_chain: ({user}: {user: string}, options?: MethodOptions) => Promise<AssembledTransaction<Array<string>>>

  /**
   * Construct and simulate a get_initialised transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns true if the contract has been initialised.
   */
  get_initialised: (options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a get_tier_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the configuration for a given tier.
   */
  get_tier_config: ({tier}: {tier: ReferralTier}, options?: MethodOptions) => Promise<AssembledTransaction<TierConfig>>

  /**
   * Construct and simulate a init_with_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Initializes the referral contract with a designated admin.
   */
  init_with_admin: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a pending_rewards transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  pending_rewards: ({referrer}: {referrer: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a set_tier_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Configures the volume threshold and reward rate bps for a tier.
   */
  set_tier_config: ({admin, tier, threshold, reward_rate_bps}: {admin: string, tier: ReferralTier, threshold: i128, reward_rate_bps: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a calculate_reward transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Calculates the potential reward for a given fee amount using the referrer's current tier rate.
   */
  calculate_reward: ({referrer, fee_amount}: {referrer: string, fee_amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a get_referral_tier transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns a referrer's current tier, cumulative volume, and next tier threshold.
   * Tiers are ratchet-only and never downgrade.
   */
  get_referral_tier: ({referrer}: {referrer: string}, options?: MethodOptions) => Promise<AssembledTransaction<ReferralTierInfo>>

  /**
   * Construct and simulate a register_referral transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Register a wallet's immutable direct referrer.
   */
  register_referral: ({wallet, referrer}: {wallet: string, referrer: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a register_referrer transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Alias for immutable referrer registration.
   */
  register_referrer: ({user, referrer}: {user: string, referrer: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_referral_chain transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the direct referrer followed by up to two successive referrers.
   */
  get_referral_chain: ({wallet}: {wallet: string}, options?: MethodOptions) => Promise<AssembledTransaction<Array<string>>>

  /**
   * Construct and simulate a get_reward_balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns a referrer's total accrued reward allocation.
   */
  get_reward_balance: ({referrer}: {referrer: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a get_tier_threshold transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the volume threshold for a given tier.
   */
  get_tier_threshold: ({tier}: {tier: ReferralTier}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a record_first_trade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns true if it was indeed the first trade (and records it as true now).
   */
  record_first_trade: ({wallet}: {wallet: string}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a distribute_referral transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Distribute a bounded multi-level referral pool for a trade.
   */
  distribute_referral: ({wallet, trade_fee}: {wallet: string, trade_fee: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_referrer_volume transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the cumulative referred trade volume for a referrer.
   */
  get_referrer_volume: ({referrer}: {referrer: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a record_trade_volume transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Convenience alias for record_trade.
   */
  record_trade_volume: ({referrer, trade_volume}: {referrer: string, trade_volume: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<ReferralTier>>>

  /**
   * Construct and simulate a get_referral_earnings transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns accumulated referral rewards for a referrer.
   */
  get_referral_earnings: ({referrer}: {referrer: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a accrue_referral_reward transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Records trade volume and accrues the referral fee in a single atomic call.
   */
  accrue_referral_reward: ({referrer, trade_volume, fee_amount}: {referrer: string, trade_volume: i128, fee_amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a claim_referral_rewards transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Referrer claims all accumulated rewards, resetting the pending balance to 0.
   */
  claim_referral_rewards: ({referrer}: {referrer: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a get_tier_reward_rate_bps transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the reward rate bps for a given tier.
   */
  get_tier_reward_rate_bps: ({tier}: {tier: ReferralTier}, options?: MethodOptions) => Promise<AssembledTransaction<u32>>

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
      new ContractSpec([ "AAAAAAAAACJJbml0aWFsaXplcyB0aGUgcmVmZXJyYWwgY29udHJhY3QuAAAAAAAEaW5pdAAAAAAAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAAACpSZXR1cm5zIHRoZSBjb250cmFjdCBhZG1pbiwgaWYgY29uZmlndXJlZC4AAAAAAAlnZXRfYWRtaW4AAAAAAAAAAAAAAQAAA+gAAAAT",
        "AAAAAAAAACNTZXRzIG9yIHVwZGF0ZXMgdGhlIGNvbnRyYWN0IGFkbWluLgAAAAAJc2V0X2FkbWluAAAAAAAAAQAAAAAAAAAJbmV3X2FkbWluAAAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAgAAAAAAAAAAAAAAB0RhdGFLZXkAAAAADwAAAAEAAAAAAAAAEEZpcnN0VHJhZGVTdGF0dXMAAAABAAAAEwAAAAEAAAAAAAAACFJlZmVycmFsAAAAAQAAABMAAAABAAAAAAAAAA1SZXdhcmRCYWxhbmNlAAAAAAAAAQAAABMAAAAAAAAAAAAAAAtJbml0aWFsaXNlZAAAAAAAAAAAAAAAAAVBZG1pbgAAAAAAAAEAAAAAAAAADlJlZmVycmVyVm9sdW1lAAAAAAABAAAAEwAAAAEAAAAAAAAADFJlZmVycmVyVGllcgAAAAEAAAATAAAAAQAAAAAAAAAPUmVmZXJyZXJSZXdhcmRzAAAAAAEAAAATAAAAAQAAAAAAAAAKVGllckNvbmZpZwAAAAAAAQAAB9AAAAAMUmVmZXJyYWxUaWVyAAAAAQAAAAAAAAAMUGVyaW9kUGF5b3V0AAAAAgAAABMAAAAEAAAAAAAAAAAAAAAJUGVyaW9kQ2FwAAAAAAAAAAAAAAAAAAAOTWF4UmVmZXJyYWxCcHMAAAAAAAAAAAAAAAAADVBlcmlvZExlZGdlcnMAAAAAAAABAAAAAAAAAAtSZXdhcmRFbnRyeQAAAAABAAAAEwAAAAAAAAAAAAAACFRyZWFzdXJ5",
        "AAAAAAAAAEVBY2NydWUgNDAlLCAyMCUsIGFuZCAxMCUgb2YgdGhlIHJlZmVycmFsIGZlZSBhbGxvY2F0aW9uIHRvIHRpZXJzIDEtMy4AAAAAAAALb25fc3dhcF9mZWUAAAAAAgAAAAAAAAAGd2FsbGV0AAAAAAATAAAAAAAAAApmZWVfYW1vdW50AAAAAAALAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAAAAAIVSZWNvcmRzIHRyYWRlIHZvbHVtZSBmb3IgYSByZWZlcnJlciwgdXBncmFkaW5nIHRoZSB0aWVyIGlmIGEgdGhyZXNob2xkIGlzIGNyb3NzZWQuCkVtaXRzIFJlZmVycmFsVGllclVwZ3JhZGVkIHdoZW4gYW4gdXBncmFkZSBvY2N1cnMuAAAAAAAADHJlY29yZF90cmFkZQAAAAIAAAAAAAAACHJlZmVycmVyAAAAEwAAAAAAAAAMdHJhZGVfdm9sdW1lAAAACwAAAAEAAAPpAAAH0AAAAAxSZWZlcnJhbFRpZXIAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAQAAACJDb25maWd1cmF0aW9uIGZvciBhIHJlZmVycmFsIHRpZXIuAAAAAAAAAAAAClRpZXJDb25maWcAAAAAAAIAAAAAAAAAD3Jld2FyZF9yYXRlX2JwcwAAAAAEAAAAAAAAAAl0aHJlc2hvbGQAAAAAAAAL",
        "AAAAAAAAAFRBY2NydWVzIHJlZmVycmFsIHJld2FyZCBmcm9tIGEgZmVlIGFtb3VudCBiYXNlZCBvbiB0aGUgcmVmZXJyZXIncyBjdXJyZW50IHRpZXIgcmF0ZS4AAAANYWNjcnVlX3Jld2FyZAAAAAAAAAIAAAAAAAAACHJlZmVycmVyAAAAEwAAAAAAAAAKZmVlX2Ftb3VudAAAAAAACwAAAAEAAAPpAAAACwAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAAAAAJNBZGp1c3RzIGN1bXVsYXRpdmUgdm9sdW1lIChlLmcuIEZvciB0cmFkZSBjYW5jZWxsYXRpb24vcmVmdW5kcykuCkd1YXJhbnRlZXMgcmF0Y2hldC1vbmx5IGJlaGF2aW9yOiBUaWVycyBORVZFUiBkb3duZ3JhZGUgcmVnYXJkbGVzcyBvZiB2b2x1bWUgZHJvcC4AAAAADWFkanVzdF92b2x1bWUAAAAAAAACAAAAAAAAAAhyZWZlcnJlcgAAABMAAAAAAAAABWRlbHRhAAAAAAAACwAAAAEAAAPpAAAH0AAAABBSZWZlcnJhbFRpZXJJbmZvAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAAAAAAAAANc3dlZXBfZXhwaXJlZAAAAAAAAAEAAAAAAAAABWFkbWluAAAAAAAAEwAAAAEAAAPpAAAACwAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAAAAAAAAAAAOY2xhaW1fcmVmZXJyYWwAAAAAAAEAAAAAAAAACHJlZmVycmVyAAAAEwAAAAEAAAPpAAAACwAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAAAAAAAAAAAOcmVmZXJyYWxfY2hhaW4AAAAAAAEAAAAAAAAABHVzZXIAAAATAAAAAQAAA+oAAAAT",
        "AAAAAwAAADFSZWZlcnJhbCB0aWVycyBpbiBhc2NlbmRpbmcgcmFuayBhbmQgcmV3YXJkIHJhdGUuAAAAAAAAAAAAAAxSZWZlcnJhbFRpZXIAAAADAAAAAAAAAAZCcm9uemUAAAAAAAAAAAAAAAAABlNpbHZlcgAAAAAAAQAAAAAAAAAER29sZAAAAAI=",
        "AAAAAAAAADJSZXR1cm5zIHRydWUgaWYgdGhlIGNvbnRyYWN0IGhhcyBiZWVuIGluaXRpYWxpc2VkLgAAAAAAD2dldF9pbml0aWFsaXNlZAAAAAAAAAAAAQAAAAE=",
        "AAAAAAAAACtSZXR1cm5zIHRoZSBjb25maWd1cmF0aW9uIGZvciBhIGdpdmVuIHRpZXIuAAAAAA9nZXRfdGllcl9jb25maWcAAAAAAQAAAAAAAAAEdGllcgAAB9AAAAAMUmVmZXJyYWxUaWVyAAAAAQAAB9AAAAAKVGllckNvbmZpZwAA",
        "AAAAAAAAADpJbml0aWFsaXplcyB0aGUgcmVmZXJyYWwgY29udHJhY3Qgd2l0aCBhIGRlc2lnbmF0ZWQgYWRtaW4uAAAAAAAPaW5pdF93aXRoX2FkbWluAAAAAAEAAAAAAAAABWFkbWluAAAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAAAAAAAAAAAPcGVuZGluZ19yZXdhcmRzAAAAAAEAAAAAAAAACHJlZmVycmVyAAAAEwAAAAEAAAAL",
        "AAAAAAAAAD9Db25maWd1cmVzIHRoZSB2b2x1bWUgdGhyZXNob2xkIGFuZCByZXdhcmQgcmF0ZSBicHMgZm9yIGEgdGllci4AAAAAD3NldF90aWVyX2NvbmZpZwAAAAAEAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAABHRpZXIAAAfQAAAADFJlZmVycmFsVGllcgAAAAAAAAAJdGhyZXNob2xkAAAAAAAACwAAAAAAAAAPcmV3YXJkX3JhdGVfYnBzAAAAAAQAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAABAAAAAAAAAAAAAAADUNvbnRyYWN0RXJyb3IAAAAAAAAMAAAAAAAAABJBbHJlYWR5SW5pdGlhbGlzZWQAAAAAAAEAAAAAAAAADFVuYXV0aG9yaXplZAAAAAIAAAAAAAAAC0ludmFsaWRSYXRlAAAAAAMAAAAAAAAAEEludmFsaWRUaHJlc2hvbGQAAAAEAAAAAAAAABFOb3RQb3NpdGl2ZUFtb3VudAAAAAAAAAUAAAAAAAAAEkFyaXRobWV0aWNPdmVyZmxvdwAAAAAABgAAAAAAAAAQTm9SZXdhcmRzUGVuZGluZwAAAAcAAAAAAAAADFNlbGZSZWZlcnJhbAAAAAgAAAAAAAAAGVJlZmVycmFsQWxyZWFkeVJlZ2lzdGVyZWQAAAAAAAAJAAAAAAAAABBJbnZhbGlkRmVlQW1vdW50AAAACgAAAAAAAAAOQ3ljbGljUmVmZXJyYWwAAAAAAAsAAAAAAAAAFUludmFsaWRSZWZlcnJhbENvbmZpZwAAAAAAAAw=",
        "AAAAAAAAAF5DYWxjdWxhdGVzIHRoZSBwb3RlbnRpYWwgcmV3YXJkIGZvciBhIGdpdmVuIGZlZSBhbW91bnQgdXNpbmcgdGhlIHJlZmVycmVyJ3MgY3VycmVudCB0aWVyIHJhdGUuAAAAAAAQY2FsY3VsYXRlX3Jld2FyZAAAAAIAAAAAAAAACHJlZmVycmVyAAAAEwAAAAAAAAAKZmVlX2Ftb3VudAAAAAAACwAAAAEAAAAL",
        "AAAAAAAAAHpSZXR1cm5zIGEgcmVmZXJyZXIncyBjdXJyZW50IHRpZXIsIGN1bXVsYXRpdmUgdm9sdW1lLCBhbmQgbmV4dCB0aWVyIHRocmVzaG9sZC4KVGllcnMgYXJlIHJhdGNoZXQtb25seSBhbmQgbmV2ZXIgZG93bmdyYWRlLgAAAAAAEWdldF9yZWZlcnJhbF90aWVyAAAAAAAAAQAAAAAAAAAIcmVmZXJyZXIAAAATAAAAAQAAB9AAAAAQUmVmZXJyYWxUaWVySW5mbw==",
        "AAAAAAAAAC5SZWdpc3RlciBhIHdhbGxldCdzIGltbXV0YWJsZSBkaXJlY3QgcmVmZXJyZXIuAAAAAAARcmVnaXN0ZXJfcmVmZXJyYWwAAAAAAAACAAAAAAAAAAZ3YWxsZXQAAAAAABMAAAAAAAAACHJlZmVycmVyAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAAAAACpBbGlhcyBmb3IgaW1tdXRhYmxlIHJlZmVycmVyIHJlZ2lzdHJhdGlvbi4AAAAAABFyZWdpc3Rlcl9yZWZlcnJlcgAAAAAAAAIAAAAAAAAABHVzZXIAAAATAAAAAAAAAAhyZWZlcnJlcgAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAAAEdSZXR1cm5zIHRoZSBkaXJlY3QgcmVmZXJyZXIgZm9sbG93ZWQgYnkgdXAgdG8gdHdvIHN1Y2Nlc3NpdmUgcmVmZXJyZXJzLgAAAAASZ2V0X3JlZmVycmFsX2NoYWluAAAAAAABAAAAAAAAAAZ3YWxsZXQAAAAAABMAAAABAAAD6gAAABM=",
        "AAAAAAAAADVSZXR1cm5zIGEgcmVmZXJyZXIncyB0b3RhbCBhY2NydWVkIHJld2FyZCBhbGxvY2F0aW9uLgAAAAAAABJnZXRfcmV3YXJkX2JhbGFuY2UAAAAAAAEAAAAAAAAACHJlZmVycmVyAAAAEwAAAAEAAAAL",
        "AAAAAAAAAC5SZXR1cm5zIHRoZSB2b2x1bWUgdGhyZXNob2xkIGZvciBhIGdpdmVuIHRpZXIuAAAAAAASZ2V0X3RpZXJfdGhyZXNob2xkAAAAAAABAAAAAAAAAAR0aWVyAAAH0AAAAAxSZWZlcnJhbFRpZXIAAAABAAAACw==",
        "AAAAAAAAAEtSZXR1cm5zIHRydWUgaWYgaXQgd2FzIGluZGVlZCB0aGUgZmlyc3QgdHJhZGUgKGFuZCByZWNvcmRzIGl0IGFzIHRydWUgbm93KS4AAAAAEnJlY29yZF9maXJzdF90cmFkZQAAAAAAAQAAAAAAAAAGd2FsbGV0AAAAAAATAAAAAQAAAAE=",
        "AAAAAQAAAFNJbmZvcm1hdGlvbiBhYm91dCBhIHJlZmVycmVyJ3MgY3VycmVudCB0aWVyLCBjdW11bGF0aXZlIHZvbHVtZSwgYW5kIG5leHQgdGhyZXNob2xkLgAAAAAAAAAAEFJlZmVycmFsVGllckluZm8AAAADAAAAAAAAAA5uZXh0X3RocmVzaG9sZAAAAAAD6AAAAAsAAAAAAAAABHRpZXIAAAfQAAAADFJlZmVycmFsVGllcgAAAAAAAAAGdm9sdW1lAAAAAAAL",
        "AAAAAAAAADtEaXN0cmlidXRlIGEgYm91bmRlZCBtdWx0aS1sZXZlbCByZWZlcnJhbCBwb29sIGZvciBhIHRyYWRlLgAAAAATZGlzdHJpYnV0ZV9yZWZlcnJhbAAAAAACAAAAAAAAAAZ3YWxsZXQAAAAAABMAAAAAAAAACXRyYWRlX2ZlZQAAAAAAAAsAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAAADxSZXR1cm5zIHRoZSBjdW11bGF0aXZlIHJlZmVycmVkIHRyYWRlIHZvbHVtZSBmb3IgYSByZWZlcnJlci4AAAATZ2V0X3JlZmVycmVyX3ZvbHVtZQAAAAABAAAAAAAAAAhyZWZlcnJlcgAAABMAAAABAAAACw==",
        "AAAAAAAAACNDb252ZW5pZW5jZSBhbGlhcyBmb3IgcmVjb3JkX3RyYWRlLgAAAAATcmVjb3JkX3RyYWRlX3ZvbHVtZQAAAAACAAAAAAAAAAhyZWZlcnJlcgAAABMAAAAAAAAADHRyYWRlX3ZvbHVtZQAAAAsAAAABAAAD6QAAB9AAAAAMUmVmZXJyYWxUaWVyAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAAADRSZXR1cm5zIGFjY3VtdWxhdGVkIHJlZmVycmFsIHJld2FyZHMgZm9yIGEgcmVmZXJyZXIuAAAAFWdldF9yZWZlcnJhbF9lYXJuaW5ncwAAAAAAAAEAAAAAAAAACHJlZmVycmVyAAAAEwAAAAEAAAAL",
        "AAAAAQAAAAAAAAAAAAAAE1JlZmVycmFsUmV3YXJkRW50cnkAAAAAAgAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAA9jbGFpbV9ieV9sZWRnZXIAAAAABA==",
        "AAAAAAAAAEpSZWNvcmRzIHRyYWRlIHZvbHVtZSBhbmQgYWNjcnVlcyB0aGUgcmVmZXJyYWwgZmVlIGluIGEgc2luZ2xlIGF0b21pYyBjYWxsLgAAAAAAFmFjY3J1ZV9yZWZlcnJhbF9yZXdhcmQAAAAAAAMAAAAAAAAACHJlZmVycmVyAAAAEwAAAAAAAAAMdHJhZGVfdm9sdW1lAAAACwAAAAAAAAAKZmVlX2Ftb3VudAAAAAAACwAAAAEAAAPpAAAACwAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAAAAAExSZWZlcnJlciBjbGFpbXMgYWxsIGFjY3VtdWxhdGVkIHJld2FyZHMsIHJlc2V0dGluZyB0aGUgcGVuZGluZyBiYWxhbmNlIHRvIDAuAAAAFmNsYWltX3JlZmVycmFsX3Jld2FyZHMAAAAAAAEAAAAAAAAACHJlZmVycmVyAAAAEwAAAAEAAAPpAAAACwAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAQAAAD5FdmVudCBkYXRhIHBheWxvYWQgZW1pdHRlZCB3aGVuIGEgcmVmZXJyZXIncyB0aWVyIGlzIHVwZ3JhZGVkLgAAAAAAAAAAABRSZWZlcnJhbFRpZXJVcGdyYWRlZAAAAAUAAAAAAAAACG5ld190aWVyAAAH0AAAAAxSZWZlcnJhbFRpZXIAAAAAAAAACG9sZF90aWVyAAAH0AAAAAxSZWZlcnJhbFRpZXIAAAAAAAAACHJlZmVycmVyAAAAEwAAAAAAAAAPcmV3YXJkX3JhdGVfYnBzAAAAAAQAAAAAAAAABnZvbHVtZQAAAAAACw==",
        "AAAAAQAAAD1FdmVudCBkYXRhIHBheWxvYWQgZW1pdHRlZCB3aGVuIHJlZmVycmFsIHJld2FyZHMgYXJlIGFjY3J1ZWQuAAAAAAAAAAAAABVSZWZlcnJhbFJld2FyZEFjY3J1ZWQAAAAAAAAEAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAACHJlZmVycmVyAAAAEwAAAAAAAAAPcmV3YXJkX3JhdGVfYnBzAAAAAAQAAAAAAAAABHRpZXIAAAfQAAAADFJlZmVycmFsVGllcg==",
        "AAAAAAAAAC1SZXR1cm5zIHRoZSByZXdhcmQgcmF0ZSBicHMgZm9yIGEgZ2l2ZW4gdGllci4AAAAAAAAYZ2V0X3RpZXJfcmV3YXJkX3JhdGVfYnBzAAAAAQAAAAAAAAAEdGllcgAAB9AAAAAMUmVmZXJyYWxUaWVyAAAAAQAAAAQ=",
        "AAAAAQAAAD1FdmVudCBkYXRhIHBheWxvYWQgZW1pdHRlZCB3aGVuIHJlZmVycmFsIHJld2FyZHMgYXJlIGNsYWltZWQuAAAAAAAAAAAAABZSZWZlcnJhbFJld2FyZHNDbGFpbWVkAAAAAAACAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAACHJlZmVycmVyAAAAEw==",
        "AAAAAQAAAFJFdmVudCBkYXRhIHBheWxvYWQgZW1pdHRlZCB3aGVuIGEgcmVmZXJyYWwgcmV3YXJkIGlzIGRpc3RyaWJ1dGVkIHRvIGEgY2hhaW4gbGV2ZWwuAAAAAAAAAAAAGVJlZmVycmFsUmV3YXJkRGlzdHJpYnV0ZWQAAAAAAAADAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAACHJlZmVycmVyAAAAEwAAAAAAAAAEdGllcgAAAAQ=" ]),
      options
    )
  }
  public readonly fromJSON = {
    init: this.txFromJSON<Result<void>>,
        get_admin: this.txFromJSON<Option<string>>,
        set_admin: this.txFromJSON<Result<void>>,
        on_swap_fee: this.txFromJSON<Result<void>>,
        record_trade: this.txFromJSON<Result<ReferralTier>>,
        accrue_reward: this.txFromJSON<Result<i128>>,
        adjust_volume: this.txFromJSON<Result<ReferralTierInfo>>,
        sweep_expired: this.txFromJSON<Result<i128>>,
        claim_referral: this.txFromJSON<Result<i128>>,
        referral_chain: this.txFromJSON<Array<string>>,
        get_initialised: this.txFromJSON<boolean>,
        get_tier_config: this.txFromJSON<TierConfig>,
        init_with_admin: this.txFromJSON<Result<void>>,
        pending_rewards: this.txFromJSON<i128>,
        set_tier_config: this.txFromJSON<Result<void>>,
        calculate_reward: this.txFromJSON<i128>,
        get_referral_tier: this.txFromJSON<ReferralTierInfo>,
        register_referral: this.txFromJSON<Result<void>>,
        register_referrer: this.txFromJSON<Result<void>>,
        get_referral_chain: this.txFromJSON<Array<string>>,
        get_reward_balance: this.txFromJSON<i128>,
        get_tier_threshold: this.txFromJSON<i128>,
        record_first_trade: this.txFromJSON<boolean>,
        distribute_referral: this.txFromJSON<Result<void>>,
        get_referrer_volume: this.txFromJSON<i128>,
        record_trade_volume: this.txFromJSON<Result<ReferralTier>>,
        get_referral_earnings: this.txFromJSON<i128>,
        accrue_referral_reward: this.txFromJSON<Result<i128>>,
        claim_referral_rewards: this.txFromJSON<Result<i128>>,
        get_tier_reward_rate_bps: this.txFromJSON<u32>
  }
}