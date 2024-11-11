use anchor_lang::prelude::*;

use crate::{errors::TokenMillError, events::TokenMillMarketFeeSharesUpdateEvent};

use super::MarketSettingsUpdate;

pub fn handler(
    ctx: Context<MarketSettingsUpdate>,
    new_creator_fee_share: u16,
    new_staking_fee_share: u16,
) -> Result<()> {
    let market = &mut ctx.accounts.market.load_mut()?;

    // Total fee share (creator + staking + protocol fee shares) must always be 100%,
    // and since the protocol fee share cannot be changed after market creation,
    // creator_fee_share + staking_fee_share must be constant.
    require_eq!(
        new_creator_fee_share + new_staking_fee_share,
        market.fees.creator_fee_share + market.fees.staking_fee_share,
        TokenMillError::InvalidFeeShare
    );

    market.fees.creator_fee_share = new_creator_fee_share;
    market.fees.staking_fee_share = new_staking_fee_share;

    emit_cpi!(TokenMillMarketFeeSharesUpdateEvent {
        market: ctx.accounts.market.key(),
        new_creator_fee_share,
        new_staking_fee_share,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::Market;
    use joelana_test_utils::joelana_env::{
        actions::token_mill::{tm_parse_error, TokenMillEnv, UpdateMarketFeeSharesAction},
        TokenMillError,
    };

    const NEW_CREATOR_FEE_SHARE: u16 = 3_000;
    const NEW_STAKING_FEE_SHARE: u16 = 6_000;

    fn setup_env() -> (TokenMillEnv, UpdateMarketFeeSharesAction) {
        let mut testing_env = TokenMillEnv::default();
        testing_env.svm.change_payer("alice");

        let action = UpdateMarketFeeSharesAction::new(NEW_CREATOR_FEE_SHARE, NEW_STAKING_FEE_SHARE);

        (testing_env, action)
    }

    #[test]
    fn update_market_fee_shares() {
        let (mut testing_env, action) = setup_env();

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_ok());

        let market = testing_env
            .svm
            .get_parsed_account::<Market>(&testing_env.market);

        assert_eq!(market.fees.creator_fee_share, NEW_CREATOR_FEE_SHARE);
        assert_eq!(market.fees.staking_fee_share, NEW_STAKING_FEE_SHARE);
    }

    #[test]
    fn update_market_fee_shares_with_invalid_distribution() {
        let (mut testing_env, mut action) = setup_env();

        action.new_staking_fee_share += 1;

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let err = tm_parse_error(result).unwrap();

        assert_eq!(err, TokenMillError::InvalidFeeShare);
    }

    #[test]
    fn update_market_fee_shares_with_invalid_signer() {
        let (mut testing_env, mut action) = setup_env();

        action.signer = testing_env.svm.change_payer("mallory");

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let err = tm_parse_error(result).unwrap();

        assert_eq!(err, TokenMillError::InvalidAuthority);
    }
}
