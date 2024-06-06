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
    associated_token::AssociatedToken,
    token::{
        self,
        spl_token::{
            instruction::AuthorityType,
            state::{Account as TokenAccountStruct, Mint},
        },
        InitializeAccount, MintTo, SetAuthority, Token, TokenAccount, Transfer,
    },
};

use borsh::{BorshDeserialize, BorshSerialize};

use std::str::FromStr;

use crate::access::types::{
    AccessControl, DonateTracker, OwnerAccount, CONSTRAINT_SEED, DONATE_SEED,
};

#[derive(Accounts)]
pub struct InitProject<'info> {
    // 8 + 1 + 32 + 32 + 8 + 4 + 4 + 8 + 8 + 1 + 1 + (1+32) + 8 + 1 + 1 + 1 + 8 * 4 + 8 + 32 + 32 + 8
    #[account(init, payer = owner, seeds = [CONSTRAINT_SEED.as_ref(), owner.key().as_ref()], space = 263, bump)]
    pub access_control: Account<'info, AccessControl>,
    #[account(mut)]
    pub owner: Signer<'info>,
    /// CHECK: SAFE
    pub token_mint: AccountInfo<'info>,
    /// CHECK: SAFE
    pub wsol_account: Account<'info, TokenAccount>,
    /// CHECK: SAFE
    pub token_account: Account<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
}

pub fn init_project(
    ctx: Context<InitProject>,
    bump: u8,
    donate_start: u32,
    donate_end: u32,
    donate_amount_min: u64,
    donate_amount_max: u64,
    sol_project_ratio: u8,
    sol_pool_ratio: u8,
    init_mint_rate: u64,
    token_project_ratio: u8,
    token_pool_ratio: u8,
    token_donator_ratio: u8,
) -> Result<()> {
    ctx.accounts.access_control.bump = bump;
    let mint = Mint::unpack(&ctx.accounts.token_mint.data.borrow())?;
    require!(
        mint.mint_authority == Some(ctx.accounts.access_control.key()).into(),
        InitProjectError::MintAuthorityError
    );

    require!(
        ctx.accounts.wsol_account.mint
            == Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
        InitProjectError::MintError
    );

    require!(
        ctx.accounts.wsol_account.owner == ctx.accounts.access_control.key(),
        InitProjectError::TokenOwnerError
    );

    require!(
        ctx.accounts.wsol_account.delegate.is_none(),
        InitProjectError::TokenOwnerError
    );

    require!(
        ctx.accounts.wsol_account.to_account_info().owner.clone() == token::ID,
        InitProjectError::AccountProgramError
    );

    require!(
        ctx.accounts.token_account.mint == ctx.accounts.token_mint.key(),
        InitProjectError::MintError
    );

    require!(
        ctx.accounts.token_account.owner == ctx.accounts.access_control.key(),
        InitProjectError::TokenOwnerError
    );

    require!(
        ctx.accounts.token_account.delegate.is_none(),
        InitProjectError::TokenOwnerError
    );

    require!(
        ctx.accounts.wsol_account.to_account_info().owner.clone() == token::ID,
        InitProjectError::AccountProgramError
    );

    require!(
        sol_pool_ratio + sol_project_ratio == 100
            && token_pool_ratio + token_project_ratio + token_donator_ratio == 100,
        InitProjectError::RatioError
    );

    ctx.accounts.access_control.token = ctx.accounts.token_mint.key();
    ctx.accounts.access_control.project_wallet = ctx.accounts.owner.key();
    ctx.accounts.access_control.donate_amount = 0;
    ctx.accounts.access_control.donate_start = donate_start;
    ctx.accounts.access_control.donate_end = donate_end;
    ctx.accounts.access_control.donate_amount_min = donate_amount_min;
    ctx.accounts.access_control.donate_amount_max = donate_amount_max;
    ctx.accounts.access_control.sol_project_ratio = sol_project_ratio;
    ctx.accounts.access_control.sol_pool_ratio = sol_pool_ratio;
    ctx.accounts.access_control.pool = None;
    ctx.accounts.access_control.init_mint_rate = init_mint_rate;
    ctx.accounts.access_control.token_project_ratio = token_project_ratio;
    ctx.accounts.access_control.token_pool_ratio = token_pool_ratio;
    ctx.accounts.access_control.token_donator_ratio = token_donator_ratio;
    ctx.accounts.access_control.sol_amount_for_project = 0;
    ctx.accounts.access_control.sol_amount_for_pool = 0;
    ctx.accounts.access_control.token_amount_for_project = 0;
    ctx.accounts.access_control.token_amount_for_pool = 0;
    ctx.accounts.access_control.minted = 0;
    ctx.accounts.access_control.wsol_account = ctx.accounts.wsol_account.key();
    ctx.accounts.access_control.access_control_token_account = ctx.accounts.token_account.key();
    ctx.accounts.access_control.donator_amount = 0;

    msg!(
        "The initial owner is {:?}",
        ctx.accounts.access_control.project_wallet,
    );

    Ok(())
}

