#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Vec};

const WINDOW: u64 = 30 * 24 * 60 * 60;
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    InvalidTier = 3,
    Overflow = 4,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Discount {
    pub tier: u32,
    pub discount_bps: u32,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tier {
    pub threshold: i128,
    pub discount_bps: u32,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
enum Key {
    Admin,
    Tiers,
    Volume(Address),
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
struct Volume {
    amount: i128,
    started_at: u64,
    tier: u32,
}
#[contract]
pub struct FeeDiscountContract;
#[contractimpl]
impl FeeDiscountContract {
    pub fn initialize(e: Env, admin: Address) -> Result<(), Error> {
        if e.storage().instance().has(&Key::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        e.storage().instance().set(&Key::Admin, &admin);
        e.storage()
            .instance()
            .set(&Key::Tiers, &Vec::<Tier>::new(&e));
        Ok(())
    }
    pub fn set_tiers(e: Env, caller: Address, tiers: Vec<Tier>) -> Result<(), Error> {
        let admin: Address = e
            .storage()
            .instance()
            .get(&Key::Admin)
            .ok_or(Error::Unauthorized)?;
        if caller != admin {
            return Err(Error::Unauthorized);
        }
        caller.require_auth();
        let mut last = 0i128;
        for t in tiers.iter() {
            if t.threshold <= last || t.discount_bps > 10_000 {
                return Err(Error::InvalidTier);
            }
            last = t.threshold;
        }
        e.storage().instance().set(&Key::Tiers, &tiers);
        Ok(())
    }
    pub fn record_volume(e: Env, wallet: Address, amount: i128) -> Result<Discount, Error> {
        wallet.require_auth();
        if amount < 0 {
            return Err(Error::InvalidTier);
        }
        let now = e.ledger().timestamp();
        let old = e
            .storage()
            .persistent()
            .get::<Key, Volume>(&Key::Volume(wallet.clone()));
        let mut v = old.unwrap_or(Volume {
            amount: 0,
            started_at: now,
            tier: 0,
        });
        if now.saturating_sub(v.started_at) >= WINDOW {
            v = Volume {
                amount: 0,
                started_at: now,
                tier: 0,
            };
        }
        v.amount = v.amount.checked_add(amount).ok_or(Error::Overflow)?;
        let tiers = e
            .storage()
            .instance()
            .get::<Key, Vec<Tier>>(&Key::Tiers)
            .unwrap_or_else(|| Vec::new(&e));
        let mut tier = 0u32;
        let mut bps = 0u32;
        for (i, t) in tiers.iter().enumerate() {
            if v.amount >= t.threshold {
                tier = (i + 1) as u32;
                bps = t.discount_bps;
            }
        }
        if tier != v.tier {
            e.events().publish(
                (soroban_sdk::symbol_short!("tier"), wallet.clone()),
                Discount {
                    tier,
                    discount_bps: bps,
                },
            );
            v.tier = tier;
        }
        e.storage().persistent().set(&Key::Volume(wallet), &v);
        Ok(Discount {
            tier,
            discount_bps: bps,
        })
    }
    pub fn get_discount(e: Env, wallet: Address) -> Discount {
        let v = e
            .storage()
            .persistent()
            .get::<Key, Volume>(&Key::Volume(wallet))
            .unwrap_or(Volume {
                amount: 0,
                started_at: e.ledger().timestamp(),
                tier: 0,
            });
        let tiers = e
            .storage()
            .instance()
            .get::<Key, Vec<Tier>>(&Key::Tiers)
            .unwrap_or_else(|| Vec::new(&e));
        let mut bps = 0;
        for (i, t) in tiers.iter().enumerate() {
            if v.amount >= t.threshold && (i + 1) as u32 == v.tier {
                bps = t.discount_bps;
            }
        }
        Discount {
            tier: v.tier,
            discount_bps: bps,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Events, Ledger},
        vec,
    };

    fn setup() -> (Env, FeeDiscountContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(FeeDiscountContract, ());
        let client = FeeDiscountContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    fn default_tiers(env: &Env) -> Vec<Tier> {
        vec![
            env,
            Tier {
                threshold: 100,
                discount_bps: 100,
            },
            Tier {
                threshold: 500,
                discount_bps: 500,
            },
            Tier {
                threshold: 1_000,
                discount_bps: 10_000,
            },
        ]
    }

    #[test]
    fn initialize_rejects_reinitialization_and_set_tiers_requires_admin() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(FeeDiscountContract, ());
        let client = FeeDiscountContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let other = Address::generate(&env);

        assert_eq!(
            client.try_set_tiers(&admin, &default_tiers(&env)),
            Err(Ok(Error::Unauthorized))
        );

        client.initialize(&admin);
        assert_eq!(
            client.try_initialize(&admin),
            Err(Ok(Error::AlreadyInitialized))
        );
        assert_eq!(
            client.try_set_tiers(&other, &default_tiers(&env)),
            Err(Ok(Error::Unauthorized))
        );
        client.set_tiers(&admin, &default_tiers(&env));
    }

    #[test]
    fn set_tiers_validates_strictly_increasing_thresholds_and_bps_bounds() {
        let (env, client, admin) = setup();

        // Zero threshold is invalid
        let zero_threshold = vec![
            &env,
            Tier {
                threshold: 0,
                discount_bps: 100,
            },
        ];
        assert_eq!(
            client.try_set_tiers(&admin, &zero_threshold),
            Err(Ok(Error::InvalidTier))
        );

        // Non-increasing threshold is invalid
        let non_increasing = vec![
            &env,
            Tier {
                threshold: 100,
                discount_bps: 100,
            },
            Tier {
                threshold: 100,
                discount_bps: 200,
            },
        ];
        assert_eq!(
            client.try_set_tiers(&admin, &non_increasing),
            Err(Ok(Error::InvalidTier))
        );

        // Discount above 10_000 bps is invalid even with valid threshold
        let over_max_bps = vec![
            &env,
            Tier {
                threshold: 100,
                discount_bps: 10_001,
            },
        ];
        assert_eq!(
            client.try_set_tiers(&admin, &over_max_bps),
            Err(Ok(Error::InvalidTier))
        );

        // Exact 10_000 bps boundary is valid
        client.set_tiers(&admin, &default_tiers(&env));
    }

    #[test]
    fn record_volume_and_get_discount_cover_tiers_boundaries_and_events() {
        let (env, client, admin) = setup();
        client.set_tiers(&admin, &default_tiers(&env));
        let wallet = Address::generate(&env);

        // Negative amount rejected; zero amount accepted at tier 0
        assert_eq!(
            client.try_record_volume(&wallet, &-1),
            Err(Ok(Error::InvalidTier))
        );
        assert_eq!(
            client.record_volume(&wallet, &0),
            Discount {
                tier: 0,
                discount_bps: 0,
            }
        );
        assert_eq!(env.events().all().len(), 0);
        assert_eq!(
            client.get_discount(&wallet),
            Discount {
                tier: 0,
                discount_bps: 0,
            }
        );

        // Reaching tier 1 exactly emits 1 tier-change event
        assert_eq!(
            client.record_volume(&wallet, &100),
            Discount {
                tier: 1,
                discount_bps: 100,
            }
        );
        assert_eq!(env.events().all().len(), 1);
        assert_eq!(
            client.get_discount(&wallet),
            Discount {
                tier: 1,
                discount_bps: 100,
            }
        );

        // Staying in tier 1 emits no additional event
        assert_eq!(
            client.record_volume(&wallet, &50),
            Discount {
                tier: 1,
                discount_bps: 100,
            }
        );
        assert_eq!(env.events().all().len(), 0);

        // Crossing into tier 2 emits a tier-change event
        assert_eq!(
            client.record_volume(&wallet, &350),
            Discount {
                tier: 2,
                discount_bps: 500,
            }
        );
        assert_eq!(env.events().all().len(), 1);
        assert_eq!(
            client.get_discount(&wallet),
            Discount {
                tier: 2,
                discount_bps: 500,
            }
        );

        // Raising tier 2 threshold above recorded volume drops bps to 0 while keeping stored tier
        let raised_tiers = vec![
            &env,
            Tier {
                threshold: 100,
                discount_bps: 100,
            },
            Tier {
                threshold: 600,
                discount_bps: 500,
            },
        ];
        client.set_tiers(&admin, &raised_tiers);
        assert_eq!(
            client.get_discount(&wallet),
            Discount {
                tier: 2,
                discount_bps: 0,
            }
        );
    }

    #[test]
    fn record_volume_resets_only_after_full_30_day_window() {
        let (env, client, admin) = setup();
        client.set_tiers(&admin, &default_tiers(&env));
        let wallet = Address::generate(&env);

        let start = 1_000u64;
        env.ledger().set_timestamp(start);
        assert_eq!(
            client.record_volume(&wallet, &400),
            Discount {
                tier: 1,
                discount_bps: 100,
            }
        );

        // 1 second before the 30-day window (2_592_000s) expires: volume still accumulates
        const THIRTY_DAYS_SECS: u64 = 2_592_000;
        env.ledger().set_timestamp(start + THIRTY_DAYS_SECS - 1);
        assert_eq!(
            client.record_volume(&wallet, &100),
            Discount {
                tier: 2,
                discount_bps: 500,
            }
        );

        // Exactly at the 30-day window boundary: volume resets before adding new amount
        env.ledger().set_timestamp(start + THIRTY_DAYS_SECS);
        assert_eq!(
            client.record_volume(&wallet, &100),
            Discount {
                tier: 1,
                discount_bps: 100,
            }
        );
    }
}
