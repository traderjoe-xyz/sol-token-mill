use anchor_lang::prelude::*;

use crate::{math::Rounding, state::Market};

#[derive(Debug, AnchorSerialize, AnchorDeserialize, Copy, Clone, PartialEq)]
pub enum SwapType {
    Buy,  // Buy base token
    Sell, // Sell base token
}

#[derive(Debug, AnchorSerialize, AnchorDeserialize, Copy, Clone, PartialEq)]
pub enum SwapAmountType {
    ExactInput,
    ExactOutput,
}

pub fn swap(
    market: &mut Market,
    swap_type: SwapType,
    swap_amount_type: SwapAmountType,
    amount: u64,
) -> Result<(u64, u64, u64)> {
    let (base_amount, quote_amount) = match (swap_type, swap_amount_type) {
        (SwapType::Buy, SwapAmountType::ExactInput) => market.get_base_amount_out(amount)?,
        (SwapType::Buy, SwapAmountType::ExactOutput) => {
            market.get_quote_amount(amount, swap_amount_type)?
        }
        (SwapType::Sell, SwapAmountType::ExactInput) => {
            market.get_quote_amount(amount, swap_amount_type)?
        }
        (SwapType::Sell, SwapAmountType::ExactOutput) => market.get_base_amount_in(amount)?,
    };

    let mut swap_fee = 0;

    match swap_type {
        SwapType::Buy => {
            let (_, buyback_amount) = market.get_quote_amount_with_parameters(
                market.circulating_supply(),
                base_amount,
                SwapAmountType::ExactInput,
                Rounding::Up,
            )?;

            if quote_amount > buyback_amount {
                swap_fee = quote_amount - buyback_amount;
            }

            market.base_reserve -= base_amount;
        }
        SwapType::Sell => {
            market.base_reserve += base_amount;
        }
    }

    Ok((base_amount, quote_amount, swap_fee))
}
