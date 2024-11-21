use joelana_test_utils::joelana_env::{
    actions::token_mill::{
        ClaimCreatorFeesAction, ClaimStakingRewardsAction, CreateMarketWithSplAction,
        CreateVestingPlanAction, DepositAction, ReleaseAction, SwapAction, TokenMillEnv,
        WithdrawAction,
    },
    SwapAmountType, SwapType,
};
use solana_sdk::pubkey::Pubkey;

// v1 -> v1.1
// v1 : Original deployment
// v1.1 : commit 11faa567a21ea1c94cd63c6d0dcd1accd0ed0a1d
#[test]
fn upgrade_v_1_1() {
    let mut env = TokenMillEnv::new_with_custom_program(
        "../../programs/token-mill/tests/upgrades/versions/token_mill_v1.so",
    )
    .with_default_quote_token_mint()
    .with_default_market()
    .with_staking(1_000_000_000);

    env.svm
        .execute_actions(&[&SwapAction::new(
            &env,
            SwapType::Buy,
            SwapAmountType::ExactInput,
            1_000_000_000,
            0,
            None,
        )])
        .unwrap();

    // Upgrade
    env.svm.set_token_mill_program_from_binary(
        "../../programs/token-mill/tests/upgrades/versions/token_mill_v1_1.so",
    );

    // Check that the new feature is effective
    let original_base_token_mint = env.base_token_mint;
    env.base_token_mint = Some(Pubkey::new_unique());
    env.svm.change_payer("alice");
    env.svm
        .execute_actions(&[CreateMarketWithSplAction::new(&env).no_badge()])
        .unwrap();

    env.svm.change_payer("bob");
    env.base_token_mint = original_base_token_mint;

    // Swaps
    env.svm
        .execute_actions(&[
            &SwapAction::new(
                &env,
                SwapType::Buy,
                SwapAmountType::ExactInput,
                1_000_000_000,
                0,
                None,
            ),
            &SwapAction::new(
                &env,
                SwapType::Sell,
                SwapAmountType::ExactOutput,
                1_000_000_000,
                u64::MAX,
                None,
            ),
        ])
        .unwrap();

    // Staking
    env.svm
        .execute_actions(&[
            &DepositAction::new(&env, 1_000_000_000),
            &SwapAction::new(
                &env,
                SwapType::Buy,
                SwapAmountType::ExactInput,
                1_000_000_000,
                0,
                None,
            ),
            &ClaimStakingRewardsAction::new(&env),
            &WithdrawAction::new(&env, 1_000_000_000),
        ])
        .unwrap();

    // Vesting
    env.svm.warp(120);
    env.svm
        .execute_actions(&[&CreateVestingPlanAction::new(
            &env,
            1_000_000_000,
            120,
            300,
            60,
        )])
        .unwrap();
    env.svm.warp(120 + 120);
    env.svm
        .execute_actions(&[&ReleaseAction::new(&env)])
        .unwrap();

    // Creator fee claim
    env.svm
        .execute_actions(&[&ClaimCreatorFeesAction::new(&env)])
        .unwrap();
}
