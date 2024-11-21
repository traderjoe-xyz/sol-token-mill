use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::{
    errors::TokenMillError,
    events::TokenMillSwapEvent,
    manager::{
        swap_manager::{self, SwapAmountType, SwapType},
        token_manager::{transfer_from_eoa, transfer_from_pda},
    },
    state::Market,
    TokenMillConfig, MARKET_PDA_SEED,
};

#[event_cpi]
#[derive(Accounts)]
pub struct Swap<'info> {
    pub config: Account<'info, TokenMillConfig>,

    #[account(
        mut,
        has_one = config @ TokenMillError::InvalidConfigAccount,
        has_one = base_token_mint @ TokenMillError::InvalidMintAccount,
        has_one = quote_token_mint @ TokenMillError::InvalidMintAccount
    )]
    pub market: AccountLoader<'info, Market>,

    pub base_token_mint: InterfaceAccount<'info, Mint>,

    pub quote_token_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = base_token_mint,
        associated_token::authority = market,
        associated_token::token_program = base_token_program
    )]
    pub market_base_token_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = quote_token_mint,
        associated_token::authority = market,
        associated_token::token_program = quote_token_program
    )]
    pub market_quote_token_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut, token::mint = base_token_mint)]
    pub user_base_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(mut, token::mint = quote_token_mint)]
    pub user_quote_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = quote_token_mint,
        associated_token::authority = config.protocol_fee_recipient,
        associated_token::token_program = quote_token_program
    )]
    pub protocol_quote_token_ata: InterfaceAccount<'info, TokenAccount>,

    // Referral account can be any token account
    // For UX purposes, LFJ's UI provides the ATA of the `ReferralAccount`, requiring the referrer to claim all the fees he receives
    #[account(mut)]
    pub referral_token_account: Option<InterfaceAccount<'info, TokenAccount>>,

    pub user: Signer<'info>,

    pub base_token_program: Interface<'info, TokenInterface>,

    pub quote_token_program: Interface<'info, TokenInterface>,
}

pub fn handler(
    ctx: Context<Swap>,
    swap_type: SwapType,
    swap_amount_type: SwapAmountType,
    amount: u64,
    other_amount_threshold: u64,
) -> Result<(u64, u64)> {
    if amount == 0 {
        return Err(TokenMillError::InvalidAmount.into());
    }

    let referral_token_account = &ctx.accounts.referral_token_account;

    let base_amount;
    let quote_amount;
    let market_bump;
    let swap_fee;
    let creator_fee;
    let staking_fee;
    let protocol_fee;
    let referral_fee;

    {
        let config = &ctx.accounts.config;
        let market = &mut ctx.accounts.market.load_mut()?;

        (base_amount, quote_amount, swap_fee) =
            swap_manager::swap(market, swap_type, swap_amount_type, amount)?;

        (creator_fee, staking_fee, protocol_fee, referral_fee) = market.fees.distribute_fee(
            swap_fee,
            referral_token_account
                .as_ref()
                .map(|_| config.referral_fee_share),
        )?;

        market_bump = market.bump;
    };

    let user = &ctx.accounts.user;
    let base_token_mint_key = ctx.accounts.base_token_mint.key();
    let seeds = [
        MARKET_PDA_SEED.as_bytes(),
        base_token_mint_key.as_ref(),
        &[market_bump],
    ];

    let (
        amount_in,
        amount_out,
        mint_in,
        mint_out,
        user_account_in,
        user_account_out,
        market_account_in,
        market_account_out,
        token_program_in,
        token_program_out,
    ) = match swap_type {
        SwapType::Buy => (
            quote_amount,
            base_amount,
            &ctx.accounts.quote_token_mint,
            &ctx.accounts.base_token_mint,
            &ctx.accounts.user_quote_token_account,
            &ctx.accounts.user_base_token_account,
            &ctx.accounts.market_quote_token_ata,
            &ctx.accounts.market_base_token_ata,
            &ctx.accounts.quote_token_program,
            &ctx.accounts.base_token_program,
        ),
        SwapType::Sell => (
            base_amount,
            quote_amount,
            &ctx.accounts.base_token_mint,
            &ctx.accounts.quote_token_mint,
            &ctx.accounts.user_base_token_account,
            &ctx.accounts.user_quote_token_account,
            &ctx.accounts.market_base_token_ata,
            &ctx.accounts.market_quote_token_ata,
            &ctx.accounts.base_token_program,
            &ctx.accounts.quote_token_program,
        ),
    };

    match swap_amount_type {
        SwapAmountType::ExactInput => {
            if amount_out < other_amount_threshold {
                return Err(TokenMillError::AmountThresholdNotMet.into());
            }
        }
        SwapAmountType::ExactOutput => {
            if amount_in > other_amount_threshold {
                return Err(TokenMillError::AmountThresholdNotMet.into());
            }
        }
    }

    transfer_from_eoa(
        mint_in,
        user,
        user_account_in,
        market_account_in,
        token_program_in,
        amount_in,
    )?;

    transfer_from_pda(
        mint_out,
        ctx.accounts.market.to_account_info(),
        market_account_out,
        user_account_out,
        token_program_out,
        amount_out,
        &seeds,
    )?;

    if protocol_fee > 0 {
        transfer_from_pda(
            &ctx.accounts.quote_token_mint,
            ctx.accounts.market.to_account_info(),
            &ctx.accounts.market_quote_token_ata,
            &ctx.accounts.protocol_quote_token_ata,
            &ctx.accounts.quote_token_program,
            protocol_fee,
            &seeds,
        )?;
    }

    if let Some(referral_token_account) = referral_token_account {
        if referral_fee > 0 {
            transfer_from_pda(
                &ctx.accounts.quote_token_mint,
                ctx.accounts.market.to_account_info(),
                &ctx.accounts.market_quote_token_ata,
                referral_token_account,
                &ctx.accounts.quote_token_program,
                referral_fee,
                &seeds,
            )?;
        }
    }

    emit_cpi!(TokenMillSwapEvent {
        user: ctx.accounts.user.key(),
        market: ctx.accounts.market.key(),
        swap_type,
        base_amount,
        quote_amount,
        referral_token_account: referral_token_account.as_ref().map(|a| a.key()),
        creator_fee,
        staking_fee,
        protocol_fee,
        referral_fee,
    });

    Ok((base_amount, quote_amount))
}