#[derive(Accounts)]
pub struct UpdatePool<'info> {
    #[account(mut, has_one = project_wallet, has_one = wsol_account, has_one = access_control_token_account)]
    pub access_control: Box<Account<'info, AccessControl>>,
    #[account(mut)]
    pub project_wallet: Signer<'info>,

    #[account(mut)]
    pub wsol_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub access_control_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: Safe. Raydium liquidity pool v4 account
    pub raydium_liquidity_pool_v4: AccountInfo<'info>,

    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    #[account(address = associated_token::ID)]
    pub associated_token_program: Program<'info, associated_token::AssociatedToken>,
    /// CHECK: Safe. Rent program
    pub rent: Sysvar<'info, Rent>,
    /// CHECK: Safe. Amm Account
    #[account(mut)]
    pub amm: AccountInfo<'info>,
    /// CHECK: Safe. Amm authority, a PDA create with seed = [b"amm authority"]
    pub amm_authority: AccountInfo<'info>,
    /// CHECK: Safe. Amm open orders Account
    #[account(mut)]
    pub amm_open_orders: AccountInfo<'info>,
    /// CHECK: Safe. Lp mint account
    #[account(mut)]
    pub lp_mint: AccountInfo<'info>,
    /// CHECK: Safe. Coin mint account, need drop authority
    #[account(mut)]
    pub coin_mint: AccountInfo<'info>,
    /// CHECK: Safe. Pc mint account
    pub pc_mint: AccountInfo<'info>,
    /// CHECK: Safe. Pool_token_coin Account. Must be non zero, owned by $authority
    #[account(mut)]
    pub pool_coin_token_account: AccountInfo<'info>,
    /// CHECK: Safe. Pool_token_pc Account. Must be non zero, owned by $authority.
    #[account(mut)]
    pub pool_pc_token_account: AccountInfo<'info>,
    /// CHECK: Safe. Withdraw queue Account. To save withdraw dest_coin & dest_pc account with must cancle orders.
    #[account(mut)]
    pub pool_withdraw_queue: AccountInfo<'info>,
    /// CHECK: Safe. Pool target orders account
    pub pool_target_orders: AccountInfo<'info>,
    /// CHECK: Safe. Token_temp_lp Account. To save withdraw lp with must cancle orders as temp to transfer later.
    #[account(mut)]
    pub pool_temp_lp: AccountInfo<'info>,
    /// CHECK: Safe. Serum dex program.
    pub serum_program: AccountInfo<'info>,
    /// CHECK: Safe. Serum market Account. serum_dex program is the owner.
    pub serum_market: AccountInfo<'info>,
    /// CHECK: Safe. The user wallet create the pool
    // #[account(mut)]
    // pub user_wallet: Signer<'info>,
    #[account(mut)]
    pub user_token_coin: Box<Account<'info, TokenAccount>>,
    /// CHECK: Safe. User pc token account to deposit into.
    #[account(mut)]
    pub user_token_pc: Box<Account<'info, TokenAccount>>,
    /// CHECK: Safe. User lp token account, to deposit the generated tokens, user is the owner
    #[account(mut)]
    pub user_lp_token_account: AccountInfo<'info>,

    #[account(mut)]
    /// CHECK: Safe. Platform associated token account
    pub platform_lp_associated_token: AccountInfo<'info>,

    #[account(constraint = platform.owner == platform_owner.key(), has_one = platform_wsol)]
    pub platform: Box<Account<'info, OwnerAccount>>,

    #[account()]
    /// CHECK: Safe.
    pub platform_owner: AccountInfo<'info>,

    #[account(mut)]
    /// CHECK: Safe.
    pub platform_wsol: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct Initialize2<'info> {
    /// CHECK: Safe. Raydium liquidity pool v4 account
    pub raydium_liquidity_pool_v4: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
    pub spl_associated_token_account: Program<'info, associated_token::AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
    /// CHECK: Safe. Amm Account
    pub amm: AccountInfo<'info>,
    /// CHECK: Safe. Amm authority, a PDA create with seed = [b"amm authority"]
    pub amm_authority: AccountInfo<'info>,
    /// CHECK: Safe. Amm open orders Account
    pub amm_open_orders: AccountInfo<'info>,
    /// CHECK: Safe. Lp mint account
    pub lp_mint: AccountInfo<'info>,
    /// CHECK: Safe. Coin mint account
    pub coin_mint: AccountInfo<'info>,
    /// CHECK: Safe. Pc mint account
    pub pc_mint: AccountInfo<'info>,
    /// CHECK: Safe. Pool_token_coin Account. Must be non zero, owned by $authority
    pub pool_coin_token_account: AccountInfo<'info>,
    /// CHECK: Safe. Pool_token_pc Account. Must be non zero, owned by $authority.
    pub pool_pc_token_account: AccountInfo<'info>,
    /// CHECK: Safe. Withdraw queue Account. To save withdraw dest_coin & dest_pc account with must cancle orders.
    pub pool_withdraw_queue: AccountInfo<'info>,
    /// CHECK: Safe. Pool target orders account
    pub pool_target_orders: AccountInfo<'info>,
    /// CHECK: Safe. Token_temp_lp Account. To save withdraw lp with must cancle orders as temp to transfer later.
    pub pool_temp_lp: AccountInfo<'info>,
    /// CHECK: Safe. Serum dex program.
    pub serum_program: AccountInfo<'info>,
    /// CHECK: Safe. Serum market Account. serum_dex program is the owner.
    pub serum_market: AccountInfo<'info>,
    /// CHECK: Safe. The user wallet create the pool
    pub user_wallet: AccountInfo<'info>,
    /// CHECK: Safe. User coin token account to deposit into.
    pub user_token_coin: AccountInfo<'info>,
    /// CHECK: Safe. User pc token account to deposit into.
    pub user_token_pc: AccountInfo<'info>,
    /// CHECK: Safe. User lp token account, to deposit the generated tokens, user is the owner
    pub user_lp_token_account: AccountInfo<'info>,
}

