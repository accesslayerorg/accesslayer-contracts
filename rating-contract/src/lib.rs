#![no_std]

use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, Address, Env, Symbol, Vec,
};

#[contractclient(name = "CreatorKeysClient")]
pub trait CreatorKeys {
    fn get_balance(env: Env, creator: Address, wallet: Address) -> u32;
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AggregateRating {
    pub average_score_scaled: u32,
    pub total_ratings: u32,
    /// Number of ratings at scores 1 through 5, respectively.
    pub distribution: Vec<u32>,
    pub valid: bool,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    CreatorKeys,
    Total(Address),
    Count(Address),
    Distribution(Address),
    Rating(Address, Address),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RatingError {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    InvalidScore = 3,
    NotAHolder = 4,
    AlreadyRated = 5,
    NotRated = 6,
    ArithmeticOverflow = 7,
}

#[contract]
pub struct RatingContract;

fn empty_distribution(env: &Env) -> Vec<u32> {
    Vec::from_array(env, [0; 5])
}

fn read_distribution(env: &Env, key_id: &Address) -> Vec<u32> {
    env.storage()
        .persistent()
        .get(&DataKey::Distribution(key_id.clone()))
        .unwrap_or_else(|| empty_distribution(env))
}

fn change_rating(
    env: &Env,
    key_id: &Address,
    holder: &Address,
    score: u32,
    is_update: bool,
) -> Result<(), RatingError> {
    holder.require_auth();
    if !(1..=5).contains(&score) {
        return Err(RatingError::InvalidScore);
    }
    let keys_contract: Address = env
        .storage()
        .instance()
        .get(&DataKey::CreatorKeys)
        .ok_or(RatingError::Unauthorized)?;
    if CreatorKeysClient::new(env, &keys_contract).get_balance(key_id, holder) == 0 {
        return Err(RatingError::NotAHolder);
    }

    let rating_key = DataKey::Rating(key_id.clone(), holder.clone());
    let previous: Option<u32> = env.storage().persistent().get(&rating_key);
    if is_update && previous.is_none() {
        return Err(RatingError::NotRated);
    }
    if !is_update && previous.is_some() {
        return Err(RatingError::AlreadyRated);
    }

    let old_total: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::Total(key_id.clone()))
        .unwrap_or(0);
    let old_count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::Count(key_id.clone()))
        .unwrap_or(0);
    let mut distribution = read_distribution(env, key_id);
    if let Some(previous_score) = previous {
        let index = (previous_score - 1) as u32;
        let current = distribution.get(index).unwrap_or(0);
        distribution.set(
            index,
            current
                .checked_sub(1)
                .ok_or(RatingError::ArithmeticOverflow)?,
        );
    }
    let index = (score - 1) as u32;
    let current = distribution.get(index).unwrap_or(0);
    distribution.set(
        index,
        current
            .checked_add(1)
            .ok_or(RatingError::ArithmeticOverflow)?,
    );
    let total = if let Some(previous_score) = previous {
        old_total
            .checked_sub(previous_score as u64)
            .and_then(|value| value.checked_add(score as u64))
            .ok_or(RatingError::ArithmeticOverflow)?
    } else {
        old_total
            .checked_add(score as u64)
            .ok_or(RatingError::ArithmeticOverflow)?
    };
    let count = if previous.is_some() {
        old_count
    } else {
        old_count
            .checked_add(1)
            .ok_or(RatingError::ArithmeticOverflow)?
    };
    env.storage()
        .persistent()
        .set(&DataKey::Total(key_id.clone()), &total);
    env.storage()
        .persistent()
        .set(&DataKey::Count(key_id.clone()), &count);
    env.storage()
        .persistent()
        .set(&DataKey::Distribution(key_id.clone()), &distribution);
    env.storage().persistent().set(&rating_key, &score);

    let name = if is_update {
        "RatingUpdated"
    } else {
        "RatingSubmitted"
    };
    env.events().publish(
        (Symbol::new(env, name), key_id.clone(), holder.clone()),
        score,
    );
    Ok(())
}

