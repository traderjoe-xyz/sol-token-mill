use joelana_test_utils::{
    joelana_env::{
        actions::token_mill::{
            ClaimCreatorFeesAction, ClaimReferralFeesAction, ClaimStakingRewardsAction,
            CreateMarketAction, CreateStakePositionAction, CreateStakingAction, DepositAction,
            SetMarketPricesAction, SwapAction, TokenMillEnv, WithdrawAction,
        },
        make_address, TokenType, ACTORS,
    },
    utils::token_mill::curve_generator::Curve,
};
use solana_sdk::pubkey::Pubkey;
use token_mill::{
    manager::swap_manager::{SwapAmountType, SwapType},
    state::Market,
};

pub struct SvmEngine {
    testing_env: TokenMillEnv,
    protocol_fee_recipient_balance: u64,
}

impl SvmEngine {
    pub fn new() -> Self {
        let testing_env = TokenMillEnv::new().with_default_quote_token_mint();

        let protocol_fee_recipient_balance = testing_env.svm.get_balance(
            &testing_env.quote_token_mint.unwrap(),
            &make_address("dave"),
        );

        Self {
            testing_env,
            protocol_fee_recipient_balance,
        }
    }

    pub fn create_market(&mut self, total_supply: u64, price_curve: Curve) {
        let base_token_mint = Pubkey::new_unique();
        self.testing_env.base_token_mint = Some(base_token_mint);

        self.testing_env.svm.change_payer("alice");

        let quote_token_mint = self.testing_env.quote_token_mint.unwrap();

        let mut create_market_action = CreateMarketAction::new(&self.testing_env);
        create_market_action.total_supply = total_supply;

        let set_prices_action =
            SetMarketPricesAction::new(price_curve).with_custom_base_token_mint(base_token_mint);

        self.testing_env
            .svm
            .execute_actions(&[create_market_action.no_badge(), &set_prices_action])
            .unwrap();

        // Create ATAs
        for actor in ACTORS {
            self.testing_env
                .svm
                .create_ata(&make_address(actor), &base_token_mint, TokenType::Token2022)
                .unwrap();
        }

        self.testing_env
            .svm
            .create_ata(
                &create_market_action.market,
                &quote_token_mint,
                self.testing_env.quote_token_type,
            )
            .unwrap();

        self.testing_env.market = create_market_action.market;
        self.testing_env.base_token_mint = Some(base_token_mint);

        self.testing_env
            .svm
            .tokens
            .insert(base_token_mint, TokenType::Token2022);

        self.testing_env.svm.change_payer("admin");

        self.testing_env
            .svm
            .execute_actions(&[&CreateStakingAction::new(&self.testing_env)])
            .unwrap();

        self.testing_env.svm.change_payer("bob");

        self.testing_env
            .svm
            .execute_actions(&[&CreateStakePositionAction::new(&self.testing_env)])
            .unwrap();
    }

    pub fn swap(
        &mut self,
        swap_type: SwapType,
        swap_amount_type: SwapAmountType,
        amount: u64,
    ) -> (u64, u64) {
        self.testing_env.svm.change_payer("bob");

        let base_balance_before = self.testing_env.svm.get_balance(
            &self.testing_env.base_token_mint.unwrap(),
            &self.testing_env.svm.payer,
        );
        let quote_balance_before = self.testing_env.svm.get_balance(
            &self.testing_env.quote_token_mint.unwrap(),
            &self.testing_env.svm.payer,
        );

        let other_amount_threshold = match swap_amount_type {
            SwapAmountType::ExactInput => 0,
            SwapAmountType::ExactOutput => u64::MAX,
        };

        let swap_action = SwapAction::new(
            &self.testing_env,
            swap_type,
            swap_amount_type,
            amount,
            other_amount_threshold,
            Some(make_address("carol")),
        );

        self.testing_env
            .svm
            .execute_actions(&[&swap_action])
            .unwrap();

        let base_balance_after = self.testing_env.svm.get_balance(
            &self.testing_env.base_token_mint.unwrap(),
            &self.testing_env.svm.payer,
        );
        let quote_balance_after = self.testing_env.svm.get_balance(
            &self.testing_env.quote_token_mint.unwrap(),
            &self.testing_env.svm.payer,
        );

        let (amount_in, amount_out) = match swap_type {
            SwapType::Buy => (
                quote_balance_before - quote_balance_after,
                base_balance_after - base_balance_before,
            ),
            SwapType::Sell => (
                base_balance_before - base_balance_after,
                quote_balance_after - quote_balance_before,
            ),
        };

        (amount_in, amount_out)
    }

