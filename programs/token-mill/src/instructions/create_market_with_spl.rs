use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    metadata::{
        create_metadata_accounts_v3, mpl_token_metadata::types::DataV2, CreateMetadataAccountsV3,
        Metadata,
    },
    token::{
        mint_to, set_authority, spl_token::instruction::AuthorityType, Mint, MintTo, SetAuthority,
        Token, TokenAccount,
    },
    token_interface::Mint as MintInterface,
};

use crate::{
    constant::{MAX_BPS, MILL_TOKEN_DECIMALS},
    errors::TokenMillError,
    events::TokenMillMarketCreationEvent,
    manager::token_manager::check_mint_extensions,
    state::{Market, TokenMillConfig},
    QuoteTokenBadge, QuoteTokenBadgeStatus, MARKET_PDA_SEED, QUOTE_TOKEN_BADGE_PDA_SEED,
};

#[event_cpi]
#[derive(Accounts)]
pub struct CreateMarketWithSpl<'info> {
    pub config: Account<'info, TokenMillConfig>,

    #[account(
        init,
        seeds = [MARKET_PDA_SEED.as_bytes(), base_token_mint.key().as_ref()],
        bump,
        payer = creator,
        space = 8 + Market::INIT_SPACE
    )]
    pub market: AccountLoader<'info, Market>,

    #[account(
        init,
        payer = creator,
        mint::authority = market,
        mint::decimals = MILL_TOKEN_DECIMALS
    )]
    pub base_token_mint: Box<Account<'info, Mint>>,

    /// CHECK: New Metaplex Account being created
    #[account(mut)]
    pub base_token_metadata: UncheckedAccount<'info>,

    #[account(
        init,
        payer = creator,
        associated_token::mint = base_token_mint,
        associated_token::authority = market,
        associated_token::token_program = token_program
    )]
    pub market_base_token_ata: Box<Account<'info, TokenAccount>>,

    #[account(
        seeds = [
            QUOTE_TOKEN_BADGE_PDA_SEED.as_bytes(),
            config.key().as_ref(),
            quote_token_mint.key().as_ref(),
        ],
        bump = quote_token_badge.bump,
        constraint = quote_token_badge.status == QuoteTokenBadgeStatus::Enabled || creator.key() == config.authority @ TokenMillError::InvalidQuoteAssetBadge,
    )]
    pub quote_token_badge: Option<Account<'info, QuoteTokenBadge>>,

    pub quote_token_mint: Box<InterfaceAccount<'info, MintInterface>>,

    #[account(mut)]
    pub creator: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub token_metadata_program: Program<'info, Metadata>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn handler(
    ctx: Context<CreateMarketWithSpl>,
    name: String,
    symbol: String,
    uri: String,
    total_supply: u64,
    creator_fee_share: u16,
    staking_fee_share: u16,
) -> Result<()> {
    let config = &ctx.accounts.config;

    require_eq!(
        creator_fee_share + staking_fee_share + config.default_protocol_fee_share,
        MAX_BPS as u16,
        TokenMillError::InvalidFeeShare
    );

    require!(
        check_mint_extensions(&ctx.accounts.quote_token_mint)?,
        TokenMillError::UnsupportedTokenMint
    );

    {
        let mut market = ctx.accounts.market.load_init()?;

        market.initialize(
            ctx.bumps.market,
            config.key(),
            ctx.accounts.creator.key(),
            ctx.accounts.base_token_mint.key(),
            ctx.accounts.quote_token_mint.key(),
            ctx.accounts.quote_token_mint.decimals,
            total_supply,
            creator_fee_share,
            staking_fee_share,
        )?;
    }

    let base_token_mint_key = ctx.accounts.base_token_mint.key();
    let market_seeds = [
        MARKET_PDA_SEED.as_bytes(),
        base_token_mint_key.as_ref(),
        &[ctx.bumps.market],
    ];

    ctx.accounts
        .initialize_token_metadata(name, symbol, uri, &market_seeds)?;

    ctx.accounts
        .mint_supply_and_remove_authority(total_supply, &market_seeds)?;

    emit_cpi!(TokenMillMarketCreationEvent {
        config: ctx.accounts.config.key(),
        market: ctx.accounts.market.key(),
        creator: ctx.accounts.creator.key(),
        base_token_mint: ctx.accounts.base_token_mint.key(),
        quote_token_mint: ctx.accounts.quote_token_mint.key(),
        total_supply,
        protocol_fee_share: config.default_protocol_fee_share,
        referral_fee_share: config.referral_fee_share,
        creator_fee_share,
        staking_fee_share,
    });

    Ok(())
}

