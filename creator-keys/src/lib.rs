#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String};

pub mod fee {
    use soroban_sdk::contracttype;

    /// Basis points per 100% (10000 = 100%).
    pub const BPS_MAX: u32 = 10_000;

    /// Maximum protocol share when configuring fees via [`assert_valid_fee_bps`].
    ///
    /// Caps the on-chain configured protocol take at 50% so fee settings stay within
    /// expected economic bounds before they affect market logic.
    pub const PROTOCOL_BPS_MAX: u32 = 5_000;

    #[derive(Clone)]
    #[contracttype]
    pub struct FeeConfig {
        pub creator_bps: u32,
        pub protocol_bps: u32,
    }

    /// Validates creator and protocol basis points for storage and fee-setting entrypoints.
    ///
    /// Requires `creator_bps + protocol_bps == BPS_MAX` and `protocol_bps <= PROTOCOL_BPS_MAX`.
    pub fn assert_valid_fee_bps(creator_bps: u32, protocol_bps: u32) {
        let Some(sum) = creator_bps.checked_add(protocol_bps) else {
            panic!("creator_bps + protocol_bps overflow");
        };
        if sum != BPS_MAX {
            panic!("creator_bps + protocol_bps must equal 10000");
        }
        if protocol_bps > PROTOCOL_BPS_MAX {
            panic!(
                "protocol_bps exceeds maximum allowed ({} bps)",
                PROTOCOL_BPS_MAX
            );
        }
    }

    /// Computes the fee split for a given total amount.
    ///
    /// Returns `(creator_amount, protocol_amount)`. Remainder from integer division
    /// is assigned to the creator. Ensures creator_amount + protocol_amount == total.
    pub fn compute_fee_split(total: i128, _creator_bps: u32, protocol_bps: u32) -> (i128, i128) {
        if total <= 0 {
            return (0, 0);
        }
        let protocol_amount = (total * protocol_bps as i128) / BPS_MAX as i128;
        let creator_amount = total - protocol_amount;
        (creator_amount, protocol_amount)
    }
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Creator(Address),
    FeeConfig,
    KeyPrice,
    KeyBalance(Address, Address),
}

#[derive(Clone)]
#[contracttype]
pub struct CreatorProfile {
    pub creator: Address,
    pub handle: String,
    pub supply: u32,
}

#[contract]
pub struct CreatorKeysContract;

#[contractimpl]
impl CreatorKeysContract {
    pub fn register_creator(env: Env, creator: Address, handle: String) {
        creator.require_auth();

        let key = DataKey::Creator(creator.clone());
        let profile = CreatorProfile {
            creator,
            handle,
            supply: 0,
        };

        env.storage().persistent().set(&key, &profile);
        env.events().publish((symbol_short!("register"),), key);
    }

    pub fn buy_key(env: Env, creator: Address, buyer: Address, payment: i128) -> u32 {
        buyer.require_auth();

        let price: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::KeyPrice)
            .unwrap_or_else(|| panic!("key price not set"));
        if payment < price {
            panic!("insufficient payment");
        }

        let key = DataKey::Creator(creator.clone());
        let mut profile: CreatorProfile = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic!("creator not registered"));

        profile.supply += 1;
        env.storage().persistent().set(&key, &profile);

        let balance_key = DataKey::KeyBalance(creator.clone(), buyer.clone());
        let current_balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        env.storage()
            .persistent()
            .set(&balance_key, &(current_balance + 1));

        env.events()
            .publish((symbol_short!("buy"), creator, buyer), profile.supply);

        profile.supply
    }

    pub fn get_key_balance(env: Env, creator: Address, wallet: Address) -> u32 {
        let key = DataKey::KeyBalance(creator, wallet);
        env.storage().persistent().get(&key).unwrap_or(0)
    }

    pub fn get_creator(env: Env, creator: Address) -> Option<CreatorProfile> {
        let key = DataKey::Creator(creator);
        env.storage().persistent().get(&key)
    }

    pub fn set_fee_config(env: Env, admin: Address, creator_bps: u32, protocol_bps: u32) {
        admin.require_auth();
        fee::assert_valid_fee_bps(creator_bps, protocol_bps);
        let config = fee::FeeConfig {
            creator_bps,
            protocol_bps,
        };
        env.storage().persistent().set(&DataKey::FeeConfig, &config);
    }

    pub fn set_key_price(env: Env, admin: Address, price: i128) {
        admin.require_auth();
        if price <= 0 {
            panic!("key price must be positive");
        }
        env.storage().persistent().set(&DataKey::KeyPrice, &price);
    }

    pub fn get_fee_config(env: Env) -> Option<fee::FeeConfig> {
        env.storage().persistent().get(&DataKey::FeeConfig)
    }

    pub fn compute_fees_for_payment(env: Env, total: i128) -> (i128, i128) {
        let config: fee::FeeConfig = env
            .storage()
            .persistent()
            .get(&DataKey::FeeConfig)
            .unwrap_or_else(|| panic!("fee config not set"));
        fee::compute_fee_split(total, config.creator_bps, config.protocol_bps)
    }
}

#[cfg(test)]
mod tests {
    use super::fee;

    #[test]
    fn test_fee_split_90_10_1000() {
        let (creator, protocol) = fee::compute_fee_split(1000, 9000, 1000);
        assert_eq!(creator, 900);
        assert_eq!(protocol, 100);
        assert_eq!(creator + protocol, 1000);
    }

    #[test]
    fn test_fee_split_100_creator() {
        let (creator, protocol) = fee::compute_fee_split(1000, 10000, 0);
        assert_eq!(creator, 1000);
        assert_eq!(protocol, 0);
        assert_eq!(creator + protocol, 1000);
    }

    #[test]
    fn test_fee_split_100_protocol() {
        let (creator, protocol) = fee::compute_fee_split(1000, 0, 10000);
        assert_eq!(creator, 0);
        assert_eq!(protocol, 1000);
        assert_eq!(creator + protocol, 1000);
    }

    #[test]
    fn test_fee_split_remainder_to_creator() {
        // 999 * 1000 / 10000 = 99 (protocol floor), creator gets remainder
        let (creator, protocol) = fee::compute_fee_split(999, 9000, 1000);
        assert_eq!(creator, 900);
        assert_eq!(protocol, 99);
        assert_eq!(creator + protocol, 999);
    }

    #[test]
    fn test_fee_split_zero_total() {
        let (creator, protocol) = fee::compute_fee_split(0, 9000, 1000);
        assert_eq!(creator, 0);
        assert_eq!(protocol, 0);
    }

    #[test]
    fn test_fee_split_dust_total_one() {
        // 1 * 1000 / 10000 = 0 protocol, creator gets full amount
        let (creator, protocol) = fee::compute_fee_split(1, 9000, 1000);
        assert_eq!(creator, 1);
        assert_eq!(protocol, 0);
        assert_eq!(creator + protocol, 1);
    }

    #[test]
    fn test_fee_split_balance_conservation() {
        for total in [100_i128, 1, 999, 10000, 1234567] {
            let (creator, protocol) = fee::compute_fee_split(total, 9000, 1000);
            assert_eq!(creator + protocol, total, "total={}", total);
        }
    }
}
