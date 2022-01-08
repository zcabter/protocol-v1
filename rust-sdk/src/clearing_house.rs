use crate::{sdk_core::{connection::Connection}};

use std::{rc::Rc, io::ErrorKind};
use anchor_lang::{AccountDeserialize, ToAccountMetas, InstructionData};
use clearing_house::{state::{market::Markets, state::State}};
use solana_sdk::{signer::Signer, pubkey::Pubkey, signature::Signature, instruction::Instruction};
use solana_client::{client_error::ClientError, rpc_client::RpcClient};

/// ClearingHouse
/// Main way to interact with the Clearing House program. ClearingHouseAccount allows users to un/subscribe to account changes and fetch account data. Users 
/// should implement their own ClearingHouseAccount if they wish to handle network calls in differently.
pub struct ClearingHouse<T> where T: ClearingHouseAccount {
    pub program_id: Pubkey,
    pub wallet: Box<dyn Signer>, 
    pub connection: Rc<Connection>, 
    pub client: DriftRpcClient,
    pub accounts: T,
}

impl <T: ClearingHouseAccount> ClearingHouse<T> {}

impl <T: ClearingHouseAccount> ClearingHouseAccountFetch for  ClearingHouse<T> {
    fn fetch_state_account(self: &Self) -> SolanaClientResult<State> {
        self.client.get_account_data(&self.accounts.state_pubkey())
    }

    fn fetch_markets_account(self: &Self) -> SolanaClientResult<Markets> {
        self.client.get_account_data(&self.accounts.markets_pubkey())
    }
}

pub trait ClearingHouseAccount {
    fn subscribe(self: Self, consumer: DriftAccountConsumer) -> Self;
    fn unsubscribe(self: Self) -> Self;  
    fn state_pubkey(self: &Self) -> &Pubkey;
    fn markets_pubkey(self: &Self) -> &Pubkey;
}

pub trait ClearingHouseAccountFetch {
    fn fetch_state_account(self: &Self) -> SolanaClientResult<State>;
    fn fetch_markets_account(self: &Self) -> SolanaClientResult<Markets>;
}

pub trait ClearingHouseTxtor {
    fn intialize_user_account(self: &Self) -> SolanaClientResult<(Signature, Pubkey)>;
    fn delete_user(self: &Self) -> ();
    fn deposit_collateral(self: &Self) -> ();
    fn withdraw_collateral(self: &Self) -> ();
    fn open_position(self: &Self) -> ();
    fn close_position(self: &Self) -> ();
    fn ins(self: &Self, args: impl InstructionData, acc: impl ToAccountMetas) -> Instruction;
}

pub enum DriftAccountConsumer {
    StateAccountConsumer(fn(State) -> ()),
    MarketsAccountConsumer(fn(Markets) -> ()),
}

pub type SolanaClientResult<T> = Result<T, ClientError>;

pub struct DriftRpcClient {
   pub  c: Rc<RpcClient>,
}

impl DriftRpcClient {
    pub fn new(rpc_client: Rc<RpcClient>) -> Self {
        DriftRpcClient { c: rpc_client }
    }
    pub fn get_account_data<T: AccountDeserialize + 'static>(
        self: &Self,
        account_pubkey: &Pubkey,
    ) -> SolanaClientResult<T> {
        let data = self.c.get_account_data(account_pubkey)?;
        let mut data: &[u8] = &data;
        T::try_deserialize(&mut data)
            .map_err(|e| std::io::Error::new(ErrorKind::InvalidInput, e).into())
    }
}
 pub mod tx {
    use anchor_lang::{ToAccountMetas, InstructionData};
    use clearing_house::{context::InitializeUserOptionalAccounts};
    use solana_sdk::{pubkey::Pubkey, signature::{Signature, Keypair, Signer}, instruction::Instruction, transaction::Transaction};
    use clearing_house::instruction as charg;
    use clearing_house::accounts as chacc;
    use crate::clearing_house::ClearingHouseAccountFetch;
    use anchor_client::Client;
    
    use super::{ClearingHouse, ClearingHouseTxtor, ClearingHouseAccount, SolanaClientResult};

    impl <T: ClearingHouseAccount> ClearingHouseTxtor for ClearingHouse<T> {
        fn intialize_user_account(self: &Self) -> SolanaClientResult<(Signature, Pubkey)> {
            let (user_pubkey, _user_nonce) = Pubkey::find_program_address(&["user".as_bytes(), &self.wallet.pubkey().to_bytes()], &self.program_id);
            let state = self.fetch_state_account()?;
            let mut optional_accounts = InitializeUserOptionalAccounts { whitelist_token: false  };
            // todo: check and set whitelist_mint
            if state.whitelist_mint != Pubkey::default() {
                optional_accounts.whitelist_token = true
                
            }
            let user_positions = Keypair::new();
            let intialize_user_ixs = [self.ins(
                charg::InitializeUser { _user_nonce, optional_accounts },
                chacc::InitializeUser {
                    user: user_pubkey,
                    state: self.accounts.state_pubkey().clone(),
                    user_positions: user_positions.pubkey(),
                    authority: self.wallet.pubkey(),
                    rent:solana_sdk::sysvar::rent::ID,
                    system_program: solana_sdk::system_program::ID,
                }
            )];
            let tx = {
                let hash = self.client.c.get_latest_blockhash()?;
                let signers: Vec<& dyn Signer> = vec![&user_positions];
                Transaction::new_signed_with_payer(
                    &intialize_user_ixs,
                    Some(&self.wallet.pubkey()),
                    &signers,
                    hash,
                )
            };
            self.client.c.send_transaction(&tx).map(|sig| (sig, user_pubkey))
        }
    
        fn delete_user(self: &Self) -> () {
            todo!()
        }
    
        fn deposit_collateral(self: &Self) -> () {
            todo!()
        }
    
        fn withdraw_collateral(self: &Self) -> () {
            todo!()
        }
    
        fn open_position(self: &Self) -> () {
            todo!()
        }
    
        fn close_position(self: &Self) -> () {
            todo!()
        }
    
        fn ins(self: &Self, args: impl InstructionData, acc: impl ToAccountMetas) -> Instruction {
            Instruction {
                program_id: self.program_id,
                accounts: acc.to_account_metas(None),
                data: args.data()
            }
        }
        
    }
 }

