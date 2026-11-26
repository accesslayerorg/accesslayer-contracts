use crate::{read_registered_creator_profile, ContractError};
use soroban_sdk::{contracttype, Address, Env, Symbol, Vec};

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum Role {
    Moderator,
    Distributor,
    Analyst,
    Custom(Symbol),
}

#[derive(Clone)]
#[contracttype]
enum RoleDataKey {
    Roles(Address, Address),
}

fn role_key(key_id: &Address, wallet: &Address) -> RoleDataKey {
    RoleDataKey::Roles(key_id.clone(), wallet.clone())
}

pub fn grant_role(
    env: &Env,
    key_id: &Address,
    wallet: &Address,
    role: Role,
) -> Result<(), ContractError> {
    key_id.require_auth();
    read_registered_creator_profile(env, key_id)?;
    let key = role_key(key_id, wallet);
    let mut roles: Vec<Role> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env));
    if !roles.contains(&role) {
        roles.push_back(role.clone());
        env.storage().persistent().set(&key, &roles);
        env.events().publish(
            (
                Symbol::new(env, "RoleGranted"),
                key_id.clone(),
                wallet.clone(),
            ),
            role,
        );
    }
    Ok(())
}

pub fn revoke_role(
    env: &Env,
    key_id: &Address,
    wallet: &Address,
    role: Role,
) -> Result<(), ContractError> {
    key_id.require_auth();
    read_registered_creator_profile(env, key_id)?;
    let key = role_key(key_id, wallet);
    let roles: Vec<Role> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env));
    let mut next = Vec::new(env);
    for existing in roles.iter() {
        if existing != role {
            next.push_back(existing);
        }
    }
    if next.len() != roles.len() {
        env.storage().persistent().set(&key, &next);
        env.events().publish(
            (
                Symbol::new(env, "RoleRevoked"),
                key_id.clone(),
                wallet.clone(),
            ),
            role,
        );
    }
    Ok(())
}

pub fn has_role(env: &Env, key_id: &Address, wallet: &Address, role: &Role) -> bool {
    env.storage()
        .persistent()
        .get::<_, Vec<Role>>(&role_key(key_id, wallet))
        .map(|roles| roles.contains(role))
        .unwrap_or(false)
}

pub fn require_role(
    env: &Env,
    key_id: &Address,
    wallet: &Address,
    role: &Role,
) -> Result<(), ContractError> {
    wallet.require_auth();
    if has_role(env, key_id, wallet, role) {
        Ok(())
    } else {
        Err(ContractError::Unauthorized)
    }
}
