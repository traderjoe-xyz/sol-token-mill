use anchor_lang::prelude::*;

use super::ConfigUpdate;
use crate::events::TokenMillProtocolFeeRecipientUpdateEvent;

pub fn handler(ctx: Context<ConfigUpdate>, new_protocol_fee_recipient: Pubkey) -> Result<()> {
    let config = &mut ctx.accounts.config;

    config.protocol_fee_recipient = new_protocol_fee_recipient;

    emit_cpi!(TokenMillProtocolFeeRecipientUpdateEvent {
        config: ctx.accounts.config.key(),
        new_protocol_fee_recipient,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::TokenMillConfig;
    use joelana_test_utils::joelana_env::{
        actions::token_mill::{tm_parse_error, TokenMillEnv, UpdateProtocolFeeRecipientAction},
        make_address, TokenMillError,
    };

    fn setup_env() -> (TokenMillEnv, UpdateProtocolFeeRecipientAction) {
        let testing_env = TokenMillEnv::new();

        let action =
            UpdateProtocolFeeRecipientAction::new(make_address("new_protocol_fee_recipient"));

        (testing_env, action)
    }

    #[test]
    fn update_protocol_fee_recipient() {
        let (mut testing_env, action) = setup_env();

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_ok());

        let config_account = testing_env
            .svm
            .get_parsed_account::<TokenMillConfig>(&action.config);

        assert_eq!(
            config_account.protocol_fee_recipient,
            make_address("new_protocol_fee_recipient")
        );
    }

    #[test]
    fn update_protocol_fee_recipient_with_invalid_signer() {
        let (mut testing_env, mut action) = setup_env();

        action.signer = testing_env.svm.change_payer("mallory");

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::InvalidAuthority);
    }
}
