//! P2P atomic swaps (#1072), snapshot dividends (#1186), streaming payouts
//! (#1250) and a staleness-guarded oracle reader (#1251).
//!
//! Each feature is a standalone contract. Keys and payout assets are token
//! contracts, so every transfer goes through the standard token interface.

use soroban_sdk::{
    contract, contractclient, contractimpl, contracttype, token, Address, Env, Symbol, Vec,
};

use crate::ContractError;

fn emit<T, D>(env: &Env, name: &str, topic: T, data: D)
where
    T: soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
    D: soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
{
    env.events().publish((Symbol::new(env, name), topic), data);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. ATOMIC P2P KEY SWAP (#1072)
// ═══════════════════════════════════════════════════════════════════════════════

/// Proposals can be accepted for one hour after they are made.
pub const SWAP_EXPIRY_SECS: u64 = 3_600;

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SwapProposal {
    pub proposer: Address,
    pub counterparty: Address,
    pub offered_key: Address,
    pub offered_amount: i128,
    pub requested_key: Address,
    pub requested_amount: i128,
    pub expires_at: u64,
}

#[derive(Clone)]
#[contracttype]
pub enum SwapKey {
    NextId,
    Proposal(u32),
}

pub mod swap_contract {
    use super::*;

    #[contract]
    pub struct KeySwapContract;

    #[contractimpl]
    impl KeySwapContract {
        /// Lock `offered_amount` of `offered_key` in escrow and propose a swap
        /// that only `counterparty` can accept.
        pub fn propose_swap(
            env: Env,
            proposer: Address,
            offered_key: Address,
            offered_amount: i128,
            requested_key: Address,
            requested_amount: i128,
            counterparty: Address,
        ) -> Result<u32, ContractError> {
            proposer.require_auth();
            if offered_amount <= 0 || requested_amount <= 0 {
                return Err(ContractError::NotPositiveAmount);
            }
            if proposer == counterparty {
                return Err(ContractError::SelfTransfer);
            }

            token::Client::new(&env, &offered_key).transfer(
                &proposer,
                &env.current_contract_address(),
                &offered_amount,
            );

            let id: u32 = env.storage().instance().get(&SwapKey::NextId).unwrap_or(0) + 1;
            env.storage().instance().set(&SwapKey::NextId, &id);
            let proposal = SwapProposal {
                proposer: proposer.clone(),
                counterparty: counterparty.clone(),
                offered_key: offered_key.clone(),
                offered_amount,
                requested_key: requested_key.clone(),
                requested_amount,
                expires_at: env.ledger().timestamp() + SWAP_EXPIRY_SECS,
            };
            env.storage()
                .persistent()
                .set(&SwapKey::Proposal(id), &proposal);

            emit(
                &env,
                "SwapProposed",
                id,
                (
                    proposer,
                    counterparty,
                    offered_key,
                    offered_amount,
                    requested_key,
                    requested_amount,
                ),
            );
            Ok(id)
        }

        /// Counterparty-only. Moves both legs in one call: requested amount to
        /// the proposer, escrowed offered amount to the counterparty.
        pub fn accept_swap(
            env: Env,
            caller: Address,
            proposal_id: u32,
        ) -> Result<(), ContractError> {
            caller.require_auth();
            let p = Self::get_swap(env.clone(), proposal_id)?;
            if caller != p.counterparty {
                return Err(ContractError::Unauthorized);
            }
            if env.ledger().timestamp() >= p.expires_at {
                return Err(ContractError::DeadlinePassed);
            }

            env.storage()
                .persistent()
                .remove(&SwapKey::Proposal(proposal_id));
            token::Client::new(&env, &p.requested_key).transfer(
                &p.counterparty,
                &p.proposer,
                &p.requested_amount,
            );
            token::Client::new(&env, &p.offered_key).transfer(
                &env.current_contract_address(),
                &p.counterparty,
                &p.offered_amount,
            );

            emit(
                &env,
                "SwapExecuted",
                proposal_id,
                (
                    p.proposer,
                    p.counterparty,
                    p.offered_amount,
                    p.requested_amount,
                ),
            );
            Ok(())
        }

        /// Proposer-only before expiry. Once the proposal has expired anyone may
        /// call it, and the escrow always returns to the proposer.
        pub fn cancel_swap(
            env: Env,
            caller: Address,
            proposal_id: u32,
        ) -> Result<(), ContractError> {
            caller.require_auth();
            let p = Self::get_swap(env.clone(), proposal_id)?;
            if caller != p.proposer && env.ledger().timestamp() < p.expires_at {
                return Err(ContractError::Unauthorized);
            }

            env.storage()
                .persistent()
                .remove(&SwapKey::Proposal(proposal_id));
            token::Client::new(&env, &p.offered_key).transfer(
                &env.current_contract_address(),
                &p.proposer,
                &p.offered_amount,
            );

            emit(
                &env,
                "SwapCancelled",
                proposal_id,
                (p.proposer, p.offered_amount),
            );
            Ok(())
        }

        pub fn get_swap(env: Env, proposal_id: u32) -> Result<SwapProposal, ContractError> {
            env.storage()
                .persistent()
                .get(&SwapKey::Proposal(proposal_id))
                .ok_or(ContractError::ProposalNotFound)
        }
    }
}

pub use swap_contract::{KeySwapContract, KeySwapContractClient};

// ═══════════════════════════════════════════════════════════════════════════════
// 2. KEY DIVIDEND DECLARATION WITH SNAPSHOT PRO-RATA CLAIMS (#1186)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct KeyDividend {
    pub creator: Address,
    pub token: Address,
    pub total_amount: i128,
    pub total_supply: i128,
    pub claimed: i128,
    pub snapshot_ledger: u32,
    pub expiry_ledger: u32,
    pub reclaimed: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum KeyDividendKey {
    NextId(Address),
    Dividend(Address, u32),
    Balance(Address, u32, Address),
    Claimed(Address, u32, Address),
}

pub mod key_dividend_contract {
    use super::*;

    #[contract]
    pub struct KeyDividendContract;

    #[contractimpl]
    impl KeyDividendContract {
        /// Snapshot `holders`' balances of the `key_id` token at the current
        /// ledger and escrow `total_amount` of `token` for pro-rata claims.
        ///
        /// Token holders cannot be enumerated on-chain, so the caller supplies
        /// the holder list; balances themselves are read from the key token.
        pub fn declare_dividend(
            env: Env,
            creator: Address,
            key_id: Address,
            token: Address,
            total_amount: i128,
            holders: Vec<Address>,
            expiry_ledger: u32,
        ) -> Result<u32, ContractError> {
            creator.require_auth();
            if total_amount <= 0 {
                return Err(ContractError::ZeroDistributionAmount);
            }
            if expiry_ledger <= env.ledger().sequence() {
                return Err(ContractError::DeadlinePassed);
            }

            let store = env.storage().persistent();
            let id: u32 = store
                .get(&KeyDividendKey::NextId(key_id.clone()))
                .unwrap_or(0)
                + 1;

            let key = token::Client::new(&env, &key_id);
            let mut total_supply: i128 = 0;
            for holder in holders.iter() {
                let slot = KeyDividendKey::Balance(key_id.clone(), id, holder.clone());
                if store.has(&slot) {
                    continue;
                }
                let balance = key.balance(&holder);
                if balance > 0 {
                    store.set(&slot, &balance);
                    total_supply += balance;
                }
            }
            if total_supply == 0 {
                return Err(ContractError::NoKeyHolders);
            }

            token::Client::new(&env, &token).transfer(
                &creator,
                &env.current_contract_address(),
                &total_amount,
            );

            store.set(&KeyDividendKey::NextId(key_id.clone()), &id);
            store.set(
                &KeyDividendKey::Dividend(key_id.clone(), id),
                &KeyDividend {
                    creator,
                    token,
                    total_amount,
                    total_supply,
                    claimed: 0,
                    snapshot_ledger: env.ledger().sequence(),
                    expiry_ledger,
                    reclaimed: false,
                },
            );
            emit(
                &env,
                "dividend_declared",
                (key_id, id),
                (total_amount, total_supply),
            );
            Ok(id)
        }

        /// Unclaimed pro-rata share; zero if the wallet had no snapshot
        /// balance, already claimed, or the dividend has expired.
        pub fn get_dividend_claimable(
            env: Env,
            key_id: Address,
            dividend_id: u32,
            wallet: Address,
        ) -> i128 {
            let Ok(d) = Self::get_dividend(env.clone(), key_id.clone(), dividend_id) else {
                return 0;
            };
            let store = env.storage().persistent();
            if env.ledger().sequence() > d.expiry_ledger
                || store.has(&KeyDividendKey::Claimed(
                    key_id.clone(),
                    dividend_id,
                    wallet.clone(),
                ))
            {
                return 0;
            }
            let balance: i128 = store
                .get(&KeyDividendKey::Balance(key_id, dividend_id, wallet))
                .unwrap_or(0);
            balance * d.total_amount / d.total_supply
        }

        pub fn claim_dividend(
            env: Env,
            wallet: Address,
            key_id: Address,
            dividend_id: u32,
        ) -> Result<i128, ContractError> {
            wallet.require_auth();
            let mut d = Self::get_dividend(env.clone(), key_id.clone(), dividend_id)?;
            let claimed_slot = KeyDividendKey::Claimed(key_id.clone(), dividend_id, wallet.clone());
            if env.storage().persistent().has(&claimed_slot) {
                return Err(ContractError::AlreadyClaimed);
            }
            if env.ledger().sequence() > d.expiry_ledger {
                return Err(ContractError::DeadlinePassed);
            }
            let share = Self::get_dividend_claimable(
                env.clone(),
                key_id.clone(),
                dividend_id,
                wallet.clone(),
            );
            if share == 0 {
                return Err(ContractError::NoDividendClaimable);
            }

            d.claimed += share;
            let store = env.storage().persistent();
            store.set(&claimed_slot, &true);
            store.set(&KeyDividendKey::Dividend(key_id.clone(), dividend_id), &d);
            token::Client::new(&env, &d.token).transfer(
                &env.current_contract_address(),
                &wallet,
                &share,
            );
            emit(
                &env,
                "dividend_claimed",
                (key_id, dividend_id),
                (wallet, share),
            );
            Ok(share)
        }

        /// After `expiry_ledger`, return everything unclaimed (including
        /// rounding dust) to the creator. Callable by anyone, once.
        pub fn reclaim_expired_dividend(
            env: Env,
            key_id: Address,
            dividend_id: u32,
        ) -> Result<i128, ContractError> {
            let mut d = Self::get_dividend(env.clone(), key_id.clone(), dividend_id)?;
            if env.ledger().sequence() <= d.expiry_ledger {
                return Err(ContractError::TimelockNotElapsed);
            }
            if d.reclaimed {
                return Err(ContractError::AlreadyClaimed);
            }
            let amount = d.total_amount - d.claimed;
            d.reclaimed = true;
            env.storage()
                .persistent()
                .set(&KeyDividendKey::Dividend(key_id.clone(), dividend_id), &d);
            if amount > 0 {
                token::Client::new(&env, &d.token).transfer(
                    &env.current_contract_address(),
                    &d.creator,
                    &amount,
                );
            }
            emit(
                &env,
                "dividend_reclaimed",
                (key_id, dividend_id),
                (d.creator, amount),
            );
            Ok(amount)
        }

        pub fn get_dividend(
            env: Env,
            key_id: Address,
            dividend_id: u32,
        ) -> Result<KeyDividend, ContractError> {
            env.storage()
                .persistent()
                .get(&KeyDividendKey::Dividend(key_id, dividend_id))
                .ok_or(ContractError::SnapshotNotFound)
        }
    }
}

pub use key_dividend_contract::{KeyDividendContract, KeyDividendContractClient};

// ═══════════════════════════════════════════════════════════════════════════════
// 3. STREAMING CREATOR PAYOUTS (#1250)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PayoutStream {
    pub creator: Address,
    pub key_id: Address,
    pub rate_per_ledger: i128,
    pub cap: i128,
    pub funded: i128,
    pub withdrawn: i128,
    /// Accrual checkpointed at `last_ledger` (or at pause time).
    pub accrued: i128,
    pub last_ledger: u32,
    pub paused: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum StreamKey {
    Admin,
    Token,
    NextId,
    Stream(u32),
}

pub mod stream_contract {
    use super::*;

    #[contract]
    pub struct PayoutStreamContract;

    fn accrued_now(env: &Env, s: &PayoutStream) -> i128 {
        if s.paused {
            return s.accrued;
        }
        let elapsed = env.ledger().sequence().saturating_sub(s.last_ledger) as i128;
        s.accrued
            .saturating_add(s.rate_per_ledger.saturating_mul(elapsed))
    }

    fn require_admin(env: &Env, admin: &Address) -> Result<(), ContractError> {
        admin.require_auth();
        let stored: Address = env
            .storage()
            .instance()
            .get(&StreamKey::Admin)
            .ok_or(ContractError::Unauthorized)?;
        if &stored != admin {
            return Err(ContractError::Unauthorized);
        }
        Ok(())
    }

    #[contractimpl]
    impl PayoutStreamContract {
        /// One-time setup: `admin` may pause/resume, `token` is the payout asset.
        pub fn init_stream(env: Env, admin: Address, token: Address) -> Result<(), ContractError> {
            if env.storage().instance().has(&StreamKey::Admin) {
                return Err(ContractError::AlreadyInitialized);
            }
            admin.require_auth();
            env.storage().instance().set(&StreamKey::Admin, &admin);
            env.storage().instance().set(&StreamKey::Token, &token);
            Ok(())
        }

        pub fn create_stream(
            env: Env,
            creator: Address,
            key_id: Address,
            rate_per_ledger: i128,
            cap: i128,
        ) -> Result<u32, ContractError> {
            creator.require_auth();
            if rate_per_ledger <= 0 || cap <= 0 {
                return Err(ContractError::NotPositiveAmount);
            }
            let id: u32 = env
                .storage()
                .instance()
                .get(&StreamKey::NextId)
                .unwrap_or(0)
                + 1;
            env.storage().instance().set(&StreamKey::NextId, &id);
            env.storage().persistent().set(
                &StreamKey::Stream(id),
                &PayoutStream {
                    creator: creator.clone(),
                    key_id: key_id.clone(),
                    rate_per_ledger,
                    cap,
                    funded: 0,
                    withdrawn: 0,
                    accrued: 0,
                    last_ledger: env.ledger().sequence(),
                    paused: false,
                },
            );
            emit(
                &env,
                "stream_created",
                id,
                (creator, key_id, rate_per_ledger, cap),
            );
            Ok(id)
        }

        /// Credit the stream; raises `cap` if funding exceeds it. Elapsed
        /// accrual is untouched.
        pub fn fund_stream(
            env: Env,
            payer: Address,
            stream_id: u32,
            amount: i128,
        ) -> Result<(), ContractError> {
            payer.require_auth();
            if amount <= 0 {
                return Err(ContractError::NotPositiveAmount);
            }
            let mut s = Self::stream_info(env.clone(), stream_id)?;
            let token: Address = env.storage().instance().get(&StreamKey::Token).unwrap();
            token::Client::new(&env, &token).transfer(
                &payer,
                &env.current_contract_address(),
                &amount,
            );
            s.funded += amount;
            s.cap = s.cap.max(s.funded);
            env.storage()
                .persistent()
                .set(&StreamKey::Stream(stream_id), &s);
            emit(&env, "stream_funded", stream_id, (payer, amount));
            Ok(())
        }

        /// `min(accrued, cap, funded) - withdrawn`.
        pub fn claimable(env: Env, stream_id: u32) -> Result<i128, ContractError> {
            let s = Self::stream_info(env.clone(), stream_id)?;
            let vested = accrued_now(&env, &s).min(s.cap).min(s.funded);
            Ok((vested - s.withdrawn).max(0))
        }

        pub fn withdraw(
            env: Env,
            creator: Address,
            stream_id: u32,
            amount: i128,
        ) -> Result<(), ContractError> {
            creator.require_auth();
            let mut s = Self::stream_info(env.clone(), stream_id)?;
            if creator != s.creator {
                return Err(ContractError::Unauthorized);
            }
            if amount <= 0 {
                return Err(ContractError::NotPositiveAmount);
            }
            if amount > Self::claimable(env.clone(), stream_id)? {
                return Err(ContractError::InsufficientBalance);
            }
            s.withdrawn += amount;
            env.storage()
                .persistent()
                .set(&StreamKey::Stream(stream_id), &s);
            let token: Address = env.storage().instance().get(&StreamKey::Token).unwrap();
            token::Client::new(&env, &token).transfer(
                &env.current_contract_address(),
                &creator,
                &amount,
            );
            emit(&env, "stream_withdrawn", stream_id, (creator, amount));
            Ok(())
        }

        /// Freeze accrual; the balance accrued so far is preserved.
        pub fn pause_stream(env: Env, admin: Address, stream_id: u32) -> Result<(), ContractError> {
            require_admin(&env, &admin)?;
            let mut s = Self::stream_info(env.clone(), stream_id)?;
            if !s.paused {
                s.accrued = accrued_now(&env, &s);
                s.paused = true;
                env.storage()
                    .persistent()
                    .set(&StreamKey::Stream(stream_id), &s);
            }
            Ok(())
        }

        /// Restart accrual from the current ledger.
        pub fn resume_stream(
            env: Env,
            admin: Address,
            stream_id: u32,
        ) -> Result<(), ContractError> {
            require_admin(&env, &admin)?;
            let mut s = Self::stream_info(env.clone(), stream_id)?;
            if s.paused {
                s.last_ledger = env.ledger().sequence();
                s.paused = false;
                env.storage()
                    .persistent()
                    .set(&StreamKey::Stream(stream_id), &s);
            }
            Ok(())
        }

        pub fn stream_info(env: Env, stream_id: u32) -> Result<PayoutStream, ContractError> {
            env.storage()
                .persistent()
                .get(&StreamKey::Stream(stream_id))
                .ok_or(ContractError::NotRegistered)
        }
    }
}

pub use stream_contract::{PayoutStreamContract, PayoutStreamContractClient};

// ═══════════════════════════════════════════════════════════════════════════════
// 4. ORACLE STALENESS GUARD WITH FALLBACK AND DEVIATION CHECK (#1251)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PriceData {
    pub price: i128,
    /// Ledger sequence at which `price` was last updated.
    pub ledger: u32,
}

/// Interface every price feed must expose.
#[contractclient(name = "PriceFeedClient")]
pub trait PriceFeed {
    fn price(env: Env, asset: Address) -> PriceData;
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct OracleConfig {
    pub primary: Address,
    pub fallback: Option<Address>,
    pub max_age_ledgers: u32,
    pub max_deviation_bps: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum PriceSource {
    Primary,
    Fallback,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct OracleStatus {
    pub primary_age: u32,
    pub fallback_age: Option<u32>,
    pub source: PriceSource,
}

#[derive(Clone)]
#[contracttype]
pub enum OracleKey {
    Admin,
    Config,
}

pub mod oracle_guard_contract {
    use super::*;

    #[contract]
    pub struct OracleGuardContract;

    /// (price, age in ledgers)
    fn read(env: &Env, feed: &Address, asset: &Address) -> (i128, u32) {
        let d = PriceFeedClient::new(env, feed).price(asset);
        (d.price, env.ledger().sequence().saturating_sub(d.ledger))
    }

    fn config(env: &Env) -> Result<OracleConfig, ContractError> {
        env.storage()
            .instance()
            .get(&OracleKey::Config)
            .ok_or(ContractError::OraclePriceNotSet)
    }

    #[contractimpl]
    impl OracleGuardContract {
        pub fn init_oracle(env: Env, admin: Address) -> Result<(), ContractError> {
            if env.storage().instance().has(&OracleKey::Admin) {
                return Err(ContractError::AlreadyInitialized);
            }
            admin.require_auth();
            env.storage().instance().set(&OracleKey::Admin, &admin);
            Ok(())
        }

        pub fn set_oracle_config(
            env: Env,
            admin: Address,
            primary: Address,
            fallback: Option<Address>,
            max_age_ledgers: u32,
            max_deviation_bps: u32,
        ) -> Result<(), ContractError> {
            admin.require_auth();
            let stored: Option<Address> = env.storage().instance().get(&OracleKey::Admin);
            if stored != Some(admin) {
                return Err(ContractError::Unauthorized);
            }
            env.storage().instance().set(
                &OracleKey::Config,
                &OracleConfig {
                    primary,
                    fallback,
                    max_age_ledgers,
                    max_deviation_bps,
                },
            );
            Ok(())
        }

        /// Fresh primary wins (after a deviation check against a fresh
        /// fallback); a stale primary falls back to a fresh secondary; both
        /// stale reverts with `StalePrice`.
        pub fn get_price(env: Env, asset: Address) -> Result<i128, ContractError> {
            let cfg = config(&env)?;
            let (p, p_age) = read(&env, &cfg.primary, &asset);
            let fb = cfg.fallback.as_ref().map(|f| read(&env, f, &asset));
            let fresh = |age: u32| age <= cfg.max_age_ledgers;

            match (fresh(p_age), fb) {
                (true, Some((f, f_age))) if fresh(f_age) => {
                    if (p - f).abs() * 10_000 > cfg.max_deviation_bps as i128 * f.abs() {
                        return Err(ContractError::PriceDeviation);
                    }
                    Ok(p)
                }
                (true, _) => Ok(p),
                (false, Some((f, f_age))) if fresh(f_age) => {
                    emit(&env, "oracle_fallback_used", asset, (f, p_age, f_age));
                    Ok(f)
                }
                _ => Err(ContractError::StalePrice),
            }
        }

        /// Feed ages and the source `get_price` would use (ignoring deviation).
        pub fn oracle_status(env: Env, asset: Address) -> Result<OracleStatus, ContractError> {
            let cfg = config(&env)?;
            let (_, primary_age) = read(&env, &cfg.primary, &asset);
            let fallback_age = cfg.fallback.as_ref().map(|f| read(&env, f, &asset).1);
            let source = if primary_age <= cfg.max_age_ledgers {
                PriceSource::Primary
            } else if fallback_age.is_some_and(|a| a <= cfg.max_age_ledgers) {
                PriceSource::Fallback
            } else {
                PriceSource::Unavailable
            };
            Ok(OracleStatus {
                primary_age,
                fallback_age,
                source,
            })
        }

        pub fn get_oracle_config(env: Env) -> Result<OracleConfig, ContractError> {
            config(&env)
        }
    }
}

pub use oracle_guard_contract::{OracleGuardContract, OracleGuardContractClient};