pub fn update_pool(ctx: Context<UpdatePool>, nonce: u8, open_time: u64) -> Result<()> {
    require!(
        ctx.accounts.raydium_liquidity_pool_v4.key()
            == Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap()
            || ctx.accounts.raydium_liquidity_pool_v4.key()
                == Pubkey::from_str("HWy1jotHpo6UqeQxx49dpYYdQB8wj9Qk9MdxwjLvDHB8").unwrap(),
        PoolError::InvalidRaydiumLiquidityPoolV4Account
    );

    let clock = Clock::get()?;

    let current_time = clock.unix_timestamp as u32;

    require!(
        current_time < ctx.accounts.access_control.donate_end
            || current_time - ctx.accounts.access_control.donate_end <= 15 * 24 * 60 * 60,
        // || current_time - ctx.accounts.access_control.donate_end <= 60 * 60,
        PoolError::PoolTimeout
    );

    require!(
        ctx.accounts.access_control.donate_amount >= ctx.accounts.access_control.donate_amount_min,
        PoolError::DonationNotSatisfied,
    );

    require!(
        ctx.accounts.access_control.token == ctx.accounts.coin_mint.key(),
        PoolError::AccountError
    );

    require!(
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap()
            == ctx.accounts.pc_mint.key(),
        PoolError::AccountError
    );

    require!(
        ctx.accounts.user_token_coin.mint == ctx.accounts.coin_mint.key(),
        PoolError::AccountError
    );

    require!(
        ctx.accounts.user_token_coin.owner == ctx.accounts.project_wallet.key(),
        PoolError::AccountError
    );

    require!(
        ctx.accounts.user_token_coin.to_account_info().owner.clone() == token::ID,
        PoolError::AccountError
    );

    require!(
        ctx.accounts.user_token_pc.mint
            == Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
        PoolError::AccountError
    );

    require!(
        ctx.accounts.user_token_pc.owner == ctx.accounts.project_wallet.key(),
        PoolError::AccountError
    );

    require!(
        ctx.accounts.user_token_pc.to_account_info().owner.clone() == token::ID,
        PoolError::AccountError
    );

    require!(
        ctx.accounts.access_control_token_account.owner == ctx.accounts.access_control.key(),
        PoolError::AccountError
    );

    require!(
        ctx.accounts.access_control_token_account.mint == ctx.accounts.access_control.token,
        PoolError::AccountError
    );

    require!(
        ctx.accounts
            .access_control_token_account
            .to_account_info()
            .owner
            .clone()
            == token::ID,
        PoolError::AccountError
    );

    {
        let mint_ix = MintTo {
            mint: ctx.accounts.coin_mint.to_account_info(),
            to: ctx.accounts.user_token_coin.to_account_info(),
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

        token::mint_to(cpi_ctx, ctx.accounts.access_control.token_amount_for_pool)?;
    }

    {
        let transfer_wsol_ix = Transfer {
            from: ctx.accounts.wsol_account.to_account_info(),
            to: ctx.accounts.user_token_pc.to_account_info(),
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
            transfer_wsol_ix,
            signer_seeds,
        );

        token::transfer(
            cpi_ctx,
            ctx.accounts.access_control.sol_amount_for_pool * 95 / 100,
        )?;
    }

    {
        let transfer_wsol_ix = Transfer {
            from: ctx.accounts.wsol_account.to_account_info(),
            to: ctx.accounts.platform_wsol.to_account_info(),
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
            transfer_wsol_ix,
            signer_seeds,
        );

        token::transfer(
            cpi_ctx,
            ctx.accounts.access_control.sol_amount_for_pool
                - ctx.accounts.access_control.sol_amount_for_pool * 95 / 100,
        )?;
    }

    {
        let cpi_program = ctx.accounts.raydium_liquidity_pool_v4.to_account_info();
        let cpi_accounts = Initialize2 {
            raydium_liquidity_pool_v4: ctx.accounts.raydium_liquidity_pool_v4.to_account_info(),
            token_program: ctx.accounts.token_program.clone(),
            system_program: ctx.accounts.system_program.clone(),
            spl_associated_token_account: ctx.accounts.associated_token_program.clone(),
            rent: ctx.accounts.rent.clone(),
            amm: ctx.accounts.amm.to_account_info(),
            amm_authority: ctx.accounts.amm_authority.to_account_info(),
            amm_open_orders: ctx.accounts.amm_open_orders.to_account_info(),
            lp_mint: ctx.accounts.lp_mint.to_account_info(),
            coin_mint: ctx.accounts.coin_mint.to_account_info(),
            pc_mint: ctx.accounts.pc_mint.to_account_info(),
            pool_coin_token_account: ctx.accounts.pool_coin_token_account.to_account_info(),
            pool_pc_token_account: ctx.accounts.pool_pc_token_account.to_account_info(),
            pool_withdraw_queue: ctx.accounts.pool_withdraw_queue.to_account_info(),
            pool_target_orders: ctx.accounts.pool_target_orders.to_account_info(),
            pool_temp_lp: ctx.accounts.pool_temp_lp.to_account_info(),
            serum_program: ctx.accounts.serum_program.to_account_info(),
            serum_market: ctx.accounts.serum_market.to_account_info(),
            user_wallet: ctx.accounts.project_wallet.to_account_info(),
            user_token_coin: ctx.accounts.user_token_coin.to_account_info(),
            user_token_pc: ctx.accounts.user_token_pc.to_account_info(),
            user_lp_token_account: ctx.accounts.user_lp_token_account.to_account_info(),
        };

        let bump = ctx.accounts.access_control.bump;
        let seeds = &[
            CONSTRAINT_SEED,
            ctx.accounts.project_wallet.key.as_ref(),
            &[bump],
        ];
        let signer_seeds = &[&seeds[..]][..];

        let cpi_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

        initialize2(
            cpi_context,
            nonce,
            open_time,
            ctx.accounts.access_control.sol_amount_for_pool * 95 / 100,
            ctx.accounts.access_control.token_amount_for_pool,
        )?;
    }

    {
        let create_cpi = CpiContext::new(
            ctx.accounts.associated_token_program.to_account_info(),
            anchor_spl::associated_token::Create {
                payer: ctx.accounts.project_wallet.to_account_info(),
                associated_token: ctx.accounts.platform_lp_associated_token.to_account_info(),
                authority: ctx.accounts.platform_owner.to_account_info(),
                mint: ctx.accounts.lp_mint.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            },
        );

        anchor_spl::associated_token::create(create_cpi)?;

        // finish

        let amount =
            TokenAccountStruct::unpack(&ctx.accounts.user_lp_token_account.data.borrow())?.amount;

        let transfer_token_ix = Transfer {
            from: ctx.accounts.user_lp_token_account.to_account_info(),
            to: ctx.accounts.platform_lp_associated_token.to_account_info(),
            authority: ctx.accounts.project_wallet.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            transfer_token_ix,
        );

        token::transfer(cpi_ctx, amount)?;
    }

    ctx.accounts.access_control.pool = Some(ctx.accounts.amm.to_account_info().key());

    Ok(())
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq, AnchorSerialize, AnchorDeserialize)]
pub enum AmmInstruction {
    #[deprecated(note = "Not supported yet, please use `Initialize2` instead")]
    Initialize(InitializeInstruction),

