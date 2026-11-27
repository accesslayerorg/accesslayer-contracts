//! Storage key registry.
//!
//! Every contract in this workspace that has been migrated to the registry
//! defines its storage keys here, in one module per contract. Each key enum
//! is declared through [`storage_keys!`], which also records the storage
//! class (instance, persistent or temporary) of every variant, so a variant
//! cannot be added without saying where it lives.
//!
//! Moving an enum into this module does not change how its keys are encoded.
//! A `#[contracttype]` enum key is encoded as a vector holding the variant
//! name as a symbol followed by the variant's fields. Neither the enum's name
//! nor the crate that defines it is part of the encoding, so the variant names
//! and field types below must stay exactly as they are.
//!
//! The tests in `common/tests/storage_key_registry.rs` check that:
//! - every variant of every registered enum has a sample, and all encoded keys
//!   of a contract are unique;
//! - no encoding changes compared to `common/tests/storage_keys.golden`;
//! - no contract source writes storage with a raw tuple or symbol key, or
//!   declares a storage key enum outside this module.
//!
//! See `docs/storage-key-registry.md` for the rules for adding keys.

/// Where a storage key's entry lives.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageClass {
    /// `env.storage().instance()`: shares the contract instance's TTL.
    Instance,
    /// `env.storage().persistent()`: its own TTL, archived when it expires.
    Persistent,
    /// `env.storage().temporary()`: its own TTL, deleted when it expires.
    Temporary,
}

impl StorageClass {
    /// Lowercase name used in docs and in the golden file.
    pub const fn as_str(self) -> &'static str {
        match self {
            StorageClass::Instance => "instance",
            StorageClass::Persistent => "persistent",
            StorageClass::Temporary => "temporary",
        }
    }
}

/// Implemented by every key enum declared with [`storage_keys!`].
pub trait RegisteredKey {
    /// Workspace crate whose storage this key space belongs to.
    const CONTRACT: &'static str;
    /// Every variant name with its storage class, in declaration order.
    const VARIANTS: &'static [(&'static str, StorageClass)];

    /// Name of this key's variant.
    fn variant_name(&self) -> &'static str;

    /// Storage class this key's entry is written to.
    fn storage_class(&self) -> StorageClass;
}

/// Declares a registered storage key enum.
///
/// Each variant is followed by `=> Instance`, `=> Persistent` or
/// `=> Temporary`. The macro emits a `#[contracttype(export = false)]` enum
/// with the given attributes and docs, and implements [`RegisteredKey`] for it.
#[macro_export]
macro_rules! storage_keys {
    (
        $(#[$meta:meta])*
        pub enum $name:ident for $contract:literal {
            $(
                $(#[doc = $doc:literal])*
                $variant:ident $( ( $($field:tt)* ) )? => $class:ident
            ),+ $(,)?
        }
    ) => {
        #[soroban_sdk::contracttype(export = false)]
        $(#[$meta])*
        pub enum $name {
            $(
                $(#[doc = $doc])*
                $variant $( ( $($field)* ) )?
            ),+
        }

        impl $crate::keys::RegisteredKey for $name {
            const CONTRACT: &'static str = $contract;
            const VARIANTS: &'static [(&'static str, $crate::keys::StorageClass)] = &[
                $( (stringify!($variant), $crate::keys::StorageClass::$class) ),+
            ];

            fn variant_name(&self) -> &'static str {
                match self {
                    $( $name::$variant { .. } => stringify!($variant) ),+
                }
            }

            fn storage_class(&self) -> $crate::keys::StorageClass {
                match self {
                    $( $name::$variant { .. } => $crate::keys::StorageClass::$class ),+
                }
            }
        }
    };
}

/// Storage keys of the `common` crate.
pub mod guard {
    crate::storage_keys! {
        /// Temporary-storage keys used by the shared `ReentrancyGuard`.
        ///
        /// Moved from `common/src/guard.rs`; encodings are unchanged.
        #[derive(Clone)]
        pub enum DataKey for "common" {
            /// Set while a guarded invocation is running.
            Locked => Temporary,
        }
    }
}

/// Storage keys of the `auto-invest-registry` contract.
pub mod auto_invest_registry {
    use soroban_sdk::Address;

    crate::storage_keys! {
        /// Persistent storage keys used by the auto-invest registry contract.
        ///
        /// Moved from `auto-invest-registry/src/lib.rs`; encodings are unchanged.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum DataKey for "auto-invest-registry" {
            /// Global auto-incrementing rule ID counter.
            NextRuleId => Persistent,
            /// Rule record by ID: `Rule(rule_id)` -> `Rule`.
            Rule(u64) => Persistent,
            /// List of rule IDs registered for a wallet: `WalletRules(wallet)` -> `Vec<u64>`.
            WalletRules(Address) => Persistent,
        }
    }
}

/// Storage keys of the `batch-trade-contract` contract.
pub mod batch_trade_contract {
    use soroban_sdk::Address;

