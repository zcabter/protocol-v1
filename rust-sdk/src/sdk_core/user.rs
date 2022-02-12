use super::{error::DriftResult, ClearingHouse};
use crate::sdk_core::{
    account::ClearingHouseAccount,
    util::{ix, ConnectionConfig, Context, DriftRpcClient},
};
use clearing_house::{
    accounts,
    context::{InitializeUserOptionalAccounts, ManagePositionOptionalAccounts},
    controller::position::PositionDirection,
    instruction,
    state::{market::Markets, user::User},
};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    system_program::ID as SYSTEM_PROGRAM_ID,
    sysvar::rent::ID as RENT_ID,
};
use spl_token::ID as TOKEN_PROGRAM_ID;
use std::{
    cell::RefCell,
    rc::Rc,
};

pub trait ClearingHouseUserTransactor: ClearingHouseUserInstruction {
    fn send_intialize_user_account(&self) -> DriftResult<(Signature, Pubkey)>;
    fn send_delete_user(&self) -> DriftResult<Signature>;
    fn send_deposit_collateral(
        &self,
        amount: u64,
        collateral_account_pubkey: Pubkey,
        user_positions_account_pubkey: Option<Pubkey>,
    ) -> DriftResult<Signature>;
    fn send_withdraw_collateral(
        &self,
        amount: u64,
        collateral_account_pubkey: &Pubkey,
    ) -> DriftResult<Signature>;
    fn send_open_position(
        &self,
        direction: PositionDirection,
        amount: u128,
        market_index: u64,
        limit_price: Option<u128>,
        discount_token: Option<Pubkey>,
        referrer: Option<Pubkey>,
    ) -> DriftResult<Signature>;
    fn send_close_position(
        &self,
        market_index: u64,
        discount_token: Option<Pubkey>,
        referrer: Option<Pubkey>,
    ) -> DriftResult<Signature>;
    fn send_initialize_user_account_and_deposit_collateral(
        &self,
        amount: u64,
        collateral_account_pubkey: &Pubkey,
    ) -> DriftResult<(Signature, Pubkey)>;
}

pub trait ClearingHouseUserInstruction {
    fn initialize_user_ix(&self) -> DriftResult<(Keypair, Pubkey, Instruction)>;
    fn deposit_collateral_ix(
        &self,
        amount: u64,
        collateral_acc_pub: Pubkey,
        user_pos_acc_pub: Option<Pubkey>,
    ) -> DriftResult<Instruction>;
}

pub struct ClearingHouseUser<T: ClearingHouseAccount> {
    pub program_id: Pubkey,
    pub wallet: Box<dyn Signer>,
    pub connection: Rc<ConnectionConfig>,
    pub client: Rc<DriftRpcClient>,
    pub accounts: T,
    _user_pubkey_and_nonce: RefCell<Option<(Pubkey, u8)>>,
    _user_account: RefCell<Option<User>>,
}

impl<T: ClearingHouseAccount> ClearingHouseUser<T> {
    pub fn new(
        wallet: Box<dyn Signer>,
        connection: Rc<ConnectionConfig>,
        client: Rc<DriftRpcClient>,
        accounts: T,
    ) -> Self {
        Self {
            program_id: clearing_house::id(),
            wallet,
            connection,
            client,
            accounts,
            _user_pubkey_and_nonce: RefCell::new(None),
            _user_account: RefCell::new(None),
        }
    }
}

impl<T: ClearingHouseAccount> ClearingHouse for ClearingHouseUser<T> {
    fn program_id(&self) -> Pubkey {
        self.program_id
    }

    fn wallet(&self) -> &dyn Signer {
        self.wallet.as_ref()
    }

    fn client(&self) -> &DriftRpcClient {
        &self.client
    }
}

impl<T: ClearingHouseAccount> ClearingHouseUser<T>{
    pub fn user_account_pubkey_and_nonce(&self) -> (Pubkey, u8) {
        if self._user_pubkey_and_nonce.borrow().is_none() {
            let result = Pubkey::find_program_address(
                &["user".as_bytes(), &self.wallet.pubkey().to_bytes()],
                &self.program_id,
            );
            self._user_pubkey_and_nonce.replace(Some(result));
        }
        self._user_pubkey_and_nonce.borrow().unwrap()
    }

