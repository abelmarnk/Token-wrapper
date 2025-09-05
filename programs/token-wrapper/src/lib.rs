use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken, token::{
        burn, mint_to, transfer_checked, Burn, Mint, MintTo, Token, TokenAccount, TransferChecked
    }
};
declare_id!("3huwuym1VECMMBjmbGxdy91s9C2QUrpPqn93DQV6dnJz");

#[program]
pub mod token_wrapper {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn create_mint(ctx: Context<CreateMint>) -> Result<()> {
        
        ctx.accounts.source_mint_exists.set_inner(
            SourceMint {
                wrapped_mint: ctx.accounts.wrapped_mint.key(),
                bump:ctx.bumps.source_mint_exists
            }
        );
        Ok(())
    }

    #[instruction(discriminator = 1)]
    pub fn swap_to_wrapped(ctx: Context<Swap>, amount:u64) -> Result<()> {

        // Initiate the transfer of tokens from the user to the vault
        let transfer_accounts = TransferChecked{
            from: ctx.accounts.buyer_mint_ata.to_account_info(),
            to:ctx.accounts.vault.to_account_info(),
            mint:ctx.accounts.source_mint.to_account_info(),
            authority:ctx.accounts.buyer.to_account_info()
        };

        let transfer_context = 
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(), 
                transfer_accounts);
        
        transfer_checked(transfer_context, amount, ctx.accounts.source_mint.decimals)?;

        let wrapped_mint_key_bytes = ctx.accounts.wrapped_mint.key().to_bytes();

        let seeds = [b"mint-authority", wrapped_mint_key_bytes.as_ref(), &[ctx.bumps.mint_authority]];

        let signer = &[&seeds[..]];

        // Initiate mint
        let mint_to_accounts = MintTo{
            mint:ctx.accounts.wrapped_mint.to_account_info(),
            to:ctx.accounts.buyer_wrapped_mint_ata.to_account_info(),
            authority:ctx.accounts.mint_authority.to_account_info()
        };

        let mint_to_context = 
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                mint_to_accounts,
                signer
            );

        mint_to(mint_to_context, amount)
    }

    #[instruction(discriminator = 2)]
    pub fn swap_to_source(ctx: Context<Swap>, amount:u64) -> Result<()> {
        
        // Initiate the transfer of tokens from the vault to the user

        let source_mint_key_bytes = ctx.accounts.source_mint.key().to_bytes();

        let seeds = [b"vault-authority", source_mint_key_bytes.as_ref(), &[ctx.bumps.vault_authority]];

        let signer = &[&seeds[..]];

        let transfer_accounts = TransferChecked{
            from: ctx.accounts.vault.to_account_info(),
            to:ctx.accounts.buyer_mint_ata.to_account_info(),
            mint:ctx.accounts.source_mint.to_account_info(),
            authority:ctx.accounts.vault_authority.to_account_info()
        };

        let transfer_context = 
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(), 
                transfer_accounts,
                signer
            );
        
        transfer_checked(transfer_context, amount, ctx.accounts.source_mint.decimals)?;

        // Initiate burn
        let burn_accounts = Burn{
            mint:ctx.accounts.wrapped_mint.to_account_info(),
            from:ctx.accounts.buyer_wrapped_mint_ata.to_account_info(),
            authority:ctx.accounts.buyer.to_account_info()
        };

        let burn_context = 
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                burn_accounts,
            );

        burn(burn_context, amount)
    }
}

#[derive(Accounts)]
pub struct CreateMint<'info>{
    #[account(
        mut
    )]
    payer:Signer<'info>,

    #[account(
        seeds = [b"mint-authority", wrapped_mint.key().as_ref()],
        bump
    )]
    /// CHECK: just signs
    mint_authority:UncheckedAccount<'info>,

    source_mint:Account<'info, Mint>,

    #[account(
        init,
        payer = payer,
        mint::decimals = source_mint.decimals,
        mint::authority = mint_authority,
    )]
    wrapped_mint:Account<'info, Mint>,

    #[account(
        seeds = [b"vault-authority", source_mint.key().as_ref()],
        bump
    )]
    /// CHECK: just signs
    vault_authority:UncheckedAccount<'info>,

    #[account(
        init,
        payer = payer,
        associated_token::mint = source_mint,
        associated_token::authority = vault_authority,
    )]
    vault:Account<'info, TokenAccount>,

    #[account(
        init,
        payer = payer,
        space = SourceMint::DISCRIMINATOR.len() + SourceMint::INIT_SPACE,
        seeds = [b"mint", source_mint.key().as_ref()],
        bump
    )]
    source_mint_exists:Account<'info, SourceMint>,
    
    system_program:Program<'info, System>,
    token_program:Program<'info, Token>,
    associated_token_program:Program<'info, AssociatedToken>
}

#[derive(Accounts)]
pub struct Swap<'info>{

    #[account(
        mut
    )]
    buyer:Signer<'info>,

    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = source_mint,
        associated_token::authority = buyer,
        associated_token::token_program = token_program,
    )]
    buyer_mint_ata:Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = wrapped_mint,
        associated_token::authority = buyer,
        associated_token::token_program = token_program,
    )]
    buyer_wrapped_mint_ata:Account<'info, TokenAccount>,

    #[account(
        seeds = [b"vault-authority", source_mint.key().as_ref()],
        bump
    )]
    /// CHECK: just signs
    vault_authority:UncheckedAccount<'info>,

    #[account(
        mut,
        associated_token::mint = source_mint,
        associated_token::authority = vault_authority,
        associated_token::token_program = token_program,
    )]
    vault:Account<'info, TokenAccount>,

    #[account(
        seeds = [b"mint-authority", wrapped_mint.key().as_ref()],
        bump
    )]
    /// CHECK: just signs
    mint_authority:UncheckedAccount<'info>,

    source_mint:Account<'info, Mint>,

    #[account(
        mut,
        mint::authority = mint_authority,
    )]
    wrapped_mint:Account<'info, Mint>,

    #[account(
        seeds = [b"mint", source_mint.key().as_ref()],
        bump = source_mint_account.bump,
        has_one = wrapped_mint
    )]
    source_mint_account:Account<'info, SourceMint>,


    system_program:Program<'info, System>,
    token_program:Program<'info, Token>,
    associated_token_program:Program<'info, AssociatedToken>,
}


#[derive(InitSpace)]
#[account(discriminator = 1)]
pub struct SourceMint{
    pub wrapped_mint:Pubkey,
    pub bump:u8
}