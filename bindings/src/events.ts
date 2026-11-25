// AUTO-GENERATED FILE - DO NOT EDIT.
// Typed event decoders derived from the contracts' event definitions
// (the `Event shape` doc blocks on each payload type in the Rust sources)
// and the payload types in the contract spec.
// Regenerate with: ./scripts/generate-bindings.sh

import type { RawContractEvent } from "./decode.js";
import { decodeEvent } from "./decode.js";
import type * as BatchTradeContract from "./contracts/batch-trade-contract.js";
import type * as FeeDiscountContract from "./contracts/fee-discount-contract.js";
import type * as HolderInsurancePoolContract from "./contracts/holder-insurance-pool.js";
import type * as KeyAnalyticsContract from "./contracts/key-analytics.js";
import type * as KeyLiquidityDepthContract from "./contracts/key-liquidity-depth.js";
import type * as KeyStakingContract from "./contracts/key-staking-contract.js";
import type * as LaunchAuctionContract from "./contracts/launch-auction.js";
import type * as LpRewardContract from "./contracts/lp-reward-contract.js";
import type * as MultisigAdminContract from "./contracts/multisig-admin.js";
import type * as ReferralContract from "./contracts/referral-contract.js";
import type * as SafeModeContract from "./contracts/safe-mode-contract.js";
import type * as SealedLaunchAuctionContract from "./contracts/sealed-launch-auction.js";
import type * as SocialMetadataContract from "./contracts/social-metadata-contract.js";
import type * as SponsoredTransactionsContract from "./contracts/sponsored-transactions.js";
import type * as SubscriptionContract from "./contracts/subscription-contract.js";
import type * as VoteEscrowContract from "./contracts/vote-escrow-contract.js";

/** Payload types for every documented `batch-trade-contract` event. */
export interface BatchTradeContractEventPayloads {
  "batch": unknown;
}

export type BatchTradeContractEventName = keyof BatchTradeContractEventPayloads;

export interface BatchTradeContractDecodedEvent<N extends BatchTradeContractEventName = BatchTradeContractEventName> {
  name: N;
  contract: "batch-trade-contract";
  topics: readonly unknown[];
  data: BatchTradeContractEventPayloads[N];
}

const batchTradeContractEventNames: ReadonlySet<string> = new Set(["batch"]);

/** Decodes a `batch-trade-contract` contract event, or returns `null` for events it does not know. */
export function decodeBatchTradeContractEvent(
  event: RawContractEvent,
): BatchTradeContractDecodedEvent | null {
  const decoded = decodeEvent(event);
  if (decoded === null || !batchTradeContractEventNames.has(decoded.name)) return null;
  return decoded as unknown as BatchTradeContractDecodedEvent;
}

/** Payload types for every documented `fee-discount-contract` event. */
export interface FeeDiscountContractEventPayloads {
  "tier": FeeDiscountContract.Discount;
}

export type FeeDiscountContractEventName = keyof FeeDiscountContractEventPayloads;

export interface FeeDiscountContractDecodedEvent<N extends FeeDiscountContractEventName = FeeDiscountContractEventName> {
  name: N;
  contract: "fee-discount-contract";
  topics: readonly unknown[];
  data: FeeDiscountContractEventPayloads[N];
}

const feeDiscountContractEventNames: ReadonlySet<string> = new Set(["tier"]);

/** Decodes a `fee-discount-contract` contract event, or returns `null` for events it does not know. */
export function decodeFeeDiscountContractEvent(
  event: RawContractEvent,
): FeeDiscountContractDecodedEvent | null {
  const decoded = decodeEvent(event);
  if (decoded === null || !feeDiscountContractEventNames.has(decoded.name)) return null;
  return decoded as unknown as FeeDiscountContractDecodedEvent;
}

/** Payload types for every documented `holder-insurance-pool` event. */
export interface HolderInsurancePoolEventPayloads {
  "claim": unknown;
  "trigger": unknown;
}

