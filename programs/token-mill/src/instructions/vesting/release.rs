use crate::{
    errors::TokenMillError,
    events::TokenMillVestingPlanReleaseEvent,
    manager::{staking_manager, token_manager::transfer_from_pda},
    state::{Market, MarketStaking, StakePosition},
    VestingPlan, MARKET_PDA_SEED,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

#[event_cpi]
#[derive(Accounts)]
pub struct Release<'info> {
    #[account(mut, has_one = base_token_mint @ TokenMillError::InvalidMintAccount)]
    pub market: AccountLoader<'info, Market>,

    #[account(mut, has_one = market @ TokenMillError::InvalidMarket)]
    pub staking: Account<'info, MarketStaking>,

    #[account(
        mut,
        has_one = market @ TokenMillError::InvalidMarket,
        has_one = user @ TokenMillError::InvalidAuthority
    )]
    pub stake_position: Account<'info, StakePosition>,

    #[account(mut, has_one = stake_position @ TokenMillError::InvalidStakePosition)]
    pub vesting_plan: Account<'info, VestingPlan>,

    pub base_token_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = base_token_mint,
        associated_token::authority = market,
        associated_token::token_program = base_token_program
    )]
    pub market_base_token_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = base_token_mint,
        associated_token::authority = user,
        associated_token::token_program = base_token_program
    )]
    pub user_base_token_ata: InterfaceAccount<'info, TokenAccount>,

    pub user: Signer<'info>,

    pub base_token_program: Interface<'info, TokenInterface>,
}

pub fn handler(ctx: Context<Release>) -> Result<u64> {
    let staking = &mut ctx.accounts.staking;
    let stake_position = &mut ctx.accounts.stake_position;
    let vesting_plan = &mut ctx.accounts.vesting_plan;

    let current_time = Clock::get().unwrap().unix_timestamp;

    let amount_released = vesting_plan.release(current_time)?;

    let market_bump = {
        let market = &mut ctx.accounts.market.load_mut()?;

        staking_manager::withdraw_vested(market, staking, stake_position, amount_released)?;

        market.bump
    };

    if amount_released > 0 {
        let base_token_mint = &ctx.accounts.base_token_mint;
        let base_token_mint_key = base_token_mint.key();
        let seeds = [
            MARKET_PDA_SEED.as_bytes(),
            base_token_mint_key.as_ref(),
            &[market_bump],
        ];

        transfer_from_pda(
            base_token_mint,
            ctx.accounts.market.to_account_info(),
            &ctx.accounts.market_base_token_ata,
            &ctx.accounts.user_base_token_ata,
            &ctx.accounts.base_token_program,
            amount_released,
            &seeds,
        )?;
    }

    emit_cpi!(TokenMillVestingPlanReleaseEvent {
        vesting_plan: vesting_plan.key(),
        amount_released,
    });

    Ok(amount_released)
}

#[cfg(test)]
mod tests {
    use crate::VestingPlan;
    use joelana_test_utils::joelana_env::{
        actions::token_mill::{CreateVestingPlanAction, ReleaseAction, TokenMillEnv},
        TokenType,
    };
    use rstest::rstest;

    const VESTING_AMOUNT: u64 = 1_000_000_000;
    const STARTING_SLOT: i64 = 333;
    const VESTING_DURATION: i64 = 300;
    const CLIFF_DURATION: i64 = 60;

    fn setup_env(base_token_type: TokenType) -> (TokenMillEnv, ReleaseAction) {
        let mut testing_env = TokenMillEnv::new()
            .with_base_token_type(base_token_type)
            .with_default_quote_token_mint()
            .with_default_market()
            .with_staking(VESTING_AMOUNT);

        testing_env.svm.warp(STARTING_SLOT);

        testing_env.svm.change_payer("bob");

        let create_vesting_action = CreateVestingPlanAction::new(
            &testing_env,
            VESTING_AMOUNT,
            STARTING_SLOT,
            VESTING_DURATION,
            CLIFF_DURATION,
        );

        testing_env
            .svm
            .execute_actions(&[&create_vesting_action])
            .unwrap();

        let action = ReleaseAction::new(&testing_env);

        (testing_env, action)
    }

    #[test]
    fn release_before_start() {
        let mut testing_env = TokenMillEnv::default().with_staking(VESTING_AMOUNT);

        testing_env.svm.warp(STARTING_SLOT);

        testing_env.svm.change_payer("bob");

        let create_vesting_action = CreateVestingPlanAction::new(
            &testing_env,
            VESTING_AMOUNT,
            STARTING_SLOT + 60,
            VESTING_DURATION,
            CLIFF_DURATION,
        );

        let action = ReleaseAction::new(&testing_env);

        testing_env.svm.warp(20);

        testing_env
            .svm
            .execute_actions(&[&create_vesting_action, &action])
            .unwrap();

        let vesting_plan = testing_env
            .svm
            .get_parsed_account::<VestingPlan>(&action.vesting_plan);

        assert_eq!(vesting_plan.amount_released, 0);
    }

    #[test]
    fn release_during_cliff() {
        let (mut testing_env, action) = setup_env(TokenType::Token2022);

        testing_env.svm.warp(CLIFF_DURATION / 2);

        testing_env.svm.execute_actions(&[&action]).unwrap();

        let vesting_plan = testing_env
            .svm
            .get_parsed_account::<VestingPlan>(&action.vesting_plan);

        assert_eq!(vesting_plan.amount_released, 0);
    }

    #[rstest]
    fn release_after_cliff(
        #[values(TokenType::Token, TokenType::Token2022)] base_token_type: TokenType,
    ) {
        let (mut testing_env, action) = setup_env(base_token_type);

        testing_env.svm.warp(VESTING_DURATION / 2);

        testing_env.svm.execute_actions(&[&action]).unwrap();

        let vesting_plan = testing_env
            .svm
            .get_parsed_account::<VestingPlan>(&action.vesting_plan);

        assert_eq!(vesting_plan.amount_released, VESTING_AMOUNT / 2);
    }

    #[test]
    fn release_after_vesting() {
        let (mut testing_env, action) = setup_env(TokenType::Token2022);

        testing_env.svm.warp(VESTING_DURATION + 1);

        testing_env.svm.execute_actions(&[&action]).unwrap();

        let vesting_plan = testing_env
            .svm
            .get_parsed_account::<VestingPlan>(&action.vesting_plan);

        assert_eq!(vesting_plan.amount_released, VESTING_AMOUNT);
    }

    #[test]
    fn consecutive_releases() {
        let (mut testing_env, action) = setup_env(TokenType::Token2022);

        testing_env.svm.warp(VESTING_DURATION / 2);

        testing_env.svm.execute_actions(&[&action]).unwrap();

        testing_env.svm.warp(VESTING_DURATION / 2);

        testing_env.svm.execute_actions(&[&action]).unwrap();

        let vesting_plan = testing_env
            .svm
            .get_parsed_account::<VestingPlan>(&action.vesting_plan);

        assert_eq!(vesting_plan.amount_released, VESTING_AMOUNT);
    }
}
