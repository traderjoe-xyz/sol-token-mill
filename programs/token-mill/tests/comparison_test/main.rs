use std::cell::RefCell;

use proptest::prelude::*;

use fixture::{Fixture, SwapScenario};
use joelana_test_utils::utils::token_mill::curve_generator::Curve;

use solana_sdk::native_token::sol_to_lamports;
use token_mill::{
    constant::*,
    manager::swap_manager::{SwapAmountType, SwapType},
};

mod evm_engine;
pub mod fixture;
mod svm_engine;

fn fuzz_supply() -> impl Strategy<Value = u64> {
    let minimal_interval_width = BASE_PRECISION;
    let maximal_interval_width = MAX_TOTAL_SUPPLY / INTERVAL_NUMBER;

    (minimal_interval_width..maximal_interval_width)
        .prop_map(|interval_width| interval_width * INTERVAL_NUMBER)
}

#[test]
fn initial_buy_with_exact_out() {
    fn fuzz_supply_and_swap_amount() -> impl Strategy<Value = (u64, u64)> {
        fuzz_supply().prop_flat_map(|supply| (Just(supply), 1..supply - 1))
    }

    let fixture_ref = RefCell::new(Fixture::new());

    proptest!(|((supply, swap_amount) in fuzz_supply_and_swap_amount())| {
        let mut fixture = fixture_ref.borrow_mut();

        fixture.create_markets(supply, Curve::default());

        fixture.swap_and_compare(SwapScenario {
            swap_type: SwapType::Buy,
            swap_amount_type: SwapAmountType::ExactOutput,
            amount: swap_amount,
        });
    });
}

#[test]
fn initial_buy_with_exact_in() {
    fn fuzz_supply_and_swap_amount() -> impl Strategy<Value = (u64, u64)> {
        fuzz_supply().prop_flat_map(|supply| (Just(supply), 1..supply - 1))
    }

    let fixture_ref = RefCell::new(Fixture::new());

    proptest!(|((supply, swap_amount_base) in fuzz_supply_and_swap_amount())| {
        let mut fixture = fixture_ref.borrow_mut();

        fixture.create_markets(supply, Curve::default());

        let market = fixture.svm_engine.get_market();
        let (_, swap_amount_quote) = market.get_quote_amount(swap_amount_base, SwapAmountType::ExactOutput)?;

        fixture.swap_and_compare(SwapScenario {
            swap_type: SwapType::Buy,
            swap_amount_type: SwapAmountType::ExactInput,
            amount: swap_amount_quote,
        });
    });
}

#[test]
fn sell_with_exact_in() {
    const SUPPLY: u64 = 1_000_000_000_000;

    fn fuzz_position_and_swap_amount() -> impl Strategy<Value = (u64, u64)> {
        (1..SUPPLY).prop_flat_map(|swap_position| (Just(swap_position), 0..swap_position - 1))
    }

    let fixture_ref = RefCell::new(Fixture::new());

    proptest!(|((curve_position, swap_amount) in fuzz_position_and_swap_amount())| {
        let mut fixture = fixture_ref.borrow_mut();

        fixture.create_markets(SUPPLY, Curve::default());

        fixture.swap_and_compare(SwapScenario {
            swap_type: SwapType::Buy,
            swap_amount_type: SwapAmountType::ExactOutput,
            amount: curve_position,
        });

        fixture.swap_and_compare(SwapScenario {
            swap_type: SwapType::Sell,
            swap_amount_type: SwapAmountType::ExactInput,
            amount: swap_amount,
        });
    });
}

#[test]
fn buy_with_exact_out() {
    const SUPPLY: u64 = 1_000_000_000_000;

    fn fuzz_position_and_swap_amount() -> impl Strategy<Value = (u64, u64)> {
        (1..SUPPLY).prop_flat_map(|swap_position| (Just(swap_position), 0..SUPPLY - swap_position))
    }

    let fixture_ref = RefCell::new(Fixture::new());

    proptest!(|((curve_position, swap_amount) in fuzz_position_and_swap_amount())| {
        let mut fixture = fixture_ref.borrow_mut();

        fixture.create_markets(SUPPLY, Curve::default());

        fixture.swap_and_compare(SwapScenario {
            swap_type: SwapType::Buy,
            swap_amount_type: SwapAmountType::ExactOutput,
            amount: curve_position,
        });

        fixture.swap_and_compare(SwapScenario {
            swap_type: SwapType::Buy,
            swap_amount_type: SwapAmountType::ExactOutput,
            amount: swap_amount,
        });
    });
}