    ///   Initializes a new AMM pool.
    ///
    ///   0. `[]` Spl Token program id
    ///   1. `[]` Associated Token program id
    ///   2. `[]` Sys program id
    ///   3. `[]` Rent program id
    ///   4. `[writable]` New AMM Account to create.
    ///   5. `[]` $authority derived from `create_program_address(&[AUTHORITY_AMM, &[nonce]])`.
    ///   6. `[writable]` AMM open orders Account
    ///   7. `[writable]` AMM lp mint Account
    ///   8. `[]` AMM coin mint Account
    ///   9. `[]` AMM pc mint Account
    ///   10. `[writable]` AMM coin vault Account. Must be non zero, owned by $authority.
    ///   11. `[writable]` AMM pc vault Account. Must be non zero, owned by $authority.
    ///   12. `[writable]` AMM target orders Account. To store plan orders informations.
    ///   13. `[]` AMM config Account, derived from `find_program_address(&[&&AMM_CONFIG_SEED])`.
    ///   14. `[]` AMM create pool fee destination Account
    ///   15. `[]` Market program id
    ///   16. `[writable]` Market Account. Market program is the owner.
    ///   17. `[writable, singer]` User wallet Account
    ///   18. `[]` User token coin Account
    ///   19. '[]` User token pc Account
    ///   20. `[writable]` User destination lp token ATA Account
    Initialize2(InitializeInstruction2),
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, AnchorSerialize, AnchorDeserialize)]
pub struct InitializeInstruction {
    /// nonce used to create valid program address
    pub nonce: u8,
    /// utc timestamps for pool open
    pub open_time: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, AnchorSerialize, AnchorDeserialize)]
