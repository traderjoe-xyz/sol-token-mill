use alloy::{dyn_abi::DynSolValue, sol, sol_types::SolCall};
use anyhow::{anyhow, Result};
use revm::primitives::{address, Address, ExecutionResult, Output, TransactTo, I256, U256};
use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
};
use TMFactory::MarketCreationParameters;

use crate::joevm_env::{JoeUniverse, DEFAULT_ADDRESS};

use super::tokens::TokenCreator;

#[derive(Default)]
pub struct TokenMillModule {
    pub factory: Address,
    pub staking: Address,
    pub markets: HashMap<String, Address>,
}

impl TokenMillModule {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn get_market(&self, token: &str) -> Result<&Address> {
        self.markets
            .get(token)
            .ok_or(anyhow!("Market not found for token: {}", token))
    }
}

pub trait TokenMillManager {
    fn deploy_token_mill(
        &mut self,
        default_protocol_fee_share: u16,
        default_referral_fee_share: u16,
    ) -> Result<()>;
    fn add_quote_token(&mut self, token: &str) -> Result<()>;

    #[allow(clippy::too_many_arguments)]
    fn create_token_and_market(
        &mut self,
        token_name: &str,
        quote_token: &str,
        total_supply: u128,
        bid_prices: Vec<u128>,
        ask_prices: Vec<u128>,
        creator_fee_share: u16,
        staking_fee_share: u16,
    ) -> Result<()>;

    fn swap(&mut self, token: &str, amount: i128, base_to_quote: bool) -> Result<()>;
    fn deposit(&mut self, token: &str, amount: u128) -> Result<()>;
    fn withdraw(&mut self, token: &str, amount: u128) -> Result<()>;
    fn claim_creator_fees(&mut self, token: &str) -> Result<u64>;
    fn claim_referral_fees(&mut self, token: &str) -> Result<u64>;
    fn claim_protocol_fees(&mut self, token: &str) -> Result<u64>;
    fn claim_staking_rewards(&mut self, token: &str) -> Result<u64>;
    fn get_market_reserves(&mut self, market: &str) -> Result<(u128, u128)>;
    fn get_amount_in(&mut self, token: &str, amount_out: i128, base_to_quote: bool)
        -> Result<u128>;
}

// Compiled contract from https://github.com/traderjoe-xyz/token-mill with `_disableInitializers()` commented out for easier deployment
sol!(TMFactory, "src/joevm_env/bindings/TMFactory.json");
sol!(TMStaking, "src/joevm_env/bindings/TMStaking.json");
sol!(TMERC20, "src/joevm_env/bindings/TMERC20.json");
sol!(TMMarket, "src/joevm_env/bindings/TMMarket.json");

impl TokenMillManager for JoeUniverse {
    fn deploy_token_mill(
        &mut self,
        default_protocol_fee_share: u16,
        default_referral_fee_share: u16,
    ) -> Result<()> {
        // Precomputed staking address
        self.token_mill_module.staking = address!("83769beeb7e5405ef0b7dc3c66c43e3a51a6d27f");

        // Deploy Factory
        let constructor_data =
            DynSolValue::Tuple(vec![DynSolValue::Address(self.token_mill_module.staking)])
                .abi_encode();

        let data = TMFactory::BYTECODE
            .clone()
            .into_iter()
            .chain(constructor_data)
            .collect::<Vec<_>>();

        let result = self.call(DEFAULT_ADDRESS, TransactTo::Create, data.into(), true);

        let contract_address = match result {
            ExecutionResult::Success {
                output: Output::Create(_, address),
                ..
            } => address.ok_or(anyhow!("TMFactory deployment failed: {result:?}"))?,
            result => return Err(anyhow!("TMFactory deployment failed: {result:?}")),
        };

        self.token_mill_module.factory = contract_address;

        // Initialize Factory
        let data = TMFactory::initializeCall {
            protocolShare: default_protocol_fee_share,
            referrerShare: default_referral_fee_share,
            protocolFeeRecipient: DEFAULT_ADDRESS,
            initialOwner: DEFAULT_ADDRESS,
        }
        .abi_encode();

        let result = self.call(
            DEFAULT_ADDRESS,
            TransactTo::Call(contract_address),
            data.into(),
            true,
        );

        match result {
            ExecutionResult::Success { .. } => {}
            result => return Err(anyhow!("TMFactory initialization failed: {result:?}")),
        }

        // Deploy TMERC20
        let constructor_data =
            DynSolValue::Tuple(vec![DynSolValue::Address(contract_address)]).abi_encode();

        let data = TMERC20::BYTECODE
            .clone()
            .into_iter()
            .chain(constructor_data)
            .collect::<Vec<_>>();

        let result = self.call(DEFAULT_ADDRESS, TransactTo::Create, data.into(), true);

        let contract_address = match result {
            ExecutionResult::Success {
                output: Output::Create(_, address),
                ..
            } => address.ok_or(anyhow!("TMERC20 deployment failed: {result:?}"))?,
            result => return Err(anyhow!("TMERC20 deployment failed: {result:?}")),
        };

        // Update Factory Token Implementation
        let data = TMFactory::updateTokenImplementationCall {
            tokenType: 1,
            implementation: contract_address,
        }
        .abi_encode();

        let result = self.call(
            DEFAULT_ADDRESS,
            TransactTo::Call(self.token_mill_module.factory),
            data.into(),
            true,
        );

        match result {
            ExecutionResult::Success { .. } => {}
            result => {
                return Err(anyhow!(
                    "TMFactory updateTokenImplementation failed: {result:?}"
                ))
            }
        }

        // Deploy Staking
        let constructor_data =
            DynSolValue::Tuple(vec![DynSolValue::Address(self.token_mill_module.factory)])
                .abi_encode();

        let data = TMStaking::BYTECODE
            .clone()
            .into_iter()
            .chain(constructor_data)
            .collect::<Vec<_>>();

        let result = self.call(DEFAULT_ADDRESS, TransactTo::Create, data.into(), true);

        let contract_address = match result {
            ExecutionResult::Success {
                output: Output::Create(_, address),
                ..
            } => address.ok_or(anyhow!("TMStaking deployment failed: {result:?}"))?,
            result => return Err(anyhow!("TMStaking deployment failed: {result:?}")),
        };

        assert_eq!(contract_address, self.token_mill_module.staking);

        Ok(())
    }

