use anchor_lang::prelude::*;

declare_id!("JoeaRXgtME3jAoz5WuFXGEndfv4NPH9nBxsLq44hk9J");

pub mod constant;
pub mod errors;
mod events;
mod instructions;
pub mod manager;
pub mod math;
pub mod state;

use instructions::*;
use manager::swap_manager::{SwapAmountType, SwapType};
use state::*;

#[program]
pub mod token_mill {
    use super::*;

    pub fn create_config(
        ctx: Context<CreateConfig>,
        authority: Pubkey,
        protocol_fee_recipient: Pubkey,
        protocol_fee_share: u16,
        referral_fee_share: u16,
    ) -> Result<()> {
        instructions::create_config::handler(
            ctx,
            authority,
            protocol_fee_recipient,
            protocol_fee_share,
            referral_fee_share,
        )
    }

    pub fn create_market(
        ctx: Context<CreateMarket>,
        name: String,
        symbol: String,
        uri: String,
        total_supply: u64,
        creator_fee_share: u16,
        staking_fee_share: u16,
    ) -> Result<()> {
        instructions::create_market::handler(
            ctx,
            name,
            symbol,
            uri,
            total_supply,
            creator_fee_share,
            staking_fee_share,
        )
    }

    pub fn create_market_with_spl(
        ctx: Context<CreateMarketWithSpl>,
        name: String,
        symbol: String,
        uri: String,
        total_supply: u64,
        creator_fee_share: u16,
        staking_fee_share: u16,
    ) -> Result<()> {
        instructions::create_market_with_spl::handler(
            ctx,
            name,
            symbol,
            uri,
            total_supply,
            creator_fee_share,
            staking_fee_share,
        )
    }

    pub fn set_market_prices(
        ctx: Context<MarketSettingsUpdate>,
        bid_prices: [u64; constant::PRICES_LENGTH],
        ask_prices: [u64; constant::PRICES_LENGTH],
    ) -> Result<()> {
        instructions::set_market_prices::handler(ctx, bid_prices, ask_prices)
    }

    pub fn swap(
        ctx: Context<Swap>,
        swap_type: SwapType,
        swap_amount_type: SwapAmountType,
        amount: u64,
        other_amount_threshold: u64,
    ) -> Result<(u64, u64)> {
        instructions::swap::handler(
            ctx,
            swap_type,
            swap_amount_type,
            amount,
            other_amount_threshold,
        )
    }

    // Staking
    pub fn create_staking(ctx: Context<CreateStaking>) -> Result<()> {
        instructions::staking::create_staking::handler(ctx)
    }

    pub fn create_stake_position(ctx: Context<CreateStakePosition>) -> Result<()> {
        instructions::staking::create_stake_position::handler(ctx)
    }

    pub fn deposit(ctx: Context<StakeUpdate>, amount: u64) -> Result<()> {
        instructions::staking::deposit::handler(ctx, amount)
    }

    pub fn withdraw(ctx: Context<StakeUpdate>, amount: u64) -> Result<()> {
        instructions::staking::withdraw::handler(ctx, amount)
    }

    pub fn claim_staking_rewards(ctx: Context<StakingRewardsClaim>) -> Result<u64> {
        instructions::staking::claim_staking_rewards::handler(ctx)
    }

    // Vesting
    pub fn create_vesting_plan(
        ctx: Context<CreateVestingPlan>,
        start: i64,
        vesting_amount: u64,
        vesting_duration: i64,
        cliff_duration: i64,
    ) -> Result<()> {
        instructions::vesting::create_vesting_plan::handler(
            ctx,
            start,
            vesting_amount,
            vesting_duration,
            cliff_duration,
        )
    }

    pub fn release(ctx: Context<Release>) -> Result<u64> {
        instructions::vesting::release::handler(ctx)
    }

    // Referrals
    pub fn create_referral_account(
        ctx: Context<CreateReferralAccount>,
        referrer: Pubkey,
    ) -> Result<()> {
        instructions::referrals::create_referral_account::handler(ctx, referrer)
    }

    pub fn claim_referral_fees(ctx: Context<ClaimReferralFees>) -> Result<()> {
        instructions::referrals::claim_referral_fees::handler(ctx)
    }

    // Creator instructions
    pub fn update_creator(ctx: Context<MarketSettingsUpdate>, new_creator: Pubkey) -> Result<()> {
        instructions::update_creator::handler(ctx, new_creator)
    }

    pub fn update_market_fee_shares(
        ctx: Context<MarketSettingsUpdate>,
        new_creator_fee_share: u16,
        new_staking_fee_share: u16,
    ) -> Result<()> {
        instructions::update_market_fee_shares::handler(
            ctx,
            new_creator_fee_share,
            new_staking_fee_share,
        )
    }

    pub fn claim_creator_fees(ctx: Context<ClaimCreatorFees>) -> Result<()> {
        instructions::claim_creator_fees::handler(ctx)
    }

    // Admin instructions
    pub fn create_quote_asset_badge(ctx: Context<CreateQuoteAssetBadge>) -> Result<()> {
        instructions::create_quote_asset_badge::handler(ctx)
    }

    pub fn update_quote_asset_badge(
        ctx: Context<UpdateQuoteAssetBadge>,
        status: QuoteTokenBadgeStatus,
    ) -> Result<()> {
        instructions::update_quote_asset_badge::handler(ctx, status)
    }

    pub fn update_default_fee_shares(
        ctx: Context<ConfigUpdate>,
        new_default_protocol_fee_share: u16,
        new_referral_fee_share: u16,
    ) -> Result<()> {
        instructions::update_default_fee_shares::handler(
            ctx,
            new_default_protocol_fee_share,
            new_referral_fee_share,
        )
    }

    pub fn update_protocol_fee_recipient(
        ctx: Context<ConfigUpdate>,
        new_protocol_fee_recipient: Pubkey,
    ) -> Result<()> {
        instructions::update_protocol_fee_recipient::handler(ctx, new_protocol_fee_recipient)
    }

    pub fn transfer_config_ownership(
        ctx: Context<ConfigUpdate>,
        pending_authority: Option<Pubkey>,
    ) -> Result<()> {
        instructions::transfer_config_ownership::handler(ctx, pending_authority)
    }

    pub fn accept_config_ownership(ctx: Context<AcceptConfigOwnership>) -> Result<()> {
        instructions::accept_config_ownership::handler(ctx)
    }
}
