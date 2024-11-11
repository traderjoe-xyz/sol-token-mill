use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::{
    errors::TokenMillError, events::TokenMillCreatorFeeClaimEvent,
    manager::token_manager::transfer_from_pda, state::Market, MARKET_PDA_SEED,
};

#[event_cpi]
#[derive(Accounts)]
pub struct ClaimCreatorFees<'info> {
    #[account(
        mut,
        has_one = creator @ TokenMillError::InvalidAuthority,
        has_one = quote_token_mint @ TokenMillError::InvalidQuoteTokenMint
    )]
    pub market: AccountLoader<'info, Market>,

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
        associated_token::authority = creator,
        associated_token::token_program = quote_token_program
    )]
    pub creator_quote_token_ata: InterfaceAccount<'info, TokenAccount>,

    pub creator: Signer<'info>,

    pub quote_token_program: Interface<'info, TokenInterface>,
}

pub fn handler(ctx: Context<ClaimCreatorFees>) -> Result<()> {
    let (pending_fees, base_token_mint, bump) = {
        let market = &mut ctx.accounts.market.load_mut()?;

        let pending_fees = market.fees.pending_creator_fees;
        market.fees.pending_creator_fees = 0;

        (pending_fees, market.base_token_mint, market.bump)
    };

    let market_seeds = [
        MARKET_PDA_SEED.as_bytes(),
        base_token_mint.as_ref(),
        &[bump],
    ];

    transfer_from_pda(
        &ctx.accounts.quote_token_mint,
        ctx.accounts.market.to_account_info(),
        &ctx.accounts.market_quote_token_ata,
        &ctx.accounts.creator_quote_token_ata,
        &ctx.accounts.quote_token_program,
        pending_fees,
        &market_seeds,
    )?;

    emit_cpi!(TokenMillCreatorFeeClaimEvent {
        market: ctx.accounts.market.key(),
        creator: ctx.accounts.creator.key(),
        fees_distributed: pending_fees,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::Market;
    use joelana_test_utils::joelana_env::{
        actions::token_mill::{
            tm_parse_error, ClaimCreatorFeesAction, SwapAction, TokenMillEnv, DEFAULT_TOTAL_SUPPLY,
        },
        SwapAmountType, SwapType, TokenMillError, TokenType,
    };
    use rstest::rstest;

    fn setup_env(token_type: TokenType) -> (TokenMillEnv, ClaimCreatorFeesAction) {
        let mut testing_env = TokenMillEnv::new()
            .with_quote_token_mint(token_type, 9)
            .with_default_market();

        testing_env.svm.change_payer("bob");

        let swap_action = SwapAction::new(
            &testing_env,
            SwapType::Buy,
            SwapAmountType::ExactOutput,
            DEFAULT_TOTAL_SUPPLY / 2,
            u64::MAX,
            None,
        );

        testing_env.svm.execute_actions(&[&swap_action]).unwrap();

        testing_env.svm.change_payer("alice");

        let action = ClaimCreatorFeesAction::new(&testing_env);

        (testing_env, action)
    }

    #[rstest]
    fn claim_creator_fees(#[values(TokenType::Token, TokenType::Token2022)] token_type: TokenType) {
        let (mut testing_env, action) = setup_env(token_type);

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_ok());

        let market = testing_env.svm.get_parsed_account::<Market>(&action.market);

        assert_eq!(market.fees.pending_creator_fees, 0);
    }

    #[test]
    fn claim_creator_fees_with_invalid_creator() {
        let (mut testing_env, mut action) = setup_env(TokenType::Token);

        action.signer = testing_env.svm.change_payer("mallory");

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::InvalidAuthority);
    }
}
