#![cfg(feature = "test-sbf")]

mod tests {
use std::collections::HashMap;
use anyhow::Result as AnyResult;
use anchor_lang::{
    prelude::{AccountMeta, Pubkey}, solana_program::instruction::Instruction, 
    system_program, InstructionData
};
use anchor_spl::{associated_token::get_associated_token_address};
use spl_token::state::{Account as TokenAccount, Mint};
use mollusk_svm_programs_token::{
    token::{create_account_for_mint, create_account_for_token_account}
};
use solana_sdk::{
    account::Account, program_option::COption::{None as CNone}, 
    program_pack::Pack, rent::Rent, signature::Keypair, signer::Signer
};
use mollusk_svm::{
    account_store::AccountStore, program::keyed_account_for_system_program, result::ContextResult, Mollusk, MolluskContext
};

#[test]
 fn passing_test_1() {
    
    let mut runtime = Mollusk::new(&token_wrapper::ID, "token_wrapper");

    mollusk_svm_programs_token::token::add_program(&mut runtime);
    let token_key_account_pair = mollusk_svm_programs_token::token::keyed_account();

    mollusk_svm_programs_token::associated_token::add_program(&mut runtime);
    let associated_token_key_account_pair = mollusk_svm_programs_token::associated_token::keyed_account();

    let system_program_key_account_pair = keyed_account_for_system_program();
    
    let mut account_store = TokenWrapperAccountStore::default();

    account_store.store_account(
        token_key_account_pair.0,
         token_key_account_pair.1
    );

    account_store.store_account(
        associated_token_key_account_pair.0, 
        associated_token_key_account_pair.1
    );

    account_store.store_account(
        system_program_key_account_pair.0, 
        system_program_key_account_pair.1
    );
    let program_test = TokenWrapperTest::new();

    program_test.setup_default(&mut account_store);

    let mut program_test_context = runtime.with_context(account_store); 

    let result = program_test.create_mint(&mut program_test_context);

    if result.program_result.is_err(){
        println!("Error:- {:?}", result.raw_result.expect_err("No error found?").to_string());
        panic!();
    }

    let mint = program_test_context.account_store.
        borrow().get_account(&program_test.wrapped_mint.pubkey()).unwrap();


    println!("Mint data:- \n\n{:?}", mint);

    println!("Token balances before swap to wrapped:-\n\n");

    log_token_accounts(&program_test_context, &program_test).unwrap();

    let result = program_test.swap_to_wrapped_default(&mut program_test_context);

    if result.program_result.is_err(){
        println!("Error:- {:?}", result.raw_result.expect_err("No error found").to_string());
        panic!();
    }

    println!("The swap to wrapped was successful!!!\n\n");

    println!("Token balances after swap to wrapped:-\n\n");

    log_token_accounts(&program_test_context, &program_test).unwrap();

    println!("Swapping from wrapped......\n\n");

    println!("Token balances before swap from wrapped:-\n\n");

    log_token_accounts(&program_test_context, &program_test).unwrap();

    let result = program_test.swap_to_source_default(&mut program_test_context);
    
    if result.program_result.is_err(){
        println!("Error:- {:?}", result.raw_result.expect_err("No error found").to_string());
        panic!();
    }

    println!("The swap from wrapped was successful!!!\n\n");

    println!("Token balances after swap from wrapped:-\n\n");

    log_token_accounts(&program_test_context, &program_test).unwrap();

    println!("Successful tests were successful.\n\n");
 
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
}

impl AccountStore for TokenWrapperAccountStore{

    fn get_account(&self, account_key: &Pubkey) -> Option<Account>{
        self.accounts_map.get(account_key).cloned()
    }

    fn store_account(&mut self, account_key: Pubkey, account: Account){
        self.accounts_map.insert(account_key, account);
    }

    fn default_account(&self, _pubkey: &Pubkey) -> Account {
        Account::new(0, 0, &system_program::ID)
    }

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
    pub wrapped_mint_exists: Pubkey,    