#[test]
fn sell_with_exact_out() {
    const SUPPLY: u64 = 1_000_000_000_000;

    let fixture_ref = RefCell::new(Fixture::new());

    proptest!(|((curve_position, swap_amount_fraction) in (1..SUPPLY, 1..100_000))| {
        let mut fixture = fixture_ref.borrow_mut();

        fixture.create_markets(SUPPLY, Curve::default());

        fixture.swap_and_compare(SwapScenario {
            swap_type: SwapType::Buy,
            swap_amount_type: SwapAmountType::ExactOutput,
            amount: curve_position,
        });

        let market = fixture.svm_engine.get_market();
        let (_, max_quote_amount) = market
            .get_quote_amount(market.circulating_supply(), SwapAmountType::ExactInput)?;

        let swap_amount =
            (swap_amount_fraction as u128 * max_quote_amount as u128 / 100_000) as u64 + 1;

        // Swapping very small amounts gives an imprecise result, but svm result is still higher than evm so it won't cause any loss for the protocol
        if swap_amount < sol_to_lamports(0.0001) {
            fixture.swap_and_compare_allow_imprecision(
                SwapScenario {
                    swap_type: SwapType::Sell,
                    swap_amount_type: SwapAmountType::ExactOutput,
                    amount: swap_amount,
                },
                10_000,
            );
        } else if swap_amount < sol_to_lamports(0.01) {
            fixture.swap_and_compare_allow_imprecision(
                SwapScenario {
                    swap_type: SwapType::Sell,
                    swap_amount_type: SwapAmountType::ExactOutput,
                    amount: swap_amount,
                },
                100,
            );
        } else {
            fixture.swap_and_compare_allow_imprecision(
                SwapScenario {
                    swap_type: SwapType::Sell,
                    swap_amount_type: SwapAmountType::ExactOutput,
                    amount: swap_amount,
                },
                10,
            );
        }
    });
}

#[test]
fn buy_with_exact_in() {
    const SUPPLY: u64 = 1_000_000_000_000;

    let fixture_ref = RefCell::new(Fixture::new());

    proptest!(|((curve_position, swap_amount_fraction) in (0..SUPPLY-1, 1..100_000))| {
        let mut fixture = fixture_ref.borrow_mut();

        fixture.create_markets(SUPPLY, Curve::default());

        fixture.swap_and_compare(SwapScenario {
            swap_type: SwapType::Buy,
            swap_amount_type: SwapAmountType::ExactOutput,
            amount: curve_position,
        });


        let market = fixture.svm_engine.get_market();
        let (_, max_quote_amount) = market.get_quote_amount(market.base_reserve, SwapAmountType::ExactOutput)?;

       let swap_amount = (swap_amount_fraction as u128 * max_quote_amount as u128 / 100_000) as u64 + 1;

        fixture.swap_and_compare(SwapScenario {
            swap_type: SwapType::Buy,
            swap_amount_type: SwapAmountType::ExactInput,
            amount: swap_amount,
        });
    });
}

#[test]
fn fees() {
    const SUPPLY: u64 = 1_000_000_000_000;

    let scenarios = vec![
        SwapScenario {
            swap_type: SwapType::Buy,
            swap_amount_type: SwapAmountType::ExactOutput,
            amount: 500_000_000,
        },
        SwapScenario {
            swap_type: SwapType::Sell,
            swap_amount_type: SwapAmountType::ExactInput,
            amount: 20_000_000,
        },
        SwapScenario {
            swap_type: SwapType::Sell,
            swap_amount_type: SwapAmountType::ExactInput,
            amount: 300_000_000,
        },
        SwapScenario {
            swap_type: SwapType::Buy,
            swap_amount_type: SwapAmountType::ExactOutput,
            amount: 833_333_333,
        },
    ];

    let mut fixture = Fixture::new();

    fixture.create_markets(SUPPLY, Curve::default());

    for scenario in scenarios.clone() {
        fixture.swap_and_compare(scenario);
    }

    fixture.claim_fees_and_compare();

    for scenario in scenarios {
        fixture.swap_and_compare(scenario);
    }

    fixture.claim_fees_and_compare();
}

#[test]
fn staking() {
    const SUPPLY: u64 = 1_000_000_000_000;

    let scenarios = vec![
        SwapScenario {
            swap_type: SwapType::Buy,
            swap_amount_type: SwapAmountType::ExactOutput,
            amount: 500_000_000,
        },
        SwapScenario {
            swap_type: SwapType::Sell,
            swap_amount_type: SwapAmountType::ExactInput,
            amount: 20_000_000,
        },
        SwapScenario {
            swap_type: SwapType::Sell,
            swap_amount_type: SwapAmountType::ExactInput,
            amount: 300_000_000,
        },
        SwapScenario {
            swap_type: SwapType::Buy,
            swap_amount_type: SwapAmountType::ExactOutput,
            amount: 833_333_333,
        },
    ];

    let mut fixture = Fixture::new();

    fixture.create_markets(SUPPLY, Curve::default());

    let amount_staked = 500_000_000;

    fixture.swap(SwapScenario {
        swap_type: SwapType::Buy,
        swap_amount_type: SwapAmountType::ExactOutput,
        amount: amount_staked,
    });

    fixture.deposit(amount_staked);

    for scenario in scenarios.clone() {
        fixture.swap(scenario);
    }

    fixture.claim_staking_rewards_and_compare();

    fixture.withdraw(amount_staked / 2);

    for scenario in scenarios {
        fixture.swap(scenario);
    }

    fixture.claim_staking_rewards_and_compare();
}
