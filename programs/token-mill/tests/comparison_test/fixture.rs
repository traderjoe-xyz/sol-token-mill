use super::{evm_engine::EvmEngine, svm_engine::SvmEngine};
use joelana_test_utils::utils::token_mill::curve_generator::Curve;
use token_mill::manager::swap_manager::{SwapAmountType, SwapType};

pub struct Fixture {
    pub svm_engine: SvmEngine,
    pub evm_engine: EvmEngine,
}

#[derive(Clone, Debug)]
pub struct SwapScenario {
    pub swap_type: SwapType,
    pub swap_amount_type: SwapAmountType,
    pub amount: u64,
}

impl Fixture {
    pub fn new() -> Self {
        let svm_engine = SvmEngine::new();
        let evm_engine = EvmEngine::new();

        Self {
            svm_engine,
            evm_engine,
        }
    }

    pub fn create_markets(&mut self, total_supply: u64, price_curve: Curve) {
        self.evm_engine.create_market(total_supply, price_curve);
        self.svm_engine.create_market(total_supply, price_curve);
    }

    pub fn swap(&mut self, scenario: SwapScenario) -> (u64, u64, u64, u64) {
        let SwapScenario {
            swap_type,
            swap_amount_type,
            amount,
        } = scenario;

        let (amount_in_svm, amount_out_svm) =
            self.svm_engine.swap(swap_type, swap_amount_type, amount);
        let (amount_in_evm, amount_out_evm) =
            self.evm_engine.swap(swap_type, swap_amount_type, amount);

        (amount_in_svm, amount_out_svm, amount_in_evm, amount_out_evm)
    }

    pub fn swap_and_compare(&mut self, scenario: SwapScenario) {
        self.swap_and_compare_allow_imprecision(scenario, 1);
    }

    pub fn swap_and_compare_allow_imprecision(&mut self, scenario: SwapScenario, threshold: u64) {
        let (amount_in_svm, amount_out_svm, amount_in_evm, amount_out_evm) = self.swap(scenario);

        // Rounding errors are only allowed if they are in favor of the protocol. Threshold is variable as uncertainties are bigger with smaller amounts
        assert!((amount_in_svm >= amount_in_evm && amount_in_svm <= amount_in_evm + threshold));
        assert!((amount_out_svm <= amount_out_evm && amount_out_svm + threshold >= amount_out_evm));
    }

    pub fn claim_fees_and_compare(&mut self) {
        let (creator_fee_evm, referral_fee_evm, protocol_fee_evm) = self.evm_engine.claim_fees();
        let (creator_fee_svm, referral_fee_svm, protocol_fee_svm) = self.svm_engine.claim_fees();

        assert_eq!(creator_fee_svm, creator_fee_evm);
        assert_eq!(referral_fee_svm, referral_fee_evm);
        assert_eq!(protocol_fee_svm, protocol_fee_evm);
    }

    pub fn deposit(&mut self, amount: u64) {
        self.svm_engine.deposit(amount);
        self.evm_engine.deposit(amount);
    }

    pub fn withdraw(&mut self, amount: u64) {
        self.svm_engine.withdraw(amount);
        self.evm_engine.withdraw(amount);
    }

    pub fn claim_staking_rewards_and_compare(&mut self) {
        let staking_rewards_svm = self.svm_engine.claim_staking_rewards();
        let staking_rewards_evm = self.evm_engine.claim_staking_rewards();

        assert!(
            (staking_rewards_svm <= staking_rewards_evm
                && staking_rewards_svm + 1 >= staking_rewards_evm)
        );
    }
}
