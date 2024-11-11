use anchor_lang::prelude::*;

use crate::constant::STAKING_SCALE;

pub const MARKET_STAKING_PDA_SEED: &str = "market_staking";
pub const STAKING_POSITION_PDA_SEED: &str = "stake_position";

#[account]
#[derive(InitSpace)]
pub struct MarketStaking {
    pub market: Pubkey,
    pub amount_staked: u64,
    pub total_amount_vested: u64,
    pub acc_reward_amount_per_share: u128,
}

impl MarketStaking {
    pub fn initialize(&mut self, market: Pubkey) -> Result<()> {
        self.market = market;
        self.amount_staked = 0;
        self.acc_reward_amount_per_share = 0;

        Ok(())
    }

    pub fn accrue_rewards(&mut self, pending_rewards: u64) -> Result<u128> {
        let total_shares = self.amount_staked + self.total_amount_vested;

        if total_shares > 0 && pending_rewards > 0 {
            self.acc_reward_amount_per_share +=
                (u128::from(pending_rewards) * STAKING_SCALE) / u128::from(total_shares);
        }

        Ok(self.acc_reward_amount_per_share)
    }
}

#[account]
#[derive(InitSpace)]
pub struct StakePosition {
    pub market: Pubkey,
    pub user: Pubkey,
    pub amount_staked: u64,
    pub total_amount_vested: u64,
    pub pending_rewards: u64,
    pub acc_reward_amount_per_share: u128,
}

impl StakePosition {
    pub fn initialize(&mut self, market: Pubkey, user: Pubkey) -> Result<()> {
        self.market = market;
        self.user = user;

        Ok(())
    }

    pub fn accrue_rewards(&mut self, acc_reward_amount_per_share: u128) -> Result<()> {
        let total_shares = self.amount_staked + self.total_amount_vested;

        if total_shares > 0 {
            self.pending_rewards += u64::try_from(
                u128::from(total_shares)
                    * (acc_reward_amount_per_share - self.acc_reward_amount_per_share)
                    / STAKING_SCALE,
            )?;
        }

        self.acc_reward_amount_per_share = acc_reward_amount_per_share;

        Ok(())
    }
}
