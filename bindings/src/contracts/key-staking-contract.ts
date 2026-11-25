// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: key-staking-contract
// WASM SHA-256: 30246d58d26d041b826d37fb1a9db6a494cfc0973a95fea2368d638cc41db5cd
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





export interface Staked {
  amount: i128;
  end_epoch: u32;
  key_id: string;
  stake_id: u64;
  staker: string;
  start_epoch: u32;
}

export type DataKey = {tag: "Treasury", values: void} | {tag: "Genesis", values: void} | {tag: "EpochDuration", values: void} | {tag: "NextStakeId", values: void} | {tag: "Stake", values: readonly [u64]} | {tag: "Unbonding", values: readonly [u64]} | {tag: "Epoch", values: readonly [u32]};


export interface Unstaked {
  principal: i128;
  rewards: i128;
  stake_id: u64;
  staker: string;
}


export interface EpochState {
  distributed: boolean;
  funded: i128;
  total_weight: i128;
}


export interface EpochFunded {
  amount: i128;
  epoch_id: u32;
  total_funded: i128;
}


export interface StakeRewards {
  /**
 * Rewards from epochs that have already completed.
 */
accrued: i128;
  /**
 * Rewards from epochs still in progress or upcoming, at current pools and weights.
 */
projected: i128;
}


export interface StakePosition {
  amount: i128;
  /**
 * First epoch in which the stake no longer earns; unstake opens here.
 */
end_epoch: u32;
  is_closed: boolean;
  key_id: string;
  stake_id: u64;
  staker: string;
  start_epoch: u32;
}

export const KeyStakingError = {
  1: {message:"NotPositiveAmount"},
  2: {message:"InvalidEpochs"},
  3: {message:"StakeNotFound"},
  /**
   * The stake's lock period (or the epoch being distributed) has not ended.
   */
  4: {message:"EpochNotComplete"},
  5: {message:"StakeAlreadyClosed"},
  6: {message:"AlreadyInitialized"},
  7: {message:"NotInitialized"},
  /**
   * The epoch being funded has already ended.
   */
  8: {message:"EpochEnded"},
  9: {message:"EpochAlreadyDistributed"},
  10: {message:"ArithmeticOverflow"},
  11: {message:"InvalidEpochDuration"},
  12: {message:"UnbondingNotReady"}
}


export interface UnbondingRequest {
  amount: i128;
  available_at: u64;
  stake_id: u64;
  staker: string;
}


export interface EpochRewardDistributed {
  epoch_id: u32;
  total_distributed: i128;
}

