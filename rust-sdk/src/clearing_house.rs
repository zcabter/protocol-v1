use crate::{sdk_core::{connection::Connection}, default::account::DriftAccount};

use std::{rc::Rc, io::ErrorKind, cell::{RefCell, Ref}};
use anchor_lang::{AccountDeserialize, ToAccountMetas, InstructionData};
use clearing_house::{state::{market::Markets, state::State, user::User, history::{trade::TradeHistory, funding_payment::FundingPaymentHistory, funding_rate::FundingRateHistory, deposit::DepositHistory, curve::CurveHistory, liquidation::LiquidationHistory}}, controller::position::PositionDirection};
use solana_sdk::{signer::Signer, pubkey::Pubkey, signature::Signature, instruction::{Instruction, AccountMeta}, transaction::Transaction};
use solana_client::{client_error::ClientError, rpc_client::RpcClient};
/// ClearingHouse
/// Main way to interact with the Clearing House program. ClearingHouseAccount allows users to un/subscribe to account changes and fetch account data. Users 
/// should implement their own ClearingHouseAccount if they wish to handle subscription differently.
pub struct ClearingHouse<T> where T: ClearingHouseAccount {
    pub program_id: Pubkey,
    pub wallet: Box<dyn Signer>, 
    pub connection: Rc<Connection>, 
    pub client: Rc<DriftRpcClient>,
    pub accounts: T,
    _user_pubkey_and_nonce: RefCell<Option<(Pubkey, u8)>>,
    _user_account: RefCell<Option<User>>
}

impl <T: ClearingHouseAccount> ClearingHouse<T> {
    pub fn new(
        wallet: Box<dyn Signer>,
        connection: Rc<Connection>,
        client: Rc<DriftRpcClient>,
        accounts: T
    ) -> ClearingHouse<T> {
            ClearingHouse {
                program_id: clearing_house::ID,
                wallet,
                connection,
                client,
                accounts,
                _user_pubkey_and_nonce: RefCell::new(None),
                _user_account: RefCell::new(None),
            }
    }
    pub fn send_tx(self: &Self, signers: Vec<&dyn Signer>, ixs: &[Instruction]) -> SolanaClientResult<Signature> {
        let mut signers = signers;
        signers.push(self.wallet.as_ref()  );
        let tx = {
            let hash = self.client.c.get_latest_blockhash()?;
            Transaction::new_signed_with_payer(
                &ixs,
                Some(&self.wallet.pubkey()),
                &signers,
                hash,
            )
        };
        self.client.c.send_transaction(&tx)
    }

    fn _user_account_pubkey_and_nonce(self: &Self) -> (Pubkey, u8) {
        if self._user_pubkey_and_nonce.borrow().is_none() {
            let result = Pubkey::find_program_address(&["user".as_bytes(), &self.wallet.pubkey().to_bytes()], &self.program_id);
            self._user_pubkey_and_nonce.replace(Some(result));
        }
        self._user_pubkey_and_nonce.borrow().unwrap()
    }

    fn _user_account(self: &Self, force: bool) -> Ref<User> {
        if force || self._user_account.borrow().is_none() {
            let user_pubkey = self._user_account_pubkey_and_nonce().0;
            self._user_account.replace(Some(self.client.get_account_data(&user_pubkey).unwrap()));
        }
        Ref::map(self._user_account.borrow(), |a| a.as_ref().unwrap())
    }
}

pub trait ClearingHouseAccount {
    fn subscribe(self: Self, consumer: DriftAccountConsumer) -> Self;
    fn unsubscribe(self: Self) -> Self;  
    fn state(self: &Self) -> &dyn DriftAccount<State>;
    fn markets(self: &Self) -> &dyn DriftAccount<Markets>;
    fn trade_history(self: &Self) -> &dyn DriftAccount<TradeHistory>;
    fn deposit_history(self: &Self) -> &dyn DriftAccount<DepositHistory>;
    fn funding_payment_history(self: &Self) -> &dyn DriftAccount<FundingPaymentHistory>;
    fn funding_rate_history(self: &Self) -> &dyn DriftAccount<FundingRateHistory>;
    fn curve_history(self: &Self) -> &dyn DriftAccount<CurveHistory>;
    fn liquidation_history(self: &Self) -> &dyn DriftAccount<LiquidationHistory>;
}