export type HolderInsurancePoolEventName = keyof HolderInsurancePoolEventPayloads;

export interface HolderInsurancePoolDecodedEvent<N extends HolderInsurancePoolEventName = HolderInsurancePoolEventName> {
  name: N;
  contract: "holder-insurance-pool";
  topics: readonly unknown[];
  data: HolderInsurancePoolEventPayloads[N];
}

const holderInsurancePoolEventNames: ReadonlySet<string> = new Set(["claim", "trigger"]);

/** Decodes a `holder-insurance-pool` contract event, or returns `null` for events it does not know. */
export function decodeHolderInsurancePoolEvent(
  event: RawContractEvent,
): HolderInsurancePoolDecodedEvent | null {
  const decoded = decodeEvent(event);
  if (decoded === null || !holderInsurancePoolEventNames.has(decoded.name)) return null;
  return decoded as unknown as HolderInsurancePoolDecodedEvent;
}

/** Payload types for every documented `key-analytics` event. */
export interface KeyAnalyticsEventPayloads {
  "snp_take": KeyAnalyticsContract.SnapshotTaken;
}

export type KeyAnalyticsEventName = keyof KeyAnalyticsEventPayloads;

export interface KeyAnalyticsDecodedEvent<N extends KeyAnalyticsEventName = KeyAnalyticsEventName> {
  name: N;
  contract: "key-analytics";
  topics: readonly unknown[];
  data: KeyAnalyticsEventPayloads[N];
}

const keyAnalyticsEventNames: ReadonlySet<string> = new Set(["snp_take"]);

/** Decodes a `key-analytics` contract event, or returns `null` for events it does not know. */
export function decodeKeyAnalyticsEvent(
  event: RawContractEvent,
): KeyAnalyticsDecodedEvent | null {
  const decoded = decodeEvent(event);
  if (decoded === null || !keyAnalyticsEventNames.has(decoded.name)) return null;
  return decoded as unknown as KeyAnalyticsDecodedEvent;
}

/** Payload types for every documented `key-liquidity-depth` event. */
export interface KeyLiquidityDepthEventPayloads {
  "depth_upd": KeyLiquidityDepthContract.DepthUpdated;
}

export type KeyLiquidityDepthEventName = keyof KeyLiquidityDepthEventPayloads;

export interface KeyLiquidityDepthDecodedEvent<N extends KeyLiquidityDepthEventName = KeyLiquidityDepthEventName> {
  name: N;
  contract: "key-liquidity-depth";
  topics: readonly unknown[];
  data: KeyLiquidityDepthEventPayloads[N];
}

const keyLiquidityDepthEventNames: ReadonlySet<string> = new Set(["depth_upd"]);

/** Decodes a `key-liquidity-depth` contract event, or returns `null` for events it does not know. */
export function decodeKeyLiquidityDepthEvent(
  event: RawContractEvent,
): KeyLiquidityDepthDecodedEvent | null {
  const decoded = decodeEvent(event);
  if (decoded === null || !keyLiquidityDepthEventNames.has(decoded.name)) return null;
  return decoded as unknown as KeyLiquidityDepthDecodedEvent;
}

/** Payload types for every documented `key-staking-contract` event. */
export interface KeyStakingContractEventPayloads {
  "ep_dist": KeyStakingContract.EpochRewardDistributed;
  "ep_fund": KeyStakingContract.EpochFunded;
  "staked": KeyStakingContract.Staked;
  "unstaked": KeyStakingContract.Unstaked;
}

export type KeyStakingContractEventName = keyof KeyStakingContractEventPayloads;

export interface KeyStakingContractDecodedEvent<N extends KeyStakingContractEventName = KeyStakingContractEventName> {
  name: N;
  contract: "key-staking-contract";
  topics: readonly unknown[];
  data: KeyStakingContractEventPayloads[N];
}

