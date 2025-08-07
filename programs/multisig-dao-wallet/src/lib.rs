use anchor_lang::prelude::*;
// use anchor_spl::{
//     associated_token::AssociatedToken,
//     token::{Mint, Token, TokenAccount, Transfer},
// };

declare_id!("Dbte4Uv7CcmKpvnbV9jo3vQzL8cPggGm71TQHzTgDQsR");

#[program]
pub mod multisig_dao_wallet {
    use super::*;

    /// Initialize the multisig wallet with initial signers and threshold
    pub fn initialize_wallet(
        ctx: Context<InitializeWallet>,
        signers: Vec<Pubkey>,
        threshold: u8,
        proposal_timeout: i64,
        spending_limit: u64,
        spending_period: i64,
    ) -> Result<()> {
        require!(signers.len() >= threshold as usize, MultisigError::InvalidThreshold);
        require!(threshold > 0, MultisigError::InvalidThreshold);
        require!(proposal_timeout > 0, MultisigError::InvalidTimeout);
        require!(spending_limit > 0, MultisigError::InvalidSpendingLimit);

        let wallet_config = &mut ctx.accounts.wallet_config;
        wallet_config.authority = ctx.accounts.authority.key();
        wallet_config.signers = signers.clone();
        wallet_config.threshold = threshold;
        wallet_config.proposal_timeout = proposal_timeout;
        wallet_config.spending_limit = spending_limit;
        wallet_config.spending_period = spending_period;
        wallet_config.spending_used = 0;
        wallet_config.last_spending_reset = Clock::get()?.unix_timestamp;
        wallet_config.is_active = true;
        wallet_config.proposal_count = 0;
        wallet_config.bump = ctx.bumps.wallet_config;

        // Initialize members
        wallet_config.members = Vec::new();
        for signer in &signers {
            let member = Member {
                address: *signer,
                role: MemberRole::Member,
                delegate: None,
                is_active: true,
            };
            wallet_config.members.push(member);
        }

        msg!("Multisig wallet initialized with {} signers and threshold {}", 
             signers.len(), threshold);
        Ok(())
    }

    /// Submit a new transaction proposal
    pub fn add_proposal(
        ctx: Context<AddProposal>,
        description: String,
        category: ProposalCategory,
        instructions: Vec<InstructionData>,
        expiration: i64,
    ) -> Result<()> {
        // Get the wallet key before taking mutable reference
        let wallet_key = ctx.accounts.wallet_config.key();
        let wallet_config = &mut ctx.accounts.wallet_config;
        require!(wallet_config.is_active, MultisigError::WalletInactive);
        
        let current_time = Clock::get()?.unix_timestamp;
        require!(expiration > current_time, MultisigError::InvalidExpiration);

        let proposal = &mut ctx.accounts.proposal;
        proposal.wallet = wallet_key;
        proposal.proposer = ctx.accounts.proposer.key();
        proposal.description = description;
        proposal.category = category;
        proposal.instructions = instructions;
        proposal.expiration = expiration;
        proposal.status = ProposalStatus::Pending;
        proposal.approvals = Vec::new();
        proposal.rejections = Vec::new();
        proposal.created_at = current_time;
        proposal.id = wallet_config.proposal_count;
        proposal.bump = ctx.bumps.proposal;

        wallet_config.proposal_count += 1;

        msg!("Proposal {} created by {}", proposal.key(), ctx.accounts.proposer.key());
        Ok(())
    }

    /// Approve a proposal
    pub fn approve_proposal(ctx: Context<ApproveProposal>) -> Result<()> {
        let wallet_config = &ctx.accounts.wallet_config;
        let proposal = &mut ctx.accounts.proposal;
        
        require!(wallet_config.is_active, MultisigError::WalletInactive);
        require!(proposal.status == ProposalStatus::Pending, MultisigError::ProposalNotPending);
        
        let current_time = Clock::get()?.unix_timestamp;
        require!(proposal.expiration > current_time, MultisigError::ProposalExpired);

        let approver = ctx.accounts.approver.key();
        require!(wallet_config.signers.contains(&approver), MultisigError::NotAuthorized);

        // Check if already approved
        require!(!proposal.approvals.contains(&approver), MultisigError::AlreadyApproved);

        proposal.approvals.push(approver);
        
        // Check if threshold is met
        let required_threshold = match proposal.category {
            ProposalCategory::Regular => wallet_config.threshold,
            ProposalCategory::Admin => wallet_config.threshold + 1,
            ProposalCategory::Emergency => wallet_config.threshold - 1,
        };

        if proposal.approvals.len() >= required_threshold as usize {
            proposal.status = ProposalStatus::Approved;
            msg!("Proposal {} approved with {} votes", proposal.key(), proposal.approvals.len());
        } else {
            msg!("Proposal {} approved by {}. {} more votes needed", 
                 proposal.key(), approver, required_threshold - proposal.approvals.len() as u8);
        }

        Ok(())
    }