    fn add_quote_token(&mut self, quote_token: &str) -> Result<()> {
        let quote_token_address = self.token_module.get_token(quote_token)?;

        let data = TMFactory::addQuoteTokenCall {
            quoteToken: *quote_token_address,
        }
        .abi_encode();

        let result = self.call(
            DEFAULT_ADDRESS,
            TransactTo::Call(self.token_mill_module.factory),
            data.into(),
            true,
        );

        match result {
            ExecutionResult::Success { .. } => Ok(()),
            result => Err(anyhow!("TMFactory addQuoteToken failed: {result:?}")),
        }
    }

    fn create_token_and_market(
        &mut self,
        token_name: &str,
        quote_token: &str,
        total_supply: u128,
        bid_prices: Vec<u128>,
        ask_prices: Vec<u128>,
        creator_fee_share: u16,
        staking_fee_share: u16,
    ) -> Result<()> {
        let quote_token_address = self.token_module.get_token(quote_token)?;

        let bid_prices = bid_prices
            .into_iter()
            .map(|x| U256::from(x))
            .collect::<Vec<_>>();

        let ask_prices = ask_prices
            .into_iter()
            .map(|x| U256::from(x))
            .collect::<Vec<_>>();

        let base_token_decimals = DynSolValue::Uint(U256::from(6), 8).abi_encode();

        let parameters = MarketCreationParameters {
            tokenType: 1,
            name: token_name.to_string(),
            symbol: "".to_string(),
            quoteToken: *quote_token_address,
            totalSupply: U256::from(total_supply),
            bidPrices: bid_prices.clone(),
            askPrices: ask_prices.clone(),
            creatorShare: creator_fee_share,
            stakingShare: staking_fee_share,
            args: base_token_decimals.into(),
        };

        let data = TMFactory::createMarketAndTokenCall { parameters }.abi_encode();

        let result = self.call(
            DEFAULT_ADDRESS,
            TransactTo::Call(self.token_mill_module.factory),
            data.into(),
            true,
        );

        let TMFactory::createMarketAndTokenReturn { baseToken, market } = match result {
            ExecutionResult::Success { output, .. } => {
                TMFactory::createMarketAndTokenCall::abi_decode_returns(output.data(), false)?
            }
            result => return Err(anyhow!("TMFactory createTokenAndMarket failed: {result:?}")),
        };

        self.token_module
            .tokens
            .insert(token_name.to_string(), baseToken);

        self.token_mill_module
            .markets
            .insert(token_name.to_string(), market);

        Ok(())
    }