const keyStakingContractEventNames: ReadonlySet<string> = new Set(["ep_dist", "ep_fund", "staked", "unstaked"]);

/** Decodes a `key-staking-contract` contract event, or returns `null` for events it does not know. */
export function decodeKeyStakingContractEvent(
  event: RawContractEvent,
): KeyStakingContractDecodedEvent | null {
  const decoded = decodeEvent(event);
  if (decoded === null || !keyStakingContractEventNames.has(decoded.name)) return null;
  return decoded as unknown as KeyStakingContractDecodedEvent;
}

/** Payload types for every documented `launch-auction` event. */
export interface LaunchAuctionEventPayloads {
  "auc_creat": LaunchAuctionContract.AuctionCreatedEvent;
  "auc_settl": LaunchAuctionContract.AuctionSettledEvent;
  "bid_place": LaunchAuctionContract.BidPlacedEvent;
}

export type LaunchAuctionEventName = keyof LaunchAuctionEventPayloads;

export interface LaunchAuctionDecodedEvent<N extends LaunchAuctionEventName = LaunchAuctionEventName> {
  name: N;
  contract: "launch-auction";
  topics: readonly unknown[];
  data: LaunchAuctionEventPayloads[N];
}

const launchAuctionEventNames: ReadonlySet<string> = new Set(["auc_creat", "auc_settl", "bid_place"]);

/** Decodes a `launch-auction` contract event, or returns `null` for events it does not know. */
export function decodeLaunchAuctionEvent(
  event: RawContractEvent,
): LaunchAuctionDecodedEvent | null {
  const decoded = decodeEvent(event);
  if (decoded === null || !launchAuctionEventNames.has(decoded.name)) return null;
  return decoded as unknown as LaunchAuctionDecodedEvent;
}

/** Payload types for every documented `lp-reward-contract` event. */
export interface LpRewardContractEventPayloads {
  "lp_claim": LpRewardContract.LPRewardClaimed;
  "lp_rem": LpRewardContract.LPRemoved;
  "LPAdded": LpRewardContract.LPAdded;
}

export type LpRewardContractEventName = keyof LpRewardContractEventPayloads;

export interface LpRewardContractDecodedEvent<N extends LpRewardContractEventName = LpRewardContractEventName> {
  name: N;
  contract: "lp-reward-contract";
  topics: readonly unknown[];
  data: LpRewardContractEventPayloads[N];
}

const lpRewardContractEventNames: ReadonlySet<string> = new Set(["lp_claim", "lp_rem", "LPAdded"]);

/** Decodes a `lp-reward-contract` contract event, or returns `null` for events it does not know. */
export function decodeLpRewardContractEvent(
  event: RawContractEvent,
): LpRewardContractDecodedEvent | null {
  const decoded = decodeEvent(event);
  if (decoded === null || !lpRewardContractEventNames.has(decoded.name)) return null;
  return decoded as unknown as LpRewardContractDecodedEvent;
}

/** Payload types for every documented `multisig-admin` event. */
export interface MultisigAdminEventPayloads {
  "initialized": readonly unknown[];
  "ThresholdUpdateExecuted": unknown;
  "ThresholdUpdateProposed": readonly unknown[];
}

export type MultisigAdminEventName = keyof MultisigAdminEventPayloads;

export interface MultisigAdminDecodedEvent<N extends MultisigAdminEventName = MultisigAdminEventName> {
  name: N;
  contract: "multisig-admin";
  topics: readonly unknown[];
  data: MultisigAdminEventPayloads[N];
}

const multisigAdminEventNames: ReadonlySet<string> = new Set(["initialized", "ThresholdUpdateExecuted", "ThresholdUpdateProposed"]);

/** Decodes a `multisig-admin` contract event, or returns `null` for events it does not know. */
export function decodeMultisigAdminEvent(
  event: RawContractEvent,
): MultisigAdminDecodedEvent | null {
  const decoded = decodeEvent(event);
  if (decoded === null || !multisigAdminEventNames.has(decoded.name)) return null;
  return decoded as unknown as MultisigAdminDecodedEvent;
}