pub struct InitializeInstruction2 {
    /// nonce used to create valid program address
    pub nonce: u8,
    /// utc timestamps for pool open
    pub open_time: u64,
    /// init token pc amount
    pub init_pc_amount: u64,
    /// init token coin amount
    pub init_coin_amount: u64,
}

pub fn initialize2_instruction(
    pool_program: &Pubkey,
    token_program: &Pubkey,
    system_program: &Pubkey,
    spl_associated_token_account: &Pubkey,
    rent: &Pubkey,
    amm: &Pubkey,
    amm_authority: &Pubkey,
    amm_open_orders: &Pubkey,
    lp_mint: &Pubkey,
    coin_mint: &Pubkey,
    pc_mint: &Pubkey,
    pool_coin_token_account: &Pubkey,
    pool_pc_token_account: &Pubkey,
    pool_withdraw_queue: &Pubkey,
    pool_target_orders: &Pubkey,
    pool_temp_lp: &Pubkey,
    serum_program: &Pubkey,
    serum_market: &Pubkey,
    user_wallet: &Pubkey,
    user_token_coin: &Pubkey,
    user_token_pc: &Pubkey,
    user_lp_token_account: &Pubkey,

    nonce: u8,
    open_time: u64,
    init_pc_amount: u64,
    init_coin_amount: u64,
) -> Result<Instruction> {
    let data = AmmInstruction::Initialize2(InitializeInstruction2 {
        nonce,
        open_time,
        init_pc_amount,
        init_coin_amount,
    })
    .try_to_vec()?;

    let mut accounts = Vec::with_capacity(21);
    accounts.push(AccountMeta::new_readonly(*token_program, false));
    accounts.push(AccountMeta::new_readonly(
        *spl_associated_token_account,
        false,
    ));
    accounts.push(AccountMeta::new_readonly(*system_program, false));
    accounts.push(AccountMeta::new_readonly(*rent, false));
    accounts.push(AccountMeta::new(*amm, false));
    accounts.push(AccountMeta::new_readonly(*amm_authority, false));
    accounts.push(AccountMeta::new(*amm_open_orders, false));
    accounts.push(AccountMeta::new(*lp_mint, false));
    accounts.push(AccountMeta::new_readonly(*coin_mint, false));
    accounts.push(AccountMeta::new_readonly(*pc_mint, false));
    accounts.push(AccountMeta::new(*pool_coin_token_account, false));
    accounts.push(AccountMeta::new(*pool_pc_token_account, false));
    accounts.push(AccountMeta::new(*pool_withdraw_queue, false));
    accounts.push(AccountMeta::new_readonly(*pool_target_orders, false));
    accounts.push(AccountMeta::new(*pool_temp_lp, false));
    accounts.push(AccountMeta::new_readonly(*serum_program, false));
    accounts.push(AccountMeta::new_readonly(*serum_market, false)); // z
    accounts.push(AccountMeta::new(*user_wallet, true));
    accounts.push(AccountMeta::new(*user_token_coin, false));
    accounts.push(AccountMeta::new(*user_token_pc, false));
    accounts.push(AccountMeta::new(*user_lp_token_account, false));

    Ok(Instruction {
        program_id: *pool_program,
        accounts,
        data,
    })
}

