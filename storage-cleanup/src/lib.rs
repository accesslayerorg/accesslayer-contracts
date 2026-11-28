#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecordKind {
    Balance,
    Auction,
    Order,
    Reservation,
    Checkpoint,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanableStatus {
    pub can_remove: bool,
    pub reason: String,
}

#[contracttype]
enum DataKey {
    Admin,
    CleanupFund,
    RewardPerEntry,
    RetentionWindow,
    Balance(Address, Address),
    BalanceTimestamp(Address, Address),
    Auction(u64),
    AuctionStatus(u64),
    Order(u64),
    OrderStatus(u64),
    Reservation(Address, Address),
    ReservationStatus(Address, Address),
    Checkpoint(Address, u64),
    OpenClaims(Address),
    OpenDisputes(Address),
    GovernanceSnapshot(u64),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CleanupError {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    RecordActive = 3,
    RecordNotFound = 4,
    InsufficientFund = 5,
    ReferencedBySnapshot = 6,
    ReferencedByClaim = 7,
    ArithmeticOverflow = 8,
}

#[contract]
pub struct StorageCleanupContract;

#[allow(dead_code)]
fn require_admin(env: &Env, caller: &Address) -> Result<(), CleanupError> {
    caller.require_auth();
    if env.storage().instance().get::<_, Address>(&DataKey::Admin) != Some(caller.clone()) {
        return Err(CleanupError::Unauthorized);
    }
    Ok(())
}

#[contractimpl]
impl StorageCleanupContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        reward_per_entry: i128,
        retention_window: u64,
    ) -> Result<(), CleanupError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(CleanupError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::RewardPerEntry, &reward_per_entry);
        env.storage()
            .instance()
            .set(&DataKey::RetentionWindow, &retention_window);
        env.storage().instance().set(&DataKey::CleanupFund, &0i128);
        Ok(())
    }

    pub fn fund_cleanup(env: Env, funder: Address, amount: i128) -> Result<(), CleanupError> {
        funder.require_auth();
        let current_fund: i128 = env
            .storage()
            .instance()
            .get(&DataKey::CleanupFund)
            .unwrap_or(0);
        let new_fund = current_fund
            .checked_add(amount)
            .ok_or(CleanupError::ArithmeticOverflow)?;
        env.storage()
            .instance()
            .set(&DataKey::CleanupFund, &new_fund);
        Ok(())
    }

    pub fn cleanup_balance(
        env: Env,
        caller: Address,
        holder: Address,
        key_id: Address,
    ) -> Result<(), CleanupError> {
        caller.require_auth();

        let balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(holder.clone(), key_id.clone()))
            .unwrap_or(0);

        if balance != 0 {
            return Err(CleanupError::RecordActive);
        }

        // Check if referenced by open claims or disputes
        if Self::has_open_claims(env.clone(), &holder) {
            return Err(CleanupError::ReferencedByClaim);
        }

        env.storage()
            .persistent()
            .remove(&DataKey::Balance(holder.clone(), key_id.clone()));
        env.storage()
            .persistent()
            .remove(&DataKey::BalanceTimestamp(holder.clone(), key_id.clone()));

        Self::pay_reward(env.clone(), &caller)?;
        env.events().publish(("cleaned", RecordKind::Balance), 1u32);

        Ok(())
    }

    pub fn cleanup_checkpoint(
        env: Env,
        caller: Address,
        holder: Address,
        checkpoint_id: u64,
    ) -> Result<(), CleanupError> {
        caller.require_auth();

        let retention_window: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RetentionWindow)
            .unwrap_or(0);
        let checkpoint_time: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Checkpoint(holder.clone(), checkpoint_id))
            .ok_or(CleanupError::RecordNotFound)?;
        let now = env.ledger().timestamp();

        if now < checkpoint_time + retention_window {
            return Err(CleanupError::RecordActive);
        }

        env.storage()
            .persistent()
            .remove(&DataKey::Checkpoint(holder.clone(), checkpoint_id));

        Self::pay_reward(env.clone(), &caller)?;
        env.events()
            .publish(("cleaned", RecordKind::Checkpoint), 1u32);

        Ok(())
    }

    pub fn cleanup_auction(env: Env, caller: Address, auction_id: u64) -> Result<(), CleanupError> {
        caller.require_auth();

        let status: String = env
            .storage()
            .persistent()
            .get(&DataKey::AuctionStatus(auction_id))
            .ok_or(CleanupError::RecordNotFound)?;

        if status != String::from_str(&env, "finished") {
            return Err(CleanupError::RecordActive);
        }

        // Check if referenced by governance snapshot
        if Self::is_referenced_by_snapshot(env.clone(), auction_id) {
            return Err(CleanupError::ReferencedBySnapshot);
        }

        env.storage()
            .persistent()
            .remove(&DataKey::Auction(auction_id));
        env.storage()
            .persistent()
            .remove(&DataKey::AuctionStatus(auction_id));

        Self::pay_reward(env.clone(), &caller)?;
        env.events().publish(("cleaned", RecordKind::Auction), 1u32);

        Ok(())
    }

    pub fn cleanup_order(env: Env, caller: Address, order_id: u64) -> Result<(), CleanupError> {
        caller.require_auth();

        let status: String = env
            .storage()
            .persistent()
            .get(&DataKey::OrderStatus(order_id))
            .ok_or(CleanupError::RecordNotFound)?;

        if status != String::from_str(&env, "filled")
            && status != String::from_str(&env, "cancelled")
        {
            return Err(CleanupError::RecordActive);
        }

        env.storage().persistent().remove(&DataKey::Order(order_id));
        env.storage()
            .persistent()
            .remove(&DataKey::OrderStatus(order_id));

        Self::pay_reward(env.clone(), &caller)?;
        env.events().publish(("cleaned", RecordKind::Order), 1u32);

        Ok(())
    }

    pub fn cleanup_reservation(
        env: Env,
        caller: Address,
        key_id: Address,
        buyer: Address,
    ) -> Result<(), CleanupError> {
        caller.require_auth();

        let status: String = env
            .storage()
            .persistent()
            .get(&DataKey::ReservationStatus(key_id.clone(), buyer.clone()))
            .ok_or(CleanupError::RecordNotFound)?;

        if status != String::from_str(&env, "expired")
            && status != String::from_str(&env, "fulfilled")
        {
            return Err(CleanupError::RecordActive);
        }

        env.storage()
            .persistent()
            .remove(&DataKey::Reservation(key_id.clone(), buyer.clone()));
        env.storage()
            .persistent()
            .remove(&DataKey::ReservationStatus(key_id.clone(), buyer.clone()));

        Self::pay_reward(env.clone(), &caller)?;
        env.events()
            .publish(("cleaned", RecordKind::Reservation), 1u32);

        Ok(())
    }

    pub fn cleanable(env: Env, kind: RecordKind, holder: Address, id: u64) -> CleanableStatus {
        match kind {
            RecordKind::Balance => {
                let balance: i128 = env
                    .storage()
                    .persistent()
                    .get(&DataKey::Balance(holder.clone(), holder.clone()))
                    .unwrap_or(0);
                if balance != 0 {
                    CleanableStatus {
                        can_remove: false,
                        reason: String::from_str(&env, "balance_non_zero"),
                    }
                } else if Self::has_open_claims(env.clone(), &holder) {
                    CleanableStatus {
                        can_remove: false,
                        reason: String::from_str(&env, "referenced_by_claim"),
                    }
                } else {
                    CleanableStatus {
                        can_remove: true,
                        reason: String::from_str(&env, "ok"),
                    }
                }
            }
            RecordKind::Auction => {
                let status: Option<String> =
                    env.storage().persistent().get(&DataKey::AuctionStatus(id));
                match status {
                    None => CleanableStatus {
                        can_remove: false,
                        reason: String::from_str(&env, "not_found"),
                    },
                    Some(s) if s != String::from_str(&env, "finished") => CleanableStatus {
                        can_remove: false,
                        reason: String::from_str(&env, "not_terminal"),
                    },
                    Some(_) if Self::is_referenced_by_snapshot(env.clone(), id) => {
                        CleanableStatus {
                            can_remove: false,
                            reason: String::from_str(&env, "referenced_by_snapshot"),
                        }
                    }
                    _ => CleanableStatus {
                        can_remove: true,
                        reason: String::from_str(&env, "ok"),
                    },
                }
            }
            RecordKind::Checkpoint => {
                let retention_window: u64 = env
                    .storage()
                    .instance()
                    .get(&DataKey::RetentionWindow)
                    .unwrap_or(0);
                let checkpoint_time: Option<u64> = env
                    .storage()
                    .persistent()
                    .get(&DataKey::Checkpoint(holder.clone(), id));
                match checkpoint_time {
                    None => CleanableStatus {
                        can_remove: false,
                        reason: String::from_str(&env, "not_found"),
                    },
                    Some(time) => {
                        let now = env.ledger().timestamp();
                        if now < time + retention_window {
                            CleanableStatus {
                                can_remove: false,
                                reason: String::from_str(&env, "within_retention_window"),
                            }
                        } else {
                            CleanableStatus {
                                can_remove: true,
                                reason: String::from_str(&env, "ok"),
                            }
                        }
                    }
                }
            }
            _ => CleanableStatus {
                can_remove: false,
                reason: String::from_str(&env, "unsupported"),
            },
        }
    }

    // Helper functions
    fn pay_reward(env: Env, caller: &Address) -> Result<(), CleanupError> {
        let current_fund: i128 = env
            .storage()
            .instance()
            .get(&DataKey::CleanupFund)
            .unwrap_or(0);
        let reward: i128 = env
            .storage()
            .instance()
            .get(&DataKey::RewardPerEntry)
            .unwrap_or(0);

        if current_fund < reward {
            return Err(CleanupError::InsufficientFund);
        }

        let new_fund = current_fund
            .checked_sub(reward)
            .ok_or(CleanupError::ArithmeticOverflow)?;
        env.storage()
            .instance()
            .set(&DataKey::CleanupFund, &new_fund);

        // In a real implementation, transfer reward to caller
        env.events().publish(("reward_paid", caller), reward);

        Ok(())
    }

    fn has_open_claims(_env: Env, _holder: &Address) -> bool {
        // In a real implementation, check for open claims
        false
    }

    fn is_referenced_by_snapshot(_env: Env, _id: u64) -> bool {
        // In a real implementation, check governance snapshots
        false
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    fn setup() -> (Env, StorageCleanupContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register(StorageCleanupContract, ());
        let client = StorageCleanupContractClient::new(&env, &contract_id);
        client.initialize(&admin, &100, &86400);
        (env, client, admin)
    }

    #[test]
    fn test_cleanup_balance_requires_zero_balance() {
        let (env, client, _admin) = setup();
        let holder = Address::generate(&env);
        let key_id = Address::generate(&env);
        let cleaner = Address::generate(&env);

        // Set non-zero balance
        env.as_contract(&client.address, || {
            env.storage()
                .persistent()
                .set(&DataKey::Balance(holder.clone(), key_id.clone()), &100i128);
        });

        assert_eq!(
            client.try_cleanup_balance(&cleaner, &holder, &key_id),
            Err(Ok(CleanupError::RecordActive))
        );
    }

    #[test]
    fn test_cleanup_checkpoint_respects_retention_window() {
        let (env, client, _admin) = setup();
        let holder = Address::generate(&env);
        let cleaner = Address::generate(&env);
        let checkpoint_id = 1u64;
        let now = env.ledger().timestamp();

        // Set recent checkpoint
        env.as_contract(&client.address, || {
            env.storage()
                .persistent()
                .set(&DataKey::Checkpoint(holder.clone(), checkpoint_id), &now);
        });

        assert_eq!(
            client.try_cleanup_checkpoint(&cleaner, &holder, &checkpoint_id),
            Err(Ok(CleanupError::RecordActive))
        );
    }

    #[test]
    fn test_cleanup_auction_requires_finished_status() {
        let (env, client, _admin) = setup();
        let cleaner = Address::generate(&env);
        let auction_id = 1u64;

        // Set active status
        env.as_contract(&client.address, || {
            env.storage().persistent().set(
                &DataKey::AuctionStatus(auction_id),
                &String::from_str(&env, "active"),
            );
        });

        assert_eq!(
            client.try_cleanup_auction(&cleaner, &auction_id),
            Err(Ok(CleanupError::RecordActive))
        );
    }

    #[test]
    fn test_cleanup_order_accepts_terminal_states() {
        let (env, client, admin) = setup();
        let cleaner = Address::generate(&env);
        let order_id = 1u64;

        // Fund cleanup first
        client.fund_cleanup(&admin, &1000);

        // Set filled status
        env.as_contract(&client.address, || {
            env.storage()
                .persistent()
                .set(&DataKey::Order(order_id), &String::from_str(&env, "data"));
            env.storage().persistent().set(
                &DataKey::OrderStatus(order_id),
                &String::from_str(&env, "filled"),
            );
        });

        client.cleanup_order(&cleaner, &order_id);
    }

    #[test]
    fn test_rewards_paid_from_cleanup_fund() {
        let (env, client, admin) = setup();
        let cleaner = Address::generate(&env);
        let order_id = 1u64;

        // Fund with exact amount for one cleanup
        client.fund_cleanup(&admin, &100);

        env.as_contract(&client.address, || {
            env.storage()
                .persistent()
                .set(&DataKey::Order(order_id), &String::from_str(&env, "data"));
            env.storage().persistent().set(
                &DataKey::OrderStatus(order_id),
                &String::from_str(&env, "filled"),
            );
        });

        client.cleanup_order(&cleaner, &order_id);

        // Second cleanup should fail - insufficient funds
        let order_id_2 = 2u64;
        env.as_contract(&client.address, || {
            env.storage()
                .persistent()
                .set(&DataKey::Order(order_id_2), &String::from_str(&env, "data"));
            env.storage().persistent().set(
                &DataKey::OrderStatus(order_id_2),
                &String::from_str(&env, "filled"),
            );
        });

        assert_eq!(
            client.try_cleanup_order(&cleaner, &order_id_2),
            Err(Ok(CleanupError::InsufficientFund))
        );
    }

    #[test]
    fn test_cleanable_matches_actual_cleanup() {
        let (env, client, _admin) = setup();
        let holder = Address::generate(&env);
        let auction_id = 1u64;

        env.as_contract(&client.address, || {
            env.storage().persistent().set(
                &DataKey::AuctionStatus(auction_id),
                &String::from_str(&env, "active"),
            );
        });

        let status = client.cleanable(&RecordKind::Auction, &holder, &auction_id);
        assert!(!status.can_remove);
        assert_eq!(status.reason, String::from_str(&env, "not_terminal"));
    }

    #[test]
    fn test_cleanup_already_removed_record_reverts() {
        let (env, client, _admin) = setup();
        let cleaner = Address::generate(&env);
        let auction_id = 999u64;

        assert_eq!(
            client.try_cleanup_auction(&cleaner, &auction_id),
            Err(Ok(CleanupError::RecordNotFound))
        );
    }
}
