use anchor_lang::prelude::*;

pub const QUOTE_TOKEN_BADGE_PDA_SEED: &str = "quote_token_badge";

#[derive(Debug, AnchorSerialize, AnchorDeserialize, Copy, Clone, InitSpace, PartialEq)]
pub enum QuoteTokenBadgeStatus {
    Disabled,
    Enabled,
}

#[account]
#[derive(InitSpace)]
pub struct QuoteTokenBadge {
    pub bump: u8,
    pub status: QuoteTokenBadgeStatus,
}

impl QuoteTokenBadge {
    pub fn initialize(&mut self, bump: u8) -> Result<()> {
        self.bump = bump;
        self.status = QuoteTokenBadgeStatus::Enabled;

        Ok(())
    }
}