#[cfg(test)]
mod tests {
    use crate::{manager::swap_manager, Market};
    use joelana_test_utils::joelana_env::{
        actions::token_mill::{tm_parse_error, SwapAction, TokenMillEnv},
        make_address, SwapAmountType, SwapType, TokenMillError, TokenType,
    };
    use rstest::rstest;

    const TOTAL_SUPPLY: u64 = 1_000_000_000_000;

    fn setup_env() -> (TokenMillEnv, SwapAction) {
        let mut testing_env = TokenMillEnv::default();
        testing_env.svm.change_payer("bob");

        let action = SwapAction::new(
            &testing_env,
            SwapType::Buy,
            SwapAmountType::ExactOutput,
            1_000_000_000,
            u64::MAX,
            None,
        );

        (testing_env, action)
    }

    #[rstest]
    fn swap(
        #[values(TokenType::Token, TokenType::Token2022)] base_token_type: TokenType,
        #[values(TokenType::Token, TokenType::Token2022)] quote_token_type: TokenType,
    ) {
        let mut testing_env = TokenMillEnv::new()
            .with_base_token_type(base_token_type)
            .with_quote_token_mint(quote_token_type, 9)
            .with_default_market();
        testing_env.svm.change_payer("bob");

        let mut swap_action = SwapAction::new(
            &testing_env,
            SwapType::Buy,
            SwapAmountType::ExactOutput,
            1_000_000_000,
            u64::MAX,
            None,
        );

        testing_env.svm.execute_actions(&[&swap_action]).unwrap();

        swap_action.swap_type = SwapType::Sell;
        swap_action.amount = 1_000_000;

        testing_env.svm.execute_actions(&[&swap_action]).unwrap();

        let market = testing_env
            .svm
            .get_parsed_account::<Market>(&testing_env.market);
        let base_balance = TOTAL_SUPPLY - market.base_reserve;

        swap_action.swap_amount_type = SwapAmountType::ExactInput;
        swap_action.other_amount_threshold = 0;
        swap_action.amount = base_balance / 2;

        testing_env.svm.execute_actions(&[&swap_action]).unwrap();

        swap_action.swap_type = SwapType::Buy;
        swap_action.amount = 1_000_000_000;

        testing_env.svm.execute_actions(&[&swap_action]).unwrap();
    }

    #[test]
    fn swap_with_invalid_amount_in() {
        let (mut testing_env, mut swap_action) = setup_env();

        swap_action.swap_type = SwapType::Buy;
        swap_action.swap_amount_type = SwapAmountType::ExactOutput;
        swap_action.amount = 1_000_000_000;
        swap_action.other_amount_threshold = u64::MAX;

        testing_env.svm.execute_actions(&[&swap_action]).unwrap();

        swap_action.other_amount_threshold = 0;

        let result = testing_env.svm.execute_actions(&[&swap_action]);

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::AmountThresholdNotMet);

        swap_action.swap_type = SwapType::Sell;
        swap_action.amount = 1_000;

