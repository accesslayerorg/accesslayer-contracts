//! Shared test utilities for accesslayer contract tests.
//! Provides mock contracts, test environment setup, and assertion helpers.

#![allow(dead_code)]

use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, Events as _, Ledger as _},
    Address, Bytes, Env, IntoVal, Symbol, Val,
};

// ---------------------------------------------------------------------------
// MockToken
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone)]
pub enum MockTokenDataKey {
    Balance(Address),
    Admin,
}

#[contract]
pub struct MockTokenContract;

#[contractimpl]
impl MockTokenContract {
    pub fn mint(env: Env, to: Address, amount: i128) {
        let key = MockTokenDataKey::Balance(to.clone());
        let mut balance: i128 = env.storage().instance().get(&key).unwrap_or(0);
        balance += amount;
        env.storage().instance().set(&key, &balance);
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        let from_key = MockTokenDataKey::Balance(from.clone());
        let to_key = MockTokenDataKey::Balance(to.clone());
        let mut from_balance: i128 = env.storage().instance().get(&from_key).unwrap_or(0);
        if amount > from_balance {
            panic!("Insufficient balance");
        }
        from_balance -= amount;
        env.storage().instance().set(&from_key, &from_balance);
        let mut to_balance: i128 = env.storage().instance().get(&to_key).unwrap_or(0);
        to_balance += amount;
        env.storage().instance().set(&to_key, &to_balance);
        env.events()
            .publish((Symbol::new(&env, "Transfer"), from, to), amount);
    }

    pub fn balance(env: Env, address: Address) -> i128 {
        let key = MockTokenDataKey::Balance(address);
        env.storage().instance().get(&key).unwrap_or(0)
    }
}

/// Mock ERC20-like token contract for testing.
pub struct MockToken;

impl MockToken {
    /// Register the mock token contract and return a client.
    pub fn register(env: &Env) -> (MockTokenContractClient<'_>, Address) {
        let id = env.register(MockTokenContract, ());
        let client = MockTokenContractClient::new(env, &id);
        (client, id)
    }

    /// Mint `amount` tokens to `to` address.
    pub fn mint(env: &Env, to: &Address, amount: i128) {
        let (client, _) = Self::register(env);
        client.mint(to, &amount);
    }

    /// Transfer `amount` from `from` to `to`.
    pub fn transfer(env: &Env, from: &Address, to: &Address, amount: i128) {
        let (client, _) = Self::register(env);
        client.transfer(from, to, &amount);
    }

    /// Get balance of `address`.
    pub fn balance(env: &Env, address: &Address) -> i128 {
        let (client, _) = Self::register(env);
        client.balance(address)
    }
}

// ---------------------------------------------------------------------------
// MockOracle
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone)]
pub enum MockOracleDataKey {
    Price(Symbol),
    Admin,
}

#[contract]
pub struct MockOracleContract;

#[contractimpl]
impl MockOracleContract {
    pub fn set_price(env: Env, asset: Symbol, price: i128) {
        env.storage()
            .instance()
            .set(&MockOracleDataKey::Price(asset), &price);
    }

    pub fn get_price(env: Env, asset: Symbol) -> i128 {
        env.storage()
            .instance()
            .get(&MockOracleDataKey::Price(asset))
            .unwrap_or(0)
    }
}

/// Mock price oracle for testing.
pub struct MockOracle;

impl MockOracle {
    /// Register the mock oracle contract and return a client.
    pub fn register(env: &Env) -> (MockOracleContractClient<'_>, Address) {
        let id = env.register(MockOracleContract, ());
        let client = MockOracleContractClient::new(env, &id);
        (client, id)
    }

    /// Set the price for a given asset identifier.
    pub fn set_price(env: &Env, asset: &str, price: i128) {
        let (client, _) = Self::register(env);
        client.set_price(&Symbol::new(env, asset), &price);
    }

    /// Get the current price for an asset.
    pub fn get_price(env: &Env, asset: &str) -> i128 {
        let (client, _) = Self::register(env);
        client.get_price(&Symbol::new(env, asset))
    }
}

// ---------------------------------------------------------------------------
// MockReceiver
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone)]
pub enum MockReceiverDataKey {
    Received(Address, i128),
}

#[contract]
pub struct MockReceiverContract;

