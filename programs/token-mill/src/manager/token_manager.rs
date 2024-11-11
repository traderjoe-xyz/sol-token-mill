use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_2022::spl_token_2022::{
        self,
        extension::{BaseStateWithExtensions, ExtensionType, StateWithExtensions},
    },
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked},
};

/// Checks that the mint account only has allowed extensions.
/// Tax-transfer quote tokens for example would cause some issues with the current implementation.
pub fn check_mint_extensions(mint_account: &InterfaceAccount<Mint>) -> Result<bool> {
    let mint_account_info = mint_account.to_account_info();
    if *mint_account_info.owner == Token::id() {
        return Ok(true);
    }

    let mint_data = mint_account_info.try_borrow_data()?;
    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;
    let extensions = mint.get_extension_types()?;

    for e in extensions {
        if e != ExtensionType::MetadataPointer && e != ExtensionType::TokenMetadata {
            return Ok(false);
        }
    }

    Ok(true)
}

pub fn transfer_from_pda<'info>(
    mint: &InterfaceAccount<'info, Mint>,
    pda: AccountInfo<'info>,
    pda_token_account: &InterfaceAccount<'info, TokenAccount>,
    recipient_token_account: &InterfaceAccount<'info, TokenAccount>,
    token_program: &Interface<'info, TokenInterface>,
    amount: u64,
    pda_seeds: &[&[u8]],
) -> Result<()> {
    transfer_checked(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            TransferChecked {
                from: pda_token_account.to_account_info(),
                mint: mint.to_account_info(),
                to: recipient_token_account.to_account_info(),
                authority: pda,
            },
            &[pda_seeds],
        ),
        amount,
        mint.decimals,
    )
}

pub fn transfer_from_eoa<'info>(
    mint: &InterfaceAccount<'info, Mint>,
    eoa: &Signer<'info>,
    eoa_token_account: &InterfaceAccount<'info, TokenAccount>,
    recipient_token_account: &InterfaceAccount<'info, TokenAccount>,
    token_program: &Interface<'info, TokenInterface>,
    amount: u64,
) -> Result<()> {
    transfer_checked(
        CpiContext::new(
            token_program.to_account_info(),
            TransferChecked {
                from: eoa_token_account.to_account_info(),
                mint: mint.to_account_info(),
                to: recipient_token_account.to_account_info(),
                authority: eoa.to_account_info(),
            },
        ),
        amount,
        mint.decimals,
    )
}
