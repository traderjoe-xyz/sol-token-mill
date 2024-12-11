use crate::{
    errors::TokenMillError,
    events::TokenMillStakingRewardsClaimEvent,
    manager::{staking_manager, token_manager::transfer_from_pda},
    state::{Market, MarketStaking, StakePosition},
    MARKET_PDA_SEED,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

#[event_cpi]
#[derive(Accounts)]
pub struct StakingRewardsClaim<'info> {
    #[account(mut, has_one = quote_token_mint @ TokenMillError::InvalidMintAccount)]
    pub market: AccountLoader<'info, Market>,

    #[account(mut, has_one = market @ TokenMillError::InvalidMarket)]
    pub staking: Account<'info, MarketStaking>,

    #[account(
        mut, 
        has_one = user @ TokenMillError::InvalidAuthority,
        has_one = market @ TokenMillError::InvalidMarket
    )]
    pub stake_position: Account<'info, StakePosition>,

    pub quote_token_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = quote_token_mint,
        associated_token::authority = market,
        associated_token::token_program = quote_token_program
    )]
    pub market_quote_token_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = quote_token_mint,
        associated_token::authority = user,
        associated_token::token_program = quote_token_program
    )]
    pub user_quote_token_ata: InterfaceAccount<'info, TokenAccount>,

    pub user: Signer<'info>,

    pub quote_token_program: Interface<'info, TokenInterface>,
}

pub fn handler(ctx: Context<StakingRewardsClaim>) -> Result<u64> {
    let pending_rewards;
    let base_token_mint;
    let market_bump;

    {
        let market = &mut ctx.accounts.market.load_mut()?;
        let staking = &mut ctx.accounts.staking;
        let stake_position = &mut ctx.accounts.stake_position;

        staking_manager::deposit(market, staking, stake_position, 0)?;

        pending_rewards = stake_position.pending_rewards;
        base_token_mint = market.base_token_mint;
        market_bump = market.bump;

        stake_position.pending_rewards = 0;
    };

    let quote_token_mint = &ctx.accounts.quote_token_mint;
    let seeds = [
        MARKET_PDA_SEED.as_bytes(),
        base_token_mint.as_ref(),
        &[market_bump],
    ];

    transfer_from_pda(
        quote_token_mint,
        ctx.accounts.market.to_account_info(),
        &ctx.accounts.market_quote_token_ata,
        &ctx.accounts.user_quote_token_ata,
        &ctx.accounts.quote_token_program,
        pending_rewards,
        &seeds,
    )?;

    emit_cpi!(TokenMillStakingRewardsClaimEvent {
        market: ctx.accounts.market.key(),
        user: ctx.accounts.user.key(),
        amount_distributed: pending_rewards,
    });

    Ok(pending_rewards)
}

#[cfg(test)]
mod tests {
    use joelana_test_utils::joelana_env::{
        actions::token_mill::{
            ClaimStakingRewardsAction, DepositAction,
            SwapAction,TokenMillEnv
        },
        make_address, SwapAmountType, SwapType, 
    };

    const STAKE_AMOUNT: u64 = 100_000_000;

    #[test]
    fn claim_staking_rewards() {
        let mut testing_env = TokenMillEnv::default().with_staking(STAKE_AMOUNT);

        testing_env.svm.change_payer("bob");

        let deposit_action = DepositAction::new(&testing_env,STAKE_AMOUNT);

        let swap_action = SwapAction::new(
            &testing_env,
            SwapType::Buy,
            SwapAmountType::ExactOutput,
            1_000_000_000_000 / 2,
            u64::MAX,
            None,
        );

        testing_env.svm.execute_actions(&[&deposit_action, &swap_action])
            .unwrap();

        let quote_balance_before = testing_env.svm.get_balance(&testing_env.quote_token_mint.unwrap(), &make_address("bob"));

        let claim_rewards_action =
            ClaimStakingRewardsAction::new(&testing_env);

        let result = testing_env.svm.execute_actions(&[&claim_rewards_action]);

        assert!(result.is_ok());

        let quote_balance_after = testing_env.svm.get_balance(&testing_env.quote_token_mint.unwrap(), &make_address("bob"));

        assert!(quote_balance_after > quote_balance_before);
    }
}
