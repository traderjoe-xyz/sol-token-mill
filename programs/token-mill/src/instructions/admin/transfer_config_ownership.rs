use anchor_lang::prelude::*;

use crate::{errors::TokenMillError, state::TokenMillConfig};

#[event_cpi]
#[derive(Accounts)]
pub struct ConfigUpdate<'info> {
    #[account(mut, has_one = authority @ TokenMillError::InvalidAuthority)]
    pub config: Account<'info, TokenMillConfig>,

    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<ConfigUpdate>, new_authority: Option<Pubkey>) -> Result<()> {
    let config = &mut ctx.accounts.config;

    config.pending_authority = new_authority;

    Ok(())
}

#[cfg(test)]
mod tests {
    use joelana_test_utils::joelana_env::{
        actions::token_mill::{tm_parse_error, TokenMillEnv, TransferConfigOwnershipAction},
        make_address, TokenMillError,
    };

    use crate::TokenMillConfig;

    fn setup_env() -> (TokenMillEnv, TransferConfigOwnershipAction) {
        let testing_env = TokenMillEnv::new();

        let action = TransferConfigOwnershipAction::new(Some(make_address("alice")));

        (testing_env, action)
    }

    #[test]
    fn transfer_config_ownership() {
        let (mut testing_env, action) = setup_env();

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_ok());

        let config_account = testing_env
            .svm
            .get_parsed_account::<TokenMillConfig>(&action.config);

        assert_eq!(
            config_account.pending_authority,
            Some(make_address("alice"))
        );
    }

    #[test]
    fn cancel_transfer_config_ownership() {
        let (mut testing_env, mut action) = setup_env();

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_ok());

        action.pending_authority = None;

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_ok());

        let config_account = testing_env
            .svm
            .get_parsed_account::<TokenMillConfig>(&action.config);

        assert_eq!(config_account.pending_authority, None);
    }

    #[test]
    fn transfer_config_ownership_with_invalid_authority() {
        let (mut testing_env, mut action) = setup_env();

        let invalid_authority = testing_env.svm.change_payer("mallory");
        action.signer = invalid_authority;
        action.pending_authority = Some(invalid_authority);

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::InvalidAuthority);
    }
}