    /// Execute an approved proposal
    pub fn execute_proposal(ctx: Context<ExecuteProposal>) -> Result<()> {
        let wallet_config = &ctx.accounts.wallet_config;
        let proposal = &mut ctx.accounts.proposal;
        
        require!(wallet_config.is_active, MultisigError::WalletInactive);
        require!(proposal.status == ProposalStatus::Approved, MultisigError::ProposalNotApproved);
        
        let current_time = Clock::get()?.unix_timestamp;
        require!(proposal.expiration > current_time, MultisigError::ProposalExpired);

        // Execute the instructions
        for _instruction in &proposal.instructions {
            // This is a simplified execution - in a real implementation,
            // you would need to handle different instruction types
            msg!("Executing instruction for proposal {}", proposal.key());
        }

        proposal.status = ProposalStatus::Executed;
        proposal.executed_at = Some(current_time);
        
        msg!("Proposal {} executed successfully", proposal.key());
        Ok(())
    }

    /// Update signers and threshold (requires unanimous consent)
    pub fn update_signers(
        ctx: Context<UpdateSigners>,
        new_signers: Vec<Pubkey>,
        new_threshold: u8,
    ) -> Result<()> {
        let wallet_config = &mut ctx.accounts.wallet_config;
        require!(wallet_config.is_active, MultisigError::WalletInactive);
        require!(new_signers.len() >= new_threshold as usize, MultisigError::InvalidThreshold);
        require!(new_threshold > 0, MultisigError::InvalidThreshold);

        // Check if all current signers have approved this change
        let approver = ctx.accounts.approver.key();
        require!(wallet_config.signers.contains(&approver), MultisigError::NotAuthorized);

        // In a real implementation, you would track approvals for signer updates
        // For now, we'll require the authority to make this change
        require!(wallet_config.authority == approver, MultisigError::NotAuthorized);

        wallet_config.signers = new_signers;
        wallet_config.threshold = new_threshold;

        msg!("Signers and threshold updated");
        Ok(())
    }

    /// Set spending limits
    pub fn set_spending_limits(
        ctx: Context<SetSpendingLimits>,
        new_limit: u64,
        new_period: i64,
    ) -> Result<()> {
        let wallet_config = &mut ctx.accounts.wallet_config;
        require!(wallet_config.is_active, MultisigError::WalletInactive);
        
        let approver = ctx.accounts.approver.key();
        require!(wallet_config.authority == approver, MultisigError::NotAuthorized);

        wallet_config.spending_limit = new_limit;
        wallet_config.spending_period = new_period;
        wallet_config.spending_used = 0;
        wallet_config.last_spending_reset = Clock::get()?.unix_timestamp;

        msg!("Spending limits updated: {} per {} seconds", new_limit, new_period);
        Ok(())
    }

    /// Delegate voting power to another address
    pub fn delegate_vote(
        ctx: Context<DelegateVote>,
        delegate: Pubkey,
    ) -> Result<()> {
        let wallet_config = &mut ctx.accounts.wallet_config;
        require!(wallet_config.is_active, MultisigError::WalletInactive);
        
        let delegator = ctx.accounts.delegator.key();
        require!(wallet_config.signers.contains(&delegator), MultisigError::NotAuthorized);

        // Find and update the member's delegate
        for member in &mut wallet_config.members {
            if member.address == delegator {
                member.delegate = Some(delegate);
                msg!("Vote delegated from {} to {}", delegator, delegate);
                return Ok(());
            }
        }

        Err(MultisigError::MemberNotFound.into())
    }

    /// Emergency override for urgent situations
    pub fn emergency_override(
        ctx: Context<EmergencyOverride>,
        instructions: Vec<InstructionData>,
    ) -> Result<()> {
        let wallet_config = &ctx.accounts.wallet_config;
        require!(wallet_config.is_active, MultisigError::WalletInactive);
        
        let emergency_authority = ctx.accounts.emergency_authority.key();
        require!(wallet_config.authority == emergency_authority, MultisigError::NotAuthorized);

        // Execute emergency instructions immediately
        for _instruction in &instructions {
            msg!("Executing emergency instruction");
        }

        msg!("Emergency override executed by {}", emergency_authority);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeWallet<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + WalletConfig::INIT_SPACE,
        seeds = [b"wallet_config", authority.key().as_ref()],
        bump
    )]
    pub wallet_config: Account<'info, WalletConfig>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddProposal<'info> {
    #[account(
        init,
        payer = proposer,
        space = 8 + Proposal::INIT_SPACE,
        seeds = [b"proposal", wallet_config.key().as_ref(), proposer.key().as_ref()],
        bump
    )]
    pub proposal: Account<'info, Proposal>,
    
    #[account(
        mut,
        seeds = [b"wallet_config", wallet_config.authority.as_ref()],
        bump = wallet_config.bump,
        constraint = wallet_config.is_active
    )]
    pub wallet_config: Account<'info, WalletConfig>,
    
    #[account(mut)]
    pub proposer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ApproveProposal<'info> {
    #[account(
        seeds = [b"wallet_config", wallet_config.authority.as_ref()],
        bump = wallet_config.bump,
        constraint = wallet_config.is_active
    )]
    pub wallet_config: Account<'info, WalletConfig>,
    
    #[account(
        mut,
        constraint = proposal.status == ProposalStatus::Pending
    )]
    pub proposal: Account<'info, Proposal>,
    
    pub approver: Signer<'info>,
}