pub trait ClearingHouseTxtor {
    fn intialize_user_account(self: &Self) -> SolanaClientResult<(Signature, Pubkey)>;
    fn delete_user(self: &Self) -> SolanaClientResult<Signature>;
    fn deposit_collateral(self: &Self, amount: u64, collateral_account_pubkey: Pubkey, user_positions_account_pubkey: Option<Pubkey>) -> SolanaClientResult<Signature>;
    fn withdraw_collateral(self: &Self, amount: u64, collateral_account_pubkey: Pubkey) -> SolanaClientResult<Signature>;
    fn open_position(self: &Self, direction: PositionDirection, amount: u128, market_index: u64, limit_price: Option<u128>, discount_token: Option<Pubkey>, referrer: Option<Pubkey>) -> SolanaClientResult<Signature>;
    fn close_position(self: &Self, market_index: u64, discount_token: Option<Pubkey>, referrer: Option<Pubkey>) -> SolanaClientResult<Signature>;
    fn ins<A: ToAccountMetas>(self: &Self, args: impl InstructionData, acc: AccMetas<A>) -> Instruction;
}

pub enum DriftAccountConsumer {
    StateConsumer(fn(State) -> ()),
    MarketsConsumer(fn(Markets) -> ()),
    TradeHistoryConsumer(fn(TradeHistory) -> ()),
    DepositHistoryConsumer(fn(DepositHistory) -> ()), 
    FundingPaymentHistoryConsumer(fn(FundingPaymentHistory) -> ()), 
    FundingRateHistoryConsumer(fn(FundingRateHistory) -> ()),
    CurveHistoryConsumer(fn(CurveHistory) -> ()), 
    LiquidationHistoryConsumer(fn(LiquidationHistory) -> ())
}

pub type SolanaClientResult<T> = Result<T, ClientError>;

pub struct DriftRpcClient {
   pub c: RpcClient,
}

