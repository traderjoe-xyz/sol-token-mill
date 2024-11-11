use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::{
    events::TokenMillReferralFeeClaimEvent, manager::token_manager::transfer_from_pda,
    ReferralAccount, REFERRAL_ACCOUNT_PDA_SEED,
};

#[event_cpi]
#[derive(Accounts)]
pub struct ClaimReferralFees<'info> {
    #[account(has_one = referrer)]
    pub referral_account: Account<'info, ReferralAccount>,

    pub quote_token_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = quote_token_mint,
        associated_token::authority = referral_account,
        associated_token::token_program = quote_token_program
    )]
    pub referral_account_quote_token_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = quote_token_mint,
        associated_token::authority = referrer,
        associated_token::token_program = quote_token_program
    )]
    pub referrer_quote_token_ata: InterfaceAccount<'info, TokenAccount>,

    pub referrer: Signer<'info>,

    pub quote_token_program: Interface<'info, TokenInterface>,
}

pub fn handler(ctx: Context<ClaimReferralFees>) -> Result<()> {
    let referral_account = &ctx.accounts.referral_account;
    let referral_account_quote_token_ata = &ctx.accounts.referral_account_quote_token_ata;
    let pending_fees = referral_account_quote_token_ata.amount;

    let referral_account_seeds = [
        REFERRAL_ACCOUNT_PDA_SEED.as_bytes(),
        referral_account.config.as_ref(),
        referral_account.referrer.as_ref(),
        &[referral_account.bump],
    ];

    transfer_from_pda(
        &ctx.accounts.quote_token_mint,
        referral_account.to_account_info(),
        referral_account_quote_token_ata,
        &ctx.accounts.referrer_quote_token_ata,
        &ctx.accounts.quote_token_program,
        pending_fees,
        &referral_account_seeds,
    )?;

    emit_cpi!(TokenMillReferralFeeClaimEvent {
        referrer: ctx.accounts.referrer.key(),
        quote_token_mint: ctx.accounts.quote_token_mint.key(),
        fees_distributed: pending_fees,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use joelana_test_utils::joelana_env::{
        actions::token_mill::{ClaimReferralFeesAction, SwapAction, TokenMillEnv},
        make_address, SwapAmountType, SwapType,
    };

    #[test]
    fn claim_referral_fees() {
        let mut testing_env = TokenMillEnv::default();

        let swap_action = SwapAction::new(
            &testing_env,
            SwapType::Buy,
            SwapAmountType::ExactOutput,
            1_000_000_000,
            u64::MAX,
            Some(make_address("carol")),
        );

        testing_env.svm.execute_actions(&[&swap_action]).unwrap();

        testing_env.svm.change_payer("carol");

        let claim_referral_fees_action = ClaimReferralFeesAction::new(&testing_env);

        testing_env
            .svm
            .execute_actions(&[&claim_referral_fees_action])
            .unwrap();
    }
}
