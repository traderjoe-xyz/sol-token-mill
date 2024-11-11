use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct TokenMillConfig {
    pub authority: Pubkey,
    pub pending_authority: Option<Pubkey>,
    pub protocol_fee_recipient: Pubkey,
    pub default_protocol_fee_share: u16,
    pub referral_fee_share: u16,
}

impl TokenMillConfig {
    pub fn initialize(
        &mut self,
        authority: Pubkey,
        protocol_fee_recipient: Pubkey,
        protocol_fee_share: u16,
        referral_fee_share: u16,
    ) -> Result<()> {
        self.authority = authority;
        self.pending_authority = None;
        self.protocol_fee_recipient = protocol_fee_recipient;
        self.default_protocol_fee_share = protocol_fee_share;
        self.referral_fee_share = referral_fee_share;

        Ok(())
    }
}