impl DriftRpcClient {
    pub fn new(rpc_client: RpcClient) -> Self {
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
    use anchor_lang::{InstructionData, ToAccountMetas};
    use clearing_house::{context::{InitializeUserOptionalAccounts, ManagePositionOptionalAccounts}, controller::position::PositionDirection, state::market::Markets};
    use solana_sdk::{pubkey::Pubkey, signature::{Signature, Keypair, Signer}, instruction::{Instruction, AccountMeta}};
    use clearing_house::instruction as charg;
    use clearing_house::accounts as chacc;
    use spl_token::ID as TOKEN_PROGRAM_ID;
    use solana_sdk::sysvar::rent::ID as RENT_ID;
    use solana_sdk::system_program::ID as SYSTEM_PROGRAM_ID;
    use super::{ClearingHouse, ClearingHouseTxtor, ClearingHouseAccount, SolanaClientResult, AccMetas};

    impl <T: ClearingHouseAccount> ClearingHouseTxtor for ClearingHouse<T> {
        fn intialize_user_account(self: &Self) -> SolanaClientResult<(Signature, Pubkey)> {
            let (user_pubkey, _user_nonce) = self._user_account_pubkey_and_nonce();
            let state =  self.accounts.state().get_data(false)?;
            let mut optional_accounts = InitializeUserOptionalAccounts { whitelist_token: false  };
            let mut remaining_accounts: Vec<AccountMeta> = vec!();
            // todo: check and set whitelist_mint
            if !state.whitelist_mint.eq(&Pubkey::default()) {
                optional_accounts.whitelist_token = true;
                let associate_token_pubkey = spl_associated_token_account::get_associated_token_address(&self.wallet.pubkey(), &state.whitelist_mint);
                remaining_accounts.push(AccountMeta {
                    pubkey: associate_token_pubkey,
                    is_signer: false,
                    is_writable: false,
                });
            }
            let user_positions = Keypair::new();
            let intialize_user_ixs = [self.ins(
                charg::InitializeUser { _user_nonce, optional_accounts },
                AccMetas {
                    accounts: chacc::InitializeUser {
                        user: user_pubkey,
                        state: self.accounts.state().pubkey(),
                        user_positions: user_positions.pubkey(),
                        authority: self.wallet.pubkey(),
                        rent: RENT_ID,
                        system_program: SYSTEM_PROGRAM_ID,
                    },
                    remaining_accounts
                }
                
            )];
            self.send_tx(vec![&user_positions], &intialize_user_ixs).map(|sig| (sig, user_pubkey))
        }
    
        fn delete_user(self: &Self) -> SolanaClientResult<Signature> {
            let user_pubkey = self._user_account_pubkey_and_nonce().0;
            let user: clearing_house::state::user::User = self.client.get_account_data(&user_pubkey)?;
            let delete_ix = [
                self.ins(
                    charg::DeleteUser {},
                    AccMetas {
                        accounts: chacc::DeleteUser { user: user_pubkey, user_positions: user.positions, authority: self.wallet.pubkey() },
                        remaining_accounts: vec![]
                    }
                    
                )
            ];
            self.send_tx(vec![], &delete_ix)
        }
        
        fn deposit_collateral(self: &Self, amount: u64, collateral_account_pubkey: Pubkey, user_positions_account_pubkey: Option<Pubkey>) -> SolanaClientResult<Signature> {
            let user_positions_account_pubkey = user_positions_account_pubkey.unwrap_or_else(|| self._user_account(false).positions);
            let state = self.accounts.state().get_data(false)?;
            let ix = self.ins(
                charg::DepositCollateral { amount },
                AccMetas {
                    accounts: chacc::DepositCollateral { 
                        state: self.accounts.state().pubkey(), 
                        user: self._user_account_pubkey_and_nonce().0, 
                        authority: self.wallet.pubkey(), 
                        collateral_vault: state.collateral_vault, 
                        user_collateral_account: collateral_account_pubkey, 
                        token_program: TOKEN_PROGRAM_ID, 
                        markets: self.accounts.markets().pubkey(), 
                        user_positions: user_positions_account_pubkey, 
                        funding_payment_history: state.funding_payment_history, 
                        deposit_history: state.deposit_history
                    }, 
                    remaining_accounts: vec![]
                }
                
            );
            self.send_tx(vec![], &[ix])
        }
    
        fn withdraw_collateral(self: &Self, amount: u64, collateral_account_pubkey: Pubkey) -> SolanaClientResult<Signature> {
            let user_account_pubkey = self._user_account_pubkey_and_nonce().0;
            let user = self._user_account(false);
            let state = self.accounts.state().get_data(false)?;
            let ix = self.ins(
                charg::WithdrawCollateral { amount },
                AccMetas {
                    accounts: chacc::WithdrawCollateral {
                        state: self.accounts.state().pubkey(),
                        user: user_account_pubkey,
                        authority: self.wallet.pubkey(),
                        collateral_vault: state.collateral_vault,
                        collateral_vault_authority: state.collateral_vault_authority,
                        insurance_vault: state.insurance_vault,
                        insurance_vault_authority: state.insurance_vault_authority,
                        user_collateral_account: collateral_account_pubkey,
                        token_program: TOKEN_PROGRAM_ID,
                        markets: state.markets,
                        user_positions: user.positions,
                        funding_payment_history: state.funding_payment_history,
                        deposit_history: state.deposit_history,
                    },
                    remaining_accounts: vec![]
                }
            );
            self.send_tx(vec![], &[ix])
        }
        
        fn open_position(
            self: &Self, 
            direction: PositionDirection, 
            quote_asset_amount: u128, 
            market_index: u64, 
            limit_price: Option<u128>, 
            discount_token: Option<Pubkey>, 
            referrer: Option<Pubkey>) -> SolanaClientResult<Signature> {
            let user_account_pubkey = self._user_account_pubkey_and_nonce().0;
            let user_account = self._user_account(false);
            let limit_price = limit_price.unwrap_or(0);
            let mut optional_accounts = ManagePositionOptionalAccounts {
                discount_token: false,
                referrer: false,
            };
            let mut remaining_accounts = vec![];
            if let Some(pubkey) = discount_token {
                optional_accounts.discount_token = true;
                remaining_accounts.push(AccountMeta {
                    pubkey,
                    is_signer: false,
                    is_writable: false,
                })
            }
            if let Some(pubkey) = referrer {
                optional_accounts.referrer = true;
                remaining_accounts.push(AccountMeta {
                    pubkey,
                    is_signer: false,
                    is_writable: true,
                })
            }
            let price_oracle = self.accounts.markets().get_data(false)?.markets[Markets::index_from_u64(market_index)].amm.oracle;
            let state = self.accounts.state().get_data(false)?;
            let ix = self.ins(
                charg::OpenPosition {
                    direction,
                    quote_asset_amount,
                    market_index,
                    limit_price,
                    optional_accounts,
                }, 
                AccMetas {
                    accounts: chacc::OpenPosition {
                        state: self.accounts.state().pubkey(),
                        user: user_account_pubkey,
                        authority: self.wallet.pubkey(),
                        markets: state.markets,
                        user_positions: user_account.positions,
                        trade_history: state.trade_history,
                        funding_payment_history: state.funding_payment_history,
                        funding_rate_history: state.funding_rate_history,
                        oracle: price_oracle,
                    }, 
                    remaining_accounts,
                }
            );
            self.send_tx(vec![], &[ix])
        }
    
        fn close_position(self: &Self, market_index: u64, discount_token: Option<Pubkey>, referrer: Option<Pubkey>) -> SolanaClientResult<Signature> {
            let user_account_pubkey = self._user_account_pubkey_and_nonce().0;
            let user_account = self._user_account(false);
            let price_oracle = self.accounts.markets().get_data(false)?.markets[Markets::index_from_u64(market_index)].amm.oracle;
            let mut optional_accounts = ManagePositionOptionalAccounts {
                discount_token: false,
                referrer: false,
            };
            let mut remaining_accounts = vec![];
            if let Some(pubkey) = discount_token {
                optional_accounts.discount_token = true;
                remaining_accounts.push(AccountMeta {
                    pubkey,
                    is_signer: false,
                    is_writable: false,
                })
            }
            if let Some(pubkey) = referrer {
                optional_accounts.referrer = true;
                remaining_accounts.push(AccountMeta {
                    pubkey,
                    is_signer: false,
                    is_writable: true,
                })
            }
            let state = self.accounts.state().get_data(false)?;
            let ix = self.ins(
                charg::ClosePosition {
                    market_index,
                    optional_accounts,
                },
                AccMetas {
                    accounts: chacc::ClosePosition {
                        state: self.accounts.state().pubkey(),
                        user: user_account_pubkey,
                        authority: self.wallet.pubkey(),
                        markets: state.markets,
                        user_positions: user_account.positions,
                        trade_history: state.trade_history,
                        funding_payment_history: state.funding_payment_history,
                        funding_rate_history: state.funding_rate_history,
                        oracle: price_oracle,
                    },
                    remaining_accounts,
                }
            );
            self.send_tx(vec![], &[ix])
        }
    
        fn ins<U: ToAccountMetas>(self: &Self, args: impl InstructionData, acc: AccMetas<U>) -> Instruction {
            Instruction {
                program_id: self.program_id,
                accounts: acc.to_account_metas(),
                data: args.data()
            }
        }
    }
 }

 // Mimic anchor-ts's accounts part of the Context
pub struct AccMetas<A: ToAccountMetas> {
    accounts: A, 
    remaining_accounts: Vec<AccountMeta>
}

impl <A: ToAccountMetas> AccMetas<A> {
    pub fn to_account_metas(self: Self) -> Vec<AccountMeta> {
        vec![self.accounts.to_account_metas(None), self.remaining_accounts].concat()
    }
}
