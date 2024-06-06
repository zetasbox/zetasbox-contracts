use anchor_lang::prelude::*;

pub const CONSTRAINT_SEED: &[u8] = b"project";
pub const DONATE_SEED: &[u8] = b"donate";
pub const PLATFORM_SEED: &[u8] = b"platform";

#[account]
#[derive(Default)]
pub struct AccessControl {
    pub bump: u8,
    pub token: Pubkey,
    pub project_wallet: Pubkey,
    pub donate_amount: u64,
    pub donate_start: u32,
    pub donate_end: u32,
    pub donate_amount_min: u64,
    pub donate_amount_max: u64,
    pub sol_project_ratio: u8,
    pub sol_pool_ratio: u8,
    pub pool: Option<Pubkey>,
    pub init_mint_rate: u64,
    pub token_project_ratio: u8,
    pub token_pool_ratio: u8,
    pub token_donator_ratio: u8,

    pub sol_amount_for_project: u64,
    pub sol_amount_for_pool: u64,

    pub token_amount_for_project: u64,
    pub token_amount_for_pool: u64,

    pub minted: u64,

    pub wsol_account: Pubkey,
    pub access_control_token_account: Pubkey,

    pub donator_amount: u64,
}

#[account]
#[derive(Default)]
pub struct DonateTracker {
    pub bump: u8,
    pub access_control: Pubkey,
    pub donate_amount: u64,
    pub token_amount: u64,
    pub donator: Pubkey,
}

#[account]
#[derive(Default)]
pub struct OwnerAccount {
    pub platform_wsol: Pubkey,
    pub owner: Pubkey,
}
