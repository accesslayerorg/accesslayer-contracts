#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Env, Symbol,
};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Admin,
    Operator,
    FeeCollector,
    Pauser,
    Upgrader,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateType {
    Holder,
    Subscriber,
    Threshold,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateConfig {
    pub gate_type: GateType,
    pub min_balance: i128,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    Role(Address, Role),
    Creator(u64),
    Gate(u64),
    Balance(Address),
    Subscription(Address, u64),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AccessControlError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    KeyNotFound = 4,
    GateNotConfigured = 5,
    InvalidParams = 6,
}

#[contract]
pub struct AccessControlContract;

fn require_admin(env: &Env, caller: &Address) -> Result<(), AccessControlError> {
    caller.require_auth();
    if !env
        .storage()
        .instance()
        .get(&DataKey::Role(caller.clone(), Role::Admin))
        .unwrap_or(false)
    {
        return Err(AccessControlError::Unauthorized);
    }
    Ok(())
}

fn wallet_balance(env: &Env, wallet: &Address) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::Balance(wallet.clone()))
        .unwrap_or(0)
}

fn is_subscribed(env: &Env, wallet: &Address, key_id: u64) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::Subscription(wallet.clone(), key_id))
        .unwrap_or(false)
}

#[contractimpl]
impl AccessControlContract {
    /// Set `admin` to the multisig contract address to require its threshold for
    /// every subsequent role change.
    pub fn initialize(env: Env, admin: Address) -> Result<(), AccessControlError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(AccessControlError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::Role(admin.clone(), Role::Admin), &true);
        env.events().publish(
            (Symbol::new(&env, "RoleGranted"), admin.clone()),
            Role::Admin,
        );
        Ok(())
    }

    pub fn grant_role(
        env: Env,
        caller: Address,
        account: Address,
        role: Role,
    ) -> Result<(), AccessControlError> {
        require_admin(&env, &caller)?;
        let key = DataKey::Role(account.clone(), role);
        if !env.storage().instance().get(&key).unwrap_or(false) {
            env.storage().instance().set(&key, &true);
            env.events()
                .publish((Symbol::new(&env, "RoleGranted"), account), role);
        }
        Ok(())
    }

    pub fn revoke_role(
        env: Env,
        caller: Address,
        account: Address,
        role: Role,
    ) -> Result<(), AccessControlError> {
        require_admin(&env, &caller)?;
        let key = DataKey::Role(account.clone(), role);
        if env.storage().instance().get(&key).unwrap_or(false) {
            env.storage().instance().remove(&key);
            env.events()
                .publish((Symbol::new(&env, "RoleRevoked"), account), role);
        }
        Ok(())
    }

    pub fn has_role(env: Env, account: Address, role: Role) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Role(account, role))
            .unwrap_or(false)
    }

    pub fn get_admin(env: Env) -> Result<Address, AccessControlError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(AccessControlError::NotInitialized)
    }

    /// Register a content key owned by `creator`. Creator is the only address
    /// allowed to configure the gate for that key.
    pub fn register_key(
        env: Env,
        creator: Address,
        key_id: u64,
    ) -> Result<(), AccessControlError> {
        creator.require_auth();
        if env.storage().instance().has(&DataKey::Creator(key_id)) {
            return Err(AccessControlError::InvalidParams);
        }
        env.storage()
            .instance()
            .set(&DataKey::Creator(key_id), &creator);
        Ok(())
    }

    /// Creator-only. Configures the access gate for `key_id`.
    pub fn set_gate(
        env: Env,
        creator: Address,
        key_id: u64,
        gate_type: GateType,
        min_balance: i128,
    ) -> Result<(), AccessControlError> {
        creator.require_auth();
        let owner: Address = env
            .storage()
            .instance()
            .get(&DataKey::Creator(key_id))
            .ok_or(AccessControlError::KeyNotFound)?;
        if owner != creator {
            return Err(AccessControlError::Unauthorized);
        }
        if gate_type == GateType::Threshold && min_balance <= 0 {
            return Err(AccessControlError::InvalidParams);
        }
        let config = GateConfig {
            gate_type,
            min_balance,
        };
        env.storage()
            .instance()
            .set(&DataKey::Gate(key_id), &config);
        Ok(())
    }

    /// Returns the current gate configuration for `key_id`.
    pub fn get_gate_config(env: Env, key_id: u64) -> Result<GateConfig, AccessControlError> {
        env.storage()
            .instance()
            .get(&DataKey::Gate(key_id))
            .ok_or(AccessControlError::GateNotConfigured)
    }

    /// Records a wallet's token balance (fed by the token layer).
    pub fn set_balance(
        env: Env,
        admin: Address,
        wallet: Address,
        amount: i128,
    ) -> Result<(), AccessControlError> {
        require_admin(&env, &admin)?;
        env.storage()
            .instance()
            .set(&DataKey::Balance(wallet), &amount);
        Ok(())
    }

    /// Records a wallet's subscription status for `key_id` (fed by the
    /// subscription layer).
    pub fn set_subscription(
        env: Env,
        admin: Address,
        wallet: Address,
        key_id: u64,
        active: bool,
    ) -> Result<(), AccessControlError> {
        require_admin(&env, &admin)?;
        env.storage()
            .instance()
            .set(&DataKey::Subscription(wallet, key_id), &active);
        Ok(())
    }

    /// Checks whether `wallet` may access `key_id` under `gate_type`, emitting
    /// `AccessGranted` or `AccessDenied` with wallet, key, and gate type.
    pub fn check_access(env: Env, wallet: Address, key_id: u64, gate_type: GateType) -> bool {
        let allowed = match gate_type {
            GateType::Holder => wallet_balance(&env, &wallet) > 0,
            GateType::Subscriber => is_subscribed(&env, &wallet, key_id),
            GateType::Threshold => {
                let min = env
                    .storage()
                    .instance()
                    .get::<DataKey, GateConfig>(&DataKey::Gate(key_id))
                    .map(|c| c.min_balance)
                    .unwrap_or(0);
                let min = if min > 0 { min } else { 1 };
                wallet_balance(&env, &wallet) >= min
            }
        };

        if allowed {
            env.events().publish(
                (Symbol::new(&env, "AccessGranted"), wallet, key_id),
                gate_type,
            );
        } else {
            env.events().publish(
                (Symbol::new(&env, "AccessDenied"), wallet, key_id),
                gate_type,
            );
        }
        allowed
    }
}

