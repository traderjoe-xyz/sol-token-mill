use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

use crate::{
    errors::TokenMillError,
    events::TokenMillQuoteTokenBadgeEvent,
    state::{QuoteTokenBadge, TokenMillConfig},
    QUOTE_TOKEN_BADGE_PDA_SEED,
};

#[event_cpi]
#[derive(Accounts)]
pub struct CreateQuoteAssetBadge<'info> {
    #[account(has_one = authority @ TokenMillError::InvalidAuthority)]
    pub config: Account<'info, TokenMillConfig>,

    #[account(
        init,
        seeds = [
            QUOTE_TOKEN_BADGE_PDA_SEED.as_bytes(),
            config.key().as_ref(),
            token_mint.key().as_ref(),
        ],
        bump,
        payer = authority,
        space = 8 + QuoteTokenBadge::INIT_SPACE
    )]
    pub quote_asset_badge: Account<'info, QuoteTokenBadge>,

    pub token_mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CreateQuoteAssetBadge>) -> Result<()> {
    let quote_asset_badge = &mut ctx.accounts.quote_asset_badge;

    quote_asset_badge.initialize(ctx.bumps.quote_asset_badge)?;

    emit_cpi!(TokenMillQuoteTokenBadgeEvent {
        config: ctx.accounts.config.key(),
        quote_token_mint: ctx.accounts.token_mint.key(),
        quote_asset_badge_status: quote_asset_badge.status,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{QuoteTokenBadge, QuoteTokenBadgeStatus};
    use joelana_test_utils::joelana_env::{
        actions::token_mill::{tm_parse_error, CreateQuoteAssetBadgeAction, TokenMillEnv},
        TokenMillError, TokenType,
    };
    use rstest::rstest;

    #[rstest]
    fn create_quote_asset_badge(
        #[values(TokenType::Token, TokenType::Token2022)] token_type: TokenType,
    ) {
        let mut testing_env = TokenMillEnv::new().with_quote_token_mint(token_type, 9);
        let action = CreateQuoteAssetBadgeAction::new(testing_env.quote_token_mint.unwrap());

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_ok());

        let quote_asset_badge = testing_env
            .svm
            .get_parsed_account::<QuoteTokenBadge>(&action.quote_asset_badge);

        assert_eq!(quote_asset_badge.status, QuoteTokenBadgeStatus::Enabled);
    }

    #[test]
    fn create_quote_asset_badge_with_invalid_signer() {
        let mut testing_env = TokenMillEnv::new().with_default_quote_token_mint();
        let mut action = CreateQuoteAssetBadgeAction::new(testing_env.quote_token_mint.unwrap());

        action.signer = testing_env.svm.change_payer("mallory");

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::InvalidAuthority);
    }
}