impl<'info> CreateMarketWithSpl<'info> {
    fn initialize_token_metadata(
        &self,
        name: String,
        symbol: String,
        uri: String,
        market_seeds: &[&[u8]],
    ) -> Result<()> {
        let token_data: DataV2 = DataV2 {
            name,
            symbol,
            uri,
            seller_fee_basis_points: 0,
            creators: None,
            collection: None,
            uses: None,
        };

        create_metadata_accounts_v3(
            CpiContext::new_with_signer(
                self.token_metadata_program.to_account_info(),
                CreateMetadataAccountsV3 {
                    payer: self.creator.to_account_info(),
                    update_authority: self.market.to_account_info(),
                    mint: self.base_token_mint.to_account_info(),
                    metadata: self.base_token_metadata.to_account_info(),
                    mint_authority: self.market.to_account_info(),
                    system_program: self.system_program.to_account_info(),
                    rent: self.system_program.to_account_info(),
                },
                &[market_seeds],
            ),
            token_data,
            false,
            true,
            None,
        )?;

        Ok(())
    }

    fn mint_supply_and_remove_authority(
        &self,
        total_supply: u64,
        market_seeds: &[&[u8]],
    ) -> Result<()> {
        let cpi_accounts = MintTo {
            mint: self.base_token_mint.to_account_info(),
            to: self.market_base_token_ata.to_account_info(),
            authority: self.market.to_account_info(),
        };

        mint_to(
            CpiContext::new_with_signer(
                self.token_program.to_account_info(),
                cpi_accounts,
                &[market_seeds],
            ),
            total_supply,
        )?;

        let cpi_accounts = SetAuthority {
            account_or_mint: self.base_token_mint.to_account_info(),
            current_authority: self.market.to_account_info(),
        };

        set_authority(
            CpiContext::new_with_signer(
                self.token_program.to_account_info(),
                cpi_accounts,
                &[market_seeds],
            ),
            AuthorityType::MintTokens,
            None,
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        constant::{INTERVAL_NUMBER, MAX_TOTAL_SUPPLY},
        Market,
    };
    use joelana_test_utils::joelana_env::{
        actions::token_mill::{
            tm_parse_error, CreateMarketWithSplAction, CreateQuoteAssetBadgeAction, TokenMillEnv,
            UpdateQuoteAssetBadgeAction, DEFAULT_TOTAL_SUPPLY,
        },
        TokenMillError, TokenType,
    };
    use rstest::rstest;

    fn setup_env(
        quote_token_type: TokenType,
        quote_token_decimals: u8,
    ) -> (TokenMillEnv, CreateMarketWithSplAction) {
        let mut testing_env =
            TokenMillEnv::new().with_quote_token_mint(quote_token_type, quote_token_decimals);

        let create_badge_action =
            CreateQuoteAssetBadgeAction::new(testing_env.quote_token_mint.unwrap());

        testing_env
            .svm
            .execute_actions(&[&create_badge_action])
            .unwrap();

        testing_env.svm.change_payer("alice");

        let action = CreateMarketWithSplAction::new(&testing_env);

        (testing_env, action)
    }

    #[rstest]
    fn create_market_with_spl(
        #[values(TokenType::Token, TokenType::Token2022)] token_type: TokenType,
        #[values(6, 9)] quote_token_decimals: u8,
    ) {
        let (mut testing_env, action) = setup_env(token_type, quote_token_decimals);

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_ok());

        let market = testing_env.svm.get_parsed_account::<Market>(&action.market);

        assert_eq!(market.config, action.config);
        assert_eq!(market.creator, testing_env.svm.payer);
        assert_eq!(market.base_token_mint, action.base_token_mint);
        assert_eq!(market.quote_token_mint, action.quote_token_mint);
        assert_eq!(market.quote_token_decimals, quote_token_decimals);
        assert_eq!(market.total_supply, DEFAULT_TOTAL_SUPPLY);
        assert_eq!(market.base_reserve, DEFAULT_TOTAL_SUPPLY);
    }

    #[test]
    fn create_market_with_disabled_quote_asset_badge() {
        let (mut testing_env, mut action) = setup_env(TokenType::Token, 6);

        let disable_action = UpdateQuoteAssetBadgeAction::new(
            action.quote_token_mint,
            joelana_test_utils::joelana_env::QuoteTokenBadgeStatus::Disabled,
        );

        testing_env.svm.execute_actions(&[&disable_action]).unwrap();

        action.signer = testing_env.svm.change_payer("alice");

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::InvalidQuoteAssetBadge);
    }

    #[test]
    fn create_market_with_no_badge_as_admin() {
        let (mut testing_env, _) = setup_env(TokenType::Token, 6);

        testing_env.svm.create_token(TokenType::Token, 6).unwrap();

        let mut action = CreateMarketWithSplAction::new(&testing_env);

        let result = testing_env.svm.execute_actions(&[action.no_badge()]);

        assert!(result.is_ok());
    }

    #[rstest]
    fn create_market_with_invalid_supply(
        #[values(10 * INTERVAL_NUMBER, MAX_TOTAL_SUPPLY + INTERVAL_NUMBER, DEFAULT_TOTAL_SUPPLY + 1)]
        total_supply: u64,
    ) {
        let (mut testing_env, mut action) = setup_env(TokenType::Token, 6);

        action.total_supply = total_supply;

        let result = testing_env.svm.execute_actions(&[&action]);

        assert!(result.is_err());

        let error = tm_parse_error(result).unwrap();

        assert_eq!(error, TokenMillError::InvalidTotalSupply);
    }
}
