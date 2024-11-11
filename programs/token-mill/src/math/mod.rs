use anchor_lang::prelude::*;
use ruint::aliases::U256;

use crate::constant::SCALE;

#[derive(PartialEq, Clone, Copy)]
pub enum Rounding {
    Up,
    Down,
}

pub fn get_delta_base_in(
    price_0: u128,
    price_1: u128,
    width_scaled: u128,
    interval_supply_available: u128,
    remaining_quote: u128,
) -> Result<(u128, u128)> {
    let price_diff = price_1 - price_0;

    let current_quote = mul_div(
        interval_supply_available,
        price_diff * interval_supply_available + 2 * price_0 * width_scaled,
        2 * SCALE * width_scaled,
        Rounding::Down,
    )
    .unwrap();

    if remaining_quote >= current_quote {
        Ok((interval_supply_available, current_quote))
    } else {
        let sqrt_discriminant = get_sqrt_discriminant(
            price_diff,
            price_0,
            width_scaled,
            current_quote - remaining_quote,
        )?;

        let rl = price_0 * width_scaled + price_diff * interval_supply_available;
        let delta_base = div(rl - sqrt_discriminant, price_diff, Rounding::Up)?;

        Ok((delta_base.into(), remaining_quote))
    }
}

pub fn get_delta_base_out(
    price_0: u128,
    price_1: u128,
    width_scaled: u128,
    interval_supply_already_used: u128,
    remaining_quote: u128,
) -> Result<(u128, u128)> {
    let price_diff = price_1 - price_0;

    let current_quote = mul_div(
        interval_supply_already_used,
        price_diff * interval_supply_already_used + 2 * price_0 * width_scaled,
        2 * SCALE * width_scaled,
        Rounding::Down,
    )
    .unwrap();

    let next_quote = div((price_0 + price_1) * width_scaled, 2 * SCALE, Rounding::Up)?;

    let max_quote = u128::from(next_quote) - current_quote;

    if remaining_quote >= max_quote {
        Ok((width_scaled - interval_supply_already_used, max_quote))
    } else {
        let sqrt_discriminant = get_sqrt_discriminant(
            price_diff,
            price_0,
            width_scaled,
            current_quote + remaining_quote,
        )?;

        let rr = price_0 * width_scaled + price_diff * interval_supply_already_used;
        let delta_base = div(sqrt_discriminant - rr, price_diff, Rounding::Down)?;

        Ok((delta_base.into(), remaining_quote))
    }
}

pub fn get_sqrt_discriminant(
    price_diff: u128,
    price_0: u128,
    width_scaled: u128,
    current_quote: u128,
) -> Result<u128> {
    let dl = U256::from(width_scaled * price_diff) * U256::from(current_quote * 2 * SCALE);
    let dr = U256::from(price_0 * width_scaled) * U256::from(price_0 * width_scaled);
    let d = dl + dr;
    let sqrt_discriminant = d.root(2);
    Ok(sqrt_discriminant.try_into().unwrap())
}

pub fn mul_div(x: u128, y: u128, denominator: u128, rounding: Rounding) -> Option<u128> {
    if denominator == 0 {
        return None;
    }

    let x = U256::from(x);
    let y = U256::from(y);
    let denominator = U256::from(denominator);

    let prod = x.checked_mul(y)?;

    match rounding {
        Rounding::Up => prod.div_ceil(denominator).try_into().ok(),
        Rounding::Down => {
            let (quotient, _) = prod.div_rem(denominator);
            quotient.try_into().ok()
        }
    }
}

pub fn div(a: u128, b: u128, rounding: Rounding) -> Result<u64> {
    let rounding = if rounding == Rounding::Up && a % b != 0 {
        1
    } else {
        0
    };

    Ok(u64::try_from(a / b)? + rounding)
}
