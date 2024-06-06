use anchor_lang::{
    accounts::signer,
    prelude::*,
    solana_program::{
        self,
        instruction::{AccountMeta, Instruction},
        program::invoke_signed,
        program_error::ProgramError,
        program_pack::Pack,
        system_instruction,
        sysvar::clock::Clock,
    },
    Result,
};
use anchor_spl::{
    associated_token,
    token::{
        self,
        spl_token::{self, instruction::AuthorityType, state::Mint},
        InitializeAccount, Mint as MintAccount, MintTo, SetAuthority, Token, TokenAccount,
        Transfer,
    },
};

use borsh::{BorshDeserialize, BorshSerialize};

use crate::access::types::{
    AccessControl, DonateTracker, OwnerAccount, CONSTRAINT_SEED, DONATE_SEED, PLATFORM_SEED,
};

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(has_one = access_control_token_account, has_one = token)]
    pub access_control: Account<'info, AccessControl>,
    #[account(mut, has_one = donator, has_one = access_control)]
    pub donate_tracker: Account<'info, DonateTracker>,
    pub donator: Signer<'info>,
    #[account(mut)]
    pub token: Account<'info, MintAccount>,
    /// CHECK: SAFE
    #[account(mut)]
    pub access_control_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub to: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

pub fn claim(ctx: Context<Claim>) -> Result<()> {
    require!(
        ctx.accounts.access_control.donate_amount >= ctx.accounts.access_control.donate_amount_min,
        ClaimError::DonationAmountLessThanMinimumDonationAmount
    );

    require!(
        ctx.accounts.access_control.pool != None,
        ClaimError::PoolNotInitialized
    );

    require!(
        ctx.accounts.access_control_token_account.owner == ctx.accounts.access_control.key(),
        ClaimError::AccountError
    );

    require!(
        ctx.accounts.access_control_token_account.mint == ctx.accounts.access_control.token,
        ClaimError::AccountError
    );

    require!(
        ctx.accounts
            .access_control_token_account
            .to_account_info()
            .owner
            .clone()
            == token::ID,
        ClaimError::AccountError
    );

    if ctx.accounts.token.mint_authority.is_some() {
        let mint_ix = MintTo {
            mint: ctx.accounts.token.to_account_info(),
            to: ctx.accounts.access_control_token_account.to_account_info(),
            authority: ctx.accounts.access_control.to_account_info(),
        };

        let bump = ctx.accounts.access_control.bump;
        let seeds = &[
            CONSTRAINT_SEED,
            ctx.accounts.access_control.project_wallet.as_ref(),
            &[bump],
        ];
        let signer_seeds = &[&seeds[..]][..];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            mint_ix,
            signer_seeds,
        );

        token::mint_to(
            cpi_ctx,
            ctx.accounts.access_control.minted - ctx.accounts.access_control.token_amount_for_pool,
        )?;

        let cpi_accounts = SetAuthority {
            current_authority: ctx.accounts.access_control.to_account_info(),
            account_or_mint: ctx.accounts.token.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let bump = ctx.accounts.access_control.bump;
        let seeds = &[
            CONSTRAINT_SEED,
            ctx.accounts.access_control.project_wallet.as_ref(),
            &[bump],
        ];
        let signer_seeds = &[&seeds[..]][..];
        let cpi_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

        token::set_authority(cpi_context, AuthorityType::MintTokens, None)?;
    } else {
    }

    let transfer_ix = Transfer {
        from: ctx.accounts.access_control_token_account.to_account_info(),
        to: ctx.accounts.to.to_account_info(),
        authority: ctx.accounts.access_control.to_account_info(),
    };

    let bump = ctx.accounts.access_control.bump;
    let seeds = &[
        CONSTRAINT_SEED,
        ctx.accounts.access_control.project_wallet.as_ref(),
        &[bump],
    ];
    let signer_seeds = &[&seeds[..]][..];

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        transfer_ix,
        signer_seeds,
    );

    token::transfer(cpi_ctx, ctx.accounts.donate_tracker.token_amount)?;

    let donate_tracker = &mut ctx.accounts.donate_tracker;
    donate_tracker.token_amount = 0;

    Ok(())
}

