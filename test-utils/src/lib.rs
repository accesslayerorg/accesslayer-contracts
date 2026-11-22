//! Shared test utilities for accesslayer contract tests.
//! Provides mock contracts, test environment setup, and assertion helpers.

#![allow(dead_code)]

use soroban_sdk {
    testutils::{Address as _, Events as _, Ledger as _},
    Address, Env, Symbol, Vec, Val,
};

/// Mock ERC20-like token contract for testing.
///
/// Provides basic token operations: mint, transfer, balance, etc.
/// Used as a drop-in replacement for real tokens in test suites.
pub struct MockToken;

impl MockToken {
    /// Register the mock token contract and return a client.
    pub fn register(env: &Env) -> (MockTokenClient<'_>, Address) {
        let id = env.register(MockTokenContract, ());
        let client = MockTokenClient::new(env, &id);
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
        client.balance(&address)
    }
}

/// MockOracle contract for testing price feeds.
///
/// Provides set_price and get_price operations for test price scenarios.
pub struct MockOracle;

impl MockOracle {
    /// Register the mock oracle contract and return a client.
    pub fn register(env: &Env) -> (MockOracleClient<'_>, Address) {
        let id = env.register(MockOracleContract, ());
        let client = MockOracleClient::new(env, &id);
        (client, id)
    }

    /// Set the price for a given asset identifier.
    pub fn set_price(env: &Env, asset: &str, price: i128) {
        let (client, _) = Self::register(env);
        client.set_price(asset, &price);
    }

    /// Get the current price for an asset.
    pub fn get_price(env: &Env, asset: &str) -> i128 {
        let (client, _) = Self::register(env);
        client.get_price(asset)
    }
}

/// MockReceiver contract for receiving token transfers.
///
/// Used to test contract reception of tokens in test scenarios.
pub struct MockReceiver;

impl MockReceiver {
    /// Register the mock receiver contract and return a client.
    pub fn register(env: &Env) -> (MockReceiverClient<'_>, Address) {
        let id = env.register(MockReceiverContract, ());
        let client = MockReceiverClient::new(env, &id);
        (client, id)
    }

    /// Notify the receiver that tokens were sent.
    ///
    /// `amount` is the amount received, `data` is optional arbitrary data.
    pub fn receive(env: &Env, from: &Address, amount: i128, data: Vec<u8>) {
        let (client, _) = Self::register(env);
        client.receive(from, &amount, &data);
    }
}

/// Helper: Advance the ledger timestamp by `n` seconds.
///
/// Updates the ledger timestamp by adding `n` seconds to the current timestamp.
pub fn advance_ledgers(env: &Env, n: u64) {
    let mut ledger = env.ledger().get();
    ledger.timestamp = ledger.timestamp.saturating_add(n);
    env.ledger().set(ledger);
}

/// Helper: Set the ledger timestamp to `ts`.
pub fn set_timestamp(env: &Env, ts: u64) {
    let mut ledger = env.ledger().get();
    ledger.timestamp = ts;
    env.ledger().set(ledger);
}

/// Helper: Fund `account` with `amount` of the mock token.
pub fn fund(env: &Env, account: &Address, amount: i128) {
    let (client, _) = MockToken::register(env);
    client.mint(account, &amount);
}

/// Test environment builder that deploys the protocol with default config
/// and returns typed clients for the main contracts.
///
/// This sets up a complete test environment with:
/// - Mock token deployed
/// - Mock oracle deployed
/// - Mock receiver deployed
/// - Default protocol admin configured
/// - Returns a structured environment with all clients
pub struct TestEnv {
    /// The mocked environment
    env: Env,
    /// Mock token client
    pub token: MockTokenClient<'static>,
    /// Mock oracle client
    pub oracle: MockOracleClient<'static>,
    /// Mock receiver client
    pub receiver: MockReceiverClient<'static>,
    /// Protocol admin address
    pub admin: Address,
    /// Treasury address
    pub treasury: Address,
}

impl TestEnv {
    /// Create a new TestEnv with default configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use test_utils::TestEnv;
    ///
    /// let env = TestEnv::default();
    /// let admin = env.admin;
    /// ```
    pub fn default() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let treasury = Address::generate(&env);

        let (token, _token_id) = MockToken::register(&env);
        let (oracle, _oracle_id) = MockOracle::register(&env);
        let (receiver, _receiver_id) = MockReceiver::register(&env);

        // Set default admin on contracts
        token.set_admin(&admin, &admin);
        oracle.set_admin(&admin, &admin);

        Self {
            env,
            token,
            oracle,
            receiver,
            admin,
            treasury,
        }
    }

    /// Advance ledger by `n` seconds.
    pub fn advance_ledgers(&self, n: u64) {
        advance_ledgers(&self.env, n);
    }

    /// Set ledger timestamp.
    pub fn set_timestamp(&self, ts: u64) {
        set_timestamp(&self.env, ts);
    }

    /// Fund account with mock tokens.
    pub fn fund(&self, account: &Address, amount: i128) {
        MockToken::mint(&self.env, account, amount);
    }

    /// Assert that an event with the given topic was emitted.
    ///
    /// # Example
    ///
    /// ```rust
    /// use test_utils::TestEnv;
    /// use soroban_sdk::symbol_short;
    ///
    /// let env = TestEnv::default();
    /// // ... perform some action that emits an event
    /// env.assert_event(symbol_short!("Transfer"), ...);
    /// ```
    pub fn assert_event(&self, topic: Symbol, expected_data: &[u8]) {
        let events: Vec<(&Symbol, Vec<u8>)> = self
            .env
            .events()
            .all()
            .iter()
            .filter_map(|(topics, data)| {
                topics.get(0).and_then(|first_topic| {
                    Symbol::try_from_val(self.env, first_topic)
                        .ok()
                        .map(|st| (st, data.clone()))
                })
            })
            .collect();

        let found = events
            .iter()
            .find(|(topic_val, _)| topic_val == &&topic);

        match found {
            Some((_, data)) => {
                assert_eq!(data, expected_data, "Event data mismatch");
            }
            None => {
                panic!(
                    "Event with topic {:?} not found. Emitted events: {:?}",
                    topic,
                    self.env.events().all()
                );
            }
        }
    }
}

/// Assert that an event was emitted with specific topic and data.
///
/// Provides a readable diff when the assertion fails.
pub fn assert_event_with_diff(env: &Env, topic: Symbol, expected_data: &[u8]) {
    let emitted_events: Vec<(&Symbol, Vec<u8>)> = env
        .events()
        .all()
        .iter()
        .filter_map(|(topics, data)| {
            topics.get(0).and_then(|first_topic| {
                Symbol::try_from_val(env, first_topic)
                    .ok()
                    .map(|st| (st, data.clone()))
            })
        })
        .collect();

    let found = emitted_events
        .iter()
        .find(|(topic_val, _)| topic_val == &&topic);

    match found {
        Some((_, data)) => {
            assert_eq!(data, expected_data, "Event data mismatch");
        }
        None => {
            panic!(
                "Event with topic {:?} not found. \
                 Emitted events count: {}, first topics: {:?}",
                topic,
                emitted_events.len(),
                emitted_events.iter().map(|(t, _)| t).collect::<Vec<_>>()
            );
        }
    }
}