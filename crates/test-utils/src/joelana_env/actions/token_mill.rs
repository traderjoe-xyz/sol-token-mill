use crate::{
    joelana_env::{
        get_event_authority, make_address, parse_custom_error, AccountMetaVecExt,
        InstructionGenerator, JoelanaEnv, TokenType, ACTORS,
    },
    utils::token_mill::{constants::*, curve_generator::Curve},
};
use anchor_lang::{prelude::AccountMeta, Id, InstructionData};
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id, metadata::Metadata,
    token::spl_token, token_2022::spl_token_2022,
};
use anyhow::Result;
use litesvm::types::{FailedTransactionMetadata, TransactionMetadata};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, transaction::TransactionError};
use token_mill::{
    errors::TokenMillError,
    manager::swap_manager::{SwapAmountType, SwapType},
    state::{
        QuoteTokenBadgeStatus, MARKET_PDA_SEED, MARKET_STAKING_PDA_SEED,
        QUOTE_TOKEN_BADGE_PDA_SEED, REFERRAL_ACCOUNT_PDA_SEED, STAKING_POSITION_PDA_SEED,
    },
};

pub const DEFAULT_TOTAL_SUPPLY: u64 = 1_000_000_000_000;

/// Actors:
/// Admin -> Config authority
/// Alice -> Market creator
/// Bob -> User
/// Carol -> Referrer
/// Dave -> Protocol fee recipient
/// Mallory -> Tries to call methods without proper authority
///

pub struct TokenMillEnv {
    pub svm: JoelanaEnv,
    pub config: Pubkey,
    pub market: Pubkey,
    pub base_token_mint: Option<Pubkey>,
    pub base_token_type: TokenType,
    pub quote_token_mint: Option<Pubkey>,
    pub quote_token_type: TokenType,
}

impl Default for TokenMillEnv {
    fn default() -> Self {
        let token_mill_env = TokenMillEnv::new();

        token_mill_env
            .with_default_quote_token_mint()
            .with_default_market()
    }
}

impl TokenMillEnv {
    pub fn new() -> Self {
        let mut svm = JoelanaEnv::new();

        svm.add_token_mill_program();
        svm.add_metadata_program();

        svm.execute_actions(&[
            &CreateConfigAction::new(),
            &CreateReferralAccountAction::new(),
        ])
        .unwrap();

        Self {
            svm,
            config: make_address("config"),
            market: Pubkey::new_unique(),
            base_token_mint: Some(make_address("base_token_mint")),
            base_token_type: TokenType::Token2022,
            quote_token_mint: None,
            quote_token_type: TokenType::Token,
        }
    }

    pub fn with_default_quote_token_mint(self) -> Self {
        self.with_quote_token_mint(TokenType::Token, 9)
    }

    pub fn with_quote_token_mint(mut self, quote_token_type: TokenType, decimals: u8) -> Self {
        let quote_token = self.svm.create_token(quote_token_type, decimals).unwrap();

        self.svm
            .create_ata(
                &CreateReferralAccountAction::new().referral_account,
                &quote_token,
                quote_token_type,
            )
            .unwrap();

        self.quote_token_mint = Some(quote_token);
        self.quote_token_type = quote_token_type;

        self
    }

    pub fn with_base_token_type(mut self, base_token_type: TokenType) -> Self {
        self.base_token_type = base_token_type;

        self
    }

    pub fn with_default_market(self) -> Self {
        self.with_market(make_address("base_token_mint"), DEFAULT_TOTAL_SUPPLY)
    }

    pub fn with_market(mut self, base_token_mint: Pubkey, total_supply: u64) -> Self {
        self.svm.change_payer("alice");

        let quote_token_mint = self.quote_token_mint.unwrap();

        let market = match self.base_token_type {
            TokenType::Token => {
                let mut create_market_action = CreateMarketWithSplAction::new(&self);
                create_market_action.total_supply = total_supply;

                let set_prices_action = SetMarketPricesAction::new(Curve::default());

                self.svm
                    .execute_actions(&[create_market_action.no_badge(), &set_prices_action])
                    .unwrap();

                create_market_action.market
            }
            TokenType::Token2022 => {
                let mut create_market_action = CreateMarketAction::new(&self);
                create_market_action.total_supply = total_supply;

                let set_prices_action = SetMarketPricesAction::new(Curve::default());

                self.svm
                    .execute_actions(&[create_market_action.no_badge(), &set_prices_action])
                    .unwrap();

                create_market_action.market
            }
        };

        // Create ATAs
        for actor in ACTORS {
            self.svm
                .create_ata(&make_address(actor), &base_token_mint, self.base_token_type)
                .unwrap();
        }

        self.svm
            .create_ata(&market, &quote_token_mint, self.quote_token_type)
            .unwrap();

        self.market = market;
        self.base_token_mint = Some(base_token_mint);

        self.svm
            .tokens
            .insert(base_token_mint, self.base_token_type);

        self
    }

    pub fn with_staking(mut self, buy_amount: u64) -> Self {
        self.svm.change_payer("admin");

        self.svm
            .execute_actions(&[&CreateStakingAction::new(&self)])
            .unwrap();

        self.svm.change_payer("bob");

        self.svm
            .execute_actions(&[&CreateStakePositionAction::new(&self)])
            .unwrap();

        if buy_amount > 0 {
            let buy_action = SwapAction::new(
                &self,
                SwapType::Buy,
                SwapAmountType::ExactOutput,
                buy_amount,
                u64::MAX,
                None,
            );

            self.svm.execute_actions(&[&buy_action]).unwrap();
        }

        self
    }
}

