use alloy::{dyn_abi::DynSolValue, sol, sol_types::SolCall};
use anyhow::{anyhow, Result};
use revm::primitives::{Address, ExecutionResult, Output, TransactTo, U256};
use std::{collections::HashMap, convert::TryFrom};

use crate::joevm_env::{JoeUniverse, DEFAULT_ADDRESS};

sol!(ERC20, "src/joevm_env/bindings/ERC20.json");

pub struct TokenModule {
    pub tokens: HashMap<String, Address>,
}

impl Default for TokenModule {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenModule {
    pub fn new() -> Self {
        Self {
            tokens: HashMap::new(),
        }
    }

    pub fn get_token(&self, name: &str) -> Result<&Address> {
        self.tokens.get(name).ok_or(anyhow!("Token not found"))
    }
}

pub trait TokenCreator {
    fn create_token(&mut self, name: &str, decimals: u8) -> Result<Address>;
    fn mint(&mut self, token: &str, to: Address, amount: u128) -> Result<()>;
    fn approve(
        &mut self,
        token: &str,
        owner: Address,
        spender: Address,
        amount: u128,
    ) -> Result<()>;
    fn balance_of(&mut self, token: &str, owner: Address) -> Result<u128>;
    fn transfer_from(
        &mut self,
        token: &str,
        caller: Address,
        from: Address,
        to: Address,
        amount: u128,
    ) -> Result<()>;
    fn transfer(&mut self, token: &str, caller: Address, to: Address, amount: u128) -> Result<()>;
}

impl TokenCreator for JoeUniverse {
    fn create_token(&mut self, name: &str, decimals: u8) -> Result<Address> {
        let constructor_data = DynSolValue::Tuple(vec![decimals.into()]).abi_encode();

        let data = ERC20::BYTECODE
            .clone()
            .into_iter()
            .chain(constructor_data)
            .collect::<Vec<u8>>();

        let result = self.call(DEFAULT_ADDRESS, TransactTo::Create, data.into(), true);

        let contract_address = match result {
            ExecutionResult::Success {
                output: Output::Create(_, address),
                ..
            } => address.ok_or(anyhow!("ERC20 deployment failed: {result:?}"))?,
            result => return Err(anyhow!("ERC20 execution failed: {result:?}")),
        };

        self.token_module
            .tokens
            .insert(name.to_string(), contract_address);

        self.mint(name, DEFAULT_ADDRESS, u128::MAX / 10)?;

        Ok(contract_address)
    }

    fn mint(&mut self, token: &str, to: Address, amount: u128) -> Result<()> {
        let token_address = self.token_module.get_token(token)?;

        let data: Vec<u8> = ERC20::mintCall {
            account: to,
            amount: U256::from(amount),
        }
        .abi_encode();

        let result = self.call(
            DEFAULT_ADDRESS,
            TransactTo::Call(*token_address),
            data.into(),
            true,
        );

        match result {
            ExecutionResult::Success { .. } => Ok(()),
            result => Err(anyhow!("Mint failed: {result:?}")),
        }
    }

    fn transfer(&mut self, token: &str, caller: Address, to: Address, amount: u128) -> Result<()> {
        let token_address = self.token_module.get_token(token)?;

        let data: Vec<u8> = ERC20::transferCall {
            to,
            value: U256::from(amount),
        }
        .abi_encode();

        let result = self.call(caller, TransactTo::Call(*token_address), data.into(), true);

        match result {
            ExecutionResult::Success { .. } => Ok(()),
            result => Err(anyhow!("Transfer failed: {result:?}")),
        }
    }

    fn approve(
        &mut self,
        token: &str,
        owner: Address,
        spender: Address,
        amount: u128,
    ) -> Result<()> {
        let token_address = self.token_module.get_token(token)?;

        let data: Vec<u8> = ERC20::approveCall {
            spender,
            value: U256::from(amount),
        }
        .abi_encode();

        let result = self.call(owner, TransactTo::Call(*token_address), data.into(), true);

        match result {
            ExecutionResult::Success { .. } => Ok(()),
            result => Err(anyhow!("Approval failed: {result:?}")),
        }
    }

    fn transfer_from(
        &mut self,
        token: &str,
        caller: Address,
        from: Address,
        to: Address,
        amount: u128,
    ) -> Result<()> {
        let token_address = self.token_module.get_token(token)?;

        let data: Vec<u8> = ERC20::transferFromCall {
            from,
            to,
            value: U256::from(amount),
        }
        .abi_encode();

        let result = self.call(caller, TransactTo::Call(*token_address), data.into(), true);

        match result {
            ExecutionResult::Success { .. } => Ok(()),
            result => Err(anyhow!("TransferFrom failed: {result:?}")),
        }
    }

    fn balance_of(&mut self, token: &str, owner: Address) -> Result<u128> {
        let token_address = self.token_module.get_token(token)?;

        sol! {
            function balanceOf(address owner) external view returns (uint256);
        }

        let data: Vec<u8> = balanceOfCall { owner }.abi_encode();

        let result = self.call(
            DEFAULT_ADDRESS,
            TransactTo::Call(*token_address),
            data.into(),
            false,
        );

        match result {
            ExecutionResult::Success {
                output: Output::Call(data),
                ..
            } => Ok(u128::try_from(
                balanceOfCall::abi_decode_returns(&data, false)?._0,
            )?),
            result => Err(anyhow!("BalanceOf failed: {result:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revm::primitives::address;

    const ALICE: Address = address!("0000000000000000000000000000000000000001");
    const BOB: Address = address!("0000000000000000000000000000000000000002");

    #[test]
    fn test_create_token() {
        let mut joe_universe = JoeUniverse::new();
        let token_address = joe_universe.create_token("TestToken", 18).unwrap();

        assert_eq!(joe_universe.token_module.tokens.len(), 1);
        assert_eq!(
            joe_universe.token_module.tokens.get("TestToken").unwrap(),
            &token_address
        );
    }

    #[test]
    fn test_mint() {
        let mut joe_universe = JoeUniverse::new();

        joe_universe.create_token("TestToken", 18).unwrap();

        joe_universe.mint("TestToken", ALICE, 100).unwrap();

        let balance = joe_universe.balance_of("TestToken", ALICE).unwrap();

        assert_eq!(balance, 100);
    }

    #[test]
    fn test_transfer() {
        let mut joe_universe = JoeUniverse::new();

        joe_universe.create_token("TestToken", 18).unwrap();

        joe_universe.mint("TestToken", ALICE, 100).unwrap();

        joe_universe.transfer("TestToken", ALICE, BOB, 50).unwrap();

        let alice_balance = joe_universe.balance_of("TestToken", ALICE).unwrap();
        assert_eq!(alice_balance, 50);

        let bob_balance = joe_universe.balance_of("TestToken", BOB).unwrap();
        assert_eq!(bob_balance, 50);
    }

    #[test]
    fn test_approve_and_transfer_from() {
        let mut joe_universe = JoeUniverse::new();

        joe_universe.create_token("TestToken", 18).unwrap();

        joe_universe.mint("TestToken", ALICE, 100).unwrap();

        joe_universe.approve("TestToken", ALICE, BOB, 100).unwrap();

        joe_universe
            .transfer_from("TestToken", BOB, ALICE, BOB, 50)
            .unwrap();

        let alice_balance = joe_universe.balance_of("TestToken", ALICE).unwrap();
        assert_eq!(alice_balance, 50);

        let bob_balance = joe_universe.balance_of("TestToken", BOB).unwrap();
        assert_eq!(bob_balance, 50);
    }
}
