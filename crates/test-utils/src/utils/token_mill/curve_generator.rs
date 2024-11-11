use token_mill::constant::{PRICES_LENGTH, SCALE};

const SCALE_EVM: u128 = 1_000_000_000_000_000_000; //1e18

#[derive(Debug, Copy, Clone)]
pub struct Curve {
    pub bid_prices: [u64; PRICES_LENGTH],
    pub ask_prices: [u64; PRICES_LENGTH],
}

impl Default for Curve {
    fn default() -> Self {
        let mut bid_prices = [0; PRICES_LENGTH];
        let mut ask_prices = [0; PRICES_LENGTH];

        for i in 0..PRICES_LENGTH {
            bid_prices[i] = i as u64 * SCALE as u64 * 9 / 10_000;
            ask_prices[i] = i as u64 * SCALE as u64 / 1_000;
        }

        Self {
            bid_prices,
            ask_prices,
        }
    }
}

impl Curve {
    pub fn to_evm(&self) -> (Vec<u128>, Vec<u128>) {
        (
            self.bid_prices
                .iter()
                .map(|p| *p as u128 * SCALE_EVM / SCALE)
                .collect(),
            self.ask_prices
                .iter()
                .map(|p| *p as u128 * SCALE_EVM / SCALE)
                .collect(),
        )
    }
}
