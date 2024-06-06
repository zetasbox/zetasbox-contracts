use anchor_lang::prelude::*;

pub mod access;

use access::*;

declare_id!("AaRJMWropnNyyaTRdJUjsSvBk9WdBwpMBY1vRmwz7rE");

#[program]
pub mod zetasbox {
    use super::*;

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
        owner::init_project(
            ctx,
            bump,
            donate_start,
            donate_end,
            donate_amount_min,
            donate_amount_max,
            sol_project_ratio,
            sol_pool_ratio,
            init_mint_rate,
            token_project_ratio,
            token_pool_ratio,
            token_donator_ratio,
        )?;
        Ok(())
    }

    pub fn donate(ctx: Context<Donate>, donate_amount: u64) -> Result<()> {
        owner::donate(ctx, donate_amount)?;
        Ok(())
    }

    pub fn init_donate(ctx: Context<InitDonate>) -> Result<()> {
        owner::init_donate(ctx)?;
        Ok(())
    }

    pub fn update_pool(ctx: Context<UpdatePool>, nonce: u8, open_time: u64) -> Result<()> {
        owner::update_pool(ctx, nonce, open_time)?;
        Ok(())
    }

    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        claim::claim(ctx)?;
        Ok(())
    }

    pub fn claim_for_project(ctx: Context<ClaimForProject>) -> Result<()> {
        claim::claim_for_project(ctx)?;
        Ok(())
    }

    pub fn refund(ctx: Context<Refund>) -> Result<()> {
        claim::refund(ctx)?;
        Ok(())
    }

    #[derive(Accounts)]
    pub struct InitPlatform<'info> {
        #[account(init, payer = owner, seeds = [PLATFORM_SEED.as_ref()], space = 8 + 32 + 32, bump)]
        pub platform: Account<'info, OwnerAccount>,
        /// CHECK: test if the authority is needed
        #[account(mut)]
        pub owner: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct ChangePlatform<'info> {
        #[account(mut, has_one = owner, seeds = [PLATFORM_SEED.as_ref()], bump)]
        pub platform: Account<'info, OwnerAccount>,
        /// CHECK: test if the auth is needed
        pub owner: Signer<'info>,
    }

    pub fn init_platform(ctx: Context<InitPlatform>, platform_wsol: Pubkey) -> Result<()> {
        msg!("init");
        ctx.accounts.platform.owner = ctx.accounts.owner.key();
        ctx.accounts.platform.platform_wsol = platform_wsol;
        msg!(
            "The initial authority is {} and the initial data is {}.",
            ctx.accounts.platform.owner,
            ctx.accounts.platform.platform_wsol
        );
        Ok(())
    }

    pub fn change_platfrom(
        ctx: Context<ChangePlatform>,
        owner: Pubkey,
        platform_wsol: Pubkey,
    ) -> Result<()> {
        msg!("change");
        let pda = &mut ctx.accounts.platform;
        pda.owner = owner;
        pda.platform_wsol = platform_wsol;
        msg!("new owner: {} wsol ata: {}", pda.owner, pda.platform_wsol);
        Ok(())
    }
}