    crate::storage_keys! {
        /// Moved from `batch-trade-contract/src/lib.rs`; encodings are unchanged.
        #[derive(Clone)]
        pub enum Key for "batch-trade-contract" {
            Market(Address) => Persistent,
        }
    }
}

/// Storage keys of the `fee-discount-contract` contract.
pub mod fee_discount_contract {
    use soroban_sdk::Address;

    crate::storage_keys! {
        /// Moved from `fee-discount-contract/src/lib.rs`; encodings are unchanged.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum Key for "fee-discount-contract" {
            Admin => Instance,
            Tiers => Instance,
            Volume(Address) => Persistent,
        }
    }
}

/// Storage keys of the `holder-insurance-pool` contract.
pub mod holder_insurance_pool {
    use soroban_sdk::Address;

    crate::storage_keys! {
        /// Moved from `holder-insurance-pool/src/lib.rs`; encodings are unchanged.
        #[derive(Clone)]
        pub enum Key for "holder-insurance-pool" {
            Admin => Instance,
            Pool => Persistent,
            NextId => Persistent,
            Cover(u64) => Persistent,
            KeyTotal(Address) => Persistent,
            Trigger(Address) => Persistent,
        }
    }
}

/// Storage keys of the `key-analytics` contract.
pub mod key_analytics {
    use soroban_sdk::Address;

    crate::storage_keys! {
        /// Storage keys used by the key analytics contract.
        ///
        /// Moved from `key-analytics/src/lib.rs`; encodings are unchanged.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum DataKey for "key-analytics" {
            /// Configured creator-keys contract address stored in instance storage.
            CreatorKeys => Instance,
            /// Historical snapshot entry in persistent storage: `(key_id, date)`.
            Snapshot(Address, u64) => Persistent,
        }
    }
}

/// Storage keys of the `key-staking-contract` contract.
pub mod key_staking_contract {
    crate::storage_keys! {
        /// Moved from `key-staking-contract/src/lib.rs`; encodings are unchanged.
        #[derive(Clone)]
        pub enum DataKey for "key-staking-contract" {
            Treasury => Persistent,
            Genesis => Persistent,
            EpochDuration => Persistent,
            NextStakeId => Persistent,
            Stake(u64) => Persistent,
            Unbonding(u64) => Persistent,
            Epoch(u32) => Persistent,
        }
    }
}

/// Storage keys of the `launch-auction` contract.
pub mod launch_auction {
    use soroban_sdk::Address;

    crate::storage_keys! {
        /// Moved from `launch-auction/src/lib.rs`; encodings are unchanged.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum DataKey for "launch-auction" {
            /// Total count of auctions created.
            AuctionCount => Persistent,
            /// Auction metadata record by auction ID.
            Auction(u32) => Persistent,
            /// List of bids submitted for an auction ID.
            AuctionBids(u32) => Persistent,
            /// Escrow balance locked per bidder for an auction ID.
            Escrow(u32, Address) => Persistent,
            /// Allocated key count won by a bidder.
            Allocation(u32, Address) => Persistent,
            /// Market clearing price established upon settlement.
            ClearingPrice(u32) => Persistent,
            /// Flag indicating whether the bonding curve has been opened.
            BondingCurveOpen(u32) => Persistent,
        }
    }
}

/// Storage keys of the `lp-reward-contract` contract.
pub mod lp_reward_contract {
    use soroban_sdk::Address;

    crate::storage_keys! {
        /// Moved from `lp-reward-contract/src/lib.rs`; encodings are unchanged.
        #[derive(Clone)]
        pub enum DataKey for "lp-reward-contract" {
            NextLpId => Persistent,
            Position(u64) => Persistent,
            Pool(Address) => Persistent,
            TotalPositions => Persistent,
        }
    }
}

/// Storage keys of the `multisig-admin` contract.
pub mod multisig_admin {
    use soroban_sdk::Address;

    crate::storage_keys! {
        /// Moved from `multisig-admin/src/lib.rs`; encodings are unchanged.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum DataKey for "multisig-admin" {
            Signers => Instance,
            Threshold => Instance,
            NextProposalId => Instance,
            Proposal(u32) => Persistent,
            Approval(u32, Address) => Persistent,
            Rejection(u32, Address) => Persistent,
            Pending => Instance,
        }
    }
}

/// Storage keys of the `nonce-registry` contract.
pub mod nonce_registry {
    use soroban_sdk::Address;

    crate::storage_keys! {
        /// Moved from `nonce-registry/src/lib.rs`; encodings are unchanged.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum DataKey for "nonce-registry" {
            /// Marks `(wallet, nonce)` as consumed. Nonces carry no ordering, so each
            /// pair is tracked independently.
            UsedNonce(Address, u64) => Persistent,
        }
    }
}

/// Storage keys of the `personhood-hook` contract.
pub mod personhood_hook {
    use soroban_sdk::{Address, BytesN};

