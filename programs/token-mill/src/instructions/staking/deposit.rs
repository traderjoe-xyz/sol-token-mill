use crate::{
    errors::TokenMillError,
    events::TokenMillStakingDepositEvent,
    manager::{staking_manager, token_manager::transfer_from_eoa},
    state::{Market, MarketStaking, StakePosition},
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

#[event_cpi]
#[derive(Accounts)]
pub struct StakeUpdate<'info> {
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

pub fn handler(ctx: Context<StakeUpdate>, amount: u64) -> Result<()> {
    let market = &mut ctx.accounts.market.load_mut()?;
    let staking = &mut ctx.accounts.staking;
    let stake_position = &mut ctx.accounts.stake_position;

    staking_manager::deposit(market, staking, stake_position, amount)?;

    transfer_from_eoa(
        &ctx.accounts.base_token_mint,
        &ctx.accounts.user,
        &ctx.accounts.user_base_token_ata,
        &ctx.accounts.market_base_token_ata,
        &ctx.accounts.base_token_program,
        amount,
    )?;

    emit_cpi!(TokenMillStakingDepositEvent {
        market: ctx.accounts.market.key(),
        user: ctx.accounts.user.key(),
        amount,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use joelana_test_utils::joelana_env::actions::token_mill::{DepositAction, TokenMillEnv};

    use crate::{MarketStaking, StakePosition};

    const STAKE_AMOUNT: u64 = 100_000_000;

    #[test]
    fn deposit() {
        let mut testing_env = TokenMillEnv::default().with_staking(STAKE_AMOUNT);

        testing_env.svm.change_payer("bob");

        let deposit_action = DepositAction::new(&testing_env, STAKE_AMOUNT);

        let result = testing_env.svm.execute_actions(&[&deposit_action]);

        assert!(result.is_ok());

        let staking = testing_env
            .svm
            .get_parsed_account::<MarketStaking>(&deposit_action.market_staking);
        assert_eq!(staking.amount_staked, STAKE_AMOUNT);

        let stake_position = testing_env
            .svm
            .get_parsed_account::<StakePosition>(&deposit_action.stake_position);
        assert_eq!(stake_position.amount_staked, STAKE_AMOUNT);
    }
}