        let result = testing_env.svm.execute_actions(&[&swap_action]);

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::AmountThresholdNotMet);
    }

    #[test]
    fn swap_with_invalid_amount_out() {
        let (mut testing_env, mut swap_action) = setup_env();

        swap_action.swap_type = SwapType::Buy;
        swap_action.swap_amount_type = SwapAmountType::ExactInput;
        swap_action.amount = 1_000_000_000;
        swap_action.other_amount_threshold = 0;

        testing_env.svm.execute_actions(&[&swap_action]).unwrap();

        swap_action.other_amount_threshold = u64::MAX;

        let result = testing_env.svm.execute_actions(&[&swap_action]);

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::AmountThresholdNotMet);

        swap_action.swap_type = SwapType::Sell;
        swap_action.amount = 1_000;

        let result = testing_env.svm.execute_actions(&[&swap_action]);

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::AmountThresholdNotMet);
    }

    #[test]
    fn swap_with_invalid_amount() {
        let (mut testing_env, mut swap_action) = setup_env();

        swap_action.swap_type = SwapType::Buy;
        swap_action.swap_amount_type = SwapAmountType::ExactInput;
        swap_action.amount = 0;
        swap_action.other_amount_threshold = 0;

        let result = testing_env.svm.execute_actions(&[&swap_action]);

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::InvalidAmount);
    }

    #[test]
    fn swap_more_than_total_supply_with_exact_output() {
        let (mut testing_env, mut swap_action) = setup_env();

        swap_action.swap_type = SwapType::Buy;
        swap_action.swap_amount_type = SwapAmountType::ExactOutput;
        swap_action.amount = TOTAL_SUPPLY + 100_000_000;
        swap_action.other_amount_threshold = u64::MAX;

        let result = testing_env.svm.execute_actions(&[&swap_action]);

        assert!(result.is_ok());

        let balance_after = testing_env.svm.get_balance(
            &testing_env.base_token_mint.unwrap(),
            &testing_env.svm.payer,
        );

        assert_eq!(balance_after, TOTAL_SUPPLY);
    }

    #[test]
    fn swap_more_than_total_supply_with_exact_input() {
        let (mut testing_env, mut swap_action) = setup_env();

        let market = testing_env
            .svm
            .get_parsed_account::<Market>(&testing_env.market);

        let (_, amount_to_buy_supply) = market
            .get_quote_amount(TOTAL_SUPPLY, swap_manager::SwapAmountType::ExactOutput)
            .unwrap();

        let quote_balance_before = testing_env.svm.get_balance(
            &testing_env.quote_token_mint.unwrap(),
            &testing_env.svm.payer,
        );

        swap_action.swap_type = SwapType::Buy;
        swap_action.swap_amount_type = SwapAmountType::ExactInput;
        swap_action.amount = amount_to_buy_supply + 100_000_000;
        swap_action.other_amount_threshold = 0;

        let result = testing_env.svm.execute_actions(&[&swap_action]);

        assert!(result.is_ok());

        let balance_after = testing_env.svm.get_balance(
            &testing_env.base_token_mint.unwrap(),
            &testing_env.svm.payer,
        );
        let quote_balance_after = testing_env.svm.get_balance(
            &testing_env.quote_token_mint.unwrap(),
            &testing_env.svm.payer,
        );

        assert_eq!(balance_after, TOTAL_SUPPLY);
        assert_eq!(
            quote_balance_before - quote_balance_after,
            amount_to_buy_supply
        );
    }

    #[test]
    fn sell_more_than_available() {
        let (mut testing_env, mut swap_action) = setup_env();

        swap_action.swap_type = SwapType::Buy;
        swap_action.swap_amount_type = SwapAmountType::ExactOutput;
        swap_action.amount = 1_000_000_000;
        swap_action.other_amount_threshold = u64::MAX;

        testing_env.svm.execute_actions(&[&swap_action]).unwrap();

        let quote_balance = testing_env
            .svm
            .get_balance(&testing_env.quote_token_mint.unwrap(), &testing_env.market);

        swap_action.swap_type = SwapType::Sell;
        swap_action.amount = quote_balance + 100;

        let result = testing_env.svm.execute_actions(&[&swap_action]);

        assert!(result.is_ok());

        let base_balance = testing_env.svm.get_balance(
            &testing_env.base_token_mint.unwrap(),
            &testing_env.svm.payer,
        );

        assert_eq!(base_balance, 0);
    }

    #[test]
    fn swap_with_referral() {
        let (mut testing_env, _) = setup_env();

        let swap_action = SwapAction::new(
            &testing_env,
            SwapType::Buy,
            SwapAmountType::ExactOutput,
            1_000_000_000,
            u64::MAX,
            Some(make_address("carol")),
        );

        testing_env.svm.execute_actions(&[&swap_action]).unwrap();
    }
}
