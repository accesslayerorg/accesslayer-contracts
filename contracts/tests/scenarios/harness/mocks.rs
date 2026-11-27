//! Minimal protocol support mocks used only by the scenario suite.
//!
//! `test-utils` already provides token and oracle mocks. These two mocks add the
//! keeper trigger and an escrow with an explicit dispute/resolution flow so the
//! scenarios can exercise cross-contract lifecycle steps without production
//! changes.

#![allow(dead_code)]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};
use test_utils::{MockOracleContractClient, MockTokenContractClient};

#[contracttype]
#[derive(Clone)]
pub enum KeeperDataKey {
    Admin,
    Oracle,
    Executions,
    LastPrice(Symbol),
}

#[contract]
pub struct MockKeeperContract;

#[contractimpl]
impl MockKeeperContract {
    pub fn initialize(env: Env, admin: Address, oracle: Address) {
        env.storage().instance().set(&KeeperDataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&KeeperDataKey::Oracle, &oracle);
        env.storage()
            .instance()
            .set(&KeeperDataKey::Executions, &0u32);
    }

    pub fn record_price(env: Env, caller: Address, asset: Symbol, price: i128) {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&KeeperDataKey::Admin)
            .expect("keeper admin is not configured");
        assert!(caller == admin, "keeper caller is not the admin");

        let oracle: Address = env
            .storage()
            .instance()
            .get(&KeeperDataKey::Oracle)
            .expect("keeper oracle is not configured");
        MockOracleContractClient::new(&env, &oracle).set_price(&asset, &price);

        env.storage()
            .instance()
            .set(&KeeperDataKey::LastPrice(asset), &price);
        let executions: u32 = env
            .storage()
            .instance()
            .get(&KeeperDataKey::Executions)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&KeeperDataKey::Executions, &(executions + 1));
    }

    pub fn executions(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&KeeperDataKey::Executions)
            .unwrap_or(0)
    }

    pub fn last_price(env: Env, asset: Symbol) -> i128 {
        env.storage()
            .instance()
            .get(&KeeperDataKey::LastPrice(asset))
            .unwrap_or(0)
    }
}

#[contracttype]
#[derive(Clone)]
pub enum EscrowDataKey {
    Arbitrator,
    Token,
    Balance(Address),
    Dispute(Address),
}

#[contracttype]
#[derive(Clone)]
pub struct EscrowDispute {
    pub active: bool,
    pub raised_by: Address,
    pub reason: Symbol,
}

#[contract]
pub struct MockEscrowContract;

#[contractimpl]
impl MockEscrowContract {
    pub fn setup(env: Env, arbitrator: Address, token: Address) {
        env.storage()
            .instance()
            .set(&EscrowDataKey::Arbitrator, &arbitrator);
        env.storage().instance().set(&EscrowDataKey::Token, &token);
    }

    pub fn deposit(env: Env, creator: Address, amount: i128) {
        creator.require_auth();
        assert!(amount > 0, "escrow deposit must be positive");

        let token: Address = env
            .storage()
            .instance()
            .get(&EscrowDataKey::Token)
            .expect("escrow token is not configured");
        MockTokenContractClient::new(&env, &token).transfer(
            &creator,
            &env.current_contract_address(),
            &amount,
        );

        let balance = stored_balance(&env, &creator);
        env.storage()
            .instance()
            .set(&EscrowDataKey::Balance(creator), &(balance + amount));
    }

    pub fn balance(env: Env, creator: Address) -> i128 {
        stored_balance(&env, &creator)
    }

    pub fn is_disputed(env: Env, creator: Address) -> bool {
        env.storage()
            .instance()
            .get::<EscrowDataKey, EscrowDispute>(&EscrowDataKey::Dispute(creator))
            .map(|dispute| dispute.active)
            .unwrap_or(false)
    }

    pub fn raise_dispute(env: Env, caller: Address, creator: Address, reason: Symbol) {
        caller.require_auth();
        env.storage().instance().set(
            &EscrowDataKey::Dispute(creator),
            &EscrowDispute {
                active: true,
                raised_by: caller,
                reason,
            },
        );
    }

    pub fn resolve(env: Env, caller: Address, creator: Address, recipient: Address, amount: i128) {
        assert_arbitrator(&env, &caller);
        let key = EscrowDataKey::Dispute(creator.clone());
        let mut dispute: EscrowDispute = env
            .storage()
            .instance()
            .get(&key)
            .expect("no escrow dispute to resolve");
        assert!(dispute.active, "escrow dispute is already resolved");

        pay(&env, &creator, &recipient, amount);
        dispute.active = false;
        env.storage().instance().set(&key, &dispute);
    }

    pub fn release(env: Env, caller: Address, creator: Address, recipient: Address, amount: i128) {
        assert_arbitrator(&env, &caller);
        pay(&env, &creator, &recipient, amount);
    }
}

fn stored_balance(env: &Env, creator: &Address) -> i128 {
    env.storage()
        .instance()
        .get(&EscrowDataKey::Balance(creator.clone()))
        .unwrap_or(0)
}

fn assert_arbitrator(env: &Env, caller: &Address) {
    caller.require_auth();
    let arbitrator: Address = env
        .storage()
        .instance()
        .get(&EscrowDataKey::Arbitrator)
        .expect("escrow arbitrator is not configured");
    assert!(*caller == arbitrator, "caller is not the escrow arbitrator");
}

fn pay(env: &Env, creator: &Address, recipient: &Address, amount: i128) {
    assert!(amount > 0, "escrow payment must be positive");
    let balance = stored_balance(env, creator);
    assert!(balance >= amount, "escrow reserve is insufficient");
    let token: Address = env
        .storage()
        .instance()
        .get(&EscrowDataKey::Token)
        .expect("escrow token is not configured");
    MockTokenContractClient::new(env, &token).transfer(
        &env.current_contract_address(),
        recipient,
        &amount,
    );
    env.storage().instance().set(
        &EscrowDataKey::Balance(creator.clone()),
        &(balance - amount),
    );
}
