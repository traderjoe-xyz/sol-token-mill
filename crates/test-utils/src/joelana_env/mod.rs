use anchor_lang::{
    prelude::{AccountMeta, Clock},
    AccountDeserialize,
};
use anchor_spl::token_interface::spl_token_2022::{
    extension::StateWithExtensions, solana_program::program_pack::Pack,
    state::Account as SplAccount,
};
use anchor_spl::{
    associated_token::spl_associated_token_account,
    token_2022::spl_token_2022::{self},
};
use anyhow::Result;
use litesvm::{
    types::{FailedTransactionMetadata, TransactionMetadata},
    LiteSVM,
};
use litesvm_token::{CreateAssociatedTokenAccount, CreateMint, MintTo};
use solana_sdk::signer::Signer;
use solana_sdk::{
    account::{Account, ReadableAccount},
    instruction::{Instruction, InstructionError},
    message::Message,
    native_token::sol_to_lamports,
    pubkey::Pubkey,
    signature::Keypair,
    transaction::{Transaction, TransactionError},
};
use std::collections::HashMap;

pub use token_mill::{
    errors::TokenMillError,
    manager::swap_manager::{SwapAmountType, SwapType},
    state::QuoteTokenBadgeStatus,
};

pub mod actions;

pub const ACTORS: [&str; 6] = ["admin", "alice", "bob", "carol", "dave", "mallory"];

#[derive(Debug, Copy, Clone)]
pub enum TokenType {
    Token,
    Token2022,
}

impl TokenType {
    pub fn program_address(&self) -> Pubkey {
        match self {
            TokenType::Token => anchor_spl::token::ID,
            TokenType::Token2022 => anchor_spl::token_2022::ID,
        }
    }

    pub fn mint_account_size(&self) -> usize {
        match self {
            TokenType::Token => anchor_spl::token::Mint::LEN,
            TokenType::Token2022 => spl_token_2022::state::Mint::LEN,
        }
    }
}

pub struct JoelanaEnv {
    svm_engine: LiteSVM,
    pub payer: Pubkey,
    pub tokens: HashMap<Pubkey, TokenType>,
}

impl JoelanaEnv {
    pub fn new() -> Self {
        let svm_engine = LiteSVM::new()
            .with_sigverify(false)
            .with_blockhash_check(false)
            .with_transaction_history(0)
            .with_spl_programs();

        let mut env = Self {
            svm_engine,
            payer: Pubkey::default(),
            tokens: HashMap::new(),
        };

        for actor in ACTORS {
            let user_address = make_address(actor);

            env.airdrop(&user_address);
        }

        env.payer = make_address("admin");

        env
    }

    pub fn change_payer(&mut self, payer_name: &str) -> Pubkey {
        let payer_address = make_address(payer_name);
        self.payer = payer_address;

        payer_address
    }

    pub fn add_token_mill_program(&mut self) {
        self.svm_engine
            .add_program_from_file(token_mill::id(), "../../target/deploy/token_mill.so")
            .unwrap();
    }

    #[allow(clippy::result_large_err)]
    pub fn execute_actions(
        &mut self,
        actions: &[&dyn InstructionGenerator],
    ) -> Result<TransactionMetadata, FailedTransactionMetadata> {
        let instructions = actions
            .iter()
            .map(|action| action.instruction())
            .collect::<Vec<_>>();

        self.execute(&instructions)
    }

    #[allow(clippy::result_large_err)]
    pub fn execute(
        &mut self,
        instructions: &[Instruction],
    ) -> Result<TransactionMetadata, FailedTransactionMetadata> {
        let tx = Transaction::new_unsigned(Message::new(instructions, Some(&self.payer)));

        self.svm_engine.send_transaction(tx)
    }

    pub fn airdrop(&mut self, pubkey: &Pubkey) {
        self.svm_engine
            .airdrop(pubkey, sol_to_lamports(100.0))
            .unwrap();
    }

    pub fn warp(&mut self, time: i64) {
        let mut clock = self.svm_engine.get_sysvar::<Clock>();
        clock.unix_timestamp += time;
        self.svm_engine.set_sysvar(&clock);
    }

    pub fn create_token(&mut self, token_type: TokenType, decimals: u8) -> Result<Pubkey> {
        let payer = Keypair::new();

        self.airdrop(&payer.pubkey());

        let token_address = CreateMint::new(&mut self.svm_engine, &payer)
            .decimals(decimals)
            .token_program_id(&token_type.program_address())
            .send()
            .unwrap();

        for actor in ACTORS {
            let actor_ata = self.create_ata(&make_address(actor), &token_address, token_type)?;

            MintTo::new(
                &mut self.svm_engine,
                &payer,
                &token_address,
                &actor_ata,
                (u64::MAX - 1) / ACTORS.len() as u64,
            )
            .token_program_id(&token_type.program_address())
            .send()
            .unwrap();
        }

        self.tokens.insert(token_address, token_type);

        Ok(token_address)
    }

