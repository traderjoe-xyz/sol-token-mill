use anchor_lang::prelude::*;

use crate::{
    constant::MAX_BPS, errors::TokenMillError, events::TokenMillDefaultFeeSharesUpdateEvent,
};

use super::ConfigUpdate;

pub fn handler(
    ctx: Context<ConfigUpdate>,
    new_default_protocol_fee_share: u16,
    new_referral_fee_share: u16,
) -> Result<()> {
    require!(
        new_default_protocol_fee_share <= MAX_BPS as u16
            && new_referral_fee_share <= MAX_BPS as u16,
        TokenMillError::InvalidFeeShare
    );

    let config = &mut ctx.accounts.config;

    config.default_protocol_fee_share = new_default_protocol_fee_share;
    config.referral_fee_share = new_referral_fee_share;

    emit_cpi!(TokenMillDefaultFeeSharesUpdateEvent {
        config: ctx.accounts.config.key(),
        new_default_protocol_fee_share,
        new_referral_fee_share,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{constant::MAX_BPS, TokenMillConfig};
    use joelana_test_utils::joelana_env::{
        actions::token_mill::{tm_parse_error, TokenMillEnv, UpdateDefaultFeeSharesAction},
        TokenMillError,
    };

    const NEW_DEFAULT_PROTOCOL_FEE_SHARE: u16 = 5_000;
    const NEW_DEFAULT_REFERRAL_FEE_SHARE: u16 = 5_000;

    fn setup_env() -> (TokenMillEnv, UpdateDefaultFeeSharesAction) {
        let testing_env = TokenMillEnv::new();

        let action = UpdateDefaultFeeSharesAction::new(
            NEW_DEFAULT_PROTOCOL_FEE_SHARE,
            NEW_DEFAULT_REFERRAL_FEE_SHARE,
        );

        (testing_env, action)
    }

    #[test]
    fn update_default_fee_shares() {
        let (mut testing_env, action) = setup_env();

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_ok());

        let config_account = testing_env
            .svm
            .get_parsed_account::<TokenMillConfig>(&action.config);

        assert_eq!(
            config_account.default_protocol_fee_share,
            NEW_DEFAULT_PROTOCOL_FEE_SHARE
        );

        assert_eq!(
            config_account.referral_fee_share,
            NEW_DEFAULT_REFERRAL_FEE_SHARE
        );
    }

    #[test]
    fn update_default_protocol_fee_share_with_invalid_value() {
        let (mut testing_env, mut action) = setup_env();

        action.new_default_protocol_fee_share = MAX_BPS as u16 + 1;

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::InvalidFeeShare);
    }

    #[test]
    fn update_default_referral_fee_share_with_invalid_value() {
        let (mut testing_env, mut action) = setup_env();

        action.new_referral_fee_share = MAX_BPS as u16 + 1;

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::InvalidFeeShare);
    }

    #[test]
    fn update_default_fee_shares_with_invalid_signer() {
        let (mut testing_env, mut action) = setup_env();

        action.signer = testing_env.svm.change_payer("mallory");

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::InvalidAuthority);
    }
}