#[cfg(test)]
mod test {
    use super::{
        AccessControlContract, AccessControlContractClient, AccessControlError, GateType, Role,
    };
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[test]
    fn grants_and_revokes_each_role() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let user = Address::generate(&env);
        let contract_id = env.register(AccessControlContract, ());
        let client = AccessControlContractClient::new(&env, &contract_id);
        env.mock_all_auths();
        client.initialize(&admin);

        let roles = [
            Role::Admin,
            Role::Operator,
            Role::FeeCollector,
            Role::Pauser,
            Role::Upgrader,
        ];
        for role in roles {
            client.grant_role(&admin, &user, &role);
            assert!(client.has_role(&user, &role));
            client.revoke_role(&admin, &user, &role);
            assert!(!client.has_role(&user, &role));
        }
    }

    #[test]
    fn non_admin_cannot_change_roles() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let user = Address::generate(&env);
        let target = Address::generate(&env);
        let contract_id = env.register(AccessControlContract, ());
        let client = AccessControlContractClient::new(&env, &contract_id);
        env.mock_all_auths();
        client.initialize(&admin);
        assert_eq!(
            client.try_grant_role(&user, &target, &Role::Pauser),
            Err(Ok(AccessControlError::Unauthorized))
        );
    }

    #[test]
    fn holder_gate_follows_balance() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let wallet = Address::generate(&env);
        let contract_id = env.register(AccessControlContract, ());
        let client = AccessControlContractClient::new(&env, &contract_id);
        env.mock_all_auths();
        client.initialize(&admin);

        assert!(!client.check_access(&wallet, &1, &GateType::Holder));
        client.set_balance(&admin, &wallet, &5);
        assert!(client.check_access(&wallet, &1, &GateType::Holder));
    }

    #[test]
    fn subscriber_gate_follows_subscription() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let wallet = Address::generate(&env);
        let contract_id = env.register(AccessControlContract, ());
        let client = AccessControlContractClient::new(&env, &contract_id);
        env.mock_all_auths();
        client.initialize(&admin);
        client.register_key(&creator, &7);
        client.set_gate(&creator, &7, &GateType::Subscriber, &0);

        assert!(!client.check_access(&wallet, &7, &GateType::Subscriber));
        client.set_subscription(&admin, &wallet, &7, &true);
        assert!(client.check_access(&wallet, &7, &GateType::Subscriber));
    }

    #[test]
    fn threshold_gate_respects_min_balance() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let wallet = Address::generate(&env);
        let contract_id = env.register(AccessControlContract, ());
        let client = AccessControlContractClient::new(&env, &contract_id);
        env.mock_all_auths();
        client.initialize(&admin);
        client.register_key(&creator, &3);
        client.set_gate(&creator, &3, &GateType::Threshold, &100);
        client.set_balance(&admin, &wallet, &99);
        assert!(!client.check_access(&wallet, &3, &GateType::Threshold));
        client.set_balance(&admin, &wallet, &100);
        assert!(client.check_access(&wallet, &3, &GateType::Threshold));
    }

    #[test]
    fn only_creator_can_set_gate() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let other = Address::generate(&env);
        let contract_id = env.register(AccessControlContract, ());
        let client = AccessControlContractClient::new(&env, &contract_id);
        env.mock_all_auths();
        client.initialize(&admin);
        client.register_key(&creator, &9);
        assert_eq!(
            client.try_set_gate(&other, &9, &GateType::Holder, &0),
            Err(Ok(AccessControlError::Unauthorized))
        );
    }
}
