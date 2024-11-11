use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

use crate::{
    errors::TokenMillError,
    events::TokenMillQuoteTokenBadgeEvent,
    state::{QuoteTokenBadge, QuoteTokenBadgeStatus, TokenMillConfig},
    QUOTE_TOKEN_BADGE_PDA_SEED,
};

#[event_cpi]
#[derive(Accounts)]
pub struct UpdateQuoteAssetBadge<'info> {
    #[account(has_one = authority @ TokenMillError::InvalidAuthority)]
    pub config: Account<'info, TokenMillConfig>,

    #[account(
        mut,
        seeds = [
            QUOTE_TOKEN_BADGE_PDA_SEED.as_bytes(),
            config.key().as_ref(),
            token_mint.key().as_ref(),
        ],
        bump = quote_asset_badge.bump,
    )]
    pub quote_asset_badge: Account<'info, QuoteTokenBadge>,

    pub token_mint: InterfaceAccount<'info, Mint>,

    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<UpdateQuoteAssetBadge>, status: QuoteTokenBadgeStatus) -> Result<()> {
    let quote_asset_badge = &mut ctx.accounts.quote_asset_badge;

    quote_asset_badge.status = status;

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
        actions::token_mill::{
            tm_parse_error, CreateQuoteAssetBadgeAction, TokenMillEnv, UpdateQuoteAssetBadgeAction,
        },
        TokenMillError,
    };

    fn setup_env() -> (TokenMillEnv, UpdateQuoteAssetBadgeAction) {
        let mut testing_env = TokenMillEnv::new().with_default_quote_token_mint();
        let quote_token = testing_env.quote_token_mint.unwrap();

        let create_quote_asset_badge_action = CreateQuoteAssetBadgeAction::new(quote_token);

        testing_env
            .svm
            .execute_actions(&[&create_quote_asset_badge_action])
            .unwrap();

        let action = UpdateQuoteAssetBadgeAction::new(
            quote_token,
            joelana_test_utils::joelana_env::QuoteTokenBadgeStatus::Disabled,
        );

        (testing_env, action)
    }

    #[test]
    fn update_quote_asset_badge() {
        let (mut testing_env, action) = setup_env();

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_ok());

        let quote_asset_badge = testing_env
            .svm
            .get_parsed_account::<QuoteTokenBadge>(&action.quote_asset_badge);

        assert_eq!(quote_asset_badge.status, QuoteTokenBadgeStatus::Disabled);
    }

    #[test]
    fn update_quote_asset_badge_with_invalid_signer() {
        let (mut testing_env, mut action) = setup_env();

        action.signer = testing_env.svm.change_payer("mallory");

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::InvalidAuthority);
    }
}
