use crate::{
    state::{Market, StakePosition},
    STAKING_POSITION_PDA_SEED,
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct CreateStakePosition<'info> {
    pub market: AccountLoader<'info, Market>,

    #[account(
        init,
        payer = user,
        space = 8 + StakePosition::INIT_SPACE,
        seeds = [STAKING_POSITION_PDA_SEED.as_bytes(), market.key().as_ref(), user.key().as_ref()],
        bump
    )]
    pub stake_position: Account<'info, StakePosition>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CreateStakePosition>) -> Result<()> {
    let stake_position = &mut ctx.accounts.stake_position;
    let market = &ctx.accounts.market;
    let user = &ctx.accounts.user;

    stake_position.initialize(market.key(), user.key())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use joelana_test_utils::joelana_env::actions::token_mill::{
        CreateStakePositionAction, CreateStakingAction, TokenMillEnv,
    };

    #[test]
    fn create_stake_position() {
        let mut testing_env = TokenMillEnv::default();

        testing_env.svm.change_payer("admin");

        let enable_staking_action = CreateStakingAction::new(&testing_env);

        testing_env
            .svm
            .execute_actions(&[&enable_staking_action])
            .unwrap();

        testing_env.svm.change_payer("bob");

        let create_stake_position_action = CreateStakePositionAction::new(&testing_env);

        let result = testing_env
            .svm
            .execute_actions(&[&create_stake_position_action]);

        assert!(result.is_ok());
    }
}