    crate::storage_keys! {
        /// Moved from `personhood-hook/src/lib.rs`; encodings are unchanged.
        #[derive(Clone)]
        pub enum DataKey for "personhood-hook" {
            Admin => Instance,
            PersonhoodProvider => Instance,
            WalletToPerson(Address) => Persistent,
            PersonToWallet(BytesN<32>) => Persistent,
            PersonWalletCount(BytesN<32>) => Persistent,
            MaxWalletsPerPerson => Instance,
            UnlinkCooldown => Instance,
            LastUnlink(Address) => Persistent,
            KeyRequiresPersonhood(Address) => Persistent,
            PersonPurchases(BytesN<32>, Address) => Persistent,
            UnverifiedCap(Address) => Persistent,
        }
    }
}

/// Storage keys of the `safe-mode-contract` contract.
pub mod safe_mode_contract {
    crate::storage_keys! {
        /// Moved from `safe-mode-contract/src/lib.rs`; encodings are unchanged.
        #[derive(Clone)]
        pub enum Key for "safe-mode-contract" {
            Admin => Instance,
            Mode => Instance,
            PendingNormal(u32) => Instance,
        }
    }
}

/// Storage keys of the `sealed-launch-auction` contract.
pub mod sealed_launch_auction {
    crate::storage_keys! {
        /// Moved from `sealed-launch-auction/src/lib.rs`; encodings are unchanged.
        #[derive(Clone)]
        pub enum Key for "sealed-launch-auction" {
            Admin => Persistent,
            Next => Persistent,
            Auction(u32) => Persistent,
            Bids(u32) => Persistent,
        }
    }
}

/// Storage keys of the `seasonal-rewards` contract.
pub mod seasonal_rewards {
    use soroban_sdk::Address;

    crate::storage_keys! {
        /// Moved from `seasonal-rewards/src/lib.rs`; encodings are unchanged.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum DataKey for "seasonal-rewards" {
            Admin => Instance,
            NextCampaignId => Instance,
            Campaign(u64) => Persistent,
            Qualification(u64, Address) => Persistent,
            Claimed(u64, Address) => Persistent,
        }
    }
}

/// Storage keys of the `session-nonce` contract.
pub mod session_nonce {
    use soroban_sdk::Address;

    crate::storage_keys! {
        /// Moved from `session-nonce/src/lib.rs`; encodings are unchanged.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum DataKey for "session-nonce" {
            /// Next unused nonce for a wallet. Absent means `0`.
            Nonce(Address) => Persistent,
        }
    }
}

/// Storage keys of the `social-metadata-contract` contract.
pub mod social_metadata_contract {
    use soroban_sdk::Address;

    crate::storage_keys! {
        /// Moved from `social-metadata-contract/src/lib.rs`; encodings are unchanged.
        #[derive(Clone)]
        pub enum Key for "social-metadata-contract" {
            Admin => Instance,
            Creator(Address) => Persistent,
            Metadata(Address) => Persistent,
        }
    }
}

/// Storage keys of the `sponsored-transactions` contract.
pub mod sponsored_transactions {
    use soroban_sdk::Address;

    crate::storage_keys! {
        /// Moved from `sponsored-transactions/src/lib.rs`; encodings are unchanged.
        #[derive(Clone)]
        pub enum Key for "sponsored-transactions" {
            Admin => Instance,
            Nonce(Address, u64) => Persistent,
        }
    }
}

/// Storage keys of the `subscription-contract` contract.
pub mod subscription_contract {
    use soroban_sdk::Address;

    crate::storage_keys! {
        /// Moved from `subscription-contract/src/lib.rs`; encodings are unchanged.
        #[derive(Clone)]
        pub enum DataKey for "subscription-contract" {
            Subscription(Address, u64) => Persistent,
        }
    }
}

/// Storage keys of the `treasury-diversification` contract.
pub mod treasury_diversification {
    crate::storage_keys! {
        /// Moved from `treasury-diversification/src/lib.rs`; encodings are unchanged.
        #[derive(Clone)]
        pub enum DataKey for "treasury-diversification" {
            Governor => Instance,
            Keeper => Instance,
            Targets => Instance,
            Routes => Instance,
            Caps => Instance,
            SwapsPaused => Instance,
            PeriodVolume(u64) => Persistent,
            TimelockDelay => Instance,
            PendingTargets => Instance,
            PendingTargetsTime => Instance,
        }
    }
}

/// Storage keys of the `vote-escrow-contract` contract.
pub mod vote_escrow_contract {
    use soroban_sdk::Address;

    crate::storage_keys! {
        /// Moved from `vote-escrow-contract/src/lib.rs`; encodings are unchanged.
        #[derive(Clone)]
        pub enum Key for "vote-escrow-contract" {
            NextId => Persistent,
            Lock(u64) => Persistent,
            HolderLock(Address, u64) => Persistent,
        }
    }
}
