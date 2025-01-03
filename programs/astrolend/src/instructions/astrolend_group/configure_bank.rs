use crate::constants::{EMISSIONS_AUTH_SEED, EMISSIONS_TOKEN_ACCOUNT_SEED};
use crate::events::{GroupEventHeader, LendingPoolBankConfigureEvent};
use crate::prelude::AstrolendError;
use crate::{check, math_error, utils};
use crate::{
    state::astrolend_group::{Bank, BankConfigOpt, AstrolendGroup},
    AstrolendResult,
};
use anchor_lang::prelude::*;
use anchor_spl::token_2022::{transfer_checked, TransferChecked};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use fixed::types::I80F48;

pub fn lending_pool_configure_bank(
    ctx: Context<LendingPoolConfigureBank>,
    bank_config: BankConfigOpt,
) -> AstrolendResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    bank.configure(&bank_config)?;

    if bank_config.oracle.is_some() {
        bank.config.validate_oracle_setup(ctx.remaining_accounts)?;
    }

    emit!(LendingPoolBankConfigureEvent {
        header: GroupEventHeader {
            astrolend_group: ctx.accounts.astrolend_group.key(),
            signer: Some(*ctx.accounts.admin.key)
        },
        bank: ctx.accounts.bank.key(),
        mint: bank.mint,
        config: bank_config,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolConfigureBank<'info> {
    pub astrolend_group: AccountLoader<'info, AstrolendGroup>,

    #[account(
        address = astrolend_group.load()?.admin,
    )]
    pub admin: Signer<'info>,

    #[account(
        mut,
        constraint = bank.load()?.group == astrolend_group.key(),
    )]
    pub bank: AccountLoader<'info, Bank>,
}

pub fn lending_pool_setup_emissions(
    ctx: Context<LendingPoolSetupEmissions>,
    emissions_flags: u64,
    emissions_rate: u64,
    total_emissions: u64,
) -> AstrolendResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    check!(
        bank.emissions_mint.eq(&Pubkey::default()),
        AstrolendError::EmissionsAlreadySetup
    );

    bank.emissions_mint = ctx.accounts.emissions_mint.key();

    bank.override_emissions_flag(emissions_flags);

    bank.emissions_rate = emissions_rate;
    bank.emissions_remaining = I80F48::from_num(total_emissions).into();

    let initial_emissions_amount_pre_fee = utils::calculate_pre_fee_spl_deposit_amount(
        ctx.accounts.emissions_mint.to_account_info(),
        total_emissions,
        Clock::get()?.epoch,
    )?;

    transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.emissions_funding_account.to_account_info(),
                to: ctx.accounts.emissions_token_account.to_account_info(),
                authority: ctx.accounts.admin.to_account_info(),
                mint: ctx.accounts.emissions_mint.to_account_info(),
            },
        ),
        initial_emissions_amount_pre_fee,
        ctx.accounts.emissions_mint.decimals,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolSetupEmissions<'info> {
    pub astrolend_group: AccountLoader<'info, AstrolendGroup>,

    #[account(
        mut,
        address = astrolend_group.load()?.admin,
    )]
    pub admin: Signer<'info>,

    #[account(
        mut,
        constraint = bank.load()?.group == astrolend_group.key(),
    )]
    pub bank: AccountLoader<'info, Bank>,

    pub emissions_mint: InterfaceAccount<'info, Mint>,

    #[account(
        seeds = [
            EMISSIONS_AUTH_SEED.as_bytes(),
            bank.key().as_ref(),
            emissions_mint.key().as_ref(),
        ],
        bump
    )]
    /// CHECK: Asserted by PDA constraints
    pub emissions_auth: AccountInfo<'info>,

    #[account(
        init,
        payer = admin,
        token::mint = emissions_mint,
        token::authority = emissions_auth,
        seeds = [
            EMISSIONS_TOKEN_ACCOUNT_SEED.as_bytes(),
            bank.key().as_ref(),
            emissions_mint.key().as_ref(),
        ],
        bump,
    )]
    pub emissions_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Account provided only for funding rewards
    #[account(mut)]
    pub emissions_funding_account: AccountInfo<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn lending_pool_update_emissions_parameters(
    ctx: Context<LendingPoolUpdateEmissionsParameters>,
    emissions_flags: Option<u64>,
    emissions_rate: Option<u64>,
    additional_emissions: Option<u64>,
) -> AstrolendResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    check!(
        bank.emissions_mint.ne(&Pubkey::default()),
        AstrolendError::EmissionsUpdateError
    );

    check!(
        bank.emissions_mint.eq(&ctx.accounts.emissions_mint.key()),
        AstrolendError::EmissionsUpdateError
    );

    if let Some(flags) = emissions_flags {
        msg!("Updating emissions flags to {:#010b}", flags);
        bank.flags = flags;
    }

    if let Some(rate) = emissions_rate {
        msg!("Updating emissions rate to {}", rate);
        bank.emissions_rate = rate;
    }

    if let Some(additional_emissions) = additional_emissions {
        bank.emissions_remaining = I80F48::from(bank.emissions_remaining)
            .checked_add(I80F48::from_num(additional_emissions))
            .ok_or_else(math_error!())?
            .into();

        msg!(
            "Adding {} emissions, total {}",
            additional_emissions,
            I80F48::from(bank.emissions_remaining)
        );

        let additional_emissions_amount_pre_fee = utils::calculate_pre_fee_spl_deposit_amount(
            ctx.accounts.emissions_mint.to_account_info(),
            additional_emissions,
            Clock::get()?.epoch,
        )?;

        transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.emissions_funding_account.to_account_info(),
                    to: ctx.accounts.emissions_token_account.to_account_info(),
                    authority: ctx.accounts.admin.to_account_info(),
                    mint: ctx.accounts.emissions_mint.to_account_info(),
                },
            ),
            additional_emissions_amount_pre_fee,
            ctx.accounts.emissions_mint.decimals,
        )?;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolUpdateEmissionsParameters<'info> {
    pub astrolend_group: AccountLoader<'info, AstrolendGroup>,

    #[account(
        mut,
        address = astrolend_group.load()?.admin,
    )]
    pub admin: Signer<'info>,

    #[account(
        mut,
        constraint = bank.load()?.group == astrolend_group.key(),
    )]
    pub bank: AccountLoader<'info, Bank>,

    pub emissions_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        seeds = [
            EMISSIONS_TOKEN_ACCOUNT_SEED.as_bytes(),
            bank.key().as_ref(),
            emissions_mint.key().as_ref(),
        ],
        bump,
    )]
    pub emissions_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Account provided only for funding rewards
    #[account(mut)]
    pub emissions_funding_account: AccountInfo<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}