    pub fn create_ata(
        &mut self,
        wallet: &Pubkey,
        token_mint: &Pubkey,
        token_type: TokenType,
    ) -> Result<Pubkey> {
        let payer = Keypair::new();

        self.airdrop(&payer.pubkey());

        let ata_address =
            CreateAssociatedTokenAccount::new(&mut self.svm_engine, &payer, token_mint)
                .owner(wallet)
                .token_program_id(&token_type.program_address())
                .send()
                .unwrap();

        Ok(ata_address)
    }

    pub fn get_account(&self, pubkey: &Pubkey) -> Account {
        self.svm_engine
            .get_account(pubkey)
            .expect("Account not found")
    }

    pub fn get_parsed_account<T>(&self, pubkey: &Pubkey) -> T
    where
        T: AccountDeserialize,
    {
        let account = self.get_account(pubkey);

        T::try_deserialize(&mut account.data()).unwrap()
    }

    pub fn get_ata_address(&self, token_mint: &Pubkey, wallet: &Pubkey) -> Pubkey {
        let token_type = self.tokens.get(token_mint).unwrap();

        spl_associated_token_account::get_associated_token_address_with_program_id(
            wallet,
            token_mint,
            &token_type.program_address(),
        )
    }

    pub fn get_balance(&self, token_mint: &Pubkey, wallet: &Pubkey) -> u64 {
        let token_ata = self.get_ata_address(token_mint, wallet);

        let account = self.get_account(&token_ata);

        let account = StateWithExtensions::<SplAccount>::unpack(account.data()).unwrap();

        account.base.amount
    }
}

pub fn parse_custom_error(
    result: Result<TransactionMetadata, FailedTransactionMetadata>,
) -> Result<u32, TransactionError> {
    let result = result.unwrap_err();

    match result.err {
        TransactionError::InstructionError(_, InstructionError::Custom(error_code)) => {
            Ok(error_code)
        }
        _ => {
            println!("{}", result.meta.logs.join("\n"));

            Err(result.err)
        }
    }
}

pub fn get_event_authority(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"__event_authority"], &program).0
}

pub fn make_address(string: &str) -> Pubkey {
    let mut array: [u8; 32] = [0; 32];

    // Iterate over each character in the input string
    for (index, byte) in string.bytes().enumerate() {
        array[index] = byte;
    }

    // Convert the array to a Pubkey
    Pubkey::new_from_array(array)
}

pub trait InstructionGenerator {
    fn accounts(&self) -> Vec<AccountMeta>;
    fn instruction(&self) -> Instruction;
}

trait AccountMetaVecExt {
    fn append_payer(&mut self, payer: Pubkey) -> &mut Self;
    fn append_system_program(&mut self) -> &mut Self;
    fn append_cpi_event_accounts(&mut self, event_authority: Pubkey) -> &mut Self;
    fn append_token_program(&mut self) -> &mut Self;
    fn append_token_2022_program(&mut self) -> &mut Self;
    fn append_associated_token_program(&mut self) -> &mut Self;
}

impl AccountMetaVecExt for Vec<AccountMeta> {
    fn append_payer(&mut self, payer: Pubkey) -> &mut Self {
        self.push(AccountMeta::new_readonly(payer, true));

        self
    }

    fn append_system_program(&mut self) -> &mut Self {
        self.push(AccountMeta::new_readonly(
            solana_sdk::system_program::ID,
            false,
        ));

        self
    }

    fn append_cpi_event_accounts(&mut self, event_authority: Pubkey) -> &mut Self {
        self.push(AccountMeta::new_readonly(event_authority, false));
        self.push(AccountMeta::new_readonly(token_mill::ID, false));

        self
    }

    fn append_token_program(&mut self) -> &mut Self {
        self.push(AccountMeta::new_readonly(anchor_spl::token::ID, false));

        self
    }

    fn append_token_2022_program(&mut self) -> &mut Self {
        self.push(AccountMeta::new_readonly(spl_token_2022::id(), false));

        self
    }

    fn append_associated_token_program(&mut self) -> &mut Self {
        self.push(AccountMeta::new_readonly(
            spl_associated_token_account::id(),
            false,
        ));

        self
    }
}
