#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol,
};

#[cfg(test)]
mod test;

pub const CAMPAIGN_CREATED_EVENT: Symbol = symbol_short!("camp_new");
pub const WALLET_QUALIFIED_EVENT: Symbol = symbol_short!("qual");
pub const REWARD_CLAIMED_EVENT: Symbol = symbol_short!("claim");

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WalletStats {
    pub volume: i128,
    pub trade_count: u32,
    pub hold_seconds: u64,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Criterion {
    Volume,
    HoldDays,
    Trades,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Criteria {
    pub kind: Criterion,
    pub threshold: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Campaign {
    pub id: u64,
    pub key_id: Address,
    pub admin: Address,
    pub start_time: u64,
    pub end_time: u64,
    pub reward_pool: i128,
    pub criteria: Criteria,
    pub total_score: i128,
    pub total_claimed: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignCreated {
    pub campaign_id: u64,
    pub key_id: Address,
    pub admin: Address,
    pub start_time: u64,
    pub end_time: u64,
    pub reward_pool: i128,
    pub criterion: Criterion,
    pub threshold: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WalletQualified {
    pub campaign_id: u64,
    pub wallet: Address,
    pub score: i128,
    pub total_score: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewardClaimed {
    pub campaign_id: u64,
    pub wallet: Address,
    pub amount: i128,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SeasonalError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    CampaignNotFound = 4,
    InvalidTimeWindow = 5,
    CampaignActive = 6,
    NotQualified = 7,
    AlreadyClaimed = 8,
    InvalidCriteria = 9,
    ArithmeticOverflow = 10,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    NextCampaignId,
    Campaign(u64),
    Qualification(u64, Address),
    Claimed(u64, Address),
}

const SECONDS_PER_DAY: i128 = 86_400;

#[contract]
pub struct SeasonalRewardsContract;

#[contractimpl]
impl SeasonalRewardsContract {
    pub fn initialize(env: Env, admin: Address) -> Result<(), SeasonalError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(SeasonalError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::NextCampaignId, &1u64);
        Ok(())
    }

    pub fn admin(env: Env) -> Result<Address, SeasonalError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(SeasonalError::NotInitialized)
    }

    pub fn create_campaign(
        env: Env,
        key_id: Address,
        start_time: u64,
        end_time: u64,
        reward_pool: i128,
        criteria: Criteria,
    ) -> Result<u64, SeasonalError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(SeasonalError::NotInitialized)?;
        admin.require_auth();

        if start_time >= end_time {
            return Err(SeasonalError::InvalidTimeWindow);
        }
        if end_time <= env.ledger().timestamp() {
            return Err(SeasonalError::InvalidTimeWindow);
        }
        if reward_pool <= 0 || criteria.threshold <= 0 {
            return Err(SeasonalError::InvalidCriteria);
        }

        let id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextCampaignId)
            .unwrap_or(1);
        let next = id.checked_add(1).ok_or(SeasonalError::ArithmeticOverflow)?;
        env.storage()
            .instance()
            .set(&DataKey::NextCampaignId, &next);

        let campaign = Campaign {
            id,
            key_id: key_id.clone(),
            admin: admin.clone(),
            start_time,
            end_time,
            reward_pool,
            criteria,
            total_score: 0,
            total_claimed: 0,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Campaign(id), &campaign);

        env.events().publish(
            (CAMPAIGN_CREATED_EVENT, id, key_id),
            CampaignCreated {
                campaign_id: id,
                key_id: campaign.key_id.clone(),
                admin,
                start_time,
                end_time,
                reward_pool,
                criterion: criteria.kind,
                threshold: criteria.threshold,
            },
        );

        Ok(id)
    }

    pub fn get_campaign(env: Env, campaign_id: u64) -> Result<Campaign, SeasonalError> {
        env.storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .ok_or(SeasonalError::CampaignNotFound)
    }

    pub fn get_qualification(env: Env, campaign_id: u64, wallet: Address) -> Option<i128> {
        env.storage()
            .persistent()
            .get(&DataKey::Qualification(campaign_id, wallet))
    }

    pub fn qualify_wallet(
        env: Env,
        campaign_id: u64,
        wallet: Address,
        stats: WalletStats,
    ) -> Result<bool, SeasonalError> {
        wallet.require_auth();

        let mut campaign: Campaign = env
            .storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .ok_or(SeasonalError::CampaignNotFound)?;

        if env.ledger().timestamp() < campaign.start_time {
            return Err(SeasonalError::InvalidTimeWindow);
        }

        if let Some(existing) = env
            .storage()
            .persistent()
            .get::<_, i128>(&DataKey::Qualification(campaign_id, wallet.clone()))
        {
            return Ok(existing > 0);
        }

        let score: i128 = match campaign.criteria.kind {
            Criterion::Volume => {
                if stats.volume >= campaign.criteria.threshold {
                    stats.volume
                } else {
                    0
                }
            }
            Criterion::Trades => {
                if (stats.trade_count as i128) >= campaign.criteria.threshold {
                    stats.trade_count as i128
                } else {
                    0
                }
            }
            Criterion::HoldDays => {
                let threshold_seconds = campaign
                    .criteria
                    .threshold
                    .checked_mul(SECONDS_PER_DAY)
                    .ok_or(SeasonalError::ArithmeticOverflow)?;
                if (stats.hold_seconds as i128) >= threshold_seconds {
                    stats.hold_seconds as i128
                } else {
                    0
                }
            }
        };

        env.storage()
            .persistent()
            .set(&DataKey::Qualification(campaign_id, wallet.clone()), &score);

        if score > 0 {
            campaign.total_score = campaign
                .total_score
                .checked_add(score)
                .ok_or(SeasonalError::ArithmeticOverflow)?;
            env.storage()
                .persistent()
                .set(&DataKey::Campaign(campaign_id), &campaign);

            env.events().publish(
                (WALLET_QUALIFIED_EVENT, campaign_id, wallet.clone()),
                WalletQualified {
                    campaign_id,
                    wallet,
                    score,
                    total_score: campaign.total_score,
                },
            );
        }

        Ok(score > 0)
    }

    pub fn claim_campaign_reward(
        env: Env,
        campaign_id: u64,
        wallet: Address,
    ) -> Result<i128, SeasonalError> {
        wallet.require_auth();

        let mut campaign: Campaign = env
            .storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .ok_or(SeasonalError::CampaignNotFound)?;

        if env.ledger().timestamp() < campaign.end_time {
            return Err(SeasonalError::CampaignActive);
        }

        if env
            .storage()
            .persistent()
            .has(&DataKey::Claimed(campaign_id, wallet.clone()))
        {
            return Err(SeasonalError::AlreadyClaimed);
        }

        let score: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Qualification(campaign_id, wallet.clone()))
            .ok_or(SeasonalError::NotQualified)?;
        if score <= 0 {
            return Err(SeasonalError::NotQualified);
        }
        if campaign.total_score <= 0 {
            return Err(SeasonalError::NotQualified);
        }

        let numerator = campaign
            .reward_pool
            .checked_mul(score)
            .ok_or(SeasonalError::ArithmeticOverflow)?;
        let payout = numerator / campaign.total_score;

        campaign.total_claimed = campaign
            .total_claimed
            .checked_add(payout)
            .ok_or(SeasonalError::ArithmeticOverflow)?;
        env.storage()
            .persistent()
            .set(&DataKey::Campaign(campaign_id), &campaign);
        env.storage()
            .persistent()
            .set(&DataKey::Claimed(campaign_id, wallet.clone()), &true);

        env.events().publish(
            (REWARD_CLAIMED_EVENT, campaign_id, wallet.clone()),
            RewardClaimed {
                campaign_id,
                wallet,
                amount: payout,
            },
        );

        Ok(payout)
    }
}
