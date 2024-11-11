use anchor_lang::prelude::*;

use crate::events::TokenMillCreatorUpdateEvent;

use super::MarketSettingsUpdate;

pub fn handler(ctx: Context<MarketSettingsUpdate>, new_creator: Pubkey) -> Result<()> {
    let market = &mut ctx.accounts.market.load_mut()?;

    market.creator = new_creator;

    emit_cpi!(TokenMillCreatorUpdateEvent {
        market: ctx.accounts.market.key(),
        new_creator,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::Market;
    use joelana_test_utils::joelana_env::{
        actions::token_mill::{tm_parse_error, TokenMillEnv, UpdateCreatorAction},
        TokenMillError,
    };
    use solana_sdk::pubkey::Pubkey;

    #[test]
    fn update_creator() {
        let mut testing_env = TokenMillEnv::default();

        let new_creator = Pubkey::new_unique();

        testing_env.svm.change_payer("alice");

        let action = UpdateCreatorAction::new(new_creator);

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_ok());

        let market = testing_env
            .svm
            .get_parsed_account::<Market>(&testing_env.market);

        assert_eq!(market.creator, new_creator);
    }

    #[test]
    fn update_creator_with_invalid_signer() {
        let mut testing_env = TokenMillEnv::default();

        let new_creator = testing_env.svm.change_payer("mallory");

        let mut action = UpdateCreatorAction::new(new_creator);
        action.signer = new_creator;

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let err = tm_parse_error(result).unwrap();

        assert_eq!(err, TokenMillError::InvalidAuthority);
    }
}