/** Payload types for every documented `referral-contract` event. */
export interface ReferralContractEventPayloads {
  "init": readonly unknown[];
  "ref_clm": ReferralContract.ReferralRewardsClaimed;
  "ref_dist": ReferralContract.ReferralRewardDistributed;
  "ref_rwd": ReferralContract.ReferralRewardAccrued;
  "tier_upg": ReferralContract.ReferralTierUpgraded;
}

export type ReferralContractEventName = keyof ReferralContractEventPayloads;

export interface ReferralContractDecodedEvent<N extends ReferralContractEventName = ReferralContractEventName> {
  name: N;
  contract: "referral-contract";
  topics: readonly unknown[];
  data: ReferralContractEventPayloads[N];
}

const referralContractEventNames: ReadonlySet<string> = new Set(["init", "ref_clm", "ref_dist", "ref_rwd", "tier_upg"]);

/** Decodes a `referral-contract` contract event, or returns `null` for events it does not know. */
export function decodeReferralContractEvent(
  event: RawContractEvent,
): ReferralContractDecodedEvent | null {
  const decoded = decodeEvent(event);
  if (decoded === null || !referralContractEventNames.has(decoded.name)) return null;
  return decoded as unknown as ReferralContractDecodedEvent;
}

/** Payload types for every documented `safe-mode-contract` event. */
export interface SafeModeContractEventPayloads {
  "mode": readonly unknown[];
}

export type SafeModeContractEventName = keyof SafeModeContractEventPayloads;

export interface SafeModeContractDecodedEvent<N extends SafeModeContractEventName = SafeModeContractEventName> {
  name: N;
  contract: "safe-mode-contract";
  topics: readonly unknown[];
  data: SafeModeContractEventPayloads[N];
}

const safeModeContractEventNames: ReadonlySet<string> = new Set(["mode"]);

/** Decodes a `safe-mode-contract` contract event, or returns `null` for events it does not know. */
export function decodeSafeModeContractEvent(
  event: RawContractEvent,
): SafeModeContractDecodedEvent | null {
  const decoded = decodeEvent(event);
  if (decoded === null || !safeModeContractEventNames.has(decoded.name)) return null;
  return decoded as unknown as SafeModeContractDecodedEvent;
}

/** Payload types for every documented `sealed-launch-auction` event. */
export interface SealedLaunchAuctionEventPayloads {
  "settled": readonly unknown[];
}

export type SealedLaunchAuctionEventName = keyof SealedLaunchAuctionEventPayloads;

export interface SealedLaunchAuctionDecodedEvent<N extends SealedLaunchAuctionEventName = SealedLaunchAuctionEventName> {
  name: N;
  contract: "sealed-launch-auction";
  topics: readonly unknown[];
  data: SealedLaunchAuctionEventPayloads[N];
}

const sealedLaunchAuctionEventNames: ReadonlySet<string> = new Set(["settled"]);

/** Decodes a `sealed-launch-auction` contract event, or returns `null` for events it does not know. */
export function decodeSealedLaunchAuctionEvent(
  event: RawContractEvent,
): SealedLaunchAuctionDecodedEvent | null {
  const decoded = decodeEvent(event);
  if (decoded === null || !sealedLaunchAuctionEventNames.has(decoded.name)) return null;
  return decoded as unknown as SealedLaunchAuctionDecodedEvent;
}

/** Payload types for every documented `social-metadata-contract` event. */
export interface SocialMetadataContractEventPayloads {
  "metadata": unknown;
}

export type SocialMetadataContractEventName = keyof SocialMetadataContractEventPayloads;

export interface SocialMetadataContractDecodedEvent<N extends SocialMetadataContractEventName = SocialMetadataContractEventName> {
  name: N;
  contract: "social-metadata-contract";
  topics: readonly unknown[];
  data: SocialMetadataContractEventPayloads[N];
}