    // Token accounts
    pub vault: Pubkey,                  
    pub buyer_mint_ata: Pubkey,         
    pub buyer_wrapped_mint_ata: Pubkey, 
}

impl TokenWrapperTest {

     pub fn new() -> Self {
        let payer = Keypair::new();

        let source_mint = Keypair::new();
        let wrapped_mint = Keypair::new();

        let (mint_authority, _) =
            Pubkey::find_program_address(
                &[b"mint--authority", wrapped_mint.pubkey().as_ref()],
                &token_wrapper::ID,
            );
        let (vault_authority, _) =
            Pubkey::find_program_address(
                &[b"vault--authority", source_mint.pubkey().as_ref()],
                &token_wrapper::ID,
            );
        let (source_mint_exists, _) =
            Pubkey::find_program_address(
                &[b"exists", source_mint.pubkey().as_ref()],
                &token_wrapper::ID,
            );
        let (wrapped_mint_exists, _) =
            Pubkey::find_program_address(
                &[b"exists", wrapped_mint.pubkey().as_ref()],
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
            wrapped_mint_exists,

            vault,
            buyer_mint_ata,
            buyer_wrapped_mint_ata,
        }
    }

    #[inline(always)]
    pub fn _generate_keys(&mut self){
        *self = TokenWrapperTest::new();
    }
        
    pub fn setup(&self, accounts: &mut TokenWrapperAccountStore, buyer_lamports: u64,
                mint_supply: u64, token_account_balance: u64){
        let rent = Rent::default();

        let buyer_minimum_balance = rent.minimum_balance(0);
        let buyer_total_lamports = buyer_lamports + buyer_minimum_balance;
        
        accounts.store_account(
            self.payer.pubkey(), 
            Account::new(buyer_total_lamports, 0, &Self::SYSTEM_PROGRAM)
        );

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
    }

    #[inline(always)]
    pub fn setup_default(&self, validator: &mut TokenWrapperAccountStore){
        self.setup(
            validator,
            1_000_000_000_000_000_000,     
            1_000_000_000_000_000,  
            1_000_000_000_000
        );
    }

    pub fn create_mint(&self, context: &mut MolluskContext<TokenWrapperAccountStore>) -> ContextResult {
        
        let mut create_mint_accounts = Vec::<AccountMeta>::with_capacity(11);

        create_mint_accounts.push(AccountMeta::new(self.payer.pubkey(), true));

        create_mint_accounts.push(AccountMeta::new_readonly(self.mint_authority, false));

        create_mint_accounts.push(AccountMeta::new_readonly(self.source_mint.pubkey(), false));

        create_mint_accounts.push(AccountMeta::new(self.wrapped_mint.pubkey(), true));

        create_mint_accounts.push(AccountMeta::new_readonly(self.vault_authority, false));

        create_mint_accounts.push(AccountMeta::new(self.vault, false));

        create_mint_accounts.push(AccountMeta::new(self.source_mint_exists, false));

        create_mint_accounts.push(AccountMeta::new(self.wrapped_mint_exists, false));

        create_mint_accounts.push(AccountMeta::new_readonly(Self::SYSTEM_PROGRAM, false));

        create_mint_accounts.push(AccountMeta::new_readonly(Self::TOKEN_PROGRAM, false));

        create_mint_accounts.push(AccountMeta::new_readonly(Self::ASSOCIATED_TOKEN_PROGRAM, false));

        let create_mint_instruction = Instruction {
            program_id: token_wrapper::ID,
            accounts: create_mint_accounts,
            data: token_wrapper::instruction::CreateMint{}.data(),
        };
        context.process_instruction(&create_mint_instruction)
    }