    pub fn claim_fees(&mut self) -> (u64, u64, u64) {
        self.testing_env.svm.change_payer("alice");

        let quote_balance_before = self.testing_env.svm.get_balance(
            &self.testing_env.quote_token_mint.unwrap(),
            &self.testing_env.svm.payer,
        );

        let claim_creator_fees_action = ClaimCreatorFeesAction::new(&self.testing_env);

        self.testing_env
            .svm
            .execute_actions(&[&claim_creator_fees_action])
            .unwrap();

        let quote_balance_after = self.testing_env.svm.get_balance(
            &self.testing_env.quote_token_mint.unwrap(),
            &self.testing_env.svm.payer,
        );

        let creator_fee = quote_balance_after - quote_balance_before;

        self.testing_env.svm.change_payer("carol");

        let quote_balance_before = self.testing_env.svm.get_balance(
            &self.testing_env.quote_token_mint.unwrap(),
            &self.testing_env.svm.payer,
        );

        let claim_referral_fees_action = ClaimReferralFeesAction::new(&self.testing_env);

        self.testing_env
            .svm
            .execute_actions(&[&claim_referral_fees_action])
            .unwrap();

        let quote_balance_after = self.testing_env.svm.get_balance(
            &self.testing_env.quote_token_mint.unwrap(),
            &self.testing_env.svm.payer,
        );

        let referral_fee = quote_balance_after - quote_balance_before;

        let quote_balance_before = self.protocol_fee_recipient_balance;

        let quote_balance_after = self.testing_env.svm.get_balance(
            &self.testing_env.quote_token_mint.unwrap(),
            &make_address("dave"),
        );

        let protocol_fee = quote_balance_after - quote_balance_before;
        self.protocol_fee_recipient_balance = quote_balance_after;

        (creator_fee, referral_fee, protocol_fee)
    }

    pub fn deposit(&mut self, amount: u64) {
        self.testing_env.svm.change_payer("bob");

        let deposit_action = DepositAction::new(&self.testing_env, amount);

        self.testing_env
            .svm
            .execute_actions(&[&deposit_action])
            .unwrap();
    }

    pub fn withdraw(&mut self, amount: u64) {
        self.testing_env.svm.change_payer("bob");

        let withdraw_action = WithdrawAction::new(&self.testing_env, amount);

        self.testing_env
            .svm
            .execute_actions(&[&withdraw_action])
            .unwrap();
    }

    pub fn claim_staking_rewards(&mut self) -> u64 {
        self.testing_env.svm.change_payer("bob");

        let quote_balance_before = self.testing_env.svm.get_balance(
            &self.testing_env.quote_token_mint.unwrap(),
            &self.testing_env.svm.payer,
        );

        let claim_staking_rewards_action = ClaimStakingRewardsAction::new(&self.testing_env);

        self.testing_env
            .svm
            .execute_actions(&[&claim_staking_rewards_action])
            .unwrap();

        let quote_balance_after = self.testing_env.svm.get_balance(
            &self.testing_env.quote_token_mint.unwrap(),
            &self.testing_env.svm.payer,
        );

        quote_balance_after - quote_balance_before
    }

    pub fn get_market(&self) -> token_mill::state::Market {
        self.testing_env
            .svm
            .get_parsed_account::<Market>(&self.testing_env.market)
    }
}