    fn swap(&mut self, token: &str, amount: i128, base_to_quote: bool) -> Result<()> {
        let market_address = self.token_mill_module.get_market(token)?;

        let data = TMMarket::swapCall {
            recipient: DEFAULT_ADDRESS,
            deltaAmount: I256::try_from(amount)?,
            swapB2Q: base_to_quote,
            data: vec![].into(),
            referrer: DEFAULT_ADDRESS,
        }
        .abi_encode();

        let result = self.call(
            DEFAULT_ADDRESS,
            TransactTo::Call(*market_address),
            data.into(),
            true,
        );

        match result {
            ExecutionResult::Success { .. } => Ok(()),
            result => Err(anyhow!("TMMarket swap failed: {result:?}")),
        }
    }

    fn deposit(&mut self, token: &str, amount: u128) -> Result<()> {
        let staking = self.token_mill_module.staking;

        self.approve(token, DEFAULT_ADDRESS, staking, amount)?;

        let token_address = self.token_module.get_token(token)?;

        let data = TMStaking::depositCall {
            token: *token_address,
            to: DEFAULT_ADDRESS,
            amount: U256::from(amount),
            minAmount: U256::from(0),
        }
        .abi_encode();

        let result = self.call(
            DEFAULT_ADDRESS,
            TransactTo::Call(staking),
            data.into(),
            true,
        );

        match result {
            ExecutionResult::Success { .. } => Ok(()),
            result => Err(anyhow!("TMStaking deposit failed: {result:?}")),
        }
    }

    fn withdraw(&mut self, token: &str, amount: u128) -> Result<()> {
        let staking = self.token_mill_module.staking;

        let token_address = self.token_module.get_token(token)?;

        let data = TMStaking::withdrawCall {
            token: *token_address,
            to: DEFAULT_ADDRESS,
            amount: U256::from(amount),
        }
        .abi_encode();

        let result = self.call(
            DEFAULT_ADDRESS,
            TransactTo::Call(staking),
            data.into(),
            true,
        );

        match result {
            ExecutionResult::Success { .. } => Ok(()),
            result => Err(anyhow!("TMStaking withdrawal failed: {result:?}")),
        }
    }

    fn claim_staking_rewards(&mut self, token: &str) -> Result<u64> {
        let staking = self.token_mill_module.staking;

        let token_address = self.token_module.get_token(token)?;

        let data = TMStaking::claimRewardsCall {
            token: *token_address,
            to: DEFAULT_ADDRESS,
        }
        .abi_encode();

        let result = self.call(
            DEFAULT_ADDRESS,
            TransactTo::Call(staking),
            data.into(),
            true,
        );

        let TMStaking::claimRewardsReturn {
            _0: pending_rewards,
        } = match result {
            ExecutionResult::Success { output, .. } => {
                TMStaking::claimRewardsCall::abi_decode_returns(output.data(), false)?
            }
            result => return Err(anyhow!("TMStaking claimRewards failed: {result:?}")),
        };

        Ok(u64::try_from(pending_rewards)?)
    }

    fn claim_creator_fees(&mut self, token: &str) -> Result<u64> {
        let market_address = self.token_mill_module.get_market(token)?;

        let data = TMFactory::claimFeesCall {
            market: *market_address,
        }
        .abi_encode();

        let result = self.call(
            DEFAULT_ADDRESS,
            TransactTo::Call(self.token_mill_module.factory),
            data.into(),
            true,
        );

        let TMFactory::claimFeesReturn { claimedFees } = match result {
            ExecutionResult::Success { output, .. } => {
                TMFactory::claimFeesCall::abi_decode_returns(output.data(), false)?
            }
            result => return Err(anyhow!("TMFactory claimFees failed: {result:?}")),
        };

        Ok(u64::try_from(claimedFees)?)
    }

    fn claim_referral_fees(&mut self, token: &str) -> Result<u64> {
        let token_address = self.token_module.get_token(token)?;

        let data = TMFactory::claimReferrerFeesCall {
            token: *token_address,
        }
        .abi_encode();

        let result = self.call(
            DEFAULT_ADDRESS,
            TransactTo::Call(self.token_mill_module.factory),
            data.into(),
            true,
        );

        let TMFactory::claimReferrerFeesReturn { claimedFees } = match result {
            ExecutionResult::Success { output, .. } => {
                TMFactory::claimReferrerFeesCall::abi_decode_returns(output.data(), false)?
            }
            result => return Err(anyhow!("TMFactory claimReferrerFees failed: {result:?}")),
        };

        Ok(u64::try_from(claimedFees)?)
    }

