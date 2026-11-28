// AUTO-GENERATED FILE - DO NOT EDIT.
// Typed error-enum registry. Every `#[contracterror]` enum from every
// generated contract spec is re-exported under its contract name, so callers
// can map a raw `u32` error code to the variant name and message.
// Regenerate with: ./scripts/generate-bindings.sh

import { Errors as batchTradeContract_Error } from "./contracts/batch-trade-contract.js";
import { Errors as creatorPenalty_Error } from "./contracts/creator-penalty.js";
import { Errors as feeDiscountContract_Error } from "./contracts/fee-discount-contract.js";
import { Errors as holderInsurancePool_Error } from "./contracts/holder-insurance-pool.js";
import { AnalyticsSnapshotError as keyAnalytics_AnalyticsSnapshotError } from "./contracts/key-analytics.js";
import { HealthScoreError as keyHealthScore_HealthScoreError } from "./contracts/key-health-score.js";
import { LiquidityDepthError as keyLiquidityDepth_LiquidityDepthError } from "./contracts/key-liquidity-depth.js";
import { KeyStakingError as keyStakingContract_KeyStakingError } from "./contracts/key-staking-contract.js";
import { AuctionError as launchAuction_AuctionError } from "./contracts/launch-auction.js";
import { LpRewardError as lpRewardContract_LpRewardError } from "./contracts/lp-reward-contract.js";
import { MultisigError as multisigAdmin_MultisigError } from "./contracts/multisig-admin.js";
import { NonceRegistryError as nonceRegistry_NonceRegistryError } from "./contracts/nonce-registry.js";
import { PersonhoodError as personhoodHook_PersonhoodError } from "./contracts/personhood-hook.js";
import { ContractError as referralContract_ContractError } from "./contracts/referral-contract.js";
import { Errors as safeModeContract_Error } from "./contracts/safe-mode-contract.js";
import { Errors as sealedLaunchAuction_Error } from "./contracts/sealed-launch-auction.js";
import { SessionNonceError as sessionNonce_SessionNonceError } from "./contracts/session-nonce.js";
import { Errors as socialMetadataContract_Error } from "./contracts/social-metadata-contract.js";
import { Errors as sponsoredTransactions_Error } from "./contracts/sponsored-transactions.js";
import { CleanupError as storageCleanup_CleanupError } from "./contracts/storage-cleanup.js";
import { ContractError as subscriptionContract_ContractError } from "./contracts/subscription-contract.js";
import { DiversificationError as treasuryDiversification_DiversificationError } from "./contracts/treasury-diversification.js";
import { Errors as voteEscrowContract_Error } from "./contracts/vote-escrow-contract.js";

export const contractErrorEnums = {
  "batch-trade-contract": {
    "Error": batchTradeContract_Error,
  },
  "creator-penalty": {
    "Error": creatorPenalty_Error,
  },
  "fee-discount-contract": {
    "Error": feeDiscountContract_Error,
  },
  "holder-insurance-pool": {
    "Error": holderInsurancePool_Error,
  },
  "key-analytics": {
    "AnalyticsSnapshotError": keyAnalytics_AnalyticsSnapshotError,
  },
  "key-health-score": {
    "HealthScoreError": keyHealthScore_HealthScoreError,
  },
  "key-liquidity-depth": {
    "LiquidityDepthError": keyLiquidityDepth_LiquidityDepthError,
  },
  "key-staking-contract": {
    "KeyStakingError": keyStakingContract_KeyStakingError,
  },
  "launch-auction": {
    "AuctionError": launchAuction_AuctionError,
  },
  "lp-reward-contract": {
    "LpRewardError": lpRewardContract_LpRewardError,
  },
  "multisig-admin": {
    "MultisigError": multisigAdmin_MultisigError,
  },
  "nonce-registry": {
    "NonceRegistryError": nonceRegistry_NonceRegistryError,
  },
  "personhood-hook": {
    "PersonhoodError": personhoodHook_PersonhoodError,
  },
  "referral-contract": {
    "ContractError": referralContract_ContractError,
  },
  "safe-mode-contract": {
    "Error": safeModeContract_Error,
  },
  "sealed-launch-auction": {
    "Error": sealedLaunchAuction_Error,
  },
  "session-nonce": {
    "SessionNonceError": sessionNonce_SessionNonceError,
  },
  "social-metadata-contract": {
    "Error": socialMetadataContract_Error,
  },
  "sponsored-transactions": {
    "Error": sponsoredTransactions_Error,
  },
  "storage-cleanup": {
    "CleanupError": storageCleanup_CleanupError,
  },
  "subscription-contract": {
    "ContractError": subscriptionContract_ContractError,
  },
  "treasury-diversification": {
    "DiversificationError": treasuryDiversification_DiversificationError,
  },
  "vote-escrow-contract": {
    "Error": voteEscrowContract_Error,
  },
} as const;

export type ContractName = keyof typeof contractErrorEnums;

/**
 * Resolves a raw contract error code (the `u32` inside a failed `Result`) to
 * the typed enum variant exported by that contract's client.
 */
export function describeContractError(
  contract: ContractName,
  enumName: string,
  code: number,
): { name: string; message: string } | undefined {
  const enums = contractErrorEnums[contract] as Record<string, Record<number, { message: string }>>;
  const errorEnum = enums[enumName];
  if (errorEnum === undefined) return undefined;
  const variant = errorEnum[code];
  if (variant === undefined) return undefined;
  return { name: enumName, message: variant.message };
}
