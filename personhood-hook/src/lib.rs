#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Bytes, BytesN, Env,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersonAttestationData {
    pub person_id: BytesN<32>,
    pub wallet: Address,
    pub valid_until: u64,
}

#[contracttype]
enum DataKey {
    Admin,
    PersonhoodProvider,
    WalletToPerson(Address),
    PersonToWallet(BytesN<32>),
    PersonWalletCount(BytesN<32>),
    MaxWalletsPerPerson,
    UnlinkCooldown,
    LastUnlink(Address),
    KeyRequiresPersonhood(Address),
    PersonPurchases(BytesN<32>, Address),
    UnverifiedCap(Address),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PersonhoodError {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    InvalidAttestation = 3,
    WalletAlreadyLinked = 4,
    MaxWalletsReached = 5,
    CooldownActive = 6,
    UnverifiedWallet = 7,
    CapExceeded = 8,
    AttestationExpired = 9,
    ArithmeticOverflow = 10,
}

#[contract]
pub struct PersonhoodHookContract;

fn require_admin(env: &Env, caller: &Address) -> Result<(), PersonhoodError> {
    caller.require_auth();
    if env.storage().instance().get::<_, Address>(&DataKey::Admin) != Some(caller.clone()) {
        return Err(PersonhoodError::Unauthorized);
    }
    Ok(())
}

#[contractimpl]
impl PersonhoodHookContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        max_wallets_per_person: u32,
        unlink_cooldown: u64,
    ) -> Result<(), PersonhoodError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PersonhoodError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::MaxWalletsPerPerson, &max_wallets_per_person);
        env.storage()
            .instance()
            .set(&DataKey::UnlinkCooldown, &unlink_cooldown);
        Ok(())
    }

    pub fn set_personhood_provider(
        env: Env,
        admin: Address,
        attestor: Address,
    ) -> Result<(), PersonhoodError> {
        require_admin(&env, &admin)?;
        env.storage()
            .instance()
            .set(&DataKey::PersonhoodProvider, &attestor);
        Ok(())
    }

    pub fn link_person(
        env: Env,
        wallet: Address,
        person_id: BytesN<32>,
        valid_until: u64,
        signature: Bytes,
    ) -> Result<(), PersonhoodError> {
        wallet.require_auth();

        // Verify attestation hasn't expired
        let now = env.ledger().timestamp();
        if valid_until <= now {
            return Err(PersonhoodError::AttestationExpired);
        }

        // Verify signature (simplified - in reality would verify against attestor)
        if signature.is_empty() {
            return Err(PersonhoodError::InvalidAttestation);
        }

        // Check if wallet is already linked
        if env
            .storage()
            .persistent()
            .has(&DataKey::WalletToPerson(wallet.clone()))
        {
            return Err(PersonhoodError::WalletAlreadyLinked);
        }

        // Check person wallet count
        let max_wallets: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MaxWalletsPerPerson)
            .unwrap_or(1);
        let wallet_count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::PersonWalletCount(person_id.clone()))
            .unwrap_or(0);

        if wallet_count >= max_wallets {
            return Err(PersonhoodError::MaxWalletsReached);
        }

        // Link wallet to person
        env.storage()
            .persistent()
            .set(&DataKey::WalletToPerson(wallet.clone()), &person_id);
        env.storage()
            .persistent()
            .set(&DataKey::PersonToWallet(person_id.clone()), &wallet);

        let new_count = wallet_count
            .checked_add(1)
            .ok_or(PersonhoodError::ArithmeticOverflow)?;
        env.storage()
            .persistent()
            .set(&DataKey::PersonWalletCount(person_id.clone()), &new_count);

        env.events()
            .publish(("person_linked", wallet.clone()), person_id);

        Ok(())
    }

    pub fn unlink_person(env: Env, wallet: Address) -> Result<(), PersonhoodError> {
        wallet.require_auth();

        let person_id: BytesN<32> = env
            .storage()
            .persistent()
            .get(&DataKey::WalletToPerson(wallet.clone()))
            .ok_or(PersonhoodError::UnverifiedWallet)?;

        // Check cooldown — only applies if the wallet has previously unlinked
        let cooldown: u64 = env
            .storage()
            .instance()
            .get(&DataKey::UnlinkCooldown)
            .unwrap_or(0);
        let now = env.ledger().timestamp();
        if let Some(last_unlink) = env
            .storage()
            .persistent()
            .get::<_, u64>(&DataKey::LastUnlink(wallet.clone()))
        {
            if now < last_unlink.saturating_add(cooldown) {
                return Err(PersonhoodError::CooldownActive);
            }
        }

        // Unlink wallet
        env.storage()
            .persistent()
            .remove(&DataKey::WalletToPerson(wallet.clone()));
        env.storage()
            .persistent()
            .remove(&DataKey::PersonToWallet(person_id.clone()));

        let wallet_count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::PersonWalletCount(person_id.clone()))
            .unwrap_or(1);
        let new_count = wallet_count.saturating_sub(1);
        env.storage()
            .persistent()
            .set(&DataKey::PersonWalletCount(person_id.clone()), &new_count);

        env.storage()
            .persistent()
            .set(&DataKey::LastUnlink(wallet.clone()), &now);

        env.events()
            .publish(("person_unlinked", wallet.clone()), person_id);

        Ok(())
    }

    pub fn person_of(env: Env, wallet: Address) -> Option<BytesN<32>> {
        env.storage()
            .persistent()
            .get(&DataKey::WalletToPerson(wallet))
    }

    pub fn set_key_requires_personhood(
        env: Env,
        creator: Address,
        key_id: Address,
        required: bool,
    ) -> Result<(), PersonhoodError> {
        creator.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::KeyRequiresPersonhood(key_id), &required);
        Ok(())
    }

    pub fn set_unverified_cap(
        env: Env,
        creator: Address,
        key_id: Address,
        cap: i128,
    ) -> Result<(), PersonhoodError> {
        creator.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::UnverifiedCap(key_id), &cap);
        Ok(())
    }

    pub fn record_purchase(
        env: Env,
        wallet: Address,
        key_id: Address,
        amount: i128,
        per_person_cap: i128,
    ) -> Result<(), PersonhoodError> {
        wallet.require_auth();

        let requires_personhood: bool = env
            .storage()
            .persistent()
            .get(&DataKey::KeyRequiresPersonhood(key_id.clone()))
            .unwrap_or(false);

        if !requires_personhood {
            // No personhood required, allow purchase
            return Ok(());
        }

        let person_id_opt: Option<BytesN<32>> = env
            .storage()
            .persistent()
            .get(&DataKey::WalletToPerson(wallet.clone()));

        match person_id_opt {
            Some(person_id) => {
                // Verified wallet - check per-person cap
                let current_purchases: i128 = env
                    .storage()
                    .persistent()
                    .get(&DataKey::PersonPurchases(person_id.clone(), key_id.clone()))
                    .unwrap_or(0);

                let new_purchases = current_purchases
                    .checked_add(amount)
                    .ok_or(PersonhoodError::ArithmeticOverflow)?;

                if new_purchases > per_person_cap {
                    return Err(PersonhoodError::CapExceeded);
                }

                env.storage().persistent().set(
                    &DataKey::PersonPurchases(person_id.clone(), key_id.clone()),
                    &new_purchases,
                );

                Ok(())
            }
            None => {
                // Unverified wallet - check fallback cap or reject
                let unverified_cap: Option<i128> = env
                    .storage()
                    .persistent()
                    .get(&DataKey::UnverifiedCap(key_id.clone()));

                match unverified_cap {
                    Some(cap) if amount <= cap => Ok(()),
                    Some(_) => Err(PersonhoodError::CapExceeded),
                    None => Err(PersonhoodError::UnverifiedWallet),
                }
            }
        }
    }

    pub fn get_person_purchases(env: Env, person_id: BytesN<32>, key_id: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::PersonPurchases(person_id, key_id))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    fn setup() -> (Env, PersonhoodHookContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register(PersonhoodHookContract, ());
        let client = PersonhoodHookContractClient::new(&env, &contract_id);
        client.initialize(&admin, &1, &86400); // 1 wallet, 1 day cooldown
        (env, client, admin)
    }

    #[test]
    fn test_link_person_binds_wallet() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);
        let person_id = BytesN::from_array(&env, &[1u8; 32]);
        let valid_until = env.ledger().timestamp() + 3600;
        let signature = Bytes::from_array(&env, &[1, 2, 3, 4]);

        client.link_person(&wallet, &person_id, &valid_until, &signature);

        assert_eq!(client.person_of(&wallet), Some(person_id));
    }

    #[test]
    fn test_purchases_by_same_person_share_cap() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);
        let person_id = BytesN::from_array(&env, &[1u8; 32]);
        let valid_until = env.ledger().timestamp() + 3600;
        let signature = Bytes::from_array(&env, &[1, 2, 3, 4]);
        let key_id = Address::generate(&env);
        let creator = Address::generate(&env);

        client.link_person(&wallet, &person_id, &valid_until, &signature);
        client.set_key_requires_personhood(&creator, &key_id, &true);

        // First purchase: 60 (cap: 100)
        client.record_purchase(&wallet, &key_id, &60, &100);

        // Second purchase: 50 (total: 110) - should fail
        assert_eq!(
            client.try_record_purchase(&wallet, &key_id, &50, &100),
            Err(Ok(PersonhoodError::CapExceeded))
        );
    }

    #[test]
    fn test_cannot_bind_more_wallets_than_limit() {
        let (env, client, _admin) = setup();
        let wallet1 = Address::generate(&env);
        let wallet2 = Address::generate(&env);
        let person_id = BytesN::from_array(&env, &[1u8; 32]);
        let valid_until = env.ledger().timestamp() + 3600;
        let signature = Bytes::from_array(&env, &[1, 2, 3, 4]);

        client.link_person(&wallet1, &person_id, &valid_until, &signature);

        // Max is 1 wallet per person
        assert_eq!(
            client.try_link_person(&wallet2, &person_id, &valid_until, &signature),
            Err(Ok(PersonhoodError::MaxWalletsReached))
        );
    }

    #[test]
    fn test_unlinking_subject_to_cooldown() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);
        let person_id = BytesN::from_array(&env, &[1u8; 32]);
        let valid_until = env.ledger().timestamp() + 3600;
        let signature = Bytes::from_array(&env, &[1, 2, 3, 4]);

        client.link_person(&wallet, &person_id, &valid_until, &signature);
        client.unlink_person(&wallet);

        // Try to link again immediately (cooldown: 86400 seconds)
        let person_id_2 = BytesN::from_array(&env, &[2u8; 32]);
        client.link_person(&wallet, &person_id_2, &valid_until, &signature);

        // Try to unlink again - should fail (cooldown active)
        assert_eq!(
            client.try_unlink_person(&wallet),
            Err(Ok(PersonhoodError::CooldownActive))
        );
    }

    #[test]
    fn test_unverified_wallets_receive_fallback_cap() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);
        let key_id = Address::generate(&env);
        let creator = Address::generate(&env);

        client.set_key_requires_personhood(&creator, &key_id, &true);
        client.set_unverified_cap(&creator, &key_id, &10);

        // Purchase within fallback cap
        client.record_purchase(&wallet, &key_id, &10, &100);

        // Purchase exceeding fallback cap
        assert_eq!(
            client.try_record_purchase(&wallet, &key_id, &11, &100),
            Err(Ok(PersonhoodError::CapExceeded))
        );
    }

    #[test]
    fn test_unverified_wallets_rejected_without_fallback() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);
        let key_id = Address::generate(&env);
        let creator = Address::generate(&env);

        client.set_key_requires_personhood(&creator, &key_id, &true);
        // No fallback cap set

        assert_eq!(
            client.try_record_purchase(&wallet, &key_id, &10, &100),
            Err(Ok(PersonhoodError::UnverifiedWallet))
        );
    }

    #[test]
    fn test_invalid_attestation_reverts() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);
        let person_id = BytesN::from_array(&env, &[1u8; 32]);
        let valid_until = env.ledger().timestamp() + 3600;
        let empty_signature = Bytes::new(&env);

        assert_eq!(
            client.try_link_person(&wallet, &person_id, &valid_until, &empty_signature),
            Err(Ok(PersonhoodError::InvalidAttestation))
        );
    }

    #[test]
    fn test_expired_attestation_reverts() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);
        let person_id = BytesN::from_array(&env, &[1u8; 32]);
        let valid_until = 0u64; // Expired: timestamp 0 is never in the future
        let signature = Bytes::from_array(&env, &[1, 2, 3, 4]);

        assert_eq!(
            client.try_link_person(&wallet, &person_id, &valid_until, &signature),
            Err(Ok(PersonhoodError::AttestationExpired))
        );
    }
}
