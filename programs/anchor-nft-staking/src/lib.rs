use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use anchor_spl::token;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Approve, Mint, Revoke, Token, TokenAccount},
};
use mpl_token_metadata::{
    instruction::{freeze_delegated_account, thaw_delegated_account},
    ID as MetadataTokenId,
};

declare_id!("AzHXyVDs8XoRXjNE71hLDv9BDJjiCyAxN66UDCZNgk3y");

mod constants {
    pub const POOL_LOCKING_PERIODS: [i64; 2] = [
        120, // 2 min
        300, // 5 min
    ];
}

#[program]
pub mod anchor_nft_staking {
    use super::*;

    pub fn stake(ctx: Context<Stake>, locking_period: i64) -> Result<()> {
        require!(
            ctx.accounts.stake_state.stake_state == StakeState::Unstaked,
            StakeError::AlreadyStaked
        );

        require!(
            constants::POOL_LOCKING_PERIODS.contains(&locking_period),
            StakeError::UnexpectedLockingPeriod
        );

        let clock = Clock::get().unwrap();
        let current_time = clock.unix_timestamp;
        msg!("Approving delegate");

        let cpi_approve_program = ctx.accounts.token_program.to_account_info();
        let cpi_approve_accounts = Approve {
            to: ctx.accounts.nft_token_account.to_account_info(),
            delegate: ctx.accounts.program_authority.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };

        let cpi_approve_ctx = CpiContext::new(cpi_approve_program, cpi_approve_accounts);
        token::approve(cpi_approve_ctx, 1)?;

        msg!("Freezing token account");
        let authority_bump = *ctx.bumps.get("program_authority").unwrap();
        invoke_signed(
            &freeze_delegated_account(
                ctx.accounts.metadata_program.key(),
                ctx.accounts.program_authority.key(),
                ctx.accounts.nft_token_account.key(),
                ctx.accounts.nft_edition.key(),
                ctx.accounts.nft_mint.key(),
            ),
            &[
                ctx.accounts.program_authority.to_account_info(),
                ctx.accounts.nft_token_account.to_account_info(),
                ctx.accounts.nft_edition.to_account_info(),
                ctx.accounts.nft_mint.to_account_info(),
                ctx.accounts.metadata_program.to_account_info(),
            ],
            &[&[b"authority", &[authority_bump]]],
        )?;

        ctx.accounts.stake_state.token_account = ctx.accounts.nft_token_account.key();
        ctx.accounts.stake_state.user_pubkey = ctx.accounts.user.key();
        ctx.accounts.stake_state.stake_state = StakeState::Staked;
        ctx.accounts.stake_state.stake_start_time = current_time;
        ctx.accounts.stake_state.end_time = current_time + locking_period;
        ctx.accounts.stake_state.is_initialized = true;

        Ok(())
    }

    pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
        require!(
            ctx.accounts.stake_state.is_initialized,
            StakeError::UninitializedAccount
        );

        require!(
            ctx.accounts.stake_state.stake_state == StakeState::Staked,
            StakeError::InvalidStakeState
        );

        let clock = Clock::get().unwrap();
        let current_time = clock.unix_timestamp;

        require!(
            ctx.accounts.stake_state.end_time < current_time,
            StakeError::EndTimeNotOver
        );

        msg!("Thawing token account");
        let authority_bump = *ctx.bumps.get("program_authority").unwrap();
        invoke_signed(
            &thaw_delegated_account(
                ctx.accounts.metadata_program.key(),
                ctx.accounts.program_authority.key(),
                ctx.accounts.nft_token_account.key(),
                ctx.accounts.nft_edition.key(),
                ctx.accounts.nft_mint.key(),
            ),
            &[
                ctx.accounts.program_authority.to_account_info(),
                ctx.accounts.nft_token_account.to_account_info(),
                ctx.accounts.nft_edition.to_account_info(),
                ctx.accounts.nft_mint.to_account_info(),
                ctx.accounts.metadata_program.to_account_info(),
            ],
            &[&[b"authority", &[authority_bump]]],
        )?;

        msg!("Revoking delegate");

        let cpi_revoke_program = ctx.accounts.token_program.to_account_info();
        let cpi_revoke_accounts = Revoke {
            source: ctx.accounts.nft_token_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };

        let cpi_revoke_ctx = CpiContext::new(cpi_revoke_program, cpi_revoke_accounts);
        token::revoke(cpi_revoke_ctx)?;

        ctx.accounts.stake_state.stake_state = StakeState::Unstaked;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        associated_token::mint=nft_mint,
        associated_token::authority=user
    )]
    pub nft_token_account: Account<'info, TokenAccount>,
    pub nft_mint: Account<'info, Mint>,
    /// CHECK: Manual validation
    #[account(owner=MetadataTokenId)]
    pub nft_edition: UncheckedAccount<'info>,
    #[account(
        init_if_needed,
        payer=user,
        space = std::mem::size_of::<UserStakeInfo>() + 8,
        seeds = [user.key().as_ref(), nft_token_account.key().as_ref()],
        bump
    )]
    pub stake_state: Account<'info, UserStakeInfo>,
    /// CHECK: Manual validation
    #[account(mut, seeds=["authority".as_bytes().as_ref()], bump)]
    pub program_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub metadata_program: Program<'info, Metadata>,
}

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        token::authority=user
    )]
    pub nft_token_account: Account<'info, TokenAccount>,
    pub nft_mint: Account<'info, Mint>,
    /// CHECK: Manual validation
    #[account(owner=MetadataTokenId)]
    pub nft_edition: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [user.key().as_ref(), nft_token_account.key().as_ref()],
        bump,
        constraint = *user.key == stake_state.user_pubkey,
        constraint = nft_token_account.key() == stake_state.token_account
    )]
    pub stake_state: Account<'info, UserStakeInfo>,
    /// CHECK: manual check
    #[account(mut, seeds=["authority".as_bytes().as_ref()], bump)]
    pub program_authority: UncheckedAccount<'info>,
    /// CHECK: manual check
    #[account(seeds = ["mint".as_bytes().as_ref()], bump)]
    pub stake_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
    pub metadata_program: Program<'info, Metadata>,
}

#[derive(Clone)]
pub struct Metadata;

impl anchor_lang::Id for Metadata {
    fn id() -> Pubkey {
        MetadataTokenId
    }
}

#[account]
pub struct UserStakeInfo {
    pub token_account: Pubkey,
    pub stake_start_time: i64,
    pub end_time: i64,
    pub user_pubkey: Pubkey,
    pub stake_state: StakeState,
    pub is_initialized: bool,
}

#[derive(Debug, PartialEq, AnchorDeserialize, AnchorSerialize, Clone)]
pub enum StakeState {
    Unstaked,
    Staked,
}

impl Default for StakeState {
    fn default() -> Self {
        StakeState::Unstaked
    }
}

#[error_code]
pub enum StakeError {
    #[msg("NFT already staked")]
    AlreadyStaked,

    #[msg("State account is uninitialized")]
    UninitializedAccount,

    #[msg("Stake state is invalid")]
    InvalidStakeState,

    #[msg("Unexpected locking period")]
    UnexpectedLockingPeriod,

    #[msg("End time not over")]
    EndTimeNotOver,
}