    pub fn user_account(&self, force: bool) -> DriftResult<User> {
        if force || self._user_account.borrow().is_none() {
            let user_pubkey = self.user_account_pubkey_and_nonce().0;
            self._user_account
                .replace(Some(*self.client.get_account_data(&user_pubkey).unwrap()));
        }
        Ok(self._user_account.borrow().as_ref().unwrap().clone())
    }
}

impl<T: ClearingHouseAccount> ClearingHouseUserInstruction for ClearingHouseUser<T> {
    fn initialize_user_ix(&self) -> DriftResult<(Keypair, Pubkey, Instruction)> {
        let (user_pubkey, _user_nonce) = self.user_account_pubkey_and_nonce();
        let state = self.accounts.state().get_account_data(false)?;
        let mut optional_accounts = InitializeUserOptionalAccounts {
            whitelist_token: false,
        };
        let mut remaining_accounts: Vec<AccountMeta> = vec![];
        // todo: check and set whitelist_mint
        if !state.whitelist_mint.eq(&Pubkey::default()) {
            optional_accounts.whitelist_token = true;
            let associate_token_pubkey = spl_associated_token_account::get_associated_token_address(
                &self.wallet.pubkey(),
                &state.whitelist_mint,
            );
            remaining_accounts.push(AccountMeta {
                pubkey: associate_token_pubkey,
                is_signer: false,
                is_writable: false,
            });
        }
        let user_positions = Keypair::new();
        let init_user_ix = ix(
            &self.program_id,
            instruction::InitializeUser {
                _user_nonce: _user_nonce.clone(),
                optional_accounts,
            },
            Context {
                accounts: &accounts::InitializeUser {
                    user: user_pubkey,
                    state: self.accounts.state().pubkey(),
                    user_positions: user_positions.pubkey(),
                    authority: self.wallet.pubkey(),
                    rent: RENT_ID,
                    system_program: SYSTEM_PROGRAM_ID,
                },
                remaining_accounts,
            },
        );
        Ok((user_positions, user_pubkey, init_user_ix))
    }

    fn deposit_collateral_ix(
        &self,
        amount: u64,
        collateral_acc_pub: Pubkey,
        user_pos_acc_pub: Option<Pubkey>,
    ) -> DriftResult<Instruction> {
        let state = self.accounts.state().get_account_data(false)?;
        
        let user_pos_acc_pub = match user_pos_acc_pub {
            Some(p) => p,
            None => self.user_account(false)?.positions,
        };
        let ix = ix(
            &self.program_id,
            instruction::DepositCollateral { amount },
            Context {
                accounts: &accounts::DepositCollateral {
                    state: self.accounts.state().pubkey(),
                    user: self.user_account_pubkey_and_nonce().0,
                    authority: self.wallet.pubkey(),
                    collateral_vault: state.collateral_vault,
                    user_collateral_account: collateral_acc_pub,
                    token_program: TOKEN_PROGRAM_ID,
                    markets: self.accounts.markets().pubkey(),
                    user_positions: user_pos_acc_pub,
                    funding_payment_history: state.funding_payment_history,
                    deposit_history: state.deposit_history,
                },
                remaining_accounts: vec![],
            },
        );
        Ok(ix)
    }
}

impl<T: ClearingHouseAccount> ClearingHouseUserTransactor for ClearingHouseUser<T> {
    fn send_intialize_user_account(&self) -> DriftResult<(Signature, Pubkey)> {
        let (user_positions, user_account_pubkey, init_user_account_ix) =
            self.initialize_user_ix()?;
        self.send_tx(vec![&user_positions], &[init_user_account_ix])
            .map(|sig| (sig, user_account_pubkey))
    }

    fn send_delete_user(&self) -> DriftResult<Signature> {
        let user_pubkey = self.user_account_pubkey_and_nonce().0;
        let user: clearing_house::state::user::User =
            *self.client.get_account_data(&user_pubkey)?;
        let delete_ix = [ix(
            &self.program_id,
            instruction::DeleteUser {},
            Context {
                accounts: &accounts::DeleteUser {
                    user: user_pubkey,
                    user_positions: user.positions,
                    authority: self.wallet.pubkey(),
                },
                remaining_accounts: vec![],
            },
        )];
        self.send_tx(vec![], &delete_ix)
    }

