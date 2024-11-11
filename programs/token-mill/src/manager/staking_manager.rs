use crate::state::{Market, MarketStaking, StakePosition};
use anchor_lang::prelude::*;

pub fn deposit(
    market: &mut Market,
    staking: &mut MarketStaking,
    stake_position: &mut StakePosition,
    amount: u64,
) -> Result<u64> {
    let pending_staking_fees = accrue_rewards(market, staking, stake_position)?;

    staking.amount_staked += amount;
    stake_position.amount_staked += amount;

    Ok(pending_staking_fees)
}

pub fn deposit_vested(
    market: &mut Market,
    staking: &mut MarketStaking,
    stake_position: &mut StakePosition,
    amount: u64,
) -> Result<u64> {
    let pending_staking_fees = accrue_rewards(market, staking, stake_position)?;

    staking.total_amount_vested += amount;
    stake_position.total_amount_vested += amount;

    Ok(pending_staking_fees)
}

pub fn withdraw(
    market: &mut Market,
    staking: &mut MarketStaking,
    stake_position: &mut StakePosition,
    amount: u64,
) -> Result<u64> {
    let pending_staking_fees = accrue_rewards(market, staking, stake_position)?;

    staking.amount_staked -= amount;
    stake_position.amount_staked -= amount;

    Ok(pending_staking_fees)
}

pub fn withdraw_vested(
    market: &mut Market,
    staking: &mut MarketStaking,
    stake_position: &mut StakePosition,
    amount: u64,
) -> Result<u64> {
    let pending_staking_fees = accrue_rewards(market, staking, stake_position)?;

    staking.total_amount_vested -= amount;
    stake_position.total_amount_vested -= amount;

    Ok(pending_staking_fees)
}

fn accrue_rewards(
    market: &mut Market,
    staking: &mut MarketStaking,
    stake_position: &mut StakePosition,
) -> Result<u64> {
    let pending_staking_fees = market.fees.pending_staking_fees;
    let acc_reward_amount_per_share = staking.accrue_rewards(pending_staking_fees)?;

    if acc_reward_amount_per_share > 0 {
        market.fees.pending_staking_fees = 0;
    }

    stake_position.accrue_rewards(acc_reward_amount_per_share)?;

    Ok(pending_staking_fees)
}
