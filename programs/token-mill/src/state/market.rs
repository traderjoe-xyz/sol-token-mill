use std::cmp::min;

use anchor_lang::prelude::*;

use crate::{
    constant::*,
    errors::TokenMillError,
    manager::swap_manager::SwapAmountType,
    math::{div, get_delta_base_in, get_delta_base_out, mul_div, Rounding},
};

pub const MARKET_PDA_SEED: &str = "market";

#[zero_copy]
#[derive(Debug, InitSpace)]
pub struct MarketFees {
    /// staking_fee_share + creator_fee_share + protocol_fee_share = 100%
    pub staking_fee_share: u16,
    pub creator_fee_share: u16,
    _space: u32,

    pub pending_staking_fees: u64,
    pub pending_creator_fees: u64,
}

#[account(zero_copy)]
#[derive(Debug, InitSpace)]
pub struct Market {
    pub config: Pubkey,
    pub creator: Pubkey,

    pub base_token_mint: Pubkey,
    pub quote_token_mint: Pubkey,

    pub base_reserve: u64,

    pub bid_prices: [u64; PRICES_LENGTH],
    pub ask_prices: [u64; PRICES_LENGTH],

    pub width_scaled: u64,
    pub total_supply: u64,

    pub fees: MarketFees,

    pub quote_token_decimals: u8,
    pub bump: u8,

    pub _space: [u8; 6],
}

impl MarketFees {
    pub fn distribute_fee(
        &mut self,
        swap_fee: u64,
        referral_fee_share: Option<u16>,
    ) -> Result<(u64, u64, u64, u64)> {
        let creator_fee = u64::try_from(
            u128::from(swap_fee) * u128::from(self.creator_fee_share) / MAX_BPS as u128,
        )?;
        let staking_fee = u64::try_from(
            u128::from(swap_fee) * u128::from(self.staking_fee_share) / MAX_BPS as u128,
        )?;
        let remaining_fee = swap_fee - creator_fee - staking_fee;

        let referral_fee = if let Some(referral_fee_share) = referral_fee_share {
            u64::try_from(
                u128::from(remaining_fee) * u128::from(referral_fee_share) / MAX_BPS as u128,
            )?
        } else {
            0
        };

        let protocol_fee = remaining_fee - referral_fee;

        self.pending_creator_fees += creator_fee;
        self.pending_staking_fees += staking_fee;

        Ok((creator_fee, staking_fee, protocol_fee, referral_fee))
    }
}

impl Market {
    #[allow(clippy::too_many_arguments)]
    pub fn initialize(
        &mut self,
        bump: u8,
        config: Pubkey,
        creator: Pubkey,
        base_token_mint: Pubkey,
        quote_token_mint: Pubkey,
        quote_token_decimals: u8,
        total_supply: u64,
        creator_fee_share: u16,
        staking_fee_share: u16,
    ) -> Result<()> {
        if total_supply > MAX_TOTAL_SUPPLY
            || total_supply / INTERVAL_NUMBER < BASE_PRECISION
            || (total_supply / INTERVAL_NUMBER) * INTERVAL_NUMBER != total_supply
        {
            return Err(TokenMillError::InvalidTotalSupply.into());
        }

        self.bump = bump;
        self.config = config;
        self.creator = creator;
        self.base_token_mint = base_token_mint;
        self.quote_token_mint = quote_token_mint;
        self.quote_token_decimals = quote_token_decimals;
        self.total_supply = total_supply;
        self.base_reserve = total_supply;
        self.width_scaled = u64::try_from(
            u128::from(total_supply / INTERVAL_NUMBER) * SCALE / u128::from(BASE_PRECISION),
        )?;

        self.fees.creator_fee_share = creator_fee_share;
        self.fees.staking_fee_share = staking_fee_share;
        Ok(())
    }

    pub fn check_and_set_prices(
        &mut self,
        bid_prices: [u64; PRICES_LENGTH],
        ask_prices: [u64; PRICES_LENGTH],
    ) -> Result<()> {
        if self.are_prices_set() {
            return Err(TokenMillError::PricesAlreadySet.into());
        }

        for i in 0..PRICES_LENGTH {
            let bid_price = bid_prices[i];
            let ask_price = ask_prices[i];

            if bid_price > ask_price {
                return Err(TokenMillError::BidAskMismatch.into());
            }

            if i > 0 && (ask_price <= ask_prices[i - 1] || bid_price <= bid_prices[i - 1]) {
                return Err(TokenMillError::DecreasingPrices.into());
            }
        }

        if ask_prices[INTERVAL_NUMBER as usize] > MAX_PRICE {
            return Err(TokenMillError::PriceTooHigh.into());
        }

        self.bid_prices = bid_prices;
        self.ask_prices = ask_prices;

        Ok(())
    }

    pub fn are_prices_set(&self) -> bool {
        self.ask_prices[INTERVAL_NUMBER as usize] != 0
    }

    pub fn circulating_supply(&self) -> u64 {
        self.total_supply - self.base_reserve
    }

    pub fn get_quote_amount(
        &self,
        base_amount: u64,
        swap_amount_type: SwapAmountType,
    ) -> Result<(u64, u64)> {
        let circulating_supply = self.circulating_supply();

        let (supply, rounding) = match swap_amount_type {
            SwapAmountType::ExactInput => (circulating_supply - base_amount, Rounding::Down),
            SwapAmountType::ExactOutput => (circulating_supply, Rounding::Up),
        };

        self.get_quote_amount_with_parameters(supply, base_amount, swap_amount_type, rounding)
    }