export interface Client {
  /**
   * Construct and simulate a stake transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Locks `amount` for `epochs` epochs starting with the current epoch.
   * Returns the new `stake_id`.
   */
  stake: ({key_id, staker, amount, epochs}: {key_id: string, staker: string, amount: i128, epochs: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>

  /**
   * Construct and simulate a unstake transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns principal plus all epoch rewards once the lock has completed.
   */
  unstake: ({stake_id}: {stake_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a get_epoch transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_epoch: ({epoch_id}: {epoch_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<EpochState>>

  /**
   * Construct and simulate a get_stake transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_stake: ({stake_id}: {stake_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<StakePosition>>>

  /**
   * Construct and simulate a fund_epoch transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Adds `amount` to the reward pool of `epoch_id` (current or future).
   * Restricted to the protocol fee treasury contract.
   */
  fund_epoch: ({epoch_id, amount}: {epoch_id: u32, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * One-time setup. `treasury` is the protocol fee treasury contract, the
   * only address allowed to fund epochs. Epoch 0 starts at the current
   * ledger timestamp.
   */
  initialize: ({treasury, epoch_duration}: {treasury: string, epoch_duration: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a claim_unstake transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  claim_unstake: ({stake_id}: {stake_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a current_epoch transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  current_epoch: (options?: MethodOptions) => Promise<AssembledTransaction<Result<u32>>>

  /**
   * Construct and simulate a request_unstake transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  request_unstake: ({stake_id}: {stake_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<UnbondingRequest>>>

  /**
   * Construct and simulate a distribute_epoch transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Finalises a completed epoch and emits `EpochRewardDistributed`.
   * Permissionless. An epoch with no stake weight distributes nothing;
   * otherwise the whole pool is allocated pro-rata (floor-division dust,
   * under one unit per stake, stays unclaimed).
   */
  distribute_epoch: ({epoch_id}: {epoch_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a get_stake_rewards transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Accrued (completed epochs) and projected (remaining epochs) rewards.
   * A closed stake reports zero for both.
   */
  get_stake_rewards: ({stake_id}: {stake_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<StakeRewards>>>

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
      new ContractSpec([ "AAAAAAAAAF9Mb2NrcyBgYW1vdW50YCBmb3IgYGVwb2Noc2AgZXBvY2hzIHN0YXJ0aW5nIHdpdGggdGhlIGN1cnJlbnQgZXBvY2guClJldHVybnMgdGhlIG5ldyBgc3Rha2VfaWRgLgAAAAAFc3Rha2UAAAAAAAAEAAAAAAAAAAZrZXlfaWQAAAAAABMAAAAAAAAABnN0YWtlcgAAAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAZlcG9jaHMAAAAAAAQAAAABAAAD6QAAAAYAAAfQAAAAD0tleVN0YWtpbmdFcnJvcgA=",
        "AAAAAAAAAEVSZXR1cm5zIHByaW5jaXBhbCBwbHVzIGFsbCBlcG9jaCByZXdhcmRzIG9uY2UgdGhlIGxvY2sgaGFzIGNvbXBsZXRlZC4AAAAAAAAHdW5zdGFrZQAAAAABAAAAAAAAAAhzdGFrZV9pZAAAAAYAAAABAAAD6QAAAAsAAAfQAAAAD0tleVN0YWtpbmdFcnJvcgA=",
        "AAAAAQAAAAAAAAAAAAAABlN0YWtlZAAAAAAABgAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAllbmRfZXBvY2gAAAAAAAAEAAAAAAAAAAZrZXlfaWQAAAAAABMAAAAAAAAACHN0YWtlX2lkAAAABgAAAAAAAAAGc3Rha2VyAAAAAAATAAAAAAAAAAtzdGFydF9lcG9jaAAAAAAE",
        "AAAAAAAAAAAAAAAJZ2V0X2Vwb2NoAAAAAAAAAQAAAAAAAAAIZXBvY2hfaWQAAAAEAAAAAQAAB9AAAAAKRXBvY2hTdGF0ZQAA",
        "AAAAAAAAAAAAAAAJZ2V0X3N0YWtlAAAAAAAAAQAAAAAAAAAIc3Rha2VfaWQAAAAGAAAAAQAAA+kAAAfQAAAADVN0YWtlUG9zaXRpb24AAAAAAAfQAAAAD0tleVN0YWtpbmdFcnJvcgA=",
        "AAAAAgAAAAAAAAAAAAAAB0RhdGFLZXkAAAAABwAAAAAAAAAAAAAACFRyZWFzdXJ5AAAAAAAAAAAAAAAHR2VuZXNpcwAAAAAAAAAAAAAAAA1FcG9jaER1cmF0aW9uAAAAAAAAAAAAAAAAAAALTmV4dFN0YWtlSWQAAAAAAQAAAAAAAAAFU3Rha2UAAAAAAAABAAAABgAAAAEAAAAAAAAACVVuYm9uZGluZwAAAAAAAAEAAAAGAAAAAQAAAAAAAAAFRXBvY2gAAAAAAAABAAAABA==",
        "AAAAAAAAAHVBZGRzIGBhbW91bnRgIHRvIHRoZSByZXdhcmQgcG9vbCBvZiBgZXBvY2hfaWRgIChjdXJyZW50IG9yIGZ1dHVyZSkuClJlc3RyaWN0ZWQgdG8gdGhlIHByb3RvY29sIGZlZSB0cmVhc3VyeSBjb250cmFjdC4AAAAAAAAKZnVuZF9lcG9jaAAAAAAAAgAAAAAAAAAIZXBvY2hfaWQAAAAEAAAAAAAAAAZhbW91bnQAAAAAAAsAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA9LZXlTdGFraW5nRXJyb3IA",
        "AAAAAAAAAJpPbmUtdGltZSBzZXR1cC4gYHRyZWFzdXJ5YCBpcyB0aGUgcHJvdG9jb2wgZmVlIHRyZWFzdXJ5IGNvbnRyYWN0LCB0aGUKb25seSBhZGRyZXNzIGFsbG93ZWQgdG8gZnVuZCBlcG9jaHMuIEVwb2NoIDAgc3RhcnRzIGF0IHRoZSBjdXJyZW50CmxlZGdlciB0aW1lc3RhbXAuAAAAAAAKaW5pdGlhbGl6ZQAAAAAAAgAAAAAAAAAIdHJlYXN1cnkAAAATAAAAAAAAAA5lcG9jaF9kdXJhdGlvbgAAAAAABgAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAD0tleVN0YWtpbmdFcnJvcgA=",
        "AAAAAQAAAAAAAAAAAAAACFVuc3Rha2VkAAAABAAAAAAAAAAJcHJpbmNpcGFsAAAAAAAACwAAAAAAAAAHcmV3YXJkcwAAAAALAAAAAAAAAAhzdGFrZV9pZAAAAAYAAAAAAAAABnN0YWtlcgAAAAAAEw==",
        "AAAAAQAAAAAAAAAAAAAACkVwb2NoU3RhdGUAAAAAAAMAAAAAAAAAC2Rpc3RyaWJ1dGVkAAAAAAEAAAAAAAAABmZ1bmRlZAAAAAAACwAAAAAAAAAMdG90YWxfd2VpZ2h0AAAACw==",
        "AAAAAAAAAAAAAAANY2xhaW1fdW5zdGFrZQAAAAAAAAEAAAAAAAAACHN0YWtlX2lkAAAABgAAAAEAAAPpAAAACwAAB9AAAAAPS2V5U3Rha2luZ0Vycm9yAA==",
        "AAAAAAAAAAAAAAANY3VycmVudF9lcG9jaAAAAAAAAAAAAAABAAAD6QAAAAQAAAfQAAAAD0tleVN0YWtpbmdFcnJvcgA=",
        "AAAAAQAAAAAAAAAAAAAAC0Vwb2NoRnVuZGVkAAAAAAMAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAIZXBvY2hfaWQAAAAEAAAAAAAAAAx0b3RhbF9mdW5kZWQAAAAL",
        "AAAAAQAAAAAAAAAAAAAADFN0YWtlUmV3YXJkcwAAAAIAAAAwUmV3YXJkcyBmcm9tIGVwb2NocyB0aGF0IGhhdmUgYWxyZWFkeSBjb21wbGV0ZWQuAAAAB2FjY3J1ZWQAAAAACwAAAFBSZXdhcmRzIGZyb20gZXBvY2hzIHN0aWxsIGluIHByb2dyZXNzIG9yIHVwY29taW5nLCBhdCBjdXJyZW50IHBvb2xzIGFuZCB3ZWlnaHRzLgAAAAlwcm9qZWN0ZWQAAAAAAAAL",
        "AAAAAAAAAAAAAAAPcmVxdWVzdF91bnN0YWtlAAAAAAEAAAAAAAAACHN0YWtlX2lkAAAABgAAAAEAAAPpAAAH0AAAABBVbmJvbmRpbmdSZXF1ZXN0AAAH0AAAAA9LZXlTdGFraW5nRXJyb3IA",
        "AAAAAQAAAAAAAAAAAAAADVN0YWtlUG9zaXRpb24AAAAAAAAHAAAAAAAAAAZhbW91bnQAAAAAAAsAAABDRmlyc3QgZXBvY2ggaW4gd2hpY2ggdGhlIHN0YWtlIG5vIGxvbmdlciBlYXJuczsgdW5zdGFrZSBvcGVucyBoZXJlLgAAAAAJZW5kX2Vwb2NoAAAAAAAABAAAAAAAAAAJaXNfY2xvc2VkAAAAAAAAAQAAAAAAAAAGa2V5X2lkAAAAAAATAAAAAAAAAAhzdGFrZV9pZAAAAAYAAAAAAAAABnN0YWtlcgAAAAAAEwAAAAAAAAALc3RhcnRfZXBvY2gAAAAABA==",
        "AAAAAAAAAPNGaW5hbGlzZXMgYSBjb21wbGV0ZWQgZXBvY2ggYW5kIGVtaXRzIGBFcG9jaFJld2FyZERpc3RyaWJ1dGVkYC4KUGVybWlzc2lvbmxlc3MuIEFuIGVwb2NoIHdpdGggbm8gc3Rha2Ugd2VpZ2h0IGRpc3RyaWJ1dGVzIG5vdGhpbmc7Cm90aGVyd2lzZSB0aGUgd2hvbGUgcG9vbCBpcyBhbGxvY2F0ZWQgcHJvLXJhdGEgKGZsb29yLWRpdmlzaW9uIGR1c3QsCnVuZGVyIG9uZSB1bml0IHBlciBzdGFrZSwgc3RheXMgdW5jbGFpbWVkKS4AAAAAEGRpc3RyaWJ1dGVfZXBvY2gAAAABAAAAAAAAAAhlcG9jaF9pZAAAAAQAAAABAAAD6QAAAAsAAAfQAAAAD0tleVN0YWtpbmdFcnJvcgA=",
        "AAAAAAAAAGpBY2NydWVkIChjb21wbGV0ZWQgZXBvY2hzKSBhbmQgcHJvamVjdGVkIChyZW1haW5pbmcgZXBvY2hzKSByZXdhcmRzLgpBIGNsb3NlZCBzdGFrZSByZXBvcnRzIHplcm8gZm9yIGJvdGguAAAAAAARZ2V0X3N0YWtlX3Jld2FyZHMAAAAAAAABAAAAAAAAAAhzdGFrZV9pZAAAAAYAAAABAAAD6QAAB9AAAAAMU3Rha2VSZXdhcmRzAAAH0AAAAA9LZXlTdGFraW5nRXJyb3IA",
        "AAAABAAAAAAAAAAAAAAAD0tleVN0YWtpbmdFcnJvcgAAAAAMAAAAAAAAABFOb3RQb3NpdGl2ZUFtb3VudAAAAAAAAAEAAAAAAAAADUludmFsaWRFcG9jaHMAAAAAAAACAAAAAAAAAA1TdGFrZU5vdEZvdW5kAAAAAAAAAwAAAEdUaGUgc3Rha2UncyBsb2NrIHBlcmlvZCAob3IgdGhlIGVwb2NoIGJlaW5nIGRpc3RyaWJ1dGVkKSBoYXMgbm90IGVuZGVkLgAAAAAQRXBvY2hOb3RDb21wbGV0ZQAAAAQAAAAAAAAAElN0YWtlQWxyZWFkeUNsb3NlZAAAAAAABQAAAAAAAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAAGAAAAAAAAAA5Ob3RJbml0aWFsaXplZAAAAAAABwAAAClUaGUgZXBvY2ggYmVpbmcgZnVuZGVkIGhhcyBhbHJlYWR5IGVuZGVkLgAAAAAAAApFcG9jaEVuZGVkAAAAAAAIAAAAAAAAABdFcG9jaEFscmVhZHlEaXN0cmlidXRlZAAAAAAJAAAAAAAAABJBcml0aG1ldGljT3ZlcmZsb3cAAAAAAAoAAAAAAAAAFEludmFsaWRFcG9jaER1cmF0aW9uAAAACwAAAAAAAAARVW5ib25kaW5nTm90UmVhZHkAAAAAAAAM",
        "AAAAAQAAAAAAAAAAAAAAEFVuYm9uZGluZ1JlcXVlc3QAAAAEAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAADGF2YWlsYWJsZV9hdAAAAAYAAAAAAAAACHN0YWtlX2lkAAAABgAAAAAAAAAGc3Rha2VyAAAAAAAT",
        "AAAAAQAAAAAAAAAAAAAAFkVwb2NoUmV3YXJkRGlzdHJpYnV0ZWQAAAAAAAIAAAAAAAAACGVwb2NoX2lkAAAABAAAAAAAAAARdG90YWxfZGlzdHJpYnV0ZWQAAAAAAAAL" ]),
      options
    )
  }
  public readonly fromJSON = {
    stake: this.txFromJSON<Result<u64>>,
        unstake: this.txFromJSON<Result<i128>>,
        get_epoch: this.txFromJSON<EpochState>,
        get_stake: this.txFromJSON<Result<StakePosition>>,
        fund_epoch: this.txFromJSON<Result<void>>,
        initialize: this.txFromJSON<Result<void>>,
        claim_unstake: this.txFromJSON<Result<i128>>,
        current_epoch: this.txFromJSON<Result<u32>>,
        request_unstake: this.txFromJSON<Result<UnbondingRequest>>,
        distribute_epoch: this.txFromJSON<Result<i128>>,
        get_stake_rewards: this.txFromJSON<Result<StakeRewards>>
  }
}