fn tm_event_authority() -> Pubkey {
    get_event_authority(token_mill::ID)
}

pub fn tm_parse_error(
    result: Result<TransactionMetadata, FailedTransactionMetadata>,
) -> Result<TokenMillError, TransactionError> {
    let error_code = parse_custom_error(result)?;

    let error = unsafe {
        std::mem::transmute::<u32, TokenMillError>(
            error_code - anchor_lang::error::ERROR_CODE_OFFSET,
        )
    };

    Ok(error)
}

pub struct CreateConfigAction {
    // Accounts
    pub config: Pubkey,
    pub signer: Pubkey,
    // Args
    pub authority: Pubkey,
    pub protocol_fee_recipient: Pubkey,
    pub protocol_fee_share: u16,
    pub referral_fee_share: u16,
}

impl Default for CreateConfigAction {
    fn default() -> Self {
        Self::new()
    }
}

impl CreateConfigAction {
    pub fn new() -> Self {
        Self {
            config: make_address("config"),
            signer: make_address("admin"),
            authority: make_address("admin"),
            protocol_fee_recipient: make_address("dave"),
            protocol_fee_share: DEFAULT_PROTOCOL_FEE_SHARE,
            referral_fee_share: DEFAULT_REFERRAL_FEE_SHARE,
        }
    }
}

impl InstructionGenerator for CreateConfigAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![AccountMeta::new(self.config, true)];

        accounts
            .append_payer(self.signer)
            .append_system_program()
            .append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::CreateConfig {
            authority: self.authority,
            protocol_fee_recipient: self.protocol_fee_recipient,
            protocol_fee_share: self.protocol_fee_share,
            referral_fee_share: self.referral_fee_share,
        };

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct TransferConfigOwnershipAction {
    // Accounts
    pub config: Pubkey,
    pub signer: Pubkey,
    // Args
    pub pending_authority: Option<Pubkey>,
}

impl TransferConfigOwnershipAction {
    pub fn new(pending_authority: Option<Pubkey>) -> Self {
        Self {
            config: make_address("config"),
            signer: make_address("admin"),
            pending_authority,
        }
    }
}

impl InstructionGenerator for TransferConfigOwnershipAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![AccountMeta::new(self.config, false)];

        accounts
            .append_payer(self.signer)
            .append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::TransferConfigOwnership {
            pending_authority: self.pending_authority,
        };

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct AcceptConfigOwnershipAction {
    // Accounts
    pub config: Pubkey,
    pub signer: Pubkey,
}

impl AcceptConfigOwnershipAction {
    pub fn new(signer: Pubkey) -> Self {
        Self {
            config: make_address("config"),
            signer,
        }
    }
}

impl InstructionGenerator for AcceptConfigOwnershipAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![AccountMeta::new(self.config, false)];

        accounts
            .append_payer(self.signer)
            .append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::AcceptConfigOwnership {};

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

#[derive(Debug)]
pub struct UpdateDefaultFeeSharesAction {
    // Accounts
    pub config: Pubkey,
    pub signer: Pubkey,
    // Args
    pub new_default_protocol_fee_share: u16,
    pub new_referral_fee_share: u16,
}

impl UpdateDefaultFeeSharesAction {
    pub fn new(new_default_protocol_fee_share: u16, new_referral_fee_share: u16) -> Self {
        Self {
            config: make_address("config"),
            signer: make_address("admin"),
            new_default_protocol_fee_share,
            new_referral_fee_share,
        }
    }
}

impl InstructionGenerator for UpdateDefaultFeeSharesAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![AccountMeta::new(self.config, false)];

        accounts
            .append_payer(self.signer)
            .append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::UpdateDefaultFeeShares {
            new_default_protocol_fee_share: self.new_default_protocol_fee_share,
            new_referral_fee_share: self.new_referral_fee_share,
        };

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

#[derive(Debug)]
pub struct UpdateProtocolFeeRecipientAction {
    // Accounts
    pub config: Pubkey,
    pub signer: Pubkey,
    // Args
    pub new_protocol_fee_recipient: Pubkey,
}

impl UpdateProtocolFeeRecipientAction {
    pub fn new(new_protocol_fee_recipient: Pubkey) -> Self {
        Self {
            config: make_address("config"),
            signer: make_address("admin"),
            new_protocol_fee_recipient,
        }
    }
}

impl InstructionGenerator for UpdateProtocolFeeRecipientAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![AccountMeta::new(self.config, false)];

        accounts
            .append_payer(self.signer)
            .append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::UpdateProtocolFeeRecipient {
            new_protocol_fee_recipient: self.new_protocol_fee_recipient,
        };

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct CreateQuoteAssetBadgeAction {
    // Accounts
    pub config: Pubkey,
    pub quote_asset_badge: Pubkey,
    pub token_mint: Pubkey,
    pub signer: Pubkey,
}

impl CreateQuoteAssetBadgeAction {
    pub fn new(token_mint: Pubkey) -> Self {
        let config = make_address("config");

        let quote_asset_badge = Pubkey::find_program_address(
            &[
                QUOTE_TOKEN_BADGE_PDA_SEED.as_bytes(),
                &config.to_bytes(),
                &token_mint.to_bytes(),
            ],
            &token_mill::ID,
        )
        .0;

        Self {
            config,
            quote_asset_badge,
            token_mint,
            signer: make_address("admin"),
        }
    }
}

