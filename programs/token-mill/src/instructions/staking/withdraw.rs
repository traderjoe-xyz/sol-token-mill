use crate::{
    errors::TokenMillError,
    events::TokenMillStakingWithdrawalEvent,
    manager::{staking_manager, token_manager::transfer_from_pda},
    MARKET_PDA_SEED,
};
use anchor_lang::prelude::*;

use super::StakeUpdate;

pub fn handler(ctx: Context<StakeUpdate>, amount: u64) -> Result<()> {
    let staking = &mut ctx.accounts.staking;
    let stake_position = &mut ctx.accounts.stake_position;

    require_gte!(
        stake_position.amount_staked,
        amount,
        TokenMillError::InsufficientStakeAmount
    );

    let market_bump = {
        let market = &mut ctx.accounts.market.load_mut()?;

        staking_manager::withdraw(market, staking, stake_position, amount)?;

        market.bump
    };

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
        amount,
        &seeds,
    )?;

    emit_cpi!(TokenMillStakingWithdrawalEvent {
        market: ctx.accounts.market.key(),
        user: ctx.accounts.user.key(),
        amount,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use joelana_test_utils::joelana_env::{
        actions::token_mill::{DepositAction, TokenMillEnv, WithdrawAction},
        TokenType,
    };
    use rstest::rstest;

    use crate::{MarketStaking, StakePosition};

    const STAKE_AMOUNT: u64 = 100_000_000;

    #[rstest]
    fn withdraw(#[values(TokenType::Token, TokenType::Token2022)] base_token_type: TokenType) {
        let mut testing_env = TokenMillEnv::new()
            .with_base_token_type(base_token_type)
            .with_default_quote_token_mint()
            .with_default_market()
            .with_staking(STAKE_AMOUNT);

        testing_env.svm.change_payer("bob");

        let deposit_action = DepositAction::new(&testing_env, STAKE_AMOUNT);

        let withdraw_action = WithdrawAction::new(&testing_env, STAKE_AMOUNT / 2);

        let result = testing_env
            .svm
            .execute_actions(&[&deposit_action, &withdraw_action]);

        assert!(result.is_ok());

        let staking = testing_env
            .svm
            .get_parsed_account::<MarketStaking>(&deposit_action.market_staking);
        assert_eq!(staking.amount_staked, STAKE_AMOUNT / 2);

        let stake_position = testing_env
            .svm
            .get_parsed_account::<StakePosition>(&deposit_action.stake_position);
        assert_eq!(stake_position.amount_staked, STAKE_AMOUNT / 2);
    }
}