#[derive(Accounts)]
pub struct ClaimForProject<'info> {
    #[account(mut, has_one = project_wallet, has_one = wsol_account, has_one = access_control_token_account, has_one = token)]
    pub access_control: Account<'info, AccessControl>,
    pub project_wallet: Signer<'info>,
    #[account(mut)]
    pub token: Account<'info, MintAccount>,
    /// CHECK: SAFE
    pub wsol: AccountInfo<'info>,
    #[account(mut)]
    pub access_control_token_account: Account<'info, TokenAccount>,
    /// CHECK: SAFE
    #[account(mut)]
    pub wsol_account: Account<'info, TokenAccount>,
    /// CHECK: SAFE
    #[account(has_one = platform_wsol)]
    pub platform: Account<'info, OwnerAccount>,
    /// CHECK: SAFE
    #[account(mut)]
    pub platform_wsol: Account<'info, TokenAccount>,
    /// CHECK: SAFE
    #[account(mut)]
    pub token_to: Account<'info, TokenAccount>,
    #[account(mut)]
    pub wsol_to: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

pub fn claim_for_project(ctx: Context<ClaimForProject>) -> Result<()> {
    require!(
        ctx.accounts.access_control.donate_amount >= ctx.accounts.access_control.donate_amount_min,
        ClaimError::DonationAmountLessThanMinimumDonationAmount
    );

    require!(
        ctx.accounts.access_control.pool != None,
        ClaimError::PoolNotInitialized
    );

    require!(
        ctx.accounts.wsol.to_account_info().key == &spl_token::native_mint::id(),
        ClaimError::WSOLAccountIsNotNativeMint
    );

    require!(
        ctx.accounts.access_control_token_account.owner == ctx.accounts.access_control.key(),
        ClaimError::AccountError
    );

    require!(
        ctx.accounts
            .access_control_token_account
            .to_account_info()
            .owner
            .clone()
            == token::ID,
        ClaimError::AccountError
    );

    require!(
        ctx.accounts.access_control_token_account.mint == ctx.accounts.access_control.token,
        ClaimError::AccountError
    );

    if ctx.accounts.token.mint_authority.is_some() {
        let mint_ix = MintTo {
            mint: ctx.accounts.token.to_account_info(),
            to: ctx.accounts.access_control_token_account.to_account_info(),
            authority: ctx.accounts.access_control.to_account_info(),
        };

        let bump = ctx.accounts.access_control.bump;
        let seeds = &[
            CONSTRAINT_SEED,
            ctx.accounts.access_control.project_wallet.as_ref(),
            &[bump],
        ];
        let signer_seeds = &[&seeds[..]][..];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            mint_ix,
            signer_seeds,
        );

        token::mint_to(
            cpi_ctx,
            ctx.accounts.access_control.minted - ctx.accounts.access_control.token_amount_for_pool,
        )?;

        let cpi_accounts = SetAuthority {
            current_authority: ctx.accounts.access_control.to_account_info(),
            account_or_mint: ctx.accounts.token.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let bump = ctx.accounts.access_control.bump;
        let seeds = &[
            CONSTRAINT_SEED,
            ctx.accounts.project_wallet.key.as_ref(),
            &[bump],
        ];
        let signer_seeds = &[&seeds[..]][..];
        let cpi_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

        token::set_authority(cpi_context, AuthorityType::MintTokens, None)?;
    } else {
    }

    let transfer_ix = Transfer {
        from: ctx.accounts.access_control_token_account.to_account_info(),
        to: ctx.accounts.token_to.to_account_info(),
        authority: ctx.accounts.access_control.to_account_info(),
    };

    let bump = ctx.accounts.access_control.bump;
    let seeds = &[
        CONSTRAINT_SEED,
        ctx.accounts.access_control.project_wallet.as_ref(),
        &[bump],
    ];
    let signer_seeds = &[&seeds[..]][..];

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        transfer_ix,
        signer_seeds,
    );
    token::transfer(
        cpi_ctx,
        ctx.accounts.access_control.token_amount_for_project,
    )?;

    let transfer_ix = Transfer {
        from: ctx.accounts.wsol_account.to_account_info(),
        to: ctx.accounts.wsol_to.to_account_info(),
        authority: ctx.accounts.access_control.to_account_info(),
    };

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        transfer_ix,
        signer_seeds,
    );

    token::transfer(
        cpi_ctx,
        ctx.accounts.access_control.sol_amount_for_project * 95 / 100,
    )?;

    let transfer_ix = Transfer {
        from: ctx.accounts.wsol_account.to_account_info(),
        to: ctx.accounts.platform_wsol.to_account_info(),
        authority: ctx.accounts.access_control.to_account_info(),
    };

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        transfer_ix,
        signer_seeds,
    );

    token::transfer(
        cpi_ctx,
        ctx.accounts.access_control.sol_amount_for_project
            - ctx.accounts.access_control.sol_amount_for_project * 95 / 100,
    )?;

    let access_control = &mut ctx.accounts.access_control;
    access_control.sol_amount_for_project = 0;
    access_control.token_amount_for_project = 0;

    Ok(())
}