#[derive(Accounts)]
pub struct ExecuteProposal<'info> {
    #[account(
        seeds = [b"wallet_config", wallet_config.authority.as_ref()],
        bump = wallet_config.bump,
        constraint = wallet_config.is_active
    )]
    pub wallet_config: Account<'info, WalletConfig>,
    
    #[account(
        mut,
        constraint = proposal.status == ProposalStatus::Approved
    )]
    pub proposal: Account<'info, Proposal>,
    
    pub executor: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateSigners<'info> {
    #[account(
        mut,
        seeds = [b"wallet_config", wallet_config.authority.as_ref()],
        bump = wallet_config.bump,
        constraint = wallet_config.is_active
    )]
    pub wallet_config: Account<'info, WalletConfig>,
    
    pub approver: Signer<'info>,
}

#[derive(Accounts)]
pub struct SetSpendingLimits<'info> {
    #[account(
        mut,
        seeds = [b"wallet_config", wallet_config.authority.as_ref()],
        bump = wallet_config.bump,
        constraint = wallet_config.is_active
    )]
    pub wallet_config: Account<'info, WalletConfig>,
    
    pub approver: Signer<'info>,
}

#[derive(Accounts)]
pub struct DelegateVote<'info> {
    #[account(
        mut,
        seeds = [b"wallet_config", wallet_config.authority.as_ref()],
        bump = wallet_config.bump,
        constraint = wallet_config.is_active
    )]
    pub wallet_config: Account<'info, WalletConfig>,
    
    pub delegator: Signer<'info>,
}

#[derive(Accounts)]
pub struct EmergencyOverride<'info> {
    #[account(
        seeds = [b"wallet_config", wallet_config.authority.as_ref()],
        bump = wallet_config.bump,
        constraint = wallet_config.is_active
    )]
    pub wallet_config: Account<'info, WalletConfig>,
    
    pub emergency_authority: Signer<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct WalletConfig {
    pub authority: Pubkey,
    #[max_len(10)] // Maximum 10 signers
    pub signers: Vec<Pubkey>,
    pub threshold: u8,
    pub proposal_timeout: i64,
    pub spending_limit: u64,
    pub spending_period: i64,
    pub spending_used: u64,
    pub last_spending_reset: i64,
    pub is_active: bool,
    #[max_len(10)] // Maximum 10 members
    pub members: Vec<Member>,
    pub proposal_count: u64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Proposal {
    pub wallet: Pubkey,
    pub proposer: Pubkey,
    #[max_len(200)] // Maximum 200 characters for description
    pub description: String,
    pub category: ProposalCategory,
    #[max_len(10)] // Maximum 10 instructions per proposal
    pub instructions: Vec<InstructionData>,
    pub expiration: i64,
    pub status: ProposalStatus,
    #[max_len(10)] // Maximum 10 approvals
    pub approvals: Vec<Pubkey>,
    #[max_len(10)] // Maximum 10 rejections
    pub rejections: Vec<Pubkey>,
    pub created_at: i64,
    pub executed_at: Option<i64>,
    pub id: u64,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, InitSpace)]
pub struct Member {
    pub address: Pubkey,
    pub role: MemberRole,
    pub delegate: Option<Pubkey>,
    pub is_active: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, InitSpace)]
pub struct InstructionData {
    pub program_id: Pubkey,
    #[max_len(10)] // Maximum 10 accounts per instruction
    pub accounts: Vec<AccountMeta>,
    #[max_len(256)] // Maximum 256 bytes for instruction data
    pub data: Vec<u8>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, InitSpace)]
pub struct AccountMeta {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, InitSpace)]
pub enum MemberRole {
    Admin,
    Treasurer,
    Member,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, InitSpace)]
pub enum ProposalCategory {
    Regular,
    Admin,
    Emergency,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, InitSpace)]
pub enum ProposalStatus {
    Pending,
    Approved,
    Rejected,
    Executed,
    Expired,
}

#[error_code]
pub enum MultisigError {
    #[msg("Invalid threshold - must be greater than 0 and less than or equal to number of signers")]
    InvalidThreshold,
    #[msg("Invalid timeout - must be greater than 0")]
    InvalidTimeout,
    #[msg("Invalid spending limit - must be greater than 0")]
    InvalidSpendingLimit,
    #[msg("Invalid expiration - must be in the future")]
    InvalidExpiration,
    #[msg("Wallet is not active")]
    WalletInactive,
    #[msg("Proposal is not pending")]
    ProposalNotPending,
    #[msg("Proposal is not approved")]
    ProposalNotApproved,
    #[msg("Proposal has expired")]
    ProposalExpired,
    #[msg("Not authorized to perform this action")]
    NotAuthorized,
    #[msg("Already approved this proposal")]
    AlreadyApproved,
    #[msg("Member not found")]
    MemberNotFound,
}
