// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: lp-reward-contract
// WASM SHA-256: 802136518857659d3e01c4d12c9d4c99ffd56c807b27d7d63e0d45cf1be2d834
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





export interface LpPool {
  acc_reward_per_share: i128;
  key_id: string;
  total_liquidity: i128;
  total_rewards_claimed: i128;
  total_rewards_collected: i128;
}

export type DataKey = {tag: "NextLpId", values: void} | {tag: "Position", values: readonly [u64]} | {tag: "Pool", values: readonly [string]} | {tag: "TotalPositions", values: void};


export interface LPAdded {
  amount: i128;
  key_id: string;
  lp_id: u64;
  provider: string;
  share: i128;
}


export interface LPRemoved {
  key_id: string;
  lp_id: u64;
  principal: i128;
  provider: string;
  rewards: i128;
  total_returned: i128;
}


export interface LpPosition {
  contribution: i128;
  key_id: string;
  lp_id: u64;
  pending_rewards: i128;
  provider: string;
  share: i128;
}

export const LpRewardError = {
  1: {message:"NotPositiveAmount"},
  2: {message:"PositionNotFound"},
  3: {message:"Unauthorized"},
  4: {message:"NoRewardsPending"},
  5: {message:"ZeroLiquidityPool"},
  6: {message:"PositionAlreadyClosed"},
  7: {message:"ArithmeticOverflow"}
}


export interface LPRewardClaimed {
  amount: i128;
  key_id: string;
  lp_id: u64;
  provider: string;
}


export interface StoredLpPosition {
  accumulated_rewards: i128;
  contribution: i128;
  is_closed: boolean;
  key_id: string;
  lp_id: u64;
  provider: string;
  reward_debt: i128;
}

export interface Client {
  /**
   * Construct and simulate a get_lp_count transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Total LP positions ever created.
   */
  get_lp_count: (options?: MethodOptions) => Promise<AssembledTransaction<u64>>