const socialMetadataContractEventNames: ReadonlySet<string> = new Set(["metadata"]);

/** Decodes a `social-metadata-contract` contract event, or returns `null` for events it does not know. */
export function decodeSocialMetadataContractEvent(
  event: RawContractEvent,
): SocialMetadataContractDecodedEvent | null {
  const decoded = decodeEvent(event);
  if (decoded === null || !socialMetadataContractEventNames.has(decoded.name)) return null;
  return decoded as unknown as SocialMetadataContractDecodedEvent;
}

/** Payload types for every documented `sponsored-transactions` event. */
export interface SponsoredTransactionsEventPayloads {
  "sponsor": readonly unknown[];
}

export type SponsoredTransactionsEventName = keyof SponsoredTransactionsEventPayloads;

export interface SponsoredTransactionsDecodedEvent<N extends SponsoredTransactionsEventName = SponsoredTransactionsEventName> {
  name: N;
  contract: "sponsored-transactions";
  topics: readonly unknown[];
  data: SponsoredTransactionsEventPayloads[N];
}

const sponsoredTransactionsEventNames: ReadonlySet<string> = new Set(["sponsor"]);

/** Decodes a `sponsored-transactions` contract event, or returns `null` for events it does not know. */
export function decodeSponsoredTransactionsEvent(
  event: RawContractEvent,
): SponsoredTransactionsDecodedEvent | null {
  const decoded = decodeEvent(event);
  if (decoded === null || !sponsoredTransactionsEventNames.has(decoded.name)) return null;
  return decoded as unknown as SponsoredTransactionsDecodedEvent;
}

/** Payload types for every documented `subscription-contract` event. */
export interface SubscriptionContractEventPayloads {
  "sub_crt": SubscriptionContract.SubscriptionCreated;
  "sub_ren": SubscriptionContract.SubscriptionRenewed;
}

export type SubscriptionContractEventName = keyof SubscriptionContractEventPayloads;

export interface SubscriptionContractDecodedEvent<N extends SubscriptionContractEventName = SubscriptionContractEventName> {
  name: N;
  contract: "subscription-contract";
  topics: readonly unknown[];
  data: SubscriptionContractEventPayloads[N];
}

const subscriptionContractEventNames: ReadonlySet<string> = new Set(["sub_crt", "sub_ren"]);

/** Decodes a `subscription-contract` contract event, or returns `null` for events it does not know. */
export function decodeSubscriptionContractEvent(
  event: RawContractEvent,
): SubscriptionContractDecodedEvent | null {
  const decoded = decodeEvent(event);
  if (decoded === null || !subscriptionContractEventNames.has(decoded.name)) return null;
  return decoded as unknown as SubscriptionContractDecodedEvent;
}

/** Payload types for every documented `vote-escrow-contract` event. */
export interface VoteEscrowContractEventPayloads {
  "extended": unknown;
  "locked": VoteEscrowContract.Lock;
  "withdraw": unknown;
}

export type VoteEscrowContractEventName = keyof VoteEscrowContractEventPayloads;

export interface VoteEscrowContractDecodedEvent<N extends VoteEscrowContractEventName = VoteEscrowContractEventName> {
  name: N;
  contract: "vote-escrow-contract";
  topics: readonly unknown[];
  data: VoteEscrowContractEventPayloads[N];
}

const voteEscrowContractEventNames: ReadonlySet<string> = new Set(["extended", "locked", "withdraw"]);

/** Decodes a `vote-escrow-contract` contract event, or returns `null` for events it does not know. */
export function decodeVoteEscrowContractEvent(
  event: RawContractEvent,
): VoteEscrowContractDecodedEvent | null {
  const decoded = decodeEvent(event);
  if (decoded === null || !voteEscrowContractEventNames.has(decoded.name)) return null;
  return decoded as unknown as VoteEscrowContractDecodedEvent;
}