    fn send_deposit_collateral(
        &self,
        amount: u64,
        collateral_account_pubkey: Pubkey,
        user_positions_account_pubkey: Option<Pubkey>,
    ) -> DriftResult<Signature> {
        let ix = self.deposit_collateral_ix(
            amount,
            collateral_account_pubkey,
            user_positions_account_pubkey,
        )?;
        self.send_tx(vec![], &[ix])
    }

    fn send_withdraw_collateral(
        &self,
        amount: u64,
        collateral_account_pubkey: &Pubkey,
    ) -> DriftResult<Signature> {
        let user_account_pubkey = self.user_account_pubkey_and_nonce().0;
        let user = self.user_account(false)?;
        let state = self.accounts.state().get_account_data(false)?;
        let ix = ix(
            &self.program_id,
            instruction::WithdrawCollateral { amount },
            Context {
                accounts: &accounts::WithdrawCollateral {
                    state: self.accounts.state().pubkey(),
                    user: user_account_pubkey,
                    authority: self.wallet.pubkey(),
                    collateral_vault: state.collateral_vault,
                    collateral_vault_authority: state.collateral_vault_authority,
                    insurance_vault: state.insurance_vault,
                    insurance_vault_authority: state.insurance_vault_authority,
                    user_collateral_account: collateral_account_pubkey.clone(),
                    token_program: TOKEN_PROGRAM_ID,
                    markets: state.markets,
                    user_positions: user.positions,
                    funding_payment_history: state.funding_payment_history,
                    deposit_history: state.deposit_history,
                },
                remaining_accounts: vec![],
            },
        );
        self.send_tx(vec![], &[ix])
    }

    fn send_open_position(
        &self,
        direction: PositionDirection,
        quote_asset_amount: u128,
        market_index: u64,
        limit_price: Option<u128>,
        discount_token: Option<Pubkey>,
        referrer: Option<Pubkey>,
    ) -> DriftResult<Signature> {
        let user_account_pubkey = self.user_account_pubkey_and_nonce().0;
        let user_account = self.user_account(false)?;
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
        let price_oracle = self.accounts.markets().get_account_data(false)?.markets
            [Markets::index_from_u64(market_index)]
        .amm
        .oracle;
        let state = self.accounts.state().get_account_data(false)?;
        let ix = ix(
            &self.program_id,
            instruction::OpenPosition {
                direction,
                quote_asset_amount,
                market_index,
                limit_price,
                optional_accounts,
            },
            Context {
                accounts: &accounts::OpenPosition {
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
            },
        );
        self.send_tx(vec![], &[ix])
    }

    fn send_close_position(
        &self,
        market_index: u64,
        discount_token: Option<Pubkey>,
        referrer: Option<Pubkey>,
    ) -> DriftResult<Signature> {
        let user_account_pubkey = self.user_account_pubkey_and_nonce().0;
        let user_account = self.user_account(false)?;
        let price_oracle = self.accounts.markets().get_account_data(false)?.markets
            [Markets::index_from_u64(market_index)]
        .amm
        .oracle;
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
        let state = self.accounts.state().get_account_data(false)?;
        let ix = ix(
            &self.program_id,
            instruction::ClosePosition {
                market_index,
                optional_accounts,
            },
            Context {
                accounts: &accounts::ClosePosition {
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
            },
        );
        self.send_tx(vec![], &[ix])
    }

    fn send_initialize_user_account_and_deposit_collateral(
        &self,
        amount: u64,
        collateral_acc_pub: &Pubkey,
    ) -> DriftResult<(Signature, Pubkey)> {
        let (user_pos_acc, user_acc_pub, init_user_acc_ix) = self.initialize_user_ix()?;
        let deposit_coll_ix =
            self.deposit_collateral_ix(amount, *collateral_acc_pub, Some(user_pos_acc.pubkey()))?;
        self.send_tx(vec![&user_pos_acc], &[init_user_acc_ix, deposit_coll_ix])
            .map(|sig| (sig, user_acc_pub))
    }
}