    pub fn get_quote_amount_with_parameters(
        &self,
        supply: u64,
        base_amount: u64,
        swap_amount_type: SwapAmountType,
        rounding: Rounding,
    ) -> Result<(u64, u64)> {
        let price_curve = match swap_amount_type {
            SwapAmountType::ExactInput => &self.bid_prices,
            SwapAmountType::ExactOutput => &self.ask_prices,
        };

        let normalized_supply = u128::from(supply) * SCALE / u128::from(BASE_PRECISION);

        let mut normalized_base_amount_left =
            u128::from(base_amount) * SCALE / u128::from(BASE_PRECISION);

        let mut normalized_quote_amount = 0;

        let mut i = usize::try_from(normalized_supply / u128::from(self.width_scaled))?;
        let mut interval_supply_already_used = normalized_supply % u128::from(self.width_scaled);

        let mut price_0 = price_curve[i];
        i += 1;

        while normalized_base_amount_left > 0 && i < PRICES_LENGTH {
            let price_1 = price_curve[i];

            let delta_base = min(
                normalized_base_amount_left,
                u128::from(self.width_scaled) - interval_supply_already_used,
            );

            let delta_quote = mul_div(
                delta_base,
                u128::from(price_1 - price_0) * (delta_base + 2 * interval_supply_already_used)
                    + 2 * u128::from(price_0) * u128::from(self.width_scaled),
                2 * SCALE * u128::from(self.width_scaled),
                rounding,
            )
            .ok_or(TokenMillError::MathError)?;

            normalized_base_amount_left -= delta_base;
            normalized_quote_amount += delta_quote;

            interval_supply_already_used = 0;
            price_0 = price_1;

            i += 1;
        }

        let base_amount_swapped = base_amount
            - div(
                normalized_base_amount_left * u128::from(BASE_PRECISION),
                SCALE,
                rounding,
            )?;

        let quote_amount_swapped = div(
            normalized_quote_amount * u128::pow(10, u32::from(self.quote_token_decimals)),
            SCALE,
            rounding,
        )?;

        Ok((base_amount_swapped, quote_amount_swapped))
    }

    pub fn get_base_amount_in(&self, quote_amount: u64) -> Result<(u64, u64)> {
        let price_curve = &self.bid_prices;
        let circulating_supply = self.circulating_supply();

        let normalized_supply = u128::from(circulating_supply) * SCALE / u128::from(BASE_PRECISION);

        let quote_precision = u128::pow(10, u32::from(self.quote_token_decimals));
        let mut normalized_quote_amount_left = u128::from(quote_amount) * SCALE / quote_precision;
        let mut normalized_base_amount = 0;

        let mut i = usize::try_from(normalized_supply / u128::from(self.width_scaled))?;
        let mut interval_supply_available = normalized_supply % u128::from(self.width_scaled);

        if interval_supply_available == 0 {
            interval_supply_available = u128::from(self.width_scaled);
        } else {
            i += 1;
        }

        let mut price_1 = price_curve[i];

        while normalized_quote_amount_left > 0 && i > 0 {
            let price_0 = price_curve[i - 1];

            let (delta_base, delta_quote) = get_delta_base_in(
                price_0.into(),
                price_1.into(),
                self.width_scaled.into(),
                interval_supply_available,
                normalized_quote_amount_left,
            )?;

            normalized_base_amount += delta_base;
            normalized_quote_amount_left -= delta_quote;

            interval_supply_available = u128::from(self.width_scaled);
            price_1 = price_0;

            i -= 1;
        }

        let base_amount_swapped = div(
            normalized_base_amount * u128::from(BASE_PRECISION),
            SCALE,
            Rounding::Up,
        )?;

        let quote_amount_swapped = quote_amount
            - div(
                normalized_quote_amount_left * quote_precision,
                SCALE,
                Rounding::Up,
            )?;

        Ok((base_amount_swapped, quote_amount_swapped))
    }

    pub fn get_base_amount_out(&self, quote_amount: u64) -> Result<(u64, u64)> {
        let price_curve = &self.ask_prices;
        let circulating_supply = self.circulating_supply();

        let normalized_supply = u128::from(circulating_supply) * SCALE / u128::from(BASE_PRECISION);

        let quote_precision = u128::pow(10, u32::from(self.quote_token_decimals));
        let mut normalized_quote_amount_left = u128::from(quote_amount) * SCALE / quote_precision;
        let mut normalized_base_amount = 0;

        let mut i = usize::try_from(normalized_supply / u128::from(self.width_scaled))?;
        let mut interval_supply_already_used = normalized_supply % u128::from(self.width_scaled);

        let mut price_0 = price_curve[i];

        while normalized_quote_amount_left > 0 && i < PRICES_LENGTH - 1 {
            let price_1 = price_curve[i + 1];

            let (delta_base, delta_quote) = get_delta_base_out(
                price_0.into(),
                price_1.into(),
                self.width_scaled.into(),
                interval_supply_already_used,
                normalized_quote_amount_left,
            )?;

            normalized_base_amount += delta_base;
            normalized_quote_amount_left -= delta_quote;

            interval_supply_already_used = 0;
            price_0 = price_1;

            i += 1;
        }

        let base_amount_swapped = div(
            normalized_base_amount * u128::from(BASE_PRECISION),
            SCALE,
            Rounding::Down,
        )?;

        let quote_amount_swapped = quote_amount
            - div(
                normalized_quote_amount_left * quote_precision,
                SCALE,
                Rounding::Down,
            )?;

        Ok((base_amount_swapped, quote_amount_swapped))
    }
}

#[cfg(test)]
mod tests {
    use anchor_lang::Space;

    use crate::state::Market;

    #[test]
    fn size() {
        let size = Market::INIT_SPACE + 8;

        println!("Size of Market: {}", size);

        assert!(size < 10_240);
    }
}