impl InstructionGenerator for CreateQuoteAssetBadgeAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![
            AccountMeta::new_readonly(self.config, false),
            AccountMeta::new(self.quote_asset_badge, false),
            AccountMeta::new_readonly(self.token_mint, false),
        ];

        accounts
            .append_payer(self.signer)
            .append_system_program()
            .append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::CreateQuoteAssetBadge {};

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct UpdateQuoteAssetBadgeAction {
    // Accounts
    pub config: Pubkey,
    pub quote_asset_badge: Pubkey,
    pub token_mint: Pubkey,
    pub signer: Pubkey,
    // Args
    pub status: QuoteTokenBadgeStatus,
}

impl UpdateQuoteAssetBadgeAction {
    pub fn new(token_mint: Pubkey, status: QuoteTokenBadgeStatus) -> Self {
        let config = make_address("config");
        let quote_asset_badge = Pubkey::find_program_address(
            &[
                QUOTE_TOKEN_BADGE_PDA_SEED.as_bytes(),
                &config.to_bytes(),
                &token_mint.to_bytes(),
            ],
            &token_mill::ID,
        )
        .0;

        Self {
            config,
            quote_asset_badge,
            token_mint,
            signer: make_address("admin"),
            status,
        }
    }
}

impl InstructionGenerator for UpdateQuoteAssetBadgeAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![
            AccountMeta::new_readonly(self.config, false),
            AccountMeta::new(self.quote_asset_badge, false),
            AccountMeta::new_readonly(self.token_mint, false),
        ];

        accounts
            .append_payer(self.signer)
            .append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::UpdateQuoteAssetBadge {
            status: self.status,
        };

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct CreateMarketAction {
    // Accounts
    pub config: Pubkey,
    pub market: Pubkey,
    pub base_token_mint: Pubkey,
    pub market_base_token_ata: Pubkey,
    pub quote_token_badge: Pubkey,
    pub quote_token_mint: Pubkey,
    pub signer: Pubkey,
    // Args
    pub total_supply: u64,
}

impl CreateMarketAction {
    pub fn new(testing_env: &TokenMillEnv) -> Self {
        let config = make_address("config");

        let base_token_mint = testing_env.base_token_mint.unwrap();

        let market = Pubkey::find_program_address(
            &[MARKET_PDA_SEED.as_bytes(), &base_token_mint.to_bytes()],
            &token_mill::ID,
        )
        .0;

        let quote_token_mint = testing_env.quote_token_mint.unwrap();

        let quote_asset_badge = Pubkey::find_program_address(
            &[
                QUOTE_TOKEN_BADGE_PDA_SEED.as_bytes(),
                &config.to_bytes(),
                &quote_token_mint.to_bytes(),
            ],
            &token_mill::ID,
        )
        .0;

        let market_base_token_ata = get_associated_token_address_with_program_id(
            &market,
            &base_token_mint,
            &spl_token_2022::id(),
        );

        Self {
            config,
            market,
            base_token_mint,
            market_base_token_ata,
            quote_token_mint,
            quote_token_badge: quote_asset_badge,
            signer: make_address("alice"),
            total_supply: DEFAULT_TOTAL_SUPPLY,
        }
    }

    pub fn no_badge(&mut self) -> &mut Self {
        self.quote_token_badge = token_mill::ID;

        self
    }
}

impl InstructionGenerator for CreateMarketAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![
            AccountMeta::new_readonly(self.config, false),
            AccountMeta::new(self.market, false),
            AccountMeta::new(self.base_token_mint, true),
            AccountMeta::new(self.market_base_token_ata, false),
            AccountMeta::new_readonly(self.quote_token_badge, false),
            AccountMeta::new_readonly(self.quote_token_mint, false),
        ];

        accounts
            .append_payer(self.signer)
            .append_system_program()
            .append_token_2022_program()
            .append_associated_token_program()
            .append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::CreateMarket {
            name: "name".to_string(),
            symbol: "symbol".to_string(),
            uri: "uri".to_string(),
            total_supply: self.total_supply,
            creator_fee_share: DEFAULT_CREATOR_FEE_SHARE,
            staking_fee_share: DEFAULT_STAKING_FEE_SHARE,
        };

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct CreateMarketWithSplAction {
    // Accounts
    pub config: Pubkey,
    pub market: Pubkey,
    pub base_token_mint: Pubkey,
    pub base_token_metadata: Pubkey,
    pub market_base_token_ata: Pubkey,
    pub quote_token_badge: Pubkey,
    pub quote_token_mint: Pubkey,
    pub signer: Pubkey,
    // Args
    pub total_supply: u64,
}

impl CreateMarketWithSplAction {
    pub fn new(testing_env: &TokenMillEnv) -> Self {
        let config = make_address("config");

        let base_token_mint = testing_env.base_token_mint.unwrap();

        let base_token_metadata = Pubkey::find_program_address(
            &[
                "metadata".as_bytes(),
                &Metadata::id().to_bytes(),
                &base_token_mint.to_bytes(),
            ],
            &Metadata::id(),
        )
        .0;

        let market = Pubkey::find_program_address(
            &[MARKET_PDA_SEED.as_bytes(), &base_token_mint.to_bytes()],
            &token_mill::ID,
        )
        .0;

        let quote_token_mint = testing_env.quote_token_mint.unwrap();

        let quote_asset_badge = Pubkey::find_program_address(
            &[
                QUOTE_TOKEN_BADGE_PDA_SEED.as_bytes(),
                &config.to_bytes(),
                &quote_token_mint.to_bytes(),
            ],
            &token_mill::ID,
        )
        .0;

        let market_base_token_ata = get_associated_token_address_with_program_id(
            &market,
            &base_token_mint,
            &spl_token::id(),
        );

        Self {
            config,
            market,
            base_token_mint,
            base_token_metadata,
            market_base_token_ata,
            quote_token_mint,
            quote_token_badge: quote_asset_badge,
            signer: make_address("alice"),
            total_supply: DEFAULT_TOTAL_SUPPLY,
        }
    }