pub fn initialize2_signed<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, Initialize2<'info>>,
    nonce: u8,
    open_time: u64,
    init_pc_amount: u64,
    init_coin_amount: u64,
) -> Result<()> {
    let ix = initialize2_instruction(
        ctx.accounts.raydium_liquidity_pool_v4.key,
        ctx.accounts.token_program.to_account_info().key,
        ctx.accounts.system_program.to_account_info().key,
        ctx.accounts
            .spl_associated_token_account
            .to_account_info()
            .key,
        ctx.accounts.rent.to_account_info().key,
        ctx.accounts.amm.to_account_info().key,
        ctx.accounts.amm_authority.to_account_info().key,
        ctx.accounts.amm_open_orders.to_account_info().key,
        ctx.accounts.lp_mint.to_account_info().key,
        ctx.accounts.coin_mint.to_account_info().key,
        ctx.accounts.pc_mint.to_account_info().key,
        ctx.accounts.pool_coin_token_account.to_account_info().key,
        ctx.accounts.pool_pc_token_account.to_account_info().key,
        ctx.accounts.pool_withdraw_queue.to_account_info().key,
        ctx.accounts.pool_target_orders.to_account_info().key,
        ctx.accounts.pool_temp_lp.to_account_info().key,
        ctx.accounts.serum_program.to_account_info().key,
        ctx.accounts.serum_market.to_account_info().key,
        ctx.accounts.user_wallet.to_account_info().key,
        ctx.accounts.user_token_coin.to_account_info().key,
        ctx.accounts.user_token_pc.to_account_info().key,
        ctx.accounts.user_lp_token_account.to_account_info().key,
        nonce,
        open_time,
        init_pc_amount,
        init_coin_amount,
    )?;

    solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.raydium_liquidity_pool_v4.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.spl_associated_token_account.to_account_info(),
            ctx.accounts.rent.to_account_info(),
            ctx.accounts.amm.to_account_info(),
            ctx.accounts.amm_authority.to_account_info(),
            ctx.accounts.amm_open_orders.to_account_info(),
            ctx.accounts.lp_mint.to_account_info(),
            ctx.accounts.coin_mint.to_account_info(),
            ctx.accounts.pc_mint.to_account_info(),
            ctx.accounts.pool_coin_token_account.to_account_info(),
            ctx.accounts.pool_pc_token_account.to_account_info(),
            ctx.accounts.pool_withdraw_queue.to_account_info(),
            ctx.accounts.pool_target_orders.to_account_info(),
            ctx.accounts.pool_temp_lp.to_account_info(),
            ctx.accounts.serum_program.to_account_info(),
            ctx.accounts.serum_market.to_account_info(),
            ctx.accounts.user_wallet.to_account_info(),
            ctx.accounts.user_token_coin.to_account_info(),
            ctx.accounts.user_token_pc.to_account_info(),
            ctx.accounts.user_lp_token_account.to_account_info(),
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn initialize2<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, Initialize2<'info>>,
    nonce: u8,
    open_time: u64,
    init_pc_amount: u64,
    init_coin_amount: u64,
) -> Result<()> {
    let ix = initialize2_instruction(
        ctx.accounts.raydium_liquidity_pool_v4.key,
        ctx.accounts.token_program.to_account_info().key,
        ctx.accounts.system_program.to_account_info().key,
        ctx.accounts
            .spl_associated_token_account
            .to_account_info()
            .key,
        ctx.accounts.rent.to_account_info().key,
        ctx.accounts.amm.to_account_info().key,
        ctx.accounts.amm_authority.to_account_info().key,
        ctx.accounts.amm_open_orders.to_account_info().key,
        ctx.accounts.lp_mint.to_account_info().key,
        ctx.accounts.coin_mint.to_account_info().key,
        ctx.accounts.pc_mint.to_account_info().key,
        ctx.accounts.pool_coin_token_account.to_account_info().key,
        ctx.accounts.pool_pc_token_account.to_account_info().key,
        ctx.accounts.pool_withdraw_queue.to_account_info().key,
        ctx.accounts.pool_target_orders.to_account_info().key,
        ctx.accounts.pool_temp_lp.to_account_info().key,
        ctx.accounts.serum_program.to_account_info().key,
        ctx.accounts.serum_market.to_account_info().key,
        ctx.accounts.user_wallet.to_account_info().key,
        ctx.accounts.user_token_coin.to_account_info().key,
        ctx.accounts.user_token_pc.to_account_info().key,
        ctx.accounts.user_lp_token_account.to_account_info().key,
        nonce,
        open_time,
        init_pc_amount,
        init_coin_amount,
    )?;

    solana_program::program::invoke(
        &ix,
        &[
            ctx.accounts.raydium_liquidity_pool_v4.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.spl_associated_token_account.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.rent.to_account_info(),
            ctx.accounts.amm.to_account_info(),
            ctx.accounts.amm_authority.to_account_info(),
            ctx.accounts.amm_open_orders.to_account_info(),
            ctx.accounts.lp_mint.to_account_info(),
            ctx.accounts.coin_mint.to_account_info(),
            ctx.accounts.pc_mint.to_account_info(),
            ctx.accounts.pool_coin_token_account.to_account_info(),
            ctx.accounts.pool_pc_token_account.to_account_info(),
            ctx.accounts.pool_withdraw_queue.to_account_info(),
            ctx.accounts.pool_target_orders.to_account_info(),
            ctx.accounts.pool_temp_lp.to_account_info(),
            ctx.accounts.serum_program.to_account_info(),
            ctx.accounts.serum_market.to_account_info(),
            ctx.accounts.user_wallet.to_account_info(),
            ctx.accounts.user_token_coin.to_account_info(),
            ctx.accounts.user_token_pc.to_account_info(),
            ctx.accounts.user_lp_token_account.to_account_info(),
        ],
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub from: Account<'info, TokenAccount>,
    #[account(mut)]
    pub to: Account<'info, TokenAccount>,
    #[account(has_one = project_wallet)]
    pub access_control: Account<'info, AccessControl>,
    pub project_wallet: Signer<'info>,
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
}

impl<'info> Withdraw<'info> {}

pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_accounts = Transfer {
        from: ctx.accounts.from.to_account_info(),
        to: ctx.accounts.to.to_account_info(),
        authority: ctx.accounts.access_control.to_account_info(),
    };

    let bump = ctx.accounts.access_control.bump;
    let seeds = &[CONSTRAINT_SEED, &[bump]];
    let signer_seeds = &[&seeds[..]][..];

    let transfer_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

    token::transfer(transfer_context, amount)?;

    Ok(())
}