    pub fn swap_to_wrapped(&self, context: &mut MolluskContext<TokenWrapperAccountStore>, amount:u64) -> ContextResult {

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

        swap_accounts.push(AccountMeta::new_readonly(self.wrapped_mint_exists, false));

        swap_accounts.push(AccountMeta::new_readonly(Self::SYSTEM_PROGRAM, false));

        swap_accounts.push(AccountMeta::new_readonly(Self::TOKEN_PROGRAM, false));

        swap_accounts.push(AccountMeta::new_readonly(Self::ASSOCIATED_TOKEN_PROGRAM, false));

        let swap_instruction = Instruction {
            program_id: token_wrapper::ID,
            accounts: swap_accounts,
            data: token_wrapper::instruction::SwapToWrapped{amount}.data()
        };
        context.process_instruction(&swap_instruction)
    }

    #[inline(always)]
    pub fn swap_to_wrapped_default(&self, context: &mut MolluskContext<TokenWrapperAccountStore>)->ContextResult{
        self.swap_to_wrapped(context, 100_000)
    }

    pub fn swap_to_source(&self, context: &mut MolluskContext<TokenWrapperAccountStore>, amount:u64)->ContextResult {

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

    swap_accounts.push(AccountMeta::new_readonly(self.wrapped_mint_exists, false));

    swap_accounts.push(AccountMeta::new_readonly(Self::SYSTEM_PROGRAM, false));

    swap_accounts.push(AccountMeta::new_readonly(Self::TOKEN_PROGRAM, false));

    swap_accounts.push(AccountMeta::new_readonly(Self::ASSOCIATED_TOKEN_PROGRAM, false));

    let swap_instruction = Instruction {
        program_id: token_wrapper::ID,
        accounts: swap_accounts,
        data: token_wrapper::instruction::SwapToSource{amount}.data()
    };

    context.process_instruction(&swap_instruction)
}

    pub fn swap_to_source_default(&self, context: &mut MolluskContext<TokenWrapperAccountStore>)->ContextResult{
        self.swap_to_source(context, 100_000)
    }

    pub const TOKEN_PROGRAM:Pubkey = spl_token::ID; 
    pub const ASSOCIATED_TOKEN_PROGRAM:Pubkey = mollusk_svm_programs_token::associated_token::ID;
    pub const SYSTEM_PROGRAM:Pubkey = system_program::ID;
}

pub  fn log_token_accounts(program_test_context: &MolluskContext<TokenWrapperAccountStore>, program_test: &TokenWrapperTest)->AnyResult<()>{
    
    // Get and display the user source token account.

    let account_result = program_test_context.account_store.borrow().
        get_account(&program_test.buyer_mint_ata);
    
    match account_result{
        Some(account)=>{
            let user_account:TokenAccount = TokenAccount::unpack(
                    &mut account.data.as_slice())?;
            println!("The user source mint token account was successfully fetched!!! \n\n");
            println!("Token account:- \n\n{:?}", user_account);
        },
        None=>{
            println!("The user source mint account does not exist yet and has a balance of 0.\n\n");
        }
    }

    // Get and display the user wrapped token account.

    let account_result = program_test_context.account_store.borrow().
        get_account(&program_test.buyer_wrapped_mint_ata);
    
    match account_result{
        
        Some(account)=>{
            let user_account:TokenAccount = TokenAccount::unpack(
                &mut account.data.as_slice())?;
                println!("The user wrapped mint token account was successfully fetched!!! \n\n");
                println!("Token account:- \n\n{:?}", user_account);
            },
            None=>{
            println!("The user wrapped mint account does not exist yet and has a balance of 0.\n\n");
        }
    }

    // Get and display the user source mint account.

    let account_result = program_test_context.account_store.borrow().
        get_account(&program_test.vault);
    
    match account_result{
        Some(account)=>{
            let user_account:TokenAccount = TokenAccount::unpack(
                    &mut account.data.as_slice())?;
            println!("The vault source mint token account was successfully fetched!!! \n\n");
            println!("Token account:- \n\n{:?}", user_account);
        },
        None=>{
            println!("The vault source mint account does not exist yet and has a balance of 0.\n\n");
        }
    }
    Ok(())
}

}