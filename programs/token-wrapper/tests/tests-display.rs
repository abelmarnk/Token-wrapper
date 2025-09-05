//#![cfg(feature = "test-sbf")]

mod tests {
use std::collections::HashMap;
use anyhow::Result as AnyResult;
use anchor_lang::{
    solana_program::{
        system_instruction::SystemError,
        program_error::ProgramError
    },
    InstructionData, prelude::{AccountMeta, Pubkey}, solana_program::instruction::Instruction, system_program
};
use anchor_spl::{associated_token::get_associated_token_address, token::spl_token::error::TokenError};
use spl_token::state::{Account as TokenAccount, GenericTokenAccount, Mint};
use mollusk_svm_programs_token::{
    token::{create_account_for_mint, create_account_for_token_account}
};
use solana_sdk::{
    account::Account, program_option::COption::None as CNone, 
    program_pack::Pack, rent::Rent, signature::Keypair, signer::Signer
};
use mollusk_svm::{
    Mollusk, account_store::AccountStore, program::keyed_account_for_system_program, result::{Check}
};
use token_wrapper::instruction::{
    CreateMint,
    SwapToWrapped,
    SwapToSource
};

#[test]
 fn passing_test_1() {
    
    let mollusk = TokenWrapperTest::get_mollusk_with_programs();

    let mut program_test = TokenWrapperTest::new();

    let mut account_store = TokenWrapperAccountStore::default();

    // Passing test
    program_test.setup_default(&mut account_store);


    let mollusk_context = mollusk.with_context(account_store);

    mollusk_context.process_and_validate_instruction_chain(
        &[
            (&program_test.get_create_mint_instruction(), &[Check::success()]),
            (&program_test.get_swap_instruction(SwapType::SwapToWrapped), &[Check::success()]),
            (&program_test.get_swap_instruction(SwapType::SwapToSource), &[Check::success()])
        ]
    );


    // Failing test - 1
    let mut account_store = mollusk_context.account_store.borrow_mut();

    account_store.accounts_map.clear();

    program_test.setup_fail_1(&mut account_store);

    
    core::mem::drop(account_store);

    mollusk_context.process_and_validate_instruction_chain(
        &[
            (&program_test.get_create_mint_instruction(), &[Check::success()]),
            (&program_test.get_swap_instruction(SwapType::SwapToWrapped), &[Check::err(TokenError::InsufficientFunds.into())]),
            // (&program_test.get_swap_instruction(SwapType::SwapToSource), &[Check::success()])
        ]
    );


    // Failing test - 2

    let mut account_store = mollusk_context.account_store.borrow_mut();

    account_store.accounts_map.clear();

    program_test.setup_fail_2(&mut account_store);
    
    core::mem::drop(account_store);

    mollusk_context.process_and_validate_instruction_chain(
        &[
            (&program_test.get_create_mint_instruction(), &[Check::success()]),
            (&program_test.get_swap_instruction(SwapType::SwapToWrapped), &[Check::success()]),
            (&program_test.get_swap_instruction(SwapType::SwapToSource), &[Check::err(TokenError::InsufficientFunds.into())])
        ]
    );

    // Failing test 3

    let mut account_store = mollusk_context.account_store.borrow_mut();

    account_store.accounts_map.clear();

    program_test.setup_default(&mut account_store);

    core::mem::drop(account_store);

    mollusk_context.process_and_validate_instruction_chain(
        &[
            (&program_test.get_create_mint_instruction(), &[Check::success()]),
            (&program_test.get_create_mint_instruction(), &[Check::err(ProgramError::Custom(SystemError::AccountAlreadyInUse as u32))]),
        ]
    );

}

pub struct TokenWrapperAccountStore{
    pub accounts_map:HashMap<Pubkey, Account>
}

impl TokenWrapperAccountStore{
    pub fn default()->Self{
        TokenWrapperAccountStore{ 
            accounts_map: HashMap::<Pubkey, Account>::default() 
        }
    }

    pub fn get_account_or_default(&self, account_key: &Pubkey)->Account{
        self.get_account(account_key).
            unwrap_or(self.default_account(&Pubkey::default()))
    }
}

impl AccountStore for TokenWrapperAccountStore{

    fn get_account(&self, account_key: &Pubkey) -> Option<Account>{
        self.accounts_map.get(account_key).cloned()
    }

    #[inline(always)]
    fn store_account(&mut self, account_key: Pubkey, account: Account){
        self.accounts_map.insert(account_key, account);
    }
}

pub enum SwapType{
    SwapToWrapped,
    SwapToSource
}

pub struct TokenWrapperTest {

    // Signers
    pub payer: Keypair,        

    // Mint accounts
    pub source_mint: Keypair,  
    pub wrapped_mint: Keypair, 

    // PDAs
    pub mint_authority: Pubkey,
    pub vault_authority: Pubkey,        
    pub source_mint_exists: Pubkey,     

    // Token accounts
    pub vault: Pubkey,                  
    pub buyer_mint_ata: Pubkey,         
    pub buyer_wrapped_mint_ata: Pubkey, 

    // Data 
    pub wrap_amount:u64,
    pub source_amount:u64,
}

impl TokenWrapperTest {

     pub fn new() -> Self {
        let payer = Keypair::new();

        let source_mint = Keypair::new();
        let wrapped_mint = Keypair::new();

        let (mint_authority, _) =
            Pubkey::find_program_address(
                &[b"mint-authority", wrapped_mint.pubkey().as_ref()],
                &token_wrapper::ID,
            );
        let (vault_authority, _) =
            Pubkey::find_program_address(
                &[b"vault-authority", source_mint.pubkey().as_ref()],
                &token_wrapper::ID,
            );
        let (source_mint_exists, _) =
            Pubkey::find_program_address(
                &[b"mint", source_mint.pubkey().as_ref()],
                &token_wrapper::ID,
            );

        let vault = get_associated_token_address(&vault_authority, &source_mint.pubkey());
        let buyer_mint_ata = get_associated_token_address(&payer.pubkey(), &source_mint.pubkey());
        let buyer_wrapped_mint_ata =
            get_associated_token_address(&payer.pubkey(), &wrapped_mint.pubkey());

        Self {
            payer,

            source_mint,
            wrapped_mint,

            mint_authority,
            vault_authority,
            source_mint_exists,

            vault,
            buyer_mint_ata,
            buyer_wrapped_mint_ata,

            wrap_amount:0,
            source_amount:0
        }
    }

    #[inline(always)]
    pub fn _generate_keys(&mut self){
        *self = TokenWrapperTest::new();
    }

    pub fn get_mollusk_with_programs() -> Mollusk{
        let mut runtime = Mollusk::new(&token_wrapper::ID, "token_wrapper");

        mollusk_svm_programs_token::token::add_program(&mut runtime);
        
        mollusk_svm_programs_token::associated_token::add_program(&mut runtime);

        runtime
    }
        
    pub fn setup(&mut self, accounts: &mut TokenWrapperAccountStore, buyer_lamports: u64,
                mint_supply: u64, token_account_balance: u64, wrap_amount:u64, source_amount:u64){
        // Add the payer account
        let rent = Rent::default();

        let buyer_minimum_balance = rent.minimum_balance(0);
        let buyer_total_lamports = buyer_lamports.max(buyer_minimum_balance);
        
        accounts.store_account(
            self.payer.pubkey(), 
            Account::new(buyer_total_lamports, 0, &system_program::ID)
        );

        // Add the source mint account
        let mint_data = Mint {
            mint_authority: CNone,
            supply: mint_supply,
            decimals: 9,
            is_initialized: true,
            freeze_authority: CNone
        };

        accounts.store_account(
            self.source_mint.pubkey(), 
            create_account_for_mint(mint_data)
        );

        // Add the user token account
        let token_account_data = TokenAccount{
            mint: self.source_mint.pubkey(),
            owner: self.payer.pubkey(),
            amount: token_account_balance,
            delegate: CNone,
            state: spl_token::state::AccountState::Initialized,
            is_native: CNone,
            delegated_amount: 0,
            close_authority: CNone
        };

        accounts.store_account(
            self.buyer_mint_ata, 
            create_account_for_token_account(token_account_data)
        );

        // Add the programs
        let token_key_account_pair = mollusk_svm_programs_token::token::keyed_account();
        let associated_token_key_account_pair = mollusk_svm_programs_token::associated_token::keyed_account();
        let system_program_key_account_pair = keyed_account_for_system_program();
        
        accounts.store_account(
            token_key_account_pair.0,
            token_key_account_pair.1
        );

        accounts.store_account(
            associated_token_key_account_pair.0, 
            associated_token_key_account_pair.1
        );

        accounts.store_account(
            system_program_key_account_pair.0, 
            system_program_key_account_pair.1
        );

        // Add the swap amounts
        self.wrap_amount = wrap_amount;
        self.source_amount = source_amount;



    }

    #[inline(always)]
    pub fn setup_default(&mut self, validator: &mut TokenWrapperAccountStore){
        // This is the default test and is expected to pass
        self.setup(
            validator,
            1_000_000_000_000_000_000,     
            1_000_000_000_000_000,  
            1_000_000_000_000,
            100_000,
            10_000
        );
    }

    #[inline(always)]
    pub fn setup_fail_1(&mut self, validator: &mut TokenWrapperAccountStore){
        // This is a failing test and it should fail because the amount the user wants to deposit
        // is larger than their balance
        self.setup(
            validator,
            1_000_000_000_000_000_000,     
            1_000_000_000_000_000,  
            1_000_000,
            100_000_000,
            10_000
        );
    }

    #[inline(always)]
    pub fn setup_fail_2(&mut self, validator: &mut TokenWrapperAccountStore){
        // This is a failing test and it should fail because the amount the user wants to swap back
        // to the source is larger than their balance for the wrapped
        self.setup(
            validator,
            1_000_000_000_000_000_000,     
            1_000_000_000_000_000,  
            1_000_000,
            100_000,
            100_001
        );
    }

    pub fn get_create_mint_instruction(&self)->Instruction{
        let mut create_mint_accounts = Vec::<AccountMeta>::with_capacity(11);
        create_mint_accounts.push(AccountMeta::new(self.payer.pubkey(), true));
        create_mint_accounts.push(AccountMeta::new_readonly(self.mint_authority, false));
        create_mint_accounts.push(AccountMeta::new_readonly(self.source_mint.pubkey(), false));
        create_mint_accounts.push(AccountMeta::new(self.wrapped_mint.pubkey(), true));
        create_mint_accounts.push(AccountMeta::new_readonly(self.vault_authority, false));
        create_mint_accounts.push(AccountMeta::new(self.vault, false));
        create_mint_accounts.push(AccountMeta::new(self.source_mint_exists, false));
        create_mint_accounts.push(AccountMeta::new_readonly(system_program::ID, false));
        create_mint_accounts.push(AccountMeta::new_readonly(spl_token::ID, false));
        create_mint_accounts.push(AccountMeta::new_readonly(mollusk_svm_programs_token::associated_token::ID, false));

        Instruction {
            program_id: token_wrapper::ID,
            accounts: create_mint_accounts.clone(),
            data: CreateMint{}.data()
        }
    }

    pub fn get_swap_instruction(&self, swap:SwapType)->Instruction{
        let mut swap_accounts = Vec::<AccountMeta>::with_capacity(13);

        swap_accounts.push(AccountMeta::new(self.payer.pubkey(), true));
        swap_accounts.push(AccountMeta::new(self.buyer_mint_ata, false));
        swap_accounts.push(AccountMeta::new(self.buyer_wrapped_mint_ata, false));
        swap_accounts.push(AccountMeta::new_readonly(self.vault_authority, false));
        swap_accounts.push(AccountMeta::new(self.vault, false));
        swap_accounts.push(AccountMeta::new_readonly(self.mint_authority, false));
        swap_accounts.push(AccountMeta::new_readonly(self.source_mint.pubkey(), false));
        swap_accounts.push(AccountMeta::new(self.wrapped_mint.pubkey(), false));
        swap_accounts.push(AccountMeta::new_readonly(self.source_mint_exists, false));
        swap_accounts.push(AccountMeta::new_readonly(system_program::ID, false));
        swap_accounts.push(AccountMeta::new_readonly(spl_token::ID, false));
        swap_accounts.push(AccountMeta::new_readonly(mollusk_svm_programs_token::associated_token::ID, false));

        let data = match swap {
            SwapType::SwapToSource=>{
                SwapToSource{amount: self.source_amount}.data()
            },
            SwapType::SwapToWrapped=>{
                SwapToWrapped{amount: self.wrap_amount}.data()
            }
        };

        Instruction {
            program_id: token_wrapper::ID,
            accounts: swap_accounts.clone(),
            data
        }
    }

    
}

pub  fn log_token_accounts(account_store: &TokenWrapperAccountStore, program_test: &TokenWrapperTest)->AnyResult<()>{
    
    // Get and display the user source token account.

    let account_result = account_store.
        get_account_or_default(&program_test.buyer_mint_ata);
    
    if TokenAccount::valid_account_data(&account_result.data.as_slice()){

        let user_account:TokenAccount = TokenAccount::unpack(
                &mut account_result.data.as_slice())?;
        println!("The user source mint token account was successfully fetched!!! \n\n");
        println!("Token account:- \n\n{:?}", user_account);
    }
    else{
        println!("The user source mint account does not exist yet and has a balance of 0.\n\n");
    }

    // Get and display the user wrapped token account.

    let account_result = account_store.
        get_account_or_default(&program_test.buyer_wrapped_mint_ata);
    
    if TokenAccount::valid_account_data(&account_result.data.as_slice()){

        let user_account:TokenAccount = TokenAccount::unpack(
                &mut account_result.data.as_slice())?;
        println!("The user wrapped mint token account was successfully fetched!!! \n\n");
        println!("Token account:- \n\n{:?}", user_account);
    }
    else{
        println!("The user wrapped mint account does not exist yet and has a balance of 0.\n\n");
    }

    // Get and display the vault source mint account.

    let account_result = account_store.
        get_account_or_default(&program_test.vault);
    
    if TokenAccount::valid_account_data(&account_result.data.as_slice()){

        let user_account:TokenAccount = TokenAccount::unpack(
                &mut account_result.data.as_slice())?;
        println!("The vault source mint token account was successfully fetched!!! \n\n");
        println!("Token account:- \n\n{:?}", user_account);
    }
    else{
        println!("The vault source mint account does not exist yet and has a balance of 0.\n\n");
    }

    Ok(())
}

}