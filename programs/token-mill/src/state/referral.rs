use anchor_lang::prelude::*;

pub const REFERRAL_ACCOUNT_PDA_SEED: &str = "referral";

#[account]
#[derive(Debug, InitSpace)]
pub struct ReferralAccount {
    pub bump: u8,
    pub config: Pubkey,
    pub referrer: Pubkey,
}

impl ReferralAccount {
    pub fn initialize(&mut self, bump: u8, config: Pubkey, referrer: Pubkey) -> Result<()> {
        self.bump = bump;
        self.config = config;
        self.referrer = referrer;
        Ok(())
    }
}
