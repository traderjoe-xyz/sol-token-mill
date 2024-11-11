use anchor_lang::prelude::*;

use crate::{
    constant::MAX_BPS, errors::TokenMillError, events::TokenMillConfigCreationEvent,
    state::TokenMillConfig,
};

#[event_cpi]
#[derive(Accounts)]
pub struct CreateConfig<'info> {
    #[account(init, payer = payer, space = 8 + TokenMillConfig::INIT_SPACE)]
    pub config: Account<'info, TokenMillConfig>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<CreateConfig>,
    authority: Pubkey,
    protocol_fee_recipient: Pubkey,
    protocol_fee_share: u16,
    referral_fee_share: u16,
) -> Result<()> {
    require!(
        protocol_fee_share <= MAX_BPS as u16 && referral_fee_share <= MAX_BPS as u16,
        TokenMillError::InvalidFeeShare
    );

    let config = &mut ctx.accounts.config;

    config.initialize(
        authority,
        protocol_fee_recipient,
        protocol_fee_share,
        referral_fee_share,
    )?;

    emit_cpi!(TokenMillConfigCreationEvent {
        config: ctx.accounts.config.key(),
        authority,
        default_protocol_fee_share: protocol_fee_share,
        referral_fee_share,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{TokenMillConfig, MAX_BPS};

    use joelana_test_utils::joelana_env::{
        actions::token_mill::{tm_parse_error, CreateConfigAction},
        JoelanaEnv, TokenMillError,
    };

    fn setup_env() -> (JoelanaEnv, CreateConfigAction) {
        let mut env = JoelanaEnv::new();
        env.add_token_mill_program();

        let action = CreateConfigAction::new();

        (env, action)
    }

    #[test]
    fn create_config() {
        let (mut env, action) = setup_env();

        let result = env.execute_actions(&[&action]);

        assert!(result.is_ok());

        let config_account = env.get_parsed_account::<TokenMillConfig>(&action.config);

        assert_eq!(config_account.authority, env.payer);
        assert_eq!(config_account.pending_authority, None);
    }

    #[test]
    fn create_config_with_invalid_protocol_fee_share() {
        let (mut env, mut action) = setup_env();

        action.protocol_fee_share = MAX_BPS as u16 + 1;

        let result = env.execute_actions(&[&action]);

        assert!(result.is_err());

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::InvalidFeeShare);
    }
}
