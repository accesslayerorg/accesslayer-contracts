//! Presale reservations with refundable deposits and priority launch
//! allocation (#1290).
//!
//! Buyers lock a deposit before a launch. They may cancel before the
//! reservation deadline, convert during the priority window with the deposit
//! credited against the purchase, or reclaim the deposit once that window
//! closes. Public purchases only open after the priority window.

#![no_std]
// Soroban's `contractimpl` generates client/args helpers that inherit the
// argument count of configuration entrypoints.
#![allow(clippy::too_many_arguments)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, Env, Symbol,
};

#[cfg(test)]
mod test;

pub const RESERVED_EVENT: &str = "reserved";
pub const RESERVATION_CANCELLED_EVENT: &str = "reservation_cancelled";
pub const RESERVATION_CONVERTED_EVENT: &str = "reservation_converted";
pub const RESERVATION_REFUNDED_EVENT: &str = "reservation_refunded";

const BPS_DENOMINATOR: i128 = 10_000;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ReservationError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    AlreadyOpen = 4,
    SaleNotFound = 5,
    InvalidConfiguration = 6,
    InvalidAmount = 7,
    ReservationClosed = 8,
    ReservationExists = 9,
    NoReservation = 10,
    CapacityExceeded = 11,
    WalletCapExceeded = 12,
    LaunchNotConfigured = 13,
    ConversionNotOpen = 14,
    ConversionWindowClosed = 15,
    InsufficientKeys = 16,
    ArithmeticOverflow = 17,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,
    PaymentToken,
    KeyToken,
    Treasury,
    RoyaltyRecipient,
    Sale(Address),
    Reservation(Address, Address),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SaleConfig {
    pub creator: Address,
    pub key_id: Address,
    pub max_reservations: u32,
    pub deposit_per_key: i128,
    pub end_ledger: u32,
    pub total_reserved: u32,
    pub available_keys: i128,
    pub proceeds: i128,
    pub launch_configured: bool,
    pub launch_price: i128,
    pub max_per_wallet: u32,
    pub public_open_ledger: u32,
    pub cancellation_fee_bps: u32,
    pub protocol_fee_bps: u32,
    pub royalty_bps: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Reservation {
    pub quantity: u32,
    pub deposit: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReservationEvent {
    pub quantity: u32,
    pub amount: i128,
}

#[contract]
pub struct PresaleReservationsContract;

#[contractimpl]
impl PresaleReservationsContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        payment_token: Address,
        key_token: Address,
        treasury: Address,
        royalty_recipient: Address,
    ) -> Result<(), ReservationError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ReservationError::AlreadyInitialized);
        }
        admin.require_auth();
        let instance = env.storage().instance();
        instance.set(&DataKey::Admin, &admin);
        instance.set(&DataKey::PaymentToken, &payment_token);
        instance.set(&DataKey::KeyToken, &key_token);
        instance.set(&DataKey::Treasury, &treasury);
        instance.set(&DataKey::RoyaltyRecipient, &royalty_recipient);
        Ok(())
    }

    /// Opens the reservation book for `key_id`. The launch parameters are
    /// configured separately with [`Self::configure_launch`].
    pub fn open_reservations(
        env: Env,
        creator: Address,
        key_id: Address,
        max_reservations: u32,
        deposit_per_key: i128,
        end_ledger: u32,
    ) -> Result<(), ReservationError> {
        Self::require_initialized(&env)?;
        creator.require_auth();
        if max_reservations == 0 || deposit_per_key <= 0 || end_ledger <= env.ledger().sequence() {
            return Err(ReservationError::InvalidConfiguration);
        }
        let key = DataKey::Sale(key_id.clone());
        if env.storage().persistent().has(&key) {
            return Err(ReservationError::AlreadyOpen);
        }
        env.storage().persistent().set(
            &key,
            &SaleConfig {
                creator,
                key_id: key_id.clone(),
                max_reservations,
                deposit_per_key,
                end_ledger,
                total_reserved: 0,
                available_keys: 0,
                proceeds: 0,
                launch_configured: false,
                launch_price: 0,
                max_per_wallet: max_reservations,
                public_open_ledger: end_ledger,
                cancellation_fee_bps: 0,
                protocol_fee_bps: 0,
                royalty_bps: 0,
            },
        );
        Ok(())
    }

    /// Sets launch pricing, the priority window and fee configuration.
    pub fn configure_launch(
        env: Env,
        creator: Address,
        key_id: Address,
        launch_price: i128,
        max_per_wallet: u32,
        public_open_ledger: u32,
        cancellation_fee_bps: u32,
        protocol_fee_bps: u32,
        royalty_bps: u32,
    ) -> Result<(), ReservationError> {
        Self::require_initialized(&env)?;
        let key = DataKey::Sale(key_id);
        let mut sale: SaleConfig = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ReservationError::SaleNotFound)?;
        if creator != sale.creator {
            return Err(ReservationError::Unauthorized);
        }
        creator.require_auth();
        if launch_price <= 0
            || max_per_wallet == 0
            || public_open_ledger <= sale.end_ledger
            || cancellation_fee_bps > BPS_DENOMINATOR as u32
            || protocol_fee_bps > BPS_DENOMINATOR as u32
            || royalty_bps > BPS_DENOMINATOR as u32
        {
            return Err(ReservationError::InvalidConfiguration);
        }
        sale.launch_configured = true;
        sale.launch_price = launch_price;
        sale.max_per_wallet = max_per_wallet;
        sale.public_open_ledger = public_open_ledger;
        sale.cancellation_fee_bps = cancellation_fee_bps;
        sale.protocol_fee_bps = protocol_fee_bps;
        sale.royalty_bps = royalty_bps;
        env.storage().persistent().set(&key, &sale);
        Ok(())
    }

    /// Escrows keys that will be delivered on conversion or public purchase.
    pub fn fund_keys(
        env: Env,
        creator: Address,
        key_id: Address,
        amount: i128,
    ) -> Result<(), ReservationError> {
        Self::require_initialized(&env)?;
        let key = DataKey::Sale(key_id.clone());
        let mut sale: SaleConfig = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ReservationError::SaleNotFound)?;
        if creator != sale.creator {
            return Err(ReservationError::Unauthorized);
        }
        creator.require_auth();
        if amount <= 0 {
            return Err(ReservationError::InvalidAmount);
        }
        let key_token: Address = env.storage().instance().get(&DataKey::KeyToken).unwrap();
        token::Client::new(&env, &key_token).transfer(
            &creator,
            &env.current_contract_address(),
            &amount,
        );
        sale.available_keys = sale
            .available_keys
            .checked_add(amount)
            .ok_or(ReservationError::ArithmeticOverflow)?;
        env.storage().persistent().set(&key, &sale);
        Ok(())
    }

    /// Locks `quantity * deposit_per_key` for `buyer`.
    pub fn reserve(
        env: Env,
        buyer: Address,
        key_id: Address,
        quantity: u32,
    ) -> Result<i128, ReservationError> {
        Self::require_initialized(&env)?;
        buyer.require_auth();
        if quantity == 0 {
            return Err(ReservationError::InvalidAmount);
        }
        let key = DataKey::Sale(key_id.clone());
        let mut sale: SaleConfig = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ReservationError::SaleNotFound)?;
        if env.ledger().sequence() >= sale.end_ledger {
            return Err(ReservationError::ReservationClosed);
        }
        let reservation_key = DataKey::Reservation(buyer.clone(), key_id.clone());
        if env.storage().persistent().has(&reservation_key) {
            return Err(ReservationError::ReservationExists);
        }
        if quantity > sale.max_per_wallet {
            return Err(ReservationError::WalletCapExceeded);
        }
        sale.total_reserved = sale
            .total_reserved
            .checked_add(quantity)
            .ok_or(ReservationError::ArithmeticOverflow)?;
        if sale.total_reserved > sale.max_reservations {
            return Err(ReservationError::CapacityExceeded);
        }

        let deposit = (quantity as i128)
            .checked_mul(sale.deposit_per_key)
            .ok_or(ReservationError::ArithmeticOverflow)?;
        let payment_token: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .unwrap();
        token::Client::new(&env, &payment_token).transfer(
            &buyer,
            &env.current_contract_address(),
            &deposit,
        );
        env.storage()
            .persistent()
            .set(&reservation_key, &Reservation { quantity, deposit });
        env.storage().persistent().set(&key, &sale);

        env.events().publish(
            (Symbol::new(&env, RESERVED_EVENT), buyer, key_id),
            ReservationEvent {
                quantity,
                amount: deposit,
            },
        );
        Ok(deposit)
    }

    /// Refunds the deposit minus the configured fee before the deadline.
    pub fn cancel_reservation(
        env: Env,
        buyer: Address,
        key_id: Address,
    ) -> Result<i128, ReservationError> {
        Self::require_initialized(&env)?;
        buyer.require_auth();
        let key = DataKey::Sale(key_id.clone());
        let mut sale: SaleConfig = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ReservationError::SaleNotFound)?;
        if env.ledger().sequence() >= sale.end_ledger {
            return Err(ReservationError::ReservationClosed);
        }
        let reservation_key = DataKey::Reservation(buyer.clone(), key_id.clone());
        let reservation: Reservation = env
            .storage()
            .persistent()
            .get(&reservation_key)
            .ok_or(ReservationError::NoReservation)?;
        let fee = reservation
            .deposit
            .checked_mul(sale.cancellation_fee_bps as i128)
            .and_then(|v| v.checked_div(BPS_DENOMINATOR))
            .ok_or(ReservationError::ArithmeticOverflow)?;
        let refund = reservation
            .deposit
            .checked_sub(fee)
            .ok_or(ReservationError::ArithmeticOverflow)?;

        env.storage().persistent().remove(&reservation_key);
        sale.total_reserved -= reservation.quantity;
        env.storage().persistent().set(&key, &sale);

        let payment_token: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .unwrap();
        let contract = env.current_contract_address();
        token::Client::new(&env, &payment_token).transfer(&contract, &buyer, &refund);
        if fee > 0 {
            let treasury: Address = env.storage().instance().get(&DataKey::Treasury).unwrap();
            token::Client::new(&env, &payment_token).transfer(&contract, &treasury, &fee);
        }

        env.events().publish(
            (
                Symbol::new(&env, RESERVATION_CANCELLED_EVENT),
                buyer,
                key_id,
            ),
            ReservationEvent {
                quantity: reservation.quantity,
                amount: refund,
            },
        );
        Ok(refund)
    }

    /// Converts a reservation during the priority window, crediting the
    /// deposit against the launch cost and fees.
    pub fn convert(env: Env, buyer: Address, key_id: Address) -> Result<i128, ReservationError> {
        Self::require_initialized(&env)?;
        buyer.require_auth();
        let key = DataKey::Sale(key_id.clone());
        let mut sale: SaleConfig = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ReservationError::SaleNotFound)?;
        if !sale.launch_configured {
            return Err(ReservationError::LaunchNotConfigured);
        }
        if env.ledger().sequence() < sale.end_ledger {
            return Err(ReservationError::ConversionNotOpen);
        }
        if env.ledger().sequence() >= sale.public_open_ledger {
            return Err(ReservationError::ConversionWindowClosed);
        }
        let reservation_key = DataKey::Reservation(buyer.clone(), key_id.clone());
        let reservation: Reservation = env
            .storage()
            .persistent()
            .get(&reservation_key)
            .ok_or(ReservationError::NoReservation)?;
        if sale.available_keys < reservation.quantity as i128 {
            return Err(ReservationError::InsufficientKeys);
        }

        let subtotal = (reservation.quantity as i128)
            .checked_mul(sale.launch_price)
            .ok_or(ReservationError::ArithmeticOverflow)?;
        let protocol_fee = Self::fee(subtotal, sale.protocol_fee_bps)?;
        let royalty_fee = Self::fee(subtotal, sale.royalty_bps)?;
        let gross = subtotal
            .checked_add(protocol_fee)
            .and_then(|v| v.checked_add(royalty_fee))
            .ok_or(ReservationError::ArithmeticOverflow)?;
        let credit = if reservation.deposit > gross {
            gross
        } else {
            reservation.deposit
        };
        let excess = reservation.deposit - credit;
        let due = gross - credit;

        env.storage().persistent().remove(&reservation_key);
        sale.total_reserved -= reservation.quantity;
        sale.available_keys -= reservation.quantity as i128;
        sale.proceeds = sale
            .proceeds
            .checked_add(subtotal)
            .ok_or(ReservationError::ArithmeticOverflow)?;
        env.storage().persistent().set(&key, &sale);

        let payment_token: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .unwrap();
        let key_token: Address = env.storage().instance().get(&DataKey::KeyToken).unwrap();
        let contract = env.current_contract_address();
        if due > 0 {
            token::Client::new(&env, &payment_token).transfer(&buyer, &contract, &due);
        }
        if excess > 0 {
            token::Client::new(&env, &payment_token).transfer(&contract, &buyer, &excess);
        }
        let key_amount = reservation.quantity as i128;
        token::Client::new(&env, &key_token).transfer(&contract, &buyer, &key_amount);
        if protocol_fee > 0 {
            let treasury: Address = env.storage().instance().get(&DataKey::Treasury).unwrap();
            token::Client::new(&env, &payment_token).transfer(&contract, &treasury, &protocol_fee);
        }
        if royalty_fee > 0 {
            let recipient: Address = env
                .storage()
                .instance()
                .get(&DataKey::RoyaltyRecipient)
                .unwrap();
            token::Client::new(&env, &payment_token).transfer(&contract, &recipient, &royalty_fee);
        }

        env.events().publish(
            (
                Symbol::new(&env, RESERVATION_CONVERTED_EVENT),
                buyer,
                key_id,
            ),
            ReservationEvent {
                quantity: reservation.quantity,
                amount: due,
            },
        );
        Ok(due)
    }

    /// Refunds unconverted deposits after the priority window closes.
    pub fn refund_unconverted(
        env: Env,
        buyer: Address,
        key_id: Address,
    ) -> Result<i128, ReservationError> {
        Self::require_initialized(&env)?;
        buyer.require_auth();
        let key = DataKey::Sale(key_id.clone());
        let mut sale: SaleConfig = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ReservationError::SaleNotFound)?;
        if !sale.launch_configured || env.ledger().sequence() < sale.public_open_ledger {
            return Err(ReservationError::ConversionWindowClosed);
        }
        let reservation_key = DataKey::Reservation(buyer.clone(), key_id.clone());
        let reservation: Reservation = env
            .storage()
            .persistent()
            .get(&reservation_key)
            .ok_or(ReservationError::NoReservation)?;

        env.storage().persistent().remove(&reservation_key);
        sale.total_reserved -= reservation.quantity;
        env.storage().persistent().set(&key, &sale);

        let payment_token: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .unwrap();
        token::Client::new(&env, &payment_token).transfer(
            &env.current_contract_address(),
            &buyer,
            &reservation.deposit,
        );

        env.events().publish(
            (Symbol::new(&env, RESERVATION_REFUNDED_EVENT), buyer, key_id),
            ReservationEvent {
                quantity: reservation.quantity,
                amount: reservation.deposit,
            },
        );
        Ok(reservation.deposit)
    }

    /// Public purchases only become available after the priority window.
    pub fn public_buy(
        env: Env,
        buyer: Address,
        key_id: Address,
        quantity: u32,
    ) -> Result<i128, ReservationError> {
        Self::require_initialized(&env)?;
        buyer.require_auth();
        if quantity == 0 {
            return Err(ReservationError::InvalidAmount);
        }
        let key = DataKey::Sale(key_id);
        let mut sale: SaleConfig = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ReservationError::SaleNotFound)?;
        if !sale.launch_configured || env.ledger().sequence() < sale.public_open_ledger {
            return Err(ReservationError::ConversionNotOpen);
        }
        if sale.available_keys < quantity as i128 {
            return Err(ReservationError::InsufficientKeys);
        }
        let subtotal = (quantity as i128)
            .checked_mul(sale.launch_price)
            .ok_or(ReservationError::ArithmeticOverflow)?;
        let protocol_fee = Self::fee(subtotal, sale.protocol_fee_bps)?;
        let royalty_fee = Self::fee(subtotal, sale.royalty_bps)?;
        let gross = subtotal
            .checked_add(protocol_fee)
            .and_then(|v| v.checked_add(royalty_fee))
            .ok_or(ReservationError::ArithmeticOverflow)?;

        sale.available_keys -= quantity as i128;
        sale.proceeds = sale
            .proceeds
            .checked_add(subtotal)
            .ok_or(ReservationError::ArithmeticOverflow)?;
        env.storage().persistent().set(&key, &sale);

        let payment_token: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .unwrap();
        let key_token: Address = env.storage().instance().get(&DataKey::KeyToken).unwrap();
        let contract = env.current_contract_address();
        let key_amount = quantity as i128;
        token::Client::new(&env, &payment_token).transfer(&buyer, &contract, &gross);
        token::Client::new(&env, &key_token).transfer(&contract, &buyer, &key_amount);
        if protocol_fee > 0 {
            let treasury: Address = env.storage().instance().get(&DataKey::Treasury).unwrap();
            token::Client::new(&env, &payment_token).transfer(&contract, &treasury, &protocol_fee);
        }
        if royalty_fee > 0 {
            let recipient: Address = env
                .storage()
                .instance()
                .get(&DataKey::RoyaltyRecipient)
                .unwrap();
            token::Client::new(&env, &payment_token).transfer(&contract, &recipient, &royalty_fee);
        }
        Ok(gross)
    }

    /// Creator withdraws sale proceeds held after fees are paid out.
    pub fn withdraw_proceeds(
        env: Env,
        creator: Address,
        key_id: Address,
    ) -> Result<i128, ReservationError> {
        Self::require_initialized(&env)?;
        let key = DataKey::Sale(key_id.clone());
        let mut sale: SaleConfig = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ReservationError::SaleNotFound)?;
        if creator != sale.creator {
            return Err(ReservationError::Unauthorized);
        }
        creator.require_auth();
        let amount = sale.proceeds;
        sale.proceeds = 0;
        env.storage().persistent().set(&key, &sale);
        let payment_token: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .unwrap();
        token::Client::new(&env, &payment_token).transfer(
            &env.current_contract_address(),
            &creator,
            &amount,
        );
        Ok(amount)
    }

    pub fn get_reservation(env: Env, buyer: Address, key_id: Address) -> Option<Reservation> {
        env.storage()
            .persistent()
            .get(&DataKey::Reservation(buyer, key_id))
    }

    pub fn get_sale(env: Env, key_id: Address) -> Option<SaleConfig> {
        env.storage().persistent().get(&DataKey::Sale(key_id))
    }

    fn fee(amount: i128, bps: u32) -> Result<i128, ReservationError> {
        amount
            .checked_mul(bps as i128)
            .and_then(|v| v.checked_div(BPS_DENOMINATOR))
            .ok_or(ReservationError::ArithmeticOverflow)
    }

    fn require_initialized(env: &Env) -> Result<(), ReservationError> {
        if env.storage().instance().has(&DataKey::Admin) {
            Ok(())
        } else {
            Err(ReservationError::NotInitialized)
        }
    }
}
