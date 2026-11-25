//! Execution of firm prices agreed off-chain between approved market makers
//! and takers (#1288).
//!
//! Quotes are ed25519-signed over a domain-separated payload and settle
//! atomically from quoter escrow. Nonces prevent replay, expiries bound staleness,
//! per-window notional caps bound exposure, and deviation checks prevent a
//! quoter from settling far away from the current curve price.

#![no_std]
// Soroban's `contractimpl` generates client/args helpers that inherit the
// argument count of configuration entrypoints.
#![allow(clippy::too_many_arguments)]
#[cfg(test)]
extern crate std;

use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, token, xdr::ToXdr,
    Address, Bytes, BytesN, Env, Symbol,
};

#[cfg(test)]
mod test;

/// Buy side: the taker receives keys and pays the quoted notional plus fees.
pub const SIDE_BUY: u32 = 0;
/// Sell side: the taker delivers keys and receives the notional minus fees.
pub const SIDE_SELL: u32 = 1;

/// `quote_executed` event topics: `(name, quoter, taker)`.
pub const QUOTE_EXECUTED_EVENT: &str = "quote_executed";

const DOMAIN_TAG: &[u8] = b"ACL_SIGNED_QUOTE_V1";
const BPS_DENOMINATOR: i128 = 10_000;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum QuoteError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidConfiguration = 4,
    InvalidAmount = 5,
    InvalidSide = 6,
    QuoterNotConfigured = 7,
    QuoterNotApproved = 8,
    Expired = 9,
    NonceUsed = 10,
    NotionalLimitExceeded = 11,
    PriceDeviationTooHigh = 12,
    InsufficientInventory = 13,
    QuoteMismatch = 14,
    ArithmeticOverflow = 15,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum DataKey {
    Admin,
    PaymentToken,
    KeyToken,
    Treasury,
    RoyaltyRecipient,
    CurveOracle,
    ProtocolFeeBps,
    RoyaltyBps,
    MaxDeviationBps,
    WindowLedgers,
    QuoterKey(Address),
    Quoter(Address),
    Inventory(Address, Address),
    UsedNonce(Address, u64),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PriceQuote {
    pub quoter: Address,
    pub key_id: Address,
    /// [`SIDE_BUY`] or [`SIDE_SELL`].
    pub side: u32,
    /// Number of keys to settle.
    pub quantity: i128,
    /// Quoted price per key, in payment-token units.
    pub price: i128,
    pub nonce: u64,
    /// Last ledger at which the quote may be executed, inclusive.
    pub expiry: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct QuoterProfile {
    pub public_key: BytesN<32>,
    pub max_notional: i128,
    pub approved: bool,
    pub window_start: u32,
    pub window_notional: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct QuoteInventory {
    pub keys: i128,
    pub payment: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct QuoteExecutedEvent {
    pub side: u32,
    pub price: i128,
    pub quantity: i128,
    pub notional: i128,
    pub protocol_fee: i128,
    pub royalty_fee: i128,
}

/// Minimal oracle interface used for the curve-deviation guard. Production
/// deployments can adapt `CreatorKeysContract::get_buy_quote` behind it.
#[contractclient(name = "CurveOracleClient")]
pub trait CurveOracle {
    fn curve_price(env: Env, key_id: Address) -> i128;
}

#[contract]
pub struct SignedQuoteExecutionContract;

#[contractimpl]
impl SignedQuoteExecutionContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        payment_token: Address,
        key_token: Address,
        treasury: Address,
        royalty_recipient: Address,
        curve_oracle: Address,
        protocol_fee_bps: u32,
        royalty_bps: u32,
        max_deviation_bps: u32,
        window_ledgers: u32,
    ) -> Result<(), QuoteError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(QuoteError::AlreadyInitialized);
        }
        if protocol_fee_bps > BPS_DENOMINATOR as u32
            || royalty_bps > BPS_DENOMINATOR as u32
            || window_ledgers == 0
        {
            return Err(QuoteError::InvalidConfiguration);
        }
        admin.require_auth();
        let instance = env.storage().instance();
        instance.set(&DataKey::Admin, &admin);
        instance.set(&DataKey::PaymentToken, &payment_token);
        instance.set(&DataKey::KeyToken, &key_token);
        instance.set(&DataKey::Treasury, &treasury);
        instance.set(&DataKey::RoyaltyRecipient, &royalty_recipient);
        instance.set(&DataKey::CurveOracle, &curve_oracle);
        instance.set(&DataKey::ProtocolFeeBps, &protocol_fee_bps);
        instance.set(&DataKey::RoyaltyBps, &royalty_bps);
        instance.set(&DataKey::MaxDeviationBps, &max_deviation_bps);
        instance.set(&DataKey::WindowLedgers, &window_ledgers);
        Ok(())
    }

    /// Registers the ed25519 key trusted to sign quotes for `quoter`.
    pub fn set_quoter_public_key(
        env: Env,
        admin: Address,
        quoter: Address,
        public_key: BytesN<32>,
    ) -> Result<(), QuoteError> {
        Self::require_admin(&env, &admin)?;
        env.storage()
            .persistent()
            .set(&DataKey::QuoterKey(quoter), &public_key);
        Ok(())
    }

    /// Approves `quoter` with a per-window notional cap. The public key must
    /// already be configured with [`Self::set_quoter_public_key`].
    pub fn approve_quoter(
        env: Env,
        admin: Address,
        quoter: Address,
        max_notional: i128,
    ) -> Result<(), QuoteError> {
        Self::require_admin(&env, &admin)?;
        if max_notional <= 0 {
            return Err(QuoteError::InvalidAmount);
        }
        let public_key: BytesN<32> = env
            .storage()
            .persistent()
            .get(&DataKey::QuoterKey(quoter.clone()))
            .ok_or(QuoteError::QuoterNotConfigured)?;
        Self::write_quoter(&env, quoter, public_key, max_notional);
        Ok(())
    }

    /// Configures the signing key and approval in one call.
    pub fn approve_quoter_with_key(
        env: Env,
        admin: Address,
        quoter: Address,
        public_key: BytesN<32>,
        max_notional: i128,
    ) -> Result<(), QuoteError> {
        Self::require_admin(&env, &admin)?;
        if max_notional <= 0 {
            return Err(QuoteError::InvalidAmount);
        }
        Self::write_quoter(&env, quoter, public_key, max_notional);
        Ok(())
    }

    pub fn revoke_quoter(env: Env, admin: Address, quoter: Address) -> Result<(), QuoteError> {
        Self::require_admin(&env, &admin)?;
        let key = DataKey::Quoter(quoter);
        let mut profile: QuoterProfile = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(QuoteError::QuoterNotConfigured)?;
        profile.approved = false;
        env.storage().persistent().set(&key, &profile);
        Ok(())
    }

    /// Escrows keys and payment inventory so either side can execute without
    /// requiring the quoter's interactive on-chain authorization.
    pub fn deposit_quote_inventory(
        env: Env,
        quoter: Address,
        key_id: Address,
        keys: i128,
        payment: i128,
    ) -> Result<(), QuoteError> {
        Self::require_initialized(&env)?;
        quoter.require_auth();
        if keys < 0 || payment < 0 || (keys == 0 && payment == 0) {
            return Err(QuoteError::InvalidAmount);
        }
        let key_token: Address = env.storage().instance().get(&DataKey::KeyToken).unwrap();
        let payment_token: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .unwrap();
        if keys > 0 {
            token::Client::new(&env, &key_token).transfer(
                &quoter,
                &env.current_contract_address(),
                &keys,
            );
        }
        if payment > 0 {
            token::Client::new(&env, &payment_token).transfer(
                &quoter,
                &env.current_contract_address(),
                &payment,
            );
        }
        let key = DataKey::Inventory(quoter, key_id);
        let inventory: QuoteInventory =
            env.storage()
                .persistent()
                .get(&key)
                .unwrap_or(QuoteInventory {
                    keys: 0,
                    payment: 0,
                });
        env.storage().persistent().set(
            &key,
            &QuoteInventory {
                keys: inventory
                    .keys
                    .checked_add(keys)
                    .ok_or(QuoteError::ArithmeticOverflow)?,
                payment: inventory
                    .payment
                    .checked_add(payment)
                    .ok_or(QuoteError::ArithmeticOverflow)?,
            },
        );
        Ok(())
    }

    pub fn withdraw_quote_inventory(
        env: Env,
        quoter: Address,
        key_id: Address,
        keys: i128,
        payment: i128,
    ) -> Result<(), QuoteError> {
        Self::require_initialized(&env)?;
        quoter.require_auth();
        let key = DataKey::Inventory(quoter.clone(), key_id);
        let mut inventory: QuoteInventory = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(QuoteError::InsufficientInventory)?;
        if keys < 0 || payment < 0 || keys > inventory.keys || payment > inventory.payment {
            return Err(QuoteError::InsufficientInventory);
        }
        inventory.keys -= keys;
        inventory.payment -= payment;
        env.storage().persistent().set(&key, &inventory);

        let key_token: Address = env.storage().instance().get(&DataKey::KeyToken).unwrap();
        let payment_token: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .unwrap();
        if keys > 0 {
            token::Client::new(&env, &key_token).transfer(
                &env.current_contract_address(),
                &quoter,
                &keys,
            );
        }
        if payment > 0 {
            token::Client::new(&env, &payment_token).transfer(
                &env.current_contract_address(),
                &quoter,
                &payment,
            );
        }
        Ok(())
    }

    /// Canonical bytes signed by the quoter. Exposed so off-chain signers and
    /// tests can reproduce the payload exactly.
    pub fn quote_payload(env: Env, quote: PriceQuote) -> Bytes {
        let mut payload = Bytes::from_slice(&env, DOMAIN_TAG);
        payload.append(&env.ledger().network_id().into());
        payload.append(&env.current_contract_address().to_xdr(&env));
        payload.append(&quote.quoter.to_xdr(&env));
        payload.append(&quote.key_id.to_xdr(&env));
        payload.extend_from_array(&quote.side.to_be_bytes());
        payload.extend_from_array(&quote.quantity.to_be_bytes());
        payload.extend_from_array(&quote.price.to_be_bytes());
        payload.extend_from_array(&quote.nonce.to_be_bytes());
        payload.extend_from_array(&quote.expiry.to_be_bytes());
        payload
    }

    /// Settles a signed quote once. Both sides are covered by escrow, so the
    /// taker can execute without the quoter sending a second transaction.
    pub fn execute_quote(
        env: Env,
        taker: Address,
        quote: PriceQuote,
        signature: BytesN<64>,
    ) -> Result<QuoteExecutedEvent, QuoteError> {
        Self::require_initialized(&env)?;
        taker.require_auth();
        if quote.quantity <= 0 || quote.price <= 0 {
            return Err(QuoteError::InvalidAmount);
        }
        if quote.side != SIDE_BUY && quote.side != SIDE_SELL {
            return Err(QuoteError::InvalidSide);
        }
        if env.ledger().sequence() > quote.expiry {
            return Err(QuoteError::Expired);
        }

        let quoter_key = DataKey::Quoter(quote.quoter.clone());
        let mut profile: QuoterProfile = env
            .storage()
            .persistent()
            .get(&quoter_key)
            .ok_or(QuoteError::QuoterNotConfigured)?;
        if !profile.approved {
            return Err(QuoteError::QuoterNotApproved);
        }
        let nonce_key = DataKey::UsedNonce(quote.quoter.clone(), quote.nonce);
        if env.storage().persistent().has(&nonce_key) {
            return Err(QuoteError::NonceUsed);
        }

        let payload = Self::quote_payload(env.clone(), quote.clone());
        env.crypto()
            .ed25519_verify(&profile.public_key, &payload, &signature);

        let notional = quote
            .quantity
            .checked_mul(quote.price)
            .ok_or(QuoteError::ArithmeticOverflow)?;
        let window_ledgers: u32 = env
            .storage()
            .instance()
            .get(&DataKey::WindowLedgers)
            .unwrap();
        if env.ledger().sequence().saturating_sub(profile.window_start) >= window_ledgers {
            profile.window_start = env.ledger().sequence();
            profile.window_notional = 0;
        }
        profile.window_notional = profile
            .window_notional
            .checked_add(notional)
            .ok_or(QuoteError::ArithmeticOverflow)?;
        if profile.window_notional > profile.max_notional {
            return Err(QuoteError::NotionalLimitExceeded);
        }
        env.storage().persistent().set(&quoter_key, &profile);

        Self::enforce_deviation(&env, &quote.key_id, quote.price)?;

        let protocol_fee_bps: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ProtocolFeeBps)
            .unwrap();
        let royalty_bps: u32 = env.storage().instance().get(&DataKey::RoyaltyBps).unwrap();
        let protocol_fee = notional
            .checked_mul(protocol_fee_bps as i128)
            .and_then(|v| v.checked_div(BPS_DENOMINATOR))
            .ok_or(QuoteError::ArithmeticOverflow)?;
        let royalty_fee = notional
            .checked_mul(royalty_bps as i128)
            .and_then(|v| v.checked_div(BPS_DENOMINATOR))
            .ok_or(QuoteError::ArithmeticOverflow)?;

        let key_token: Address = env.storage().instance().get(&DataKey::KeyToken).unwrap();
        let payment_token: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .unwrap();
        let treasury: Address = env.storage().instance().get(&DataKey::Treasury).unwrap();
        let royalty_recipient: Address = env
            .storage()
            .instance()
            .get(&DataKey::RoyaltyRecipient)
            .unwrap();
        let contract = env.current_contract_address();
        let inventory_key = DataKey::Inventory(quote.quoter.clone(), quote.key_id.clone());
        let mut inventory: QuoteInventory = env
            .storage()
            .persistent()
            .get(&inventory_key)
            .ok_or(QuoteError::InsufficientInventory)?;

        if quote.side == SIDE_BUY {
            if inventory.keys < quote.quantity {
                return Err(QuoteError::InsufficientInventory);
            }
            let gross = notional
                .checked_add(protocol_fee)
                .and_then(|v| v.checked_add(royalty_fee))
                .ok_or(QuoteError::ArithmeticOverflow)?;
            token::Client::new(&env, &payment_token).transfer(&taker, &contract, &gross);
            token::Client::new(&env, &payment_token).transfer(&contract, &quote.quoter, &notional);
            token::Client::new(&env, &payment_token).transfer(&contract, &treasury, &protocol_fee);
            token::Client::new(&env, &payment_token).transfer(
                &contract,
                &royalty_recipient,
                &royalty_fee,
            );
            token::Client::new(&env, &key_token).transfer(&contract, &taker, &quote.quantity);
            inventory.keys -= quote.quantity;
        } else {
            if inventory.payment < notional {
                return Err(QuoteError::InsufficientInventory);
            }
            let net = notional
                .checked_sub(protocol_fee)
                .and_then(|v| v.checked_sub(royalty_fee))
                .ok_or(QuoteError::ArithmeticOverflow)?;
            token::Client::new(&env, &key_token).transfer(&taker, &contract, &quote.quantity);
            inventory.keys = inventory
                .keys
                .checked_add(quote.quantity)
                .ok_or(QuoteError::ArithmeticOverflow)?;
            token::Client::new(&env, &payment_token).transfer(&contract, &taker, &net);
            token::Client::new(&env, &payment_token).transfer(&contract, &treasury, &protocol_fee);
            token::Client::new(&env, &payment_token).transfer(
                &contract,
                &royalty_recipient,
                &royalty_fee,
            );
            inventory.payment -= notional;
        }
        env.storage().persistent().set(&inventory_key, &inventory);
        env.storage().persistent().set(&nonce_key, &true);

        let event = QuoteExecutedEvent {
            side: quote.side,
            price: quote.price,
            quantity: quote.quantity,
            notional,
            protocol_fee,
            royalty_fee,
        };
        env.events().publish(
            (Symbol::new(&env, QUOTE_EXECUTED_EVENT), quote.quoter, taker),
            event.clone(),
        );
        Ok(event)
    }

    pub fn quoter_profile(env: Env, quoter: Address) -> Option<QuoterProfile> {
        env.storage().persistent().get(&DataKey::Quoter(quoter))
    }

    pub fn quote_inventory(env: Env, quoter: Address, key_id: Address) -> QuoteInventory {
        env.storage()
            .persistent()
            .get(&DataKey::Inventory(quoter, key_id))
            .unwrap_or(QuoteInventory {
                keys: 0,
                payment: 0,
            })
    }

    pub fn is_nonce_used(env: Env, quoter: Address, nonce: u64) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::UsedNonce(quoter, nonce))
    }

    fn write_quoter(env: &Env, quoter: Address, public_key: BytesN<32>, max_notional: i128) {
        let key = DataKey::Quoter(quoter);
        let previous: Option<QuoterProfile> = env.storage().persistent().get(&key);
        let profile = QuoterProfile {
            public_key,
            max_notional,
            approved: true,
            window_start: previous
                .as_ref()
                .map(|p| p.window_start)
                .unwrap_or_else(|| env.ledger().sequence()),
            window_notional: 0,
        };
        env.storage().persistent().set(&key, &profile);
    }

    fn require_initialized(env: &Env) -> Result<(), QuoteError> {
        if env.storage().instance().has(&DataKey::Admin) {
            Ok(())
        } else {
            Err(QuoteError::NotInitialized)
        }
    }

    fn require_admin(env: &Env, admin: &Address) -> Result<(), QuoteError> {
        Self::require_initialized(env)?;
        admin.require_auth();
        let configured: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != &configured {
            return Err(QuoteError::Unauthorized);
        }
        Ok(())
    }

    fn enforce_deviation(env: &Env, key_id: &Address, price: i128) -> Result<(), QuoteError> {
        let oracle: Address = env.storage().instance().get(&DataKey::CurveOracle).unwrap();
        let curve_price = CurveOracleClient::new(env, &oracle).curve_price(key_id);
        if curve_price <= 0 {
            return Err(QuoteError::PriceDeviationTooHigh);
        }
        let deviation = price
            .checked_sub(curve_price)
            .and_then(|v| v.checked_abs())
            .ok_or(QuoteError::ArithmeticOverflow)?;
        let allowed = curve_price
            .checked_mul(
                env.storage()
                    .instance()
                    .get::<_, u32>(&DataKey::MaxDeviationBps)
                    .unwrap() as i128,
            )
            .and_then(|v| v.checked_div(BPS_DENOMINATOR))
            .ok_or(QuoteError::ArithmeticOverflow)?;
        if deviation > allowed {
            return Err(QuoteError::PriceDeviationTooHigh);
        }
        Ok(())
    }
}