#[contractimpl]
impl MockReceiverContract {
    pub fn receive(env: Env, from: Address, amount: i128, data: Bytes) {
        env.events()
            .publish((Symbol::new(&env, "Received"), from), (amount, data));
    }
}

/// Mock receiver for testing token transfers.
pub struct MockReceiver;

impl MockReceiver {
    /// Register the mock receiver contract and return a client.
    pub fn register(env: &Env) -> (MockReceiverContractClient<'_>, Address) {
        let id = env.register(MockReceiverContract, ());
        let client = MockReceiverContractClient::new(env, &id);
        (client, id)
    }

    /// Notify the receiver that tokens were sent.
    pub fn receive(env: &Env, from: &Address, amount: i128, data: Bytes) {
        let (client, _) = Self::register(env);
        client.receive(from, &amount, &data);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Helper: Advance the ledger timestamp by `n` seconds.
pub fn advance_ledgers(env: &Env, n: u64) {
    let mut ledger = env.ledger().get();
    ledger.timestamp = ledger.timestamp.saturating_add(n);
    env.ledger().set(ledger);
}

/// Helper: Set the ledger timestamp to `ts`.
pub fn set_timestamp(env: &Env, ts: u64) {
    env.ledger().set_timestamp(ts);
}

/// Helper: Fund `account` with `amount` of the mock token.
pub fn fund(env: &Env, account: &Address, amount: i128) {
    let (client, _) = MockToken::register(env);
    client.mint(account, &amount);
}

// ---------------------------------------------------------------------------
// TestEnv
// ---------------------------------------------------------------------------

/// Test environment builder that deploys mock contracts with default config.
pub struct TestEnv {
    env: &'static Env,
    pub token: MockTokenContractClient<'static>,
    pub oracle: MockOracleContractClient<'static>,
    pub receiver: MockReceiverContractClient<'static>,
    pub admin: Address,
    pub treasury: Address,
}

impl Default for TestEnv {
    /// Create a new TestEnv with default configuration.
    fn default() -> Self {
        let env = Box::leak(Box::new(Env::default()));
        env.mock_all_auths();

        let admin = Address::generate(env);
        let treasury = Address::generate(env);

        let (token, _token_id) = MockToken::register(env);
        let (oracle, _oracle_id) = MockOracle::register(env);
        let (receiver, _receiver_id) = MockReceiver::register(env);

        Self {
            env,
            token,
            oracle,
            receiver,
            admin,
            treasury,
        }
    }
}

impl TestEnv {
    /// Advance ledger by `n` seconds.
    pub fn advance_ledgers(&self, n: u64) {
        advance_ledgers(self.env, n);
    }

    /// Set ledger timestamp.
    pub fn set_timestamp(&self, ts: u64) {
        set_timestamp(self.env, ts);
    }

    /// Fund account with mock tokens.
    pub fn fund(&self, account: &Address, amount: i128) {
        self.token.mint(account, &amount);
    }

    /// Assert that an event with the given topic was emitted.
    pub fn assert_event(&self, topic: Symbol, expected_data: &[u8]) {
        assert_event_with_diff(self.env, topic, expected_data);
    }
}

// ---------------------------------------------------------------------------
// Assertion helpers
// ---------------------------------------------------------------------------

/// Assert that an event was emitted with specific topic and data.
pub fn assert_event(env: &Env, topic: Symbol, expected_data: &[u8]) {
    assert_event_with_diff(env, topic, expected_data);
}

/// Assert event with readable diff on failure.
pub fn assert_event_with_diff(env: &Env, topic: Symbol, expected_data: &[u8]) {
    let emitted = env.events().all();
    let mut found: Option<Val> = None;
    for (_, topics, data) in emitted.iter() {
        if let Some(first) = topics.get(0) {
            let sym: Symbol = first.into_val(env);
            if sym == topic {
                found = Some(data);
                break;
            }
        }
    }

    let expected_val: Val = Bytes::from_slice(env, expected_data).into_val(env);

    match found {
        Some(data) => {
            assert_eq!(
                format!("{:?}", data),
                format!("{:?}", expected_val),
                "Event data mismatch"
            );
        }
        None => {
            panic!(
                "Event with topic {:?} not found. Emitted events count: {}",
                topic,
                emitted.len()
            );
        }
    }
}
