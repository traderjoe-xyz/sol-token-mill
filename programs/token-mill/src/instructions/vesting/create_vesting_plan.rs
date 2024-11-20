use crate::{
    errors::TokenMillError,
    events::TokenMillVestingPlanCreationEvent,
    manager::{staking_manager, token_manager::transfer_from_eoa},
    state::{Market, MarketStaking, StakePosition},
    VestingPlan,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

#[event_cpi]
#[derive(Accounts)]
pub struct CreateVestingPlan<'info> {
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

    #[account(init, payer = user, space = 8 + VestingPlan::INIT_SPACE)]
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

    #[account(mut)]
    pub user: Signer<'info>,

    pub base_token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<CreateVestingPlan>,
    start: i64,
    vesting_amount: u64,
    vesting_duration: i64,
    cliff_duration: i64,
) -> Result<()> {
    require!(
        start > 0 && vesting_duration > 0 && cliff_duration > 0,
        TokenMillError::InvalidVestingDuration
    );

    require!(
        vesting_duration > cliff_duration,
        TokenMillError::InvalidVestingDuration
    );

    require!(
        start + vesting_duration > Clock::get()?.unix_timestamp,
        TokenMillError::InvalidVestingStartTime
    );

    let market = &mut ctx.accounts.market.load_mut()?;
    let staking = &mut ctx.accounts.staking;
    let stake_position = &mut ctx.accounts.stake_position;
    let vesting_plan = &mut ctx.accounts.vesting_plan;

    vesting_plan.initialize(
        stake_position.key(),
        start,
        vesting_amount,
        vesting_duration,
        cliff_duration,
    )?;

    staking_manager::deposit_vested(market, staking, stake_position, vesting_amount)?;

    transfer_from_eoa(
        &ctx.accounts.base_token_mint,
        &ctx.accounts.user,
        &ctx.accounts.user_base_token_ata,
        &ctx.accounts.market_base_token_ata,
        &ctx.accounts.base_token_program,
        vesting_amount,
    )?;

    emit_cpi!(TokenMillVestingPlanCreationEvent {
        market: ctx.accounts.market.key(),
        user: ctx.accounts.user.key(),
        vesting_plan: vesting_plan.key(),
        vesting_amount,
        start,
        vesting_duration,
        cliff_duration,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::VestingPlan;
    use joelana_test_utils::joelana_env::actions::token_mill::{
        CreateVestingPlanAction, TokenMillEnv,
    };

    const VESTING_AMOUNT: u64 = 1_000_000_000;
    const STARTING_SLOT: i64 = 333;

    #[test]
    fn create_vesting_plan() {
        let mut testing_env = TokenMillEnv::default().with_staking(VESTING_AMOUNT);

        testing_env.svm.warp(STARTING_SLOT);

        testing_env.svm.change_payer("bob");

        let create_vesting_action =
            CreateVestingPlanAction::new(&testing_env, VESTING_AMOUNT, STARTING_SLOT, 300, 60);

        testing_env
            .svm
            .execute_actions(&[&create_vesting_action])
            .unwrap();

        let vesting_plan = testing_env
            .svm
            .get_parsed_account::<VestingPlan>(&create_vesting_action.vesting_plan);

        assert_eq!(vesting_plan.amount_vested, VESTING_AMOUNT);
        assert_eq!(vesting_plan.start, STARTING_SLOT as i64);
        assert_eq!(vesting_plan.vesting_duration, 300);
        assert_eq!(vesting_plan.cliff_duration, 60);
    }
}