#[derive(Accounts)]
pub struct Refund<'info> {
    #[account(has_one = wsol_account)]
    pub access_control: Account<'info, AccessControl>,
    #[account(mut, has_one = donator, has_one = access_control)]
    pub donate_tracker: Account<'info, DonateTracker>,
    pub donator: Signer<'info>,
    /// CHECK: SAFE
    pub wsol: AccountInfo<'info>,
    /// CHECK: SAFE
    #[account(mut)]
    pub wsol_account: Account<'info, TokenAccount>,
    /// CHECK: SAFE
    #[account(mut)]
    pub to: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

pub fn refund(ctx: Context<Refund>) -> Result<()> {
    require!(
        ctx.accounts.wsol.to_account_info().key == &spl_token::native_mint::id(),
        ClaimError::WSOLAccountIsNotNativeMint
    );

    require!(
        !ctx.accounts.access_control.pool.is_some(),
        RefundError::PoolInitialized
    );

    let current_timestamp = Clock::get()?.unix_timestamp as u32;

    require!(
        (ctx.accounts.access_control.donate_amount < ctx.accounts.access_control.donate_amount_min
            && ctx.accounts.access_control.donate_end < current_timestamp)
            || (ctx.accounts.access_control.donate_end + 15 * 24 * 60 * 60 < current_timestamp), // deploy时间戳
        RefundError::InvalidStatus
    );

    let bump = ctx.accounts.access_control.bump;
    let seeds = &[
        CONSTRAINT_SEED,
        ctx.accounts.access_control.project_wallet.as_ref(),
        &[bump],
    ];
    let signer_seeds = &[&seeds[..]][..];

    let transfer_ix = Transfer {
        from: ctx.accounts.wsol_account.to_account_info(),
        to: ctx.accounts.to.to_account_info(),
        authority: ctx.accounts.access_control.to_account_info(),
    };

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        transfer_ix,
        signer_seeds,
    );

    token::transfer(cpi_ctx, ctx.accounts.donate_tracker.donate_amount)?;

    let donate_tracker = &mut ctx.accounts.donate_tracker;
    donate_tracker.donate_amount = 0;

    Ok(())
}

#[error_code]
pub enum ClaimError {
    #[msg("Donation amount is less than minimum donation amount")]
    DonationAmountLessThanMinimumDonationAmount,
    #[msg("Donation end time is not reached")]
    DonationEndTimeNotReached,
    #[msg("Pool is not initialized")]
    PoolNotInitialized,
    #[msg("WSOL account is not native mint")]
    WSOLAccountIsNotNativeMint,
    #[msg("Account incorrect")]
    AccountError,
}

#[error_code]
pub enum RefundError {
    #[msg("Donation amount is more than minimum donation amount")]
    DonationAmountMoreThanMinimumDonationAmount,
    #[msg("Invalid status")]
    InvalidStatus,
    #[msg("Donation end time is not reached")]
    DonationEndTimeNotReached,
    #[msg("WSOL account is not native mint")]
    WSOLAccountIsNotNativeMint,
    #[msg("Pool is initialized")]
    PoolInitialized,
    #[msg("Account incorrect")]
    AccountError,
}
