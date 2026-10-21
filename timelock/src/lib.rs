#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Bytes, Env, Symbol, Vec,
};

/// Default minimum delay: 48 hours (in seconds).
pub const DEFAULT_MIN_DELAY: u64 = 48 * 60 * 60;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    MinDelay,
    NextActionId,
    ActiveIds,
    Action(u64),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedAction {
    pub id: u64,
    pub action_type: Symbol,
    pub params: Bytes,
    pub eta: u64,
    pub queued_at: u64,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TimelockError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    MinDelayNotMet = 4,
    TimeRemaining = 5,
    ActionNotFound = 6,
    InvalidDelay = 7,
}

#[contract]
pub struct TimelockContract;

#[contractimpl]
impl TimelockContract {
    pub fn initialize(env: Env, admin: Address, min_delay: u64) -> Result<(), TimelockError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(TimelockError::AlreadyInitialized);
        }
        admin.require_auth();

        let delay = if min_delay == 0 {
            DEFAULT_MIN_DELAY
        } else {
            min_delay
        };

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::MinDelay, &delay);
        env.storage().instance().set(&DataKey::NextActionId, &0u64);
        env.storage()
            .persistent()
            .set(&DataKey::ActiveIds, &Vec::<u64>::new(&env));

        Ok(())
    }

    pub fn set_min_delay(env: Env, new_delay: u64) -> Result<(), TimelockError> {
        Self::require_admin_auth(&env)?;
        if new_delay == 0 {
            return Err(TimelockError::InvalidDelay);
        }
        env.storage().instance().set(&DataKey::MinDelay, &new_delay);
        Ok(())
    }

    pub fn admin(env: Env) -> Result<Address, TimelockError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(TimelockError::NotInitialized)
    }

    pub fn min_delay(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::MinDelay)
            .unwrap_or(DEFAULT_MIN_DELAY)
    }

    pub fn get_queued_actions(env: Env) -> Vec<QueuedAction> {
        let ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::ActiveIds)
            .unwrap_or(Vec::new(&env));

        let mut out = Vec::new(&env);
        for id in ids.iter() {
            if let Some(action) = env
                .storage()
                .persistent()
                .get::<DataKey, QueuedAction>(&DataKey::Action(id))
            {
                out.push_back(action);
            }
        }
        out
    }

    pub fn get_action(env: Env, action_id: u64) -> Result<QueuedAction, TimelockError> {
        env.storage()
            .persistent()
            .get(&DataKey::Action(action_id))
            .ok_or(TimelockError::ActionNotFound)
    }

    pub fn time_remaining(env: Env, action_id: u64) -> Result<u64, TimelockError> {
        let action = Self::get_action(env.clone(), action_id)?;
        Ok(action.eta.saturating_sub(env.ledger().timestamp()))
    }

    pub fn queue_action(
        env: Env,
        action_type: Symbol,
        params: Bytes,
        eta: u64,
    ) -> Result<u64, TimelockError> {
        Self::require_admin_auth(&env)?;

        let now = env.ledger().timestamp();
        let min_delay = Self::min_delay(env.clone());

        if eta < now.saturating_add(min_delay) {
            return Err(TimelockError::MinDelayNotMet);
        }

        let id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextActionId)
            .unwrap_or(0);

        let action = QueuedAction {
            id,
            action_type: action_type.clone(),
            params: params.clone(),
            eta,
            queued_at: now,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Action(id), &action);

        let mut ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::ActiveIds)
            .unwrap_or(Vec::new(&env));
        ids.push_back(id);
        env.storage().persistent().set(&DataKey::ActiveIds, &ids);

        env.storage()
            .instance()
            .set(&DataKey::NextActionId, &(id + 1));

        env.events().publish(
            (Symbol::new(&env, "ActionQueued"), id),
            (action_type, params, eta, now),
        );

        Ok(id)
    }

    pub fn execute_action(env: Env, action_id: u64) -> Result<(), TimelockError> {
        Self::require_admin_auth(&env)?;

        let action: QueuedAction = env
            .storage()
            .persistent()
            .get(&DataKey::Action(action_id))
            .ok_or(TimelockError::ActionNotFound)?;

        let now = env.ledger().timestamp();
        if now < action.eta {
            return Err(TimelockError::TimeRemaining);
        }

        env.storage()
            .persistent()
            .remove(&DataKey::Action(action_id));
        Self::remove_active(&env, action_id);

        env.events().publish(
            (Symbol::new(&env, "ActionExecuted"), action_id),
            (action.action_type, action.params, action.eta, now),
        );

        Ok(())
    }

    pub fn cancel_action(env: Env, action_id: u64) -> Result<(), TimelockError> {
        Self::require_admin_auth(&env)?;

        let action: QueuedAction = env
            .storage()
            .persistent()
            .get(&DataKey::Action(action_id))
            .ok_or(TimelockError::ActionNotFound)?;

        env.storage()
            .persistent()
            .remove(&DataKey::Action(action_id));
        Self::remove_active(&env, action_id);

        env.events().publish(
            (Symbol::new(&env, "ActionCancelled"), action_id),
            (action.action_type, action.eta),
        );

        Ok(())
    }

    fn require_admin_auth(env: &Env) -> Result<(), TimelockError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(TimelockError::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }

    fn remove_active(env: &Env, id: u64) {
        let ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::ActiveIds)
            .unwrap_or(Vec::new(env));
        let mut out = Vec::new(env);
        for existing in ids.iter() {
            if existing != id {
                out.push_back(existing);
            }
        }
        env.storage().persistent().set(&DataKey::ActiveIds, &out);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};
    use soroban_sdk::{Bytes, Env, Symbol};

    const ONE_HOUR: u64 = 60 * 60;
    const MIN_DELAY: u64 = 48 * ONE_HOUR;

    fn setup(env: &Env) -> (Address, TimelockContractClient<'_>) {
        env.mock_all_auths();
        let admin = Address::generate(env);
        let cid = env.register(TimelockContract, ());
        let client = TimelockContractClient::new(env, &cid);
        client.initialize(&admin, &0);
        (admin, client)
    }

    fn params(env: &Env, s: &str) -> Bytes {
        Bytes::from_slice(env, s.as_bytes())
    }

    #[test]
    fn default_min_delay_is_48_hours() {
        let env = Env::default();
        let (_admin, client) = setup(&env);
        assert_eq!(client.min_delay(), MIN_DELAY);
    }

    #[test]
    fn queue_enforces_minimum_delay() {
        let env = Env::default();
        let (_admin, client) = setup(&env);
        env.ledger().set_timestamp(1_000);

        let at = Symbol::new(&env, "upgrade");
        let eta_too_soon = 1_000 + MIN_DELAY - 1;
        assert!(client
            .try_queue_action(&at, &params(&env, "x"), &eta_too_soon)
            .is_err());

        let eta_ok = 1_000 + MIN_DELAY;
        let id = client.queue_action(&at, &params(&env, "x"), &eta_ok);
        assert_eq!(id, 0);
    }

    #[test]
    fn execute_blocked_before_eta() {
        let env = Env::default();
        let (_admin, client) = setup(&env);
        env.ledger().set_timestamp(5_000);

        let eta = 5_000 + MIN_DELAY;
        let id = client.queue_action(&Symbol::new(&env, "upgrade"), &params(&env, "v2"), &eta);

        assert!(client.try_execute_action(&id).is_err());

        env.ledger().set_timestamp(eta - 1);
        assert!(client.try_execute_action(&id).is_err());

        env.ledger().set_timestamp(eta);
        client.execute_action(&id);

        assert_eq!(client.get_queued_actions().len(), 0);
    }

    #[test]
    fn time_remaining_reports_seconds_left() {
        let env = Env::default();
        let (_admin, client) = setup(&env);
        env.ledger().set_timestamp(10_000);
        let eta = 10_000 + MIN_DELAY;
        let id = client.queue_action(&Symbol::new(&env, "upgrade"), &params(&env, "v2"), &eta);
        assert_eq!(client.time_remaining(&id), MIN_DELAY);
        env.ledger().set_timestamp(eta - 60);
        assert_eq!(client.time_remaining(&id), 60);
        env.ledger().set_timestamp(eta + 5);
        assert_eq!(client.time_remaining(&id), 0);
    }

    #[test]
    fn cancel_removes_action() {
        let env = Env::default();
        let (_admin, client) = setup(&env);
        env.ledger().set_timestamp(1_000);
        let eta = 1_000 + MIN_DELAY;
        let id = client.queue_action(&Symbol::new(&env, "upgrade"), &params(&env, "v2"), &eta);
        assert_eq!(client.get_queued_actions().len(), 1);

        client.cancel_action(&id);
        assert_eq!(client.get_queued_actions().len(), 0);
        assert!(client.try_get_action(&id).is_err());
    }

    #[test]
    fn cancel_unauthorized_fails() {
        let env = Env::default();
        let (_admin, client) = setup(&env);
        env.ledger().set_timestamp(1_000);
        let id = client.queue_action(
            &Symbol::new(&env, "upgrade"),
            &params(&env, "v2"),
            &(1_000 + MIN_DELAY),
        );

        env.set_auths(&[]);
        assert!(client.try_cancel_action(&id).is_err());
    }

    #[test]
    fn get_queued_actions_returns_params_and_eta() {
        let env = Env::default();
        let (_admin, client) = setup(&env);
        env.ledger().set_timestamp(1_000);

        let a = Symbol::new(&env, "upgrade");
        let b = Symbol::new(&env, "set_fee");
        let eta_a = 1_000 + MIN_DELAY;
        let eta_b = 1_000 + MIN_DELAY + 3600;

        let id_a = client.queue_action(&a, &params(&env, "v2"), &eta_a);
        let id_b = client.queue_action(&b, &params(&env, "100"), &eta_b);

        let queued = client.get_queued_actions();
        assert_eq!(queued.len(), 2);
        assert_eq!(queued.get(0).unwrap().id, id_a);
        assert_eq!(queued.get(0).unwrap().eta, eta_a);
        assert_eq!(queued.get(1).unwrap().id, id_b);
        assert_eq!(queued.get(1).unwrap().eta, eta_b);
    }

    #[test]
    fn execute_unknown_action_fails() {
        let env = Env::default();
        let (_admin, client) = setup(&env);
        assert!(client.try_execute_action(&99).is_err());
    }

    #[test]
    fn set_min_delay_admin_only() {
        let env = Env::default();
        let (_admin, client) = setup(&env);
        client.set_min_delay(&(24 * ONE_HOUR));
        assert_eq!(client.min_delay(), 24 * ONE_HOUR);

        env.set_auths(&[]);
        assert!(client.try_set_min_delay(&ONE_HOUR).is_err());
    }
}
