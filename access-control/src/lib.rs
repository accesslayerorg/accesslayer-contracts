#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol};

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
#[derive(Clone)]
enum DataKey {
    Admin,
    Role(Address, Role),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AccessControlError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
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
}

#[cfg(test)]
mod test {
    use super::{AccessControlContract, AccessControlContractClient, AccessControlError, Role};
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
}
