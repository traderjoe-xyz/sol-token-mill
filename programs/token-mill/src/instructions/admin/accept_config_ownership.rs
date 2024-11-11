use anchor_lang::prelude::*;

use crate::{
    errors::TokenMillError, events::TokenMillConfigOwnershipTransferEvent, state::TokenMillConfig,
};

#[event_cpi]
#[derive(Accounts)]
pub struct AcceptConfigOwnership<'info> {
    #[account(mut, constraint = config.pending_authority == Some(pending_authority.key()) @ TokenMillError::InvalidAuthority)]
    pub config: Account<'info, TokenMillConfig>,

    pub pending_authority: Signer<'info>,
}

pub fn handler(ctx: Context<AcceptConfigOwnership>) -> Result<()> {
    let config = &mut ctx.accounts.config;

    config.authority = ctx.accounts.pending_authority.key();
    config.pending_authority = None;

    emit_cpi!(TokenMillConfigOwnershipTransferEvent {
        config: config.key(),
        new_authority: config.authority,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::TokenMillConfig;

    use joelana_test_utils::joelana_env::{
        actions::token_mill::{
            tm_parse_error, AcceptConfigOwnershipAction, TokenMillEnv,
            TransferConfigOwnershipAction,
        },
        make_address, TokenMillError,
    };

    fn setup_env() -> (TokenMillEnv, AcceptConfigOwnershipAction) {
        let mut testing_env = TokenMillEnv::new();

        let new_authority = make_address("alice");

        let transfer_config_action = TransferConfigOwnershipAction::new(Some(new_authority));

        testing_env
            .svm
            .execute_actions(&[&transfer_config_action])
            .unwrap();

        let action = AcceptConfigOwnershipAction::new(new_authority);

        testing_env.svm.change_payer("alice");

        (testing_env, action)
    }

    #[test]
    fn accept_config_ownership() {
        let (mut testing_env, action) = setup_env();

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_ok());

        let config_account = testing_env
            .svm
            .get_parsed_account::<TokenMillConfig>(&action.config);

        assert_eq!(config_account.authority, make_address("alice"));
    }

    #[test]
    fn accept_config_ownership_with_invalid_signer() {
        let (mut testing_env, mut action) = setup_env();

        action.signer = testing_env.svm.change_payer("mallory");

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::InvalidAuthority);
    }
}