#[contractimpl]
impl RatingContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        creator_keys_contract: Address,
    ) -> Result<(), RatingError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(RatingError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::CreatorKeys, &creator_keys_contract);
        Ok(())
    }

    pub fn submit_rating(
        env: Env,
        key_id: Address,
        holder: Address,
        score: u32,
    ) -> Result<(), RatingError> {
        change_rating(&env, &key_id, &holder, score, false)
    }

    pub fn update_rating(
        env: Env,
        key_id: Address,
        holder: Address,
        score: u32,
    ) -> Result<(), RatingError> {
        change_rating(&env, &key_id, &holder, score, true)
    }

    pub fn get_rating(env: Env, key_id: Address, holder: Address) -> Option<u32> {
        env.storage()
            .persistent()
            .get(&DataKey::Rating(key_id, holder))
    }

    pub fn get_aggregate_rating(env: Env, key_id: Address) -> AggregateRating {
        let total: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Total(key_id.clone()))
            .unwrap_or(0);
        let count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Count(key_id.clone()))
            .unwrap_or(0);
        AggregateRating {
            average_score_scaled: if count == 0 {
                0
            } else {
                ((total * 100) / count as u64) as u32
            },
            total_ratings: count,
            distribution: read_distribution(&env, &key_id),
            valid: count >= 5,
        }
    }
}

#[cfg(test)]
mod test {
    use super::{RatingContract, RatingContractClient, RatingError};
    use soroban_sdk::{contract, contractimpl, testutils::Address as _, Address, Env};

    #[contract]
    struct MockCreatorKeys;

    #[contractimpl]
    impl MockCreatorKeys {
        pub fn get_balance(_env: Env, _creator: Address, _wallet: Address) -> u32 {
            1
        }
    }

    #[test]
    fn one_rating_per_holder_and_updates_recalculate_aggregate() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let holder = Address::generate(&env);
        let keys_id = env.register(MockCreatorKeys, ());
        let rating_id = env.register(RatingContract, ());
        let client = RatingContractClient::new(&env, &rating_id);
        client.initialize(&admin, &keys_id);

        client.submit_rating(&creator, &holder, &5);
        assert_eq!(client.get_rating(&creator, &holder), Some(5));
        assert_eq!(
            client.try_submit_rating(&creator, &holder, &4),
            Err(Ok(RatingError::AlreadyRated))
        );
        let aggregate = client.get_aggregate_rating(&creator);
        assert_eq!(aggregate.total_ratings, 1);
        assert_eq!(aggregate.average_score_scaled, 500);
        assert_eq!(aggregate.distribution.get(4), Some(1));
        assert!(!aggregate.valid);

        client.update_rating(&creator, &holder, &3);
        let aggregate = client.get_aggregate_rating(&creator);
        assert_eq!(aggregate.total_ratings, 1);
        assert_eq!(aggregate.average_score_scaled, 300);
        assert_eq!(aggregate.distribution.get(2), Some(1));
        assert_eq!(aggregate.distribution.get(4), Some(0));
    }

    #[test]
    fn aggregate_becomes_valid_at_five_ratings() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let keys_id = env.register(MockCreatorKeys, ());
        let rating_id = env.register(RatingContract, ());
        let client = RatingContractClient::new(&env, &rating_id);
        client.initialize(&admin, &keys_id);
        for score in 1..=5 {
            let holder = Address::generate(&env);
            client.submit_rating(&creator, &holder, &score);
        }
        let aggregate = client.get_aggregate_rating(&creator);
        assert_eq!(aggregate.total_ratings, 5);
        assert_eq!(aggregate.average_score_scaled, 300);
        assert_eq!(
            aggregate.distribution,
            soroban_sdk::Vec::from_array(&env, [1, 1, 1, 1, 1])
        );
        assert!(aggregate.valid);
    }
}