#[derive(Accounts)]
pub struct InitDonate<'info> {
    #[account(init, payer = donator, seeds = [DONATE_SEED.as_ref(), access_control.key().as_ref(), donator.key().as_ref()], space = 8 + 1 + 32 + 8 + 8 + 32, bump)]
    pub donate_tracker: Account<'info, DonateTracker>,
    #[account(mut)]
    pub access_control: Account<'info, AccessControl>,
    #[account(mut)]
    pub donator: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn init_donate(ctx: Context<InitDonate>) -> Result<()> {
    let access_control = &mut ctx.accounts.access_control;

    let donate_tracker = &mut ctx.accounts.donate_tracker;

    let (_, bump) = Pubkey::find_program_address(
        &[
            DONATE_SEED,
            access_control.key().as_ref(),
            ctx.accounts.donator.key().as_ref(),
        ],
        ctx.program_id,
    );
    donate_tracker.bump = bump;
    donate_tracker.token_amount = 0;
    donate_tracker.donate_amount = 0;
    donate_tracker.access_control = access_control.key();
    donate_tracker.donator = ctx.accounts.donator.key();

    access_control.donator_amount += 1;
    Ok(())
}

#[derive(Accounts)]
pub struct Donate<'info> {
    #[account(mut, seeds = [DONATE_SEED.as_ref(), access_control.key().as_ref(), donator.key().as_ref()], bump)]
    pub donate_tracker: Account<'info, DonateTracker>,
    #[account(mut)]
    pub access_control: Account<'info, AccessControl>,
    #[account(mut)]
    pub from: Account<'info, TokenAccount>,
    #[account(mut)]
    pub to: Account<'info, TokenAccount>,
    #[account(mut)]
    pub donator: Signer<'info>,
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn donate(ctx: Context<Donate>, donate_amount: u64) -> Result<()> {
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_accounts = Transfer {
        from: ctx.accounts.from.to_account_info(),
        to: ctx.accounts.to.to_account_info(),
        authority: ctx.accounts.donator.to_account_info(),
    };

    let transfer_context = CpiContext::new(cpi_program, cpi_accounts);

    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;

    token::transfer(transfer_context, donate_amount)?;

    let access_control = &mut ctx.accounts.access_control;

    require!(
        access_control.donate_start as i64 <= current_time,
        DonateError::DonationNotOpen
    );
    require!(
        access_control.donate_end as i64 >= current_time,
        DonateError::DonationClosed
    );

    require!(
        !access_control.pool.is_some(),
        DonateError::PoolAlreadyInitialized
    );

    let donate_tracker = &mut ctx.accounts.donate_tracker;
    let mint_token_amount =
        access_control.init_mint_rate as u128 * donate_amount as u128 / (1e9 as u128);

    require!(
        ctx.accounts.donator.key() == donate_tracker.donator,
        DonateError::DonateAccountError
    );
    require!(
        access_control.key() == donate_tracker.access_control,
        DonateError::AccessControlAccountError
    );
    donate_tracker.token_amount +=
        (mint_token_amount as u128 * access_control.token_donator_ratio as u128 / 100) as u64;
    donate_tracker.donate_amount += donate_amount;

    access_control.donate_amount += donate_amount;
    access_control.sol_amount_for_pool +=
        (donate_amount as u128 * access_control.sol_pool_ratio as u128 / 100) as u64;
    access_control.sol_amount_for_project +=
        (donate_amount as u128 * access_control.sol_project_ratio as u128 / 100) as u64;
    access_control.token_amount_for_pool +=
        (mint_token_amount as u128 * access_control.token_pool_ratio as u128 / 100) as u64;
    access_control.token_amount_for_project += (mint_token_amount
        - mint_token_amount as u128 * access_control.token_pool_ratio as u128 / 100
        - mint_token_amount as u128 * access_control.token_donator_ratio as u128 / 100)
        as u64;
    access_control.minted += mint_token_amount as u64;

    require!(
        access_control.donate_amount <= access_control.donate_amount_max,
        DonateError::DonationAmountMax
    );

    Ok(())
}

#[error_code]
pub enum DonateError {
    #[msg("Donation is closed")]
    DonationClosed,
    #[msg("Donation is not open yet")]
    DonationNotOpen,
    #[msg("Pool is already initialized")]
    PoolAlreadyInitialized,
    #[msg("Donation amount max")]
    DonationAmountMax,
    #[msg("DonatorTrack account is error")]
    DonateAccountError,
    #[msg("AccessControl account is error")]
    AccessControlAccountError,
}

#[error_code]
pub enum InitProjectError {
    #[msg("Mint authority must set to the program hosted account")]
    MintAuthorityError,
    #[msg("Token mint incorrect")]
    MintError,
    #[msg("Token owenr must be Access Control")]
    TokenOwnerError,
    #[msg("Token program not correct")]
    AccountProgramError,
    #[msg("Ratio Incorrect")]
    RatioError,
}

#[error_code]
pub enum PoolError {
    #[msg("Invalid Raydium liquidity pool v4 account")]
    InvalidRaydiumLiquidityPoolV4Account,
    #[msg("Create Pool timeout")]
    PoolTimeout,
    #[msg("Donation not satisfied")]
    DonationNotSatisfied,
    #[msg("Account incorrect")]
    AccountError,
}