    pub fn no_badge(&mut self) -> &mut Self {
        self.quote_token_badge = token_mill::ID;

        self
    }
}

impl InstructionGenerator for CreateMarketWithSplAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![
            AccountMeta::new_readonly(self.config, false),
            AccountMeta::new(self.market, false),
            AccountMeta::new(self.base_token_mint, true),
            AccountMeta::new(self.base_token_metadata, false),
            AccountMeta::new(self.market_base_token_ata, false),
            AccountMeta::new_readonly(self.quote_token_badge, false),
            AccountMeta::new_readonly(self.quote_token_mint, false),
        ];

        accounts
            .append_payer(self.signer)
            .append_system_program()
            .append_rent_program()
            .append_token_program()
            .append_metadata_program()
            .append_associated_token_program()
            .append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::CreateMarketWithSpl {
            name: "name".to_string(),
            symbol: "symbol".to_string(),
            uri: "uri".to_string(),
            total_supply: self.total_supply,
            creator_fee_share: DEFAULT_CREATOR_FEE_SHARE,
            staking_fee_share: DEFAULT_STAKING_FEE_SHARE,
        };

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct SetMarketPricesAction {
    // Accounts
    pub market: Pubkey,
    pub signer: Pubkey,
    // Args
    pub price_curve: Curve,
}

impl SetMarketPricesAction {
    pub fn new(curve: Curve) -> Self {
        let market = Pubkey::find_program_address(
            &[
                MARKET_PDA_SEED.as_bytes(),
                &make_address("base_token_mint").to_bytes(),
            ],
            &token_mill::ID,
        )
        .0;

        Self {
            market,
            signer: make_address("alice"),
            price_curve: curve,
        }
    }

    pub fn with_custom_base_token_mint(mut self, base_token_mint: Pubkey) -> Self {
        self.market = Pubkey::find_program_address(
            &[MARKET_PDA_SEED.as_bytes(), &base_token_mint.to_bytes()],
            &token_mill::ID,
        )
        .0;

        self
    }
}

impl InstructionGenerator for SetMarketPricesAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![AccountMeta::new(self.market, false)];

        accounts
            .append_payer(self.signer)
            .append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let Curve {
            bid_prices,
            ask_prices,
        } = self.price_curve;

        let input = token_mill::instruction::SetMarketPrices {
            bid_prices,
            ask_prices,
        };

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct SwapAction {
    // Accounts
    pub config: Pubkey,
    pub market: Pubkey,
    pub base_token_mint: Pubkey,
    pub quote_token_mint: Pubkey,
    pub market_base_token_ata: Pubkey,
    pub market_quote_token_ata: Pubkey,
    pub user_base_token_ata: Pubkey,
    pub user_quote_token_ata: Pubkey,
    pub protocol_quote_token_ata: Pubkey,
    pub referral_quote_token_ata: Pubkey,
    pub signer: Pubkey,
    pub base_token_program: Pubkey,
    pub quote_token_program: Pubkey,
    // Args
    pub swap_type: SwapType,
    pub swap_amount_type: SwapAmountType,
    pub amount: u64,
    pub other_amount_threshold: u64,
}

impl SwapAction {
    pub fn new(
        token_mill_env: &TokenMillEnv,
        swap_type: SwapType,
        swap_amount_type: SwapAmountType,
        amount: u64,
        other_amount_threshold: u64,
        referrer: Option<Pubkey>,
    ) -> Self {
        let config = make_address("config");
        let base_token_mint = token_mill_env.base_token_mint.unwrap();
        let base_token_program = token_mill_env.base_token_type.program_address();

        let signer = make_address("bob");

        let market = Pubkey::find_program_address(
            &[MARKET_PDA_SEED.as_bytes(), &base_token_mint.to_bytes()],
            &token_mill::ID,
        )
        .0;

        let quote_token_mint = token_mill_env.quote_token_mint.unwrap();
        let quote_token_program = token_mill_env.quote_token_type.program_address();

        let market_base_token_ata = get_associated_token_address_with_program_id(
            &market,
            &base_token_mint,
            &token_mill_env.base_token_type.program_address(),
        );

        let market_quote_token_ata = get_associated_token_address_with_program_id(
            &market,
            &quote_token_mint,
            &quote_token_program,
        );

        let user_base_token_ata = get_associated_token_address_with_program_id(
            &signer,
            &base_token_mint,
            &token_mill_env.base_token_type.program_address(),
        );

        let user_quote_token_ata = get_associated_token_address_with_program_id(
            &signer,
            &quote_token_mint,
            &quote_token_program,
        );

        let protocol_quote_token_ata = get_associated_token_address_with_program_id(
            &make_address("dave"),
            &quote_token_mint,
            &quote_token_program,
        );

        let referral_account = if let Some(referrer) = referrer {
            Pubkey::find_program_address(
                &[
                    REFERRAL_ACCOUNT_PDA_SEED.as_bytes(),
                    &config.to_bytes(),
                    &referrer.to_bytes(),
                ],
                &token_mill::ID,
            )
            .0
        } else {
            Pubkey::new_unique()
        };

        let referral_quote_token_ata = if referrer.is_some() {
            get_associated_token_address_with_program_id(
                &referral_account,
                &quote_token_mint,
                &quote_token_program,
            )
        } else {
            token_mill::ID
        };

        Self {
            config,
            market,
            base_token_mint,
            quote_token_mint,
            market_base_token_ata,
            market_quote_token_ata,
            user_base_token_ata,
            user_quote_token_ata,
            protocol_quote_token_ata,
            referral_quote_token_ata,
            signer,
            base_token_program,
            quote_token_program,
            swap_type,
            swap_amount_type,
            amount,
            other_amount_threshold,
        }
    }
}

impl InstructionGenerator for SwapAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![
            AccountMeta::new_readonly(self.config, false),
            AccountMeta::new(self.market, false),
            AccountMeta::new_readonly(self.base_token_mint, false),
            AccountMeta::new_readonly(self.quote_token_mint, false),
            AccountMeta::new(self.market_base_token_ata, false),
            AccountMeta::new(self.market_quote_token_ata, false),
            AccountMeta::new(self.user_base_token_ata, false),
            AccountMeta::new(self.user_quote_token_ata, false),
            AccountMeta::new(self.protocol_quote_token_ata, false),
            AccountMeta::new(self.referral_quote_token_ata, false),
        ];

        accounts.append_payer(self.signer);

        accounts.push(AccountMeta::new_readonly(self.base_token_program, false));
        accounts.push(AccountMeta::new_readonly(self.quote_token_program, false));

        accounts.append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::Swap {
            swap_type: self.swap_type,
            swap_amount_type: self.swap_amount_type,
            amount: self.amount,
            other_amount_threshold: self.other_amount_threshold,
        };

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct UpdateCreatorAction {
    // Accounts
    pub market: Pubkey,
    pub signer: Pubkey,
    // Args
    pub new_creator: Pubkey,
}

impl UpdateCreatorAction {
    pub fn new(new_creator: Pubkey) -> Self {
        let market = Pubkey::find_program_address(
            &[
                MARKET_PDA_SEED.as_bytes(),
                &make_address("base_token_mint").to_bytes(),
            ],
            &token_mill::ID,
        )
        .0;

        Self {
            market,
            signer: make_address("alice"),
            new_creator,
        }
    }
}

impl InstructionGenerator for UpdateCreatorAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![AccountMeta::new(self.market, false)];

        accounts
            .append_payer(self.signer)
            .append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::UpdateCreator {
            new_creator: self.new_creator,
        };

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct UpdateMarketFeeSharesAction {
    // Accounts
    pub market: Pubkey,
    pub signer: Pubkey,
    // Args
    pub new_creator_fee_share: u16,
    pub new_staking_fee_share: u16,
}

impl UpdateMarketFeeSharesAction {
    pub fn new(new_creator_fee_share: u16, new_staking_fee_share: u16) -> Self {
        let market = Pubkey::find_program_address(
            &[
                MARKET_PDA_SEED.as_bytes(),
                &make_address("base_token_mint").to_bytes(),
            ],
            &token_mill::ID,
        )
        .0;

        Self {
            market,
            signer: make_address("alice"),
            new_creator_fee_share,
            new_staking_fee_share,
        }
    }
}

impl InstructionGenerator for UpdateMarketFeeSharesAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![AccountMeta::new(self.market, false)];

        accounts
            .append_payer(self.signer)
            .append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::UpdateMarketFeeShares {
            new_creator_fee_share: self.new_creator_fee_share,
            new_staking_fee_share: self.new_staking_fee_share,
        };

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct ClaimCreatorFeesAction {
    // Accounts
    pub market: Pubkey,
    pub quote_token_mint: Pubkey,
    pub market_quote_token_ata: Pubkey,
    pub creator_quote_token_ata: Pubkey,
    pub signer: Pubkey,
    pub quote_token_program: Pubkey,
}

impl ClaimCreatorFeesAction {
    pub fn new(token_mill_env: &TokenMillEnv) -> Self {
        let market = Pubkey::find_program_address(
            &[
                MARKET_PDA_SEED.as_bytes(),
                &token_mill_env.base_token_mint.unwrap().to_bytes(),
            ],
            &token_mill::ID,
        )
        .0;

        let signer = make_address("alice");

        let quote_token_mint = token_mill_env.quote_token_mint.unwrap();
        let quote_token_program = token_mill_env.quote_token_type.program_address();

        let market_quote_token_ata = get_associated_token_address_with_program_id(
            &market,
            &quote_token_mint,
            &quote_token_program,
        );

        let creator_quote_token_ata = get_associated_token_address_with_program_id(
            &signer,
            &quote_token_mint,
            &quote_token_program,
        );

        Self {
            market,
            quote_token_mint,
            market_quote_token_ata,
            creator_quote_token_ata,
            signer,
            quote_token_program,
        }
    }
}

impl InstructionGenerator for ClaimCreatorFeesAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![
            AccountMeta::new(self.market, false),
            AccountMeta::new_readonly(self.quote_token_mint, false),
            AccountMeta::new(self.market_quote_token_ata, false),
            AccountMeta::new(self.creator_quote_token_ata, false),
        ];

        accounts.append_payer(self.signer);

        match self.quote_token_program {
            spl_token::ID => accounts.append_token_program(),
            spl_token_2022::ID => accounts.append_token_2022_program(),
            _ => unreachable!(),
        };

        accounts.append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::ClaimCreatorFees {};

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct ClaimProtocolFeesAction {
    // Accounts
    pub config: Pubkey,
    pub market: Pubkey,
    pub quote_token_mint: Pubkey,
    pub market_quote_token_ata: Pubkey,
    pub authority_quote_token_ata: Pubkey,
    pub signer: Pubkey,
    pub quote_token_program: Pubkey,
}

pub struct CreateStakingAction {
    // Accounts
    pub market: Pubkey,
    pub staking: Pubkey,
    pub signer: Pubkey,
}

impl CreateStakingAction {
    pub fn new(testing_env: &TokenMillEnv) -> Self {
        let market = Pubkey::find_program_address(
            &[
                MARKET_PDA_SEED.as_bytes(),
                &testing_env.base_token_mint.unwrap().to_bytes(),
            ],
            &token_mill::ID,
        )
        .0;

        let staking = Pubkey::find_program_address(
            &[MARKET_STAKING_PDA_SEED.as_bytes(), &market.to_bytes()],
            &token_mill::ID,
        )
        .0;

        Self {
            market,
            staking,
            signer: make_address("admin"),
        }
    }
}

impl InstructionGenerator for CreateStakingAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![
            AccountMeta::new_readonly(self.market, false),
            AccountMeta::new(self.staking, false),
        ];

        accounts
            .append_payer(self.signer)
            .append_system_program()
            .append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::CreateStaking {};

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct CreateStakePositionAction {
    // Accounts
    pub market: Pubkey,
    pub stake_position: Pubkey,
    pub signer: Pubkey,
}

impl CreateStakePositionAction {
    pub fn new(testing_env: &TokenMillEnv) -> Self {
        let market = Pubkey::find_program_address(
            &[
                MARKET_PDA_SEED.as_bytes(),
                &testing_env.base_token_mint.unwrap().to_bytes(),
            ],
            &token_mill::ID,
        )
        .0;

        let signer: Pubkey = make_address("bob");

        let stake_position = Pubkey::find_program_address(
            &[
                STAKING_POSITION_PDA_SEED.as_bytes(),
                &market.to_bytes(),
                &signer.to_bytes(),
            ],
            &token_mill::ID,
        )
        .0;

        Self {
            market,
            stake_position,
            signer,
        }
    }
}

impl InstructionGenerator for CreateStakePositionAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![
            AccountMeta::new_readonly(self.market, false),
            AccountMeta::new(self.stake_position, false),
        ];

        accounts
            .append_payer(self.signer)
            .append_system_program()
            .append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::CreateStakePosition {};

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct DepositAction {
    // Accounts
    pub market: Pubkey,
    pub market_staking: Pubkey,
    pub stake_position: Pubkey,
    pub base_token_mint: Pubkey,
    pub base_token_program: Pubkey,
    pub market_base_token_ata: Pubkey,
    pub user_base_token_ata: Pubkey,
    pub signer: Pubkey,
    // Args
    pub amount: u64,
}

impl DepositAction {
    pub fn new(testing_env: &TokenMillEnv, amount: u64) -> Self {
        let signer = make_address("bob");
        let base_token_mint = testing_env.base_token_mint.unwrap();
        let base_token_program = testing_env.base_token_type.program_address();

        let market = Pubkey::find_program_address(
            &[MARKET_PDA_SEED.as_bytes(), &base_token_mint.to_bytes()],
            &token_mill::ID,
        )
        .0;

        let market_staking = Pubkey::find_program_address(
            &[MARKET_STAKING_PDA_SEED.as_bytes(), &market.to_bytes()],
            &token_mill::ID,
        )
        .0;

        let stake_position = Pubkey::find_program_address(
            &[
                STAKING_POSITION_PDA_SEED.as_bytes(),
                &market.to_bytes(),
                &signer.to_bytes(),
            ],
            &token_mill::ID,
        )
        .0;

        let market_base_token_ata = get_associated_token_address_with_program_id(
            &market,
            &base_token_mint,
            &base_token_program,
        );

        let user_base_token_ata = get_associated_token_address_with_program_id(
            &signer,
            &base_token_mint,
            &base_token_program,
        );

        Self {
            market,
            market_staking,
            stake_position,
            base_token_mint,
            base_token_program,
            market_base_token_ata,
            user_base_token_ata,
            signer,
            amount,
        }
    }
}

impl InstructionGenerator for DepositAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![
            AccountMeta::new(self.market, false),
            AccountMeta::new(self.market_staking, false),
            AccountMeta::new(self.stake_position, false),
            AccountMeta::new_readonly(self.base_token_mint, false),
            AccountMeta::new(self.market_base_token_ata, false),
            AccountMeta::new(self.user_base_token_ata, false),
        ];

        accounts.append_payer(self.signer);
        accounts.push(AccountMeta::new_readonly(self.base_token_program, false));
        accounts.append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::Deposit {
            amount: self.amount,
        };

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct WithdrawAction {
    // Accounts
    pub market: Pubkey,
    pub market_staking: Pubkey,
    pub stake_position: Pubkey,
    pub base_token_mint: Pubkey,
    pub base_token_program: Pubkey,
    pub market_base_token_ata: Pubkey,
    pub user_base_token_ata: Pubkey,
    pub signer: Pubkey,
    // Args
    pub amount: u64,
}

impl WithdrawAction {
    pub fn new(testing_env: &TokenMillEnv, amount: u64) -> Self {
        let deposit_action = DepositAction::new(testing_env, amount);

        Self {
            market: deposit_action.market,
            market_staking: deposit_action.market_staking,
            stake_position: deposit_action.stake_position,
            base_token_mint: deposit_action.base_token_mint,
            base_token_program: deposit_action.base_token_program,
            market_base_token_ata: deposit_action.market_base_token_ata,
            user_base_token_ata: deposit_action.user_base_token_ata,
            signer: deposit_action.signer,
            amount,
        }
    }
}

impl InstructionGenerator for WithdrawAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![
            AccountMeta::new(self.market, false),
            AccountMeta::new(self.market_staking, false),
            AccountMeta::new(self.stake_position, false),
            AccountMeta::new_readonly(self.base_token_mint, false),
            AccountMeta::new(self.market_base_token_ata, false),
            AccountMeta::new(self.user_base_token_ata, false),
        ];

        accounts.append_payer(self.signer);
        accounts.push(AccountMeta::new_readonly(self.base_token_program, false));
        accounts.append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::Withdraw {
            amount: self.amount,
        };

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct ClaimStakingRewardsAction {
    // Accounts
    pub market: Pubkey,
    pub market_staking: Pubkey,
    pub stake_position: Pubkey,
    pub quote_token_mint: Pubkey,
    pub market_quote_token_ata: Pubkey,
    pub user_quote_token_ata: Pubkey,
    pub quote_token_program: Pubkey,
    pub signer: Pubkey,
}

impl ClaimStakingRewardsAction {
    pub fn new(token_mill_env: &TokenMillEnv) -> Self {
        let base_token_mint = token_mill_env.base_token_mint.unwrap();
        let signer = make_address("bob");

        let market = Pubkey::find_program_address(
            &[MARKET_PDA_SEED.as_bytes(), &base_token_mint.to_bytes()],
            &token_mill::ID,
        )
        .0;

        let market_staking = Pubkey::find_program_address(
            &[MARKET_STAKING_PDA_SEED.as_bytes(), &market.to_bytes()],
            &token_mill::ID,
        )
        .0;

        let stake_position = Pubkey::find_program_address(
            &[
                STAKING_POSITION_PDA_SEED.as_bytes(),
                &market.to_bytes(),
                &signer.to_bytes(),
            ],
            &token_mill::ID,
        )
        .0;

        let quote_token_mint = token_mill_env.quote_token_mint.unwrap();
        let quote_token_program = token_mill_env.quote_token_type.program_address();

        let market_quote_token_ata = get_associated_token_address_with_program_id(
            &market,
            &quote_token_mint,
            &quote_token_program,
        );

        let user_quote_token_ata = get_associated_token_address_with_program_id(
            &signer,
            &quote_token_mint,
            &quote_token_program,
        );

        Self {
            market,
            market_staking,
            stake_position,
            quote_token_mint,
            market_quote_token_ata,
            user_quote_token_ata,
            quote_token_program,
            signer,
        }
    }
}

impl InstructionGenerator for ClaimStakingRewardsAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![
            AccountMeta::new(self.market, false),
            AccountMeta::new(self.market_staking, false),
            AccountMeta::new(self.stake_position, false),
            AccountMeta::new_readonly(self.quote_token_mint, false),
            AccountMeta::new(self.market_quote_token_ata, false),
            AccountMeta::new(self.user_quote_token_ata, false),
        ];

        accounts.append_payer(self.signer);

        match self.quote_token_program {
            spl_token::ID => accounts.append_token_program(),
            spl_token_2022::ID => accounts.append_token_2022_program(),
            _ => unreachable!(),
        };

        accounts.append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::ClaimStakingRewards {};

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct CreateReferralAccountAction {
    // Accounts
    pub config: Pubkey,
    pub referral_account: Pubkey,
    pub signer: Pubkey,
    // Args
    pub referrer: Pubkey,
}

impl Default for CreateReferralAccountAction {
    fn default() -> Self {
        Self::new()
    }
}

impl CreateReferralAccountAction {
    pub fn new() -> Self {
        let config = make_address("config");

        let referrer = make_address("carol");

        let referral_account = Pubkey::find_program_address(
            &[
                REFERRAL_ACCOUNT_PDA_SEED.as_bytes(),
                (config.as_ref()),
                (referrer.as_ref()),
            ],
            &token_mill::ID,
        )
        .0;

        Self {
            config,
            referral_account,
            signer: make_address("admin"),
            referrer,
        }
    }
}

impl InstructionGenerator for CreateReferralAccountAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![
            AccountMeta::new_readonly(self.config, false),
            AccountMeta::new(self.referral_account, false),
        ];

        accounts
            .append_payer(self.signer)
            .append_system_program()
            .append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::CreateReferralAccount {
            referrer: self.referrer,
        };

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct ClaimReferralFeesAction {
    pub config: Pubkey,
    pub referral_account: Pubkey,
    pub quote_token_mint: Pubkey,
    pub referral_account_quote_token_ata: Pubkey,
    pub referrer_quote_token_ata: Pubkey,
    pub signer: Pubkey,
    pub quote_token_program: Pubkey,
}

impl ClaimReferralFeesAction {
    pub fn new(token_mill_env: &TokenMillEnv) -> Self {
        let config = make_address("config");
        let signer = make_address("carol");

        let referral_account = Pubkey::find_program_address(
            &[
                REFERRAL_ACCOUNT_PDA_SEED.as_bytes(),
                (config.as_ref()),
                (signer.as_ref()),
            ],
            &token_mill::ID,
        )
        .0;

        let quote_token_mint = token_mill_env.quote_token_mint.unwrap();
        let quote_token_program = token_mill_env.quote_token_type.program_address();

        let referral_account_quote_token_ata = get_associated_token_address_with_program_id(
            &referral_account,
            &quote_token_mint,
            &quote_token_program,
        );

        let referrer_quote_token_ata = get_associated_token_address_with_program_id(
            &signer,
            &quote_token_mint,
            &quote_token_program,
        );

        Self {
            config,
            referral_account,
            quote_token_mint,
            referral_account_quote_token_ata,
            referrer_quote_token_ata,
            signer,
            quote_token_program,
        }
    }
}

impl InstructionGenerator for ClaimReferralFeesAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![
            AccountMeta::new_readonly(self.referral_account, false),
            AccountMeta::new_readonly(self.quote_token_mint, false),
            AccountMeta::new(self.referral_account_quote_token_ata, false),
            AccountMeta::new(self.referrer_quote_token_ata, false),
        ];

        accounts.append_payer(self.signer);

        match self.quote_token_program {
            spl_token::ID => accounts.append_token_program(),
            spl_token_2022::ID => accounts.append_token_2022_program(),
            _ => unreachable!(),
        };

        accounts.append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::ClaimReferralFees {};

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct CreateVestingPlanAction {
    // Accounts
    pub market: Pubkey,
    pub staking: Pubkey,
    pub staking_position: Pubkey,
    pub vesting_plan: Pubkey,
    pub base_token_mint: Pubkey,
    pub base_token_program: Pubkey,
    pub market_base_token_ata: Pubkey,
    pub user_base_token_ata: Pubkey,
    pub signer: Pubkey,
    // Args
    pub vesting_amount: u64,
    pub start: i64,
    pub vesting_duration: i64,
    pub cliff_duration: i64,
}

impl CreateVestingPlanAction {
    pub fn new(
        token_mill_env: &TokenMillEnv,
        vesting_amount: u64,
        start: i64,
        vesting_duration: i64,
        cliff_duration: i64,
    ) -> Self {
        let base_token_mint = make_address("base_token_mint");
        let base_token_program = token_mill_env.base_token_type.program_address();

        let vesting_plan = make_address("vesting_plan");
        let signer = make_address("bob");

        let market = Pubkey::find_program_address(
            &[MARKET_PDA_SEED.as_bytes(), &base_token_mint.to_bytes()],
            &token_mill::ID,
        )
        .0;

        let staking = Pubkey::find_program_address(
            &[MARKET_STAKING_PDA_SEED.as_bytes(), &market.to_bytes()],
            &token_mill::ID,
        )
        .0;

        let staking_position = Pubkey::find_program_address(
            &[
                STAKING_POSITION_PDA_SEED.as_bytes(),
                &market.to_bytes(),
                &signer.to_bytes(),
            ],
            &token_mill::ID,
        )
        .0;

        let market_base_token_ata = get_associated_token_address_with_program_id(
            &market,
            &base_token_mint,
            &base_token_program,
        );

        let user_base_token_ata = get_associated_token_address_with_program_id(
            &signer,
            &base_token_mint,
            &base_token_program,
        );

        Self {
            market,
            staking,
            staking_position,
            vesting_plan,
            base_token_mint,
            base_token_program,
            market_base_token_ata,
            user_base_token_ata,
            signer,
            vesting_amount,
            start,
            vesting_duration,
            cliff_duration,
        }
    }
}

impl InstructionGenerator for CreateVestingPlanAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![
            AccountMeta::new(self.market, false),
            AccountMeta::new(self.staking, false),
            AccountMeta::new(self.staking_position, false),
            AccountMeta::new(self.vesting_plan, true),
            AccountMeta::new_readonly(self.base_token_mint, false),
            AccountMeta::new(self.market_base_token_ata, false),
            AccountMeta::new(self.user_base_token_ata, false),
        ];

        accounts.append_payer(self.signer);
        accounts.push(AccountMeta::new_readonly(self.base_token_program, false));
        accounts
            .append_system_program()
            .append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::CreateVestingPlan {
            vesting_amount: self.vesting_amount,
            start: self.start,
            vesting_duration: self.vesting_duration,
            cliff_duration: self.cliff_duration,
        };

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}

pub struct ReleaseAction {
    // Accounts
    pub market: Pubkey,
    pub staking: Pubkey,
    pub staking_position: Pubkey,
    pub vesting_plan: Pubkey,
    pub base_token_mint: Pubkey,
    pub base_token_program: Pubkey,
    pub market_base_token_ata: Pubkey,
    pub user_base_token_ata: Pubkey,
    pub signer: Pubkey,
}

impl ReleaseAction {
    pub fn new(token_mill_env: &TokenMillEnv) -> Self {
        let create_vesting_plan_action = CreateVestingPlanAction::new(token_mill_env, 0, 0, 0, 0);

        Self {
            market: create_vesting_plan_action.market,
            staking: create_vesting_plan_action.staking,
            staking_position: create_vesting_plan_action.staking_position,
            vesting_plan: create_vesting_plan_action.vesting_plan,
            base_token_mint: create_vesting_plan_action.base_token_mint,
            base_token_program: create_vesting_plan_action.base_token_program,
            market_base_token_ata: create_vesting_plan_action.market_base_token_ata,
            user_base_token_ata: create_vesting_plan_action.user_base_token_ata,
            signer: create_vesting_plan_action.signer,
        }
    }
}

impl InstructionGenerator for ReleaseAction {
    fn accounts(&self) -> Vec<AccountMeta> {
        let mut accounts = vec![
            AccountMeta::new(self.market, false),
            AccountMeta::new(self.staking, false),
            AccountMeta::new(self.staking_position, false),
            AccountMeta::new(self.vesting_plan, false),
            AccountMeta::new_readonly(self.base_token_mint, false),
            AccountMeta::new(self.market_base_token_ata, false),
            AccountMeta::new(self.user_base_token_ata, false),
        ];

        accounts.append_payer(self.signer);
        accounts.push(AccountMeta::new_readonly(self.base_token_program, false));
        accounts.append_cpi_event_accounts(tm_event_authority());

        accounts
    }

    fn instruction(&self) -> Instruction {
        let input = token_mill::instruction::Release {};

        Instruction {
            program_id: token_mill::ID,
            accounts: self.accounts(),
            data: input.data(),
        }
    }
}
