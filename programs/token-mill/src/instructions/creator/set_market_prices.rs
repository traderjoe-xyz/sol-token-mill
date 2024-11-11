use anchor_lang::prelude::*;

use crate::{
    constant::PRICES_LENGTH, errors::TokenMillError, events::TokenMillMarketPriceSetEvent,
    state::Market,
};

#[event_cpi]
#[derive(Accounts)]
pub struct MarketSettingsUpdate<'info> {
    #[account(mut, has_one = creator @ TokenMillError::InvalidAuthority)]
    pub market: AccountLoader<'info, Market>,

    pub creator: Signer<'info>,
}

pub fn handler(
    ctx: Context<MarketSettingsUpdate>,
    bid_prices: [u64; PRICES_LENGTH],
    ask_prices: [u64; PRICES_LENGTH],
) -> Result<()> {
    let market = &mut ctx.accounts.market.load_mut()?;

    market.check_and_set_prices(bid_prices, ask_prices)?;

    emit_cpi!(TokenMillMarketPriceSetEvent {
        market: ctx.accounts.market.key(),
        bid_prices,
        ask_prices,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        constant::{INTERVAL_NUMBER, MAX_PRICE},
        Market,
    };
    use joelana_test_utils::{
        joelana_env::{
            actions::token_mill::{
                tm_parse_error, CreateMarketAction, CreateQuoteAssetBadgeAction,
                SetMarketPricesAction, TokenMillEnv,
            },
            TokenMillError,
        },
        utils::token_mill::curve_generator::Curve,
    };

    fn setup_env() -> (TokenMillEnv, SetMarketPricesAction) {
        let mut testing_env = TokenMillEnv::new().with_default_quote_token_mint();

        let action = CreateQuoteAssetBadgeAction::new(testing_env.quote_token_mint.unwrap());

        testing_env.svm.execute_actions(&[&action]).unwrap();

        testing_env.svm.change_payer("alice");

        let create_market_action = CreateMarketAction::new(&testing_env);

        testing_env
            .svm
            .execute_actions(&[&create_market_action])
            .unwrap();

        let action = SetMarketPricesAction::new(Curve::default());

        (testing_env, action)
    }

    #[test]
    fn set_market_prices() {
        let (mut testing_env, action) = setup_env();

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_ok());

        let market = testing_env.svm.get_parsed_account::<Market>(&action.market);

        assert!(market.are_prices_set());

        assert_eq!(market.bid_prices[2], action.price_curve.bid_prices[2]);
        assert_eq!(market.ask_prices[2], action.price_curve.ask_prices[2]);
    }

    #[test]
    fn set_market_prices_twice() {
        let (mut testing_env, action) = setup_env();

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_ok());

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::PricesAlreadySet);
    }

    #[test]
    fn set_market_prices_with_bid_ask_mismatch() {
        let (mut testing_env, mut action) = setup_env();

        action.price_curve.bid_prices[2] = action.price_curve.ask_prices[2] + 1;

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::BidAskMismatch);
    }

    #[test]
    fn set_market_prices_with_decreasing_bid_prices() {
        let (mut testing_env, mut action) = setup_env();

        action.price_curve.bid_prices[2] = action.price_curve.bid_prices[3] + 1;
        action.price_curve.ask_prices[2] = action.price_curve.bid_prices[2];

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::DecreasingPrices);
    }

    #[test]
    fn set_market_prices_with_decreasing_ask_prices() {
        let (mut testing_env, mut action) = setup_env();

        action.price_curve.ask_prices[2] = action.price_curve.ask_prices[3] + 1;

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::DecreasingPrices);
    }

    #[test]
    fn set_market_prices_with_price_too_high() {
        let (mut testing_env, mut action) = setup_env();

        action.price_curve.ask_prices[INTERVAL_NUMBER as usize] = MAX_PRICE + 1;

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::PriceTooHigh);
    }

    #[test]
    fn set_market_prices_with_invalid_signer() {
        let (mut testing_env, mut action) = setup_env();

        action.signer = testing_env.svm.change_payer("mallory");

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::InvalidAuthority);
    }
}
