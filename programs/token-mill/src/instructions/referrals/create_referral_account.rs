use anchor_lang::prelude::*;

use crate::{ReferralAccount, TokenMillConfig, REFERRAL_ACCOUNT_PDA_SEED};

#[derive(Accounts)]
#[instruction(referrer: Pubkey)]
pub struct CreateReferralAccount<'info> {
    pub config: Account<'info, TokenMillConfig>,

    #[account(
        init,
        seeds = [REFERRAL_ACCOUNT_PDA_SEED.as_bytes(), config.key().as_ref(), referrer.as_ref()],
        bump,
        payer = user,
        space = 8 + ReferralAccount::INIT_SPACE
    )]
    pub referral_account: Account<'info, ReferralAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CreateReferralAccount>, referrer: Pubkey) -> Result<()> {
    let config = &ctx.accounts.config;
    let referral_account = &mut ctx.accounts.referral_account;

    referral_account.initialize(ctx.bumps.referral_account, config.key(), referrer)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use joelana_test_utils::joelana_env::{
        actions::token_mill::{CreateReferralAccountAction, TokenMillEnv},
        make_address,
    };

    use crate::ReferralAccount;

    #[test]
    fn create_referral_account() {
        let mut testing_env = TokenMillEnv::new();

        testing_env.svm.change_payer("carol");

        // Account is created on setup
        let create_referral_account_action = CreateReferralAccountAction::new();

        let referral_account = testing_env.svm.get_parsed_account::<ReferralAccount>(
            &create_referral_account_action.referral_account,
        );

        assert_eq!(referral_account.referrer, make_address("carol"));
        assert_eq!(referral_account.config, testing_env.config);
    }
}