  /**
   * Construct and simulate a add_liquidity transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Locks tokens as liquidity and records LP share.
   * Returns the unique `lp_id` identifying the position.
   */
  add_liquidity: ({key_id, provider, amount}: {key_id: string, provider: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>

  /**
   * Construct and simulate a get_lp_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns contribution, share, and pending rewards at any point.
   */
  get_lp_position: ({lp_id}: {lp_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<LpPosition>>>

  /**
   * Construct and simulate a claim_lp_rewards transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Claims rewards without removing liquidity.
   */
  claim_lp_rewards: ({lp_id}: {lp_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a get_pool_rewards transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Read-only view of total collected trading rewards for a key pair.
   */
  get_pool_rewards: ({key_id}: {key_id: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a remove_liquidity transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns tokens plus accrued fee rewards, closing the position.
   */
  remove_liquidity: ({lp_id}: {lp_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a add_liquidity_for transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Convenience alias with (provider, key_id, amount) parameter order.
   */
  add_liquidity_for: ({provider, key_id, amount}: {provider: string, key_id: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>

  /**
   * Construct and simulate a accrue_trading_fee transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Accrues fee rewards to key pair pool proportional to trading volume.
   */
  accrue_trading_fee: ({key_id, fee_amount}: {key_id: string, fee_amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_total_liquidity transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Read-only view of total pool liquidity for a key pair.
   */
  get_total_liquidity: ({key_id}: {key_id: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

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
      new ContractSpec([ "AAAAAQAAAAAAAAAAAAAABkxwUG9vbAAAAAAABQAAAAAAAAAUYWNjX3Jld2FyZF9wZXJfc2hhcmUAAAALAAAAAAAAAAZrZXlfaWQAAAAAABMAAAAAAAAAD3RvdGFsX2xpcXVpZGl0eQAAAAALAAAAAAAAABV0b3RhbF9yZXdhcmRzX2NsYWltZWQAAAAAAAALAAAAAAAAABd0b3RhbF9yZXdhcmRzX2NvbGxlY3RlZAAAAAAL",
        "AAAAAgAAAAAAAAAAAAAAB0RhdGFLZXkAAAAABAAAAAAAAAAAAAAACE5leHRMcElkAAAAAQAAAAAAAAAIUG9zaXRpb24AAAABAAAABgAAAAEAAAAAAAAABFBvb2wAAAABAAAAEwAAAAAAAAAAAAAADlRvdGFsUG9zaXRpb25zAAA=",
        "AAAAAQAAAAAAAAAAAAAAB0xQQWRkZWQAAAAABQAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAZrZXlfaWQAAAAAABMAAAAAAAAABWxwX2lkAAAAAAAABgAAAAAAAAAIcHJvdmlkZXIAAAATAAAAAAAAAAVzaGFyZQAAAAAAAAs=",
        "AAAAAQAAAAAAAAAAAAAACUxQUmVtb3ZlZAAAAAAAAAYAAAAAAAAABmtleV9pZAAAAAAAEwAAAAAAAAAFbHBfaWQAAAAAAAAGAAAAAAAAAAlwcmluY2lwYWwAAAAAAAALAAAAAAAAAAhwcm92aWRlcgAAABMAAAAAAAAAB3Jld2FyZHMAAAAACwAAAAAAAAAOdG90YWxfcmV0dXJuZWQAAAAAAAs=",
        "AAAAAAAAACBUb3RhbCBMUCBwb3NpdGlvbnMgZXZlciBjcmVhdGVkLgAAAAxnZXRfbHBfY291bnQAAAAAAAAAAQAAAAY=",
        "AAAAAQAAAAAAAAAAAAAACkxwUG9zaXRpb24AAAAAAAYAAAAAAAAADGNvbnRyaWJ1dGlvbgAAAAsAAAAAAAAABmtleV9pZAAAAAAAEwAAAAAAAAAFbHBfaWQAAAAAAAAGAAAAAAAAAA9wZW5kaW5nX3Jld2FyZHMAAAAACwAAAAAAAAAIcHJvdmlkZXIAAAATAAAAAAAAAAVzaGFyZQAAAAAAAAs=",
        "AAAAAAAAAGRMb2NrcyB0b2tlbnMgYXMgbGlxdWlkaXR5IGFuZCByZWNvcmRzIExQIHNoYXJlLgpSZXR1cm5zIHRoZSB1bmlxdWUgYGxwX2lkYCBpZGVudGlmeWluZyB0aGUgcG9zaXRpb24uAAAADWFkZF9saXF1aWRpdHkAAAAAAAADAAAAAAAAAAZrZXlfaWQAAAAAABMAAAAAAAAACHByb3ZpZGVyAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAQAAA+kAAAAGAAAH0AAAAA1McFJld2FyZEVycm9yAAAA",
        "AAAAAAAAAD5SZXR1cm5zIGNvbnRyaWJ1dGlvbiwgc2hhcmUsIGFuZCBwZW5kaW5nIHJld2FyZHMgYXQgYW55IHBvaW50LgAAAAAAD2dldF9scF9wb3NpdGlvbgAAAAABAAAAAAAAAAVscF9pZAAAAAAAAAYAAAABAAAD6QAAB9AAAAAKTHBQb3NpdGlvbgAAAAAH0AAAAA1McFJld2FyZEVycm9yAAAA",
        "AAAABAAAAAAAAAAAAAAADUxwUmV3YXJkRXJyb3IAAAAAAAAHAAAAAAAAABFOb3RQb3NpdGl2ZUFtb3VudAAAAAAAAAEAAAAAAAAAEFBvc2l0aW9uTm90Rm91bmQAAAACAAAAAAAAAAxVbmF1dGhvcml6ZWQAAAADAAAAAAAAABBOb1Jld2FyZHNQZW5kaW5nAAAABAAAAAAAAAARWmVyb0xpcXVpZGl0eVBvb2wAAAAAAAAFAAAAAAAAABVQb3NpdGlvbkFscmVhZHlDbG9zZWQAAAAAAAAGAAAAAAAAABJBcml0aG1ldGljT3ZlcmZsb3cAAAAAAAc=",
        "AAAAAAAAACpDbGFpbXMgcmV3YXJkcyB3aXRob3V0IHJlbW92aW5nIGxpcXVpZGl0eS4AAAAAABBjbGFpbV9scF9yZXdhcmRzAAAAAQAAAAAAAAAFbHBfaWQAAAAAAAAGAAAAAQAAA+kAAAALAAAH0AAAAA1McFJld2FyZEVycm9yAAAA",
        "AAAAAAAAAEFSZWFkLW9ubHkgdmlldyBvZiB0b3RhbCBjb2xsZWN0ZWQgdHJhZGluZyByZXdhcmRzIGZvciBhIGtleSBwYWlyLgAAAAAAABBnZXRfcG9vbF9yZXdhcmRzAAAAAQAAAAAAAAAGa2V5X2lkAAAAAAATAAAAAQAAAAs=",
        "AAAAAAAAAD5SZXR1cm5zIHRva2VucyBwbHVzIGFjY3J1ZWQgZmVlIHJld2FyZHMsIGNsb3NpbmcgdGhlIHBvc2l0aW9uLgAAAAAAEHJlbW92ZV9saXF1aWRpdHkAAAABAAAAAAAAAAVscF9pZAAAAAAAAAYAAAABAAAD6QAAAAsAAAfQAAAADUxwUmV3YXJkRXJyb3IAAAA=",
        "AAAAAAAAAEJDb252ZW5pZW5jZSBhbGlhcyB3aXRoIChwcm92aWRlciwga2V5X2lkLCBhbW91bnQpIHBhcmFtZXRlciBvcmRlci4AAAAAABFhZGRfbGlxdWlkaXR5X2ZvcgAAAAAAAAMAAAAAAAAACHByb3ZpZGVyAAAAEwAAAAAAAAAGa2V5X2lkAAAAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAABAAAD6QAAAAYAAAfQAAAADUxwUmV3YXJkRXJyb3IAAAA=",
        "AAAAAQAAAAAAAAAAAAAAD0xQUmV3YXJkQ2xhaW1lZAAAAAAEAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAABmtleV9pZAAAAAAAEwAAAAAAAAAFbHBfaWQAAAAAAAAGAAAAAAAAAAhwcm92aWRlcgAAABM=",
        "AAAAAAAAAERBY2NydWVzIGZlZSByZXdhcmRzIHRvIGtleSBwYWlyIHBvb2wgcHJvcG9ydGlvbmFsIHRvIHRyYWRpbmcgdm9sdW1lLgAAABJhY2NydWVfdHJhZGluZ19mZWUAAAAAAAIAAAAAAAAABmtleV9pZAAAAAAAEwAAAAAAAAAKZmVlX2Ftb3VudAAAAAAACwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADUxwUmV3YXJkRXJyb3IAAAA=",
        "AAAAAQAAAAAAAAAAAAAAEFN0b3JlZExwUG9zaXRpb24AAAAHAAAAAAAAABNhY2N1bXVsYXRlZF9yZXdhcmRzAAAAAAsAAAAAAAAADGNvbnRyaWJ1dGlvbgAAAAsAAAAAAAAACWlzX2Nsb3NlZAAAAAAAAAEAAAAAAAAABmtleV9pZAAAAAAAEwAAAAAAAAAFbHBfaWQAAAAAAAAGAAAAAAAAAAhwcm92aWRlcgAAABMAAAAAAAAAC3Jld2FyZF9kZWJ0AAAAAAs=",
        "AAAAAAAAADZSZWFkLW9ubHkgdmlldyBvZiB0b3RhbCBwb29sIGxpcXVpZGl0eSBmb3IgYSBrZXkgcGFpci4AAAAAABNnZXRfdG90YWxfbGlxdWlkaXR5AAAAAAEAAAAAAAAABmtleV9pZAAAAAAAEwAAAAEAAAAL" ]),
      options
    )
  }
  public readonly fromJSON = {
    get_lp_count: this.txFromJSON<u64>,
        add_liquidity: this.txFromJSON<Result<u64>>,
        get_lp_position: this.txFromJSON<Result<LpPosition>>,
        claim_lp_rewards: this.txFromJSON<Result<i128>>,
        get_pool_rewards: this.txFromJSON<i128>,
        remove_liquidity: this.txFromJSON<Result<i128>>,
        add_liquidity_for: this.txFromJSON<Result<u64>>,
        accrue_trading_fee: this.txFromJSON<Result<void>>,
        get_total_liquidity: this.txFromJSON<i128>
  }
}