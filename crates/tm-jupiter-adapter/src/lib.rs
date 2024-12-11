use anchor_lang::{prelude::AccountMeta, AccountDeserialize};
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id, token, token_2022,
};
use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use token_mill::{
    manager::swap_manager::{SwapAmountType, SwapType},
    state::{Market, TokenMillConfig},
};

use jupiter_amm_interface::{
    try_get_account_data, AccountMap, Amm, AmmContext, KeyedAccount, Quote, QuoteParams, Swap,
    SwapAndAccountMetas, SwapMode, SwapParams,
};

pub struct TokenMillAmm {
    key: Pubkey,
    label: String,
    protocol_fee_recipient: Pubkey,
    quote_token_program_id: Pubkey,
    state: Market,
    program_id: Pubkey,
}

impl Clone for TokenMillAmm {
    fn clone(&self) -> Self {
        TokenMillAmm {
            key: self.key,
            label: self.label.clone(),
            protocol_fee_recipient: self.protocol_fee_recipient,
            quote_token_program_id: token::ID,
            state: Market {
                config: self.state.config.clone(),
                creator: self.state.creator,

                base_token_mint: self.state.base_token_mint,
                quote_token_mint: self.state.quote_token_mint,

                base_reserve: self.state.base_reserve,

                bid_prices: self.state.bid_prices,
                ask_prices: self.state.ask_prices,

                width_scaled: self.state.width_scaled,
                total_supply: self.state.total_supply,

                fees: self.state.fees,

                quote_token_decimals: self.state.quote_token_decimals,
                bump: self.state.bump,

                _space: self.state._space,
            },
            program_id: self.program_id,
        }
    }
}

impl Amm for TokenMillAmm {
    fn from_keyed_account(keyed_account: &KeyedAccount, _amm_context: &AmmContext) -> Result<Self> {
        let mut data_slice: &[u8] = &keyed_account.account.data;
        let state = Market::try_deserialize(&mut data_slice)?;

        let label = "Token Mill".to_string();

        Ok(Self {
            key: keyed_account.key,
            label,
            protocol_fee_recipient: Pubkey::default(),
            quote_token_program_id: Pubkey::default(),
            state,
            program_id: keyed_account.account.owner,
        })
    }

    fn label(&self) -> String {
        self.label.clone()
    }

    fn program_id(&self) -> Pubkey {
        self.program_id
    }

    fn key(&self) -> Pubkey {
        self.key
    }

    fn get_accounts_to_update(&self) -> Vec<Pubkey> {
        vec![self.key, self.state.config, self.state.quote_token_mint]
    }

    fn update(&mut self, account_map: &AccountMap) -> Result<()> {
        // Market
        let account = try_get_account_data(account_map, &self.key)?;

        let mut data_slice: &[u8] = &account;
        let market = Market::try_deserialize(&mut data_slice)?;
        self.state.base_reserve = market.base_reserve;

        // Config
        let account = try_get_account_data(account_map, &self.state.config)?;
        let mut data_slice: &[u8] = &account;
        let config = TokenMillConfig::try_deserialize(&mut data_slice)?;
        self.protocol_fee_recipient = config.protocol_fee_recipient;

        // TODO: Get owner of the quote token mint
        self.quote_token_program_id = token::ID;

        Ok(())
    }

    fn quote(&self, quote_params: &QuoteParams) -> Result<Quote> {
        let QuoteParams {
            amount,
            input_mint,
            swap_mode,
            ..
        } = quote_params;

        let market = &self.state;

        let swap_type = match input_mint {
            mint if mint == &market.base_token_mint => SwapType::Sell,
            mint if mint == &market.quote_token_mint => SwapType::Buy,
            _ => return Err(anyhow::anyhow!("Invalid input mint")),
        };

        let swap_amount_type = match swap_mode {
            SwapMode::ExactIn => SwapAmountType::ExactInput,
            SwapMode::ExactOut => SwapAmountType::ExactOutput,
        };

        let (base_amount, quote_amount) = match (swap_type, swap_amount_type) {
            (SwapType::Buy, SwapAmountType::ExactInput) => market.get_base_amount_out(*amount)?,
            (SwapType::Buy, SwapAmountType::ExactOutput) => {
                market.get_quote_amount(*amount, swap_amount_type)?
            }
            (SwapType::Sell, SwapAmountType::ExactInput) => {
                market.get_quote_amount(*amount, swap_amount_type)?
            }
            (SwapType::Sell, SwapAmountType::ExactOutput) => market.get_base_amount_in(*amount)?,
        };

        let mut fee_amount = 0;
        let (in_amount, out_amount);

        match swap_type {
            SwapType::Buy => {
                let (_, buyback_amount) = market.get_quote_amount_with_parameters(
                    market.circulating_supply(),
                    base_amount,
                    SwapAmountType::ExactInput,
                    token_mill::math::Rounding::Up,
                )?;

                if quote_amount > buyback_amount {
                    fee_amount = quote_amount - buyback_amount;
                }

                in_amount = quote_amount;
                out_amount = base_amount;
            }
            SwapType::Sell => {
                in_amount = base_amount;
                out_amount = quote_amount;
            }
        }

        Ok(Quote {
            in_amount,
            out_amount,
            fee_amount,
            fee_mint: self.state.quote_token_mint,
            ..Default::default()
        })
    }

    fn get_reserve_mints(&self) -> Vec<Pubkey> {
        vec![self.state.base_token_mint, self.state.quote_token_mint]
    }

    fn get_swap_and_account_metas(&self, swap_params: &SwapParams) -> Result<SwapAndAccountMetas> {
        let mut account_metas: Vec<AccountMeta> = Vec::new();

        // Config
        account_metas.push(AccountMeta {
            pubkey: self.state.config,
            is_signer: false,
            is_writable: false,
        });

        // Market
        account_metas.push(AccountMeta {
            pubkey: self.key,
            is_signer: false,
            is_writable: true,
        });

        // Mints
        account_metas.push(AccountMeta {
            pubkey: self.state.base_token_mint,
            is_signer: false,
            is_writable: false,
        });
        account_metas.push(AccountMeta {
            pubkey: self.state.quote_token_mint,
            is_signer: false,
            is_writable: false,
        });

        // ATAs
        let market_base_token_account = get_associated_token_address_with_program_id(
            &self.key,
            &self.state.base_token_mint,
            &token_2022::ID,
        );
        account_metas.push(AccountMeta {
            pubkey: market_base_token_account,
            is_signer: false,
            is_writable: true,
        });

        let market_quote_token_account = get_associated_token_address_with_program_id(
            &self.key,
            &self.state.quote_token_mint,
            &token_2022::ID,
        );
        account_metas.push(AccountMeta {
            pubkey: market_quote_token_account,
            is_signer: false,
            is_writable: true,
        });

        let user_base_token_account = get_associated_token_address_with_program_id(
            &swap_params.token_transfer_authority,
            &self.state.base_token_mint,
            &token_2022::ID,
        );
        account_metas.push(AccountMeta {
            pubkey: user_base_token_account,
            is_signer: false,
            is_writable: true,
        });

        let user_quote_token_account = get_associated_token_address_with_program_id(
            &swap_params.token_transfer_authority,
            &self.state.quote_token_mint,
            &token_2022::ID,
        );
        account_metas.push(AccountMeta {
            pubkey: user_quote_token_account,
            is_signer: false,
            is_writable: true,
        });

        let protocol_quote_token_account: Pubkey = get_associated_token_address_with_program_id(
            &self.protocol_fee_recipient,
            &self.state.quote_token_mint,
            &token_2022::ID,
        );
        account_metas.push(AccountMeta {
            pubkey: protocol_quote_token_account,
            is_signer: false,
            is_writable: true,
        });

        // User
        account_metas.push(AccountMeta {
            pubkey: swap_params.token_transfer_authority,
            is_signer: true,
            is_writable: false,
        });

        // Token program
        account_metas.push(AccountMeta {
            pubkey: token_2022::ID,
            is_signer: false,
            is_writable: false,
        });
        account_metas.push(AccountMeta {
            pubkey: self.quote_token_program_id,
            is_signer: false,
            is_writable: false,
        });

        Ok(SwapAndAccountMetas {
            swap: Swap::TokenSwap,
            account_metas,
        })
    }

    fn supports_exact_out(&self) -> bool {
        true
    }

    fn clone_amm(&self) -> Box<dyn Amm + Send + Sync> {
        Box::new(self.clone())
    }
}