    fn claim_protocol_fees(&mut self, token: &str) -> Result<u64> {
        let token_address = self.token_module.get_token(token)?;

        let data = TMFactory::claimProtocolFeesCall {
            token: *token_address,
        }
        .abi_encode();

        let result = self.call(
            DEFAULT_ADDRESS,
            TransactTo::Call(self.token_mill_module.factory),
            data.into(),
            true,
        );

        let TMFactory::claimProtocolFeesReturn { claimedFees } = match result {
            ExecutionResult::Success { output, .. } => {
                TMFactory::claimProtocolFeesCall::abi_decode_returns(output.data(), false)?
            }
            result => return Err(anyhow!("TMFactory claimProtocolFees failed: {result:?}")),
        };

        Ok(u64::try_from(claimedFees)?)
    }

    fn get_amount_in(
        &mut self,
        token: &str,
        amount_out: i128,
        base_to_quote: bool,
    ) -> Result<u128> {
        let market_address = self.token_mill_module.get_market(token)?;

        sol!(
            function getDeltaAmounts(int256 deltaAmount, bool swapB2Q)
        external
        view
        returns (int256 deltaBaseAmount, int256 deltaQuoteAmount);
        );

        let data = getDeltaAmountsCall {
            deltaAmount: I256::try_from(amount_out)?,
            swapB2Q: base_to_quote,
        }
        .abi_encode();

        let result = self.call(
            DEFAULT_ADDRESS,
            TransactTo::Call(*market_address),
            data.into(),
            false,
        );

        let getDeltaAmountsReturn {
            deltaBaseAmount,
            deltaQuoteAmount,
        } = match result {
            ExecutionResult::Success { output, .. } => {
                getDeltaAmountsCall::abi_decode_returns(output.data(), false)?
            }
            result => return Err(anyhow!("TMMarket getDeltaAmounts failed: {result:?}")),
        };

        match base_to_quote {
            true => Ok(deltaBaseAmount.try_into()?),
            false => Ok(deltaQuoteAmount.try_into()?),
        }
    }

    fn get_market_reserves(&mut self, market: &str) -> Result<(u128, u128)> {
        let market_address = self.token_mill_module.get_market(market)?;

        sol!(
            function getReserves()  returns (uint256 baseReserve, uint256 quoteReserve);
        );

        let data = getReservesCall {}.abi_encode();

        let result = self.call(
            DEFAULT_ADDRESS,
            TransactTo::Call(*market_address),
            data.into(),
            false,
        );

        let getReservesReturn {
            baseReserve,
            quoteReserve,
        } = match result {
            ExecutionResult::Success { output, .. } => {
                getReservesCall::abi_decode_returns(output.data(), false)?
            }
            result => return Err(anyhow!("TMERC20 getReserves failed: {result:?}")),
        };

        Ok((baseReserve.try_into()?, quoteReserve.try_into()?))
    }
}

#[cfg(test)]
mod tests {
    use crate::joevm_env::modules::tokens::TokenCreator;

    use super::*;

    #[test]
    fn test_deploy_token_mill() {
        let mut joe_universe = JoeUniverse::new();
        joe_universe.deploy_token_mill(3_000, 3_000).unwrap();
        joe_universe.create_token("Quote Token", 6).unwrap();
        joe_universe.add_quote_token("Quote Token").unwrap();

        joe_universe
            .create_token_and_market(
                "TM Token",
                "Quote Token",
                1_000_000_000,
                vec![0, 9 * 10u128.pow(18), 90 * 10u128.pow(18)],
                vec![0, 10 * 10u128.pow(18), 100 * 10u128.pow(18)],
                3_000,
                4_000,
            )
            .unwrap();

        let market = *joe_universe
            .token_mill_module
            .get_market("TM Token")
            .unwrap();

        let amount_out = 1_000_000i128;

        let amount_in = joe_universe
            .get_amount_in("TM Token", amount_out, false)
            .unwrap();

        joe_universe.mint("Quote Token", market, amount_in).unwrap();

        joe_universe.swap("TM Token", amount_out, false).unwrap();

        joe_universe.claim_creator_fees("TM Token").unwrap();
        joe_universe.claim_referral_fees("Quote Token").unwrap();
        joe_universe.claim_protocol_fees("Quote Token").unwrap();

        joe_universe
            .deposit("TM Token", amount_out as u128)
            .unwrap();

        let amount_in = 100_000_000;

        joe_universe.mint("Quote Token", market, amount_in).unwrap();

        joe_universe
            .swap("TM Token", amount_in as i128, false)
            .unwrap();

        joe_universe
            .withdraw("TM Token", amount_out as u128)
            .unwrap();

        joe_universe.claim_staking_rewards("TM Token").unwrap();
    }
}
