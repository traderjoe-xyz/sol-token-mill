use crate::{
    state::{Market, MarketStaking},
    MARKET_STAKING_PDA_SEED,
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct CreateStaking<'info> {
    pub market: AccountLoader<'info, Market>,

    #[account(
        init,
        payer = payer,
        space = 8 + MarketStaking::INIT_SPACE,
        seeds = [MARKET_STAKING_PDA_SEED.as_bytes(), market.key().as_ref()],
        bump
    )]
    pub staking: Account<'info, MarketStaking>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CreateStaking>) -> Result<()> {
    let staking = &mut ctx.accounts.staking;
    let market = &ctx.accounts.market;

    staking.initialize(market.key())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use joelana_test_utils::joelana_env::actions::token_mill::{CreateStakingAction, TokenMillEnv};

    #[test]
    fn create_staking() {
        let mut testing_env = TokenMillEnv::default();

        testing_env.svm.change_payer("admin");

        let enable_staking_action = CreateStakingAction::new(&testing_env);

        let result = testing_env.svm.execute_actions(&[&enable_staking_action]);

        assert!(result.is_ok());
    }
}
