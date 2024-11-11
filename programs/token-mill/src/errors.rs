use anchor_lang::prelude::*;

#[error_code]
#[derive(PartialEq)]
pub enum TokenMillError {
    PricesAlreadySet,
    BidAskMismatch,
    DecreasingPrices,
    PriceTooHigh,
    InvalidTotalSupply,
    InvalidAmount,
    MathError,
    InvalidAuthority,
    InvalidConfig,
    InvalidQuoteAssetBadge,
    InvalidFeeShare,
    AmountThresholdNotMet,
    InsufficientStakeAmount,
    InvalidMarket,
    InvalidMintAccount,
    InvalidReferralAccount,
    InvalidQuoteTokenMint,
    UnsupportedTokenMint,
    InvalidConfigAccount,
    InvalidStakePosition,
    InvalidVestingDuration,
    InvalidVestingStartTime,
}
