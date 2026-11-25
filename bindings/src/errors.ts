// AUTO-GENERATED FILE - DO NOT EDIT.
// Typed error-enum registry. Every `#[contracterror]` enum from every
// generated contract spec is re-exported under its contract name, so callers
// can map a raw `u32` error code to the variant name and message.
// Regenerate with: ./scripts/generate-bindings.sh

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
import { SessionNonceError as sessionNonce_SessionNonceError } from "./contracts/session-nonce.js";
import { ContractError as subscriptionContract_ContractError } from "./contracts/subscription-contract.js";
import { DiversificationError as treasuryDiversification_DiversificationError } from "./contracts/treasury-diversification.js";

export const contractErrorEnums = {
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
  "session-nonce": {
    "SessionNonceError": sessionNonce_SessionNonceError,
  },
  "subscription-contract": {
    "ContractError": subscriptionContract_ContractError,
  },
  "treasury-diversification": {
    "DiversificationError": treasuryDiversification_DiversificationError,
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
