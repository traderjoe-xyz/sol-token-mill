use anchor_lang::event;
use anchor_lang::prelude::*;

use crate::constant::PRICES_LENGTH;
use crate::manager::swap_manager::SwapType;
use crate::QuoteTokenBadgeStatus;

#[event]
pub struct TokenMillConfigCreationEvent {
    pub config: Pubkey,
    pub authority: Pubkey,
    pub default_protocol_fee_share: u16,
    pub referral_fee_share: u16,
}

#[event]
pub struct TokenMillConfigOwnershipTransferEvent {
    pub config: Pubkey,
    pub new_authority: Pubkey,
}

#[event]
pub struct TokenMillQuoteTokenBadgeEvent {
    pub config: Pubkey,
    pub quote_token_mint: Pubkey,
    pub quote_asset_badge_status: QuoteTokenBadgeStatus,
}

#[event]
pub struct TokenMillMarketCreationEvent {
    pub config: Pubkey,
    pub market: Pubkey,
    pub creator: Pubkey,
    pub base_token_mint: Pubkey,
    pub quote_token_mint: Pubkey,
    pub total_supply: u64,
    pub protocol_fee_share: u16,
    pub referral_fee_share: u16,
    pub creator_fee_share: u16,
    pub staking_fee_share: u16,
}

#[event]
pub struct TokenMillMarketPriceSetEvent {
    pub market: Pubkey,
    pub bid_prices: [u64; PRICES_LENGTH],
    pub ask_prices: [u64; PRICES_LENGTH],
}

#[event]
pub struct TokenMillSwapEvent {
    pub user: Pubkey,
    pub market: Pubkey,
    pub swap_type: SwapType,
    pub base_amount: u64,
    pub quote_amount: u64,
    pub referral_token_account: Option<Pubkey>,
    pub creator_fee: u64,
    pub staking_fee: u64,
    pub protocol_fee: u64,
    pub referral_fee: u64,
}

#[event]
pub struct TokenMillCreatorFeeClaimEvent {
    pub market: Pubkey,
    pub creator: Pubkey,
    pub fees_distributed: u64,
}

#[event]
pub struct TokenMillDefaultFeeSharesUpdateEvent {
    pub config: Pubkey,
    pub new_default_protocol_fee_share: u16,
    pub new_referral_fee_share: u16,
}

#[event]
pub struct TokenMillProtocolFeeRecipientUpdateEvent {
    pub config: Pubkey,
    pub new_protocol_fee_recipient: Pubkey,
}

#[event]
pub struct TokenMillCreatorUpdateEvent {
    pub market: Pubkey,
    pub new_creator: Pubkey,
}

#[event]
pub struct TokenMillMarketFeeSharesUpdateEvent {
    pub market: Pubkey,
    pub new_creator_fee_share: u16,
    pub new_staking_fee_share: u16,
}

#[event]
pub struct TokenMillStakingDepositEvent {
    pub market: Pubkey,
    pub user: Pubkey,
    pub amount: u64,
}

#[event]
pub struct TokenMillStakingWithdrawalEvent {
    pub market: Pubkey,
    pub user: Pubkey,
    pub amount: u64,
}

#[event]
pub struct TokenMillReferralFeeClaimEvent {
    pub referrer: Pubkey,
    pub quote_token_mint: Pubkey,
    pub fees_distributed: u64,
}

#[event]
pub struct TokenMillStakingRewardsClaimEvent {
    pub market: Pubkey,
    pub user: Pubkey,
    pub amount_distributed: u64,
}

#[event]
pub struct TokenMillVestingPlanCreationEvent {
    pub market: Pubkey,
    pub user: Pubkey,
    pub vesting_plan: Pubkey,
    pub vesting_amount: u64,
    pub start: i64,
    pub vesting_duration: i64,
    pub cliff_duration: i64,
}

#[event]
pub struct TokenMillVestingPlanReleaseEvent {
    pub vesting_plan: Pubkey,
    pub amount_released: u64,
}
