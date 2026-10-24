//! Atomic multi-key trades using the existing bonding-curve execution paths.
use crate::{
    compute_bonding_curve_price, compute_sell_proceeds, compute_slippage_bps, compute_trade_fee,
    constants, fee, read_protocol_fee_config, read_registered_creator_profile, read_royalty_config,
    AuctionConfig, ContractError, CreatorKeysContract, CreatorKeysContractArgs,
    CreatorKeysContractClient, SwapDirection,
};
use soroban_sdk::{contractimpl, contracttype, symbol_short, Address, Env, Map, Vec};

pub const MAX_BATCH_SWAP_ITEMS: u32 = 15;

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BatchSwapItem {
    pub key_id: Address,
    pub amount: u32,
    pub direction: SwapDirection,
    pub max_slippage_bps: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BatchSwapOutput {
    pub key_id: Address,
    pub amount: u32,
    pub direction: SwapDirection,
    /// Buy: gross payment, including allocated fees. Sell: net proceeds.
    pub output: i128,
    pub price_impact_bps: u32,
    pub fee: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BatchTradeQuote {
    pub items: Vec<BatchSwapOutput>,
    pub total_fee: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BatchSwapExecuted {
    pub trader: Address,
    pub items: Vec<BatchSwapOutput>,
    pub total_fee: i128,
    pub ledger: u32,
}

fn validate(items: &Vec<BatchSwapItem>) -> Result<(), ContractError> {
    if items.is_empty() || items.len() > MAX_BATCH_SWAP_ITEMS {
        return Err(ContractError::BatchSizeExceeded);
    }
    for item in items.iter() {
        if item.amount == 0 {
            return Err(ContractError::NotPositiveAmount);
        }
        if item.max_slippage_bps > 10_000 {
            return Err(ContractError::InvalidFeeConfig);
        }
    }
    Ok(())
}

fn buy_fees(env: &Env, creator: &Address, price: i128) -> Result<i128, ContractError> {
    let trade_fee = compute_trade_fee(env, price)?;
    let mut net = price
        .checked_sub(trade_fee)
        .ok_or(ContractError::Overflow)?;
    let mut fees = trade_fee;
    if env
        .storage()
        .persistent()
        .has(&constants::storage::LP_CONTRACT_ADDRESS)
    {
        let bps: u32 = env
            .storage()
            .persistent()
            .get(&constants::storage::LP_ALLOCATION_BPS)
            .unwrap_or(0);
        let allocation = fee::apply_percentage_fee(net, bps).ok_or(ContractError::Overflow)?;
        net = net.checked_sub(allocation).ok_or(ContractError::Overflow)?;
        fees = fees
            .checked_add(allocation)
            .ok_or(ContractError::Overflow)?;
    }
    if let Some(config) = read_protocol_fee_config(env) {
        let (creator_fee, protocol_fee) =
            fee::checked_compute_fee_split(net, config.creator_bps, config.protocol_bps)
                .ok_or(ContractError::Overflow)?;
        fees = fees
            .checked_add(creator_fee)
            .and_then(|v| v.checked_add(protocol_fee))
            .ok_or(ContractError::Overflow)?;
    }
    if let Some(royalty) = read_royalty_config(env, creator) {
        fees = fees
            .checked_add(
                fee::apply_percentage_fee(price, royalty.buy_fee_bps)
                    .ok_or(ContractError::Overflow)?,
            )
            .ok_or(ContractError::Overflow)?;
    }
    Ok(fees)
}

fn quote_item(
    env: &Env,
    item: &BatchSwapItem,
    supply: u32,
) -> Result<(BatchSwapOutput, u32), ContractError> {
    let base: i128 = env
        .storage()
        .persistent()
        .get(&constants::storage::KEY_PRICE)
        .ok_or(ContractError::KeyPriceNotSet)?;
    let after = match item.direction {
        SwapDirection::Buy => supply
            .checked_add(item.amount)
            .ok_or(ContractError::Overflow)?,
        SwapDirection::Sell => supply
            .checked_sub(item.amount)
            .ok_or(ContractError::InsufficientSupply)?,
    };
    let pre = compute_bonding_curve_price(env, &item.key_id, base, supply)?;
    let post = compute_bonding_curve_price(env, &item.key_id, base, after)?;
    let impact = compute_slippage_bps(pre, post)?;
    let mut output: i128 = 0;
    let mut fees: i128 = 0;
    match item.direction {
        SwapDirection::Buy => {
            let auction: Option<AuctionConfig> = env
                .storage()
                .persistent()
                .get(&constants::storage::auction_config(&item.key_id));
            let auction_price = auction
                .filter(|a| supply < a.auction_supply)
                .map(|a| a.auction_price);
            for index in 0..item.amount {
                let price = match auction_price {
                    Some(price) => price,
                    None => compute_bonding_curve_price(
                        env,
                        &item.key_id,
                        base,
                        supply.checked_add(index).ok_or(ContractError::Overflow)?,
                    )?,
                };
                output = output.checked_add(price).ok_or(ContractError::Overflow)?;
            }
            fees = buy_fees(env, &item.key_id, output)?;
        }
        SwapDirection::Sell => {
            for index in 1..=item.amount {
                let price = compute_bonding_curve_price(
                    env,
                    &item.key_id,
                    base,
                    supply
                        .checked_sub(index)
                        .ok_or(ContractError::InsufficientSupply)?,
                )?;
                let proceeds = compute_sell_proceeds(env, price)?;
                output = output
                    .checked_add(proceeds)
                    .ok_or(ContractError::Overflow)?;
                let mut charged = price.checked_sub(proceeds).ok_or(ContractError::Overflow)?;
                if let Some(royalty) = read_royalty_config(env, &item.key_id) {
                    charged = charged
                        .checked_add(
                            fee::apply_percentage_fee(price, royalty.sell_fee_bps)
                                .ok_or(ContractError::Overflow)?,
                        )
                        .ok_or(ContractError::Overflow)?;
                }
                fees = fees.checked_add(charged).ok_or(ContractError::Overflow)?;
            }
        }
    }
    Ok((
        BatchSwapOutput {
            key_id: item.key_id.clone(),
            amount: item.amount,
            direction: item.direction,
            output,
            price_impact_bps: impact,
            fee: fees,
        },
        after,
    ))
}

#[contractimpl]
impl CreatorKeysContract {
    /// Atomically execute up to 15 items. Zero tolerance retains swap's opt-out.
    pub fn batch_swap(
        env: Env,
        trader: Address,
        items: Vec<BatchSwapItem>,
    ) -> Result<BatchSwapExecuted, ContractError> {
        trader.require_auth();
        validate(&items)?;
        let mut outputs = Vec::new(&env);
        let mut total_fee: i128 = 0;
        for item in items.iter() {
            let supply = read_registered_creator_profile(&env, &item.key_id)?.supply;
            let (mut result, _) = quote_item(&env, &item, supply)?;
            if item.max_slippage_bps != 0 && result.price_impact_bps > item.max_slippage_bps {
                return Err(ContractError::SlippageExceeded);
            }
            match item.direction {
                SwapDirection::Buy => {
                    let outcome = Self::buy_keys_core(
                        env.clone(),
                        item.key_id.clone(),
                        trader.clone(),
                        item.amount,
                        result.output,
                        None,
                        None,
                        None,
                    )?;
                    if outcome.keys_bought != item.amount {
                        return Err(ContractError::SupplyCapExceeded);
                    }
                    result.output = outcome.total_price;
                }
                SwapDirection::Sell => {
                    let orders = Vec::from_array(&env, [(item.key_id.clone(), item.amount)]);
                    let sold = Self::batch_sell_core(env.clone(), trader.clone(), orders)?;
                    result.output = sold
                        .get(0)
                        .ok_or(ContractError::InsufficientSupply)?
                        .proceeds;
                }
            }
            total_fee = total_fee
                .checked_add(result.fee)
                .ok_or(ContractError::Overflow)?;
            outputs.push_back(result);
        }
        let receipt = BatchSwapExecuted {
            trader: trader.clone(),
            items: outputs,
            total_fee,
            ledger: env.ledger().sequence(),
        };
        env.events()
            .publish((symbol_short!("batch_sw"), trader), receipt.clone());
        Ok(receipt)
    }

    /// Price items in order using virtual supplies, including repeated keys.
    /// Pricing only: this does not authorize trades or bypass execution guards.
    pub fn get_batch_quote(
        env: Env,
        items: Vec<BatchSwapItem>,
    ) -> Result<BatchTradeQuote, ContractError> {
        validate(&items)?;
        let mut supplies: Map<Address, u32> = Map::new(&env);
        let mut outputs = Vec::new(&env);
        let mut total_fee: i128 = 0;
        for item in items.iter() {
            let supply = match supplies.get(item.key_id.clone()) {
                Some(supply) => supply,
                None => read_registered_creator_profile(&env, &item.key_id)?.supply,
            };
            let (result, after) = quote_item(&env, &item, supply)?;
            supplies.set(item.key_id, after);
            total_fee = total_fee
                .checked_add(result.fee)
                .ok_or(ContractError::Overflow)?;
            outputs.push_back(result);
        }
        Ok(BatchTradeQuote {
            items: outputs,
            total_fee,
        })
    }
}
