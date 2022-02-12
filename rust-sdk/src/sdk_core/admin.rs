use std::rc::Rc;
use crate::sdk_core::{
    error::{DriftError, DriftResult},
    util::{ix, Context, read_wallet_from_default, ConnectionConfig},
};
use anchor_client::Cluster;
use clearing_house::{
    accounts, instruction,
    state::{market::Markets, state::State},
};
use solana_client::{client_error::ClientErrorKind, rpc_request::RpcError, rpc_client::RpcClient};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer, commitment_config::CommitmentConfig,
};
use spl_token::ID as TOKEN_PROGRAM_ID;

use super::{
    util::{size::*, DriftRpcClient, get_state_pubkey},
    ClearingHouse,
};

pub struct DefaultClearingHouseAdmin {
    pub program_id: Pubkey,
    pub wallet: Box<dyn Signer>,
    pub client: DriftRpcClient,
}

impl DefaultClearingHouseAdmin {
    pub fn default(cluster: Cluster) -> Self {
        let wallet = Box::new(read_wallet_from_default().unwrap());
        let commitment = CommitmentConfig::processed();
        Self::new(wallet, cluster, commitment)
    }

    pub fn default_with_commitment(cluster: Cluster, commitment_config: CommitmentConfig) -> Self {
        let wallet = Box::new(read_wallet_from_default().unwrap());
        Self::new(wallet, cluster, commitment_config)
    }

    pub fn new(wallet: Box<dyn Signer>, cluster: Cluster, commitment_config: CommitmentConfig) -> Self {
        let conn = Rc::new(ConnectionConfig::from(cluster, commitment_config.clone()));
        let rpc_client = RpcClient::new_with_commitment(conn.get_rpc_url(), commitment_config);
        let rpc_client = DriftRpcClient::new(rpc_client, conn);
        DefaultClearingHouseAdmin {
            program_id: clearing_house::id(),
            wallet,
            client: rpc_client,
        }
    }
}

impl ClearingHouse for DefaultClearingHouseAdmin {
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

impl ClearingHouseAdmin for DefaultClearingHouseAdmin {}

pub trait ClearingHouseAdmin: ClearingHouse {
    fn send_initialize_clearing_house(
        &self,
        usdc_mint: &Pubkey,
        admin_controls_prices: bool,
    ) -> DriftResult<(Signature, Signature)> {
        match self.client().c.get_account(&get_state_pubkey()) {
            Ok(_) => Err(DriftError::AccountCannotBeInitialized {
                name: String::from("State"),
                reason: String::from("Clearing house already initialized"),
            }),
            Err(err) => match err.kind {
                ClientErrorKind::RpcError(RpcError::ForUser(_)) => Ok(()),
                _ => Err(DriftError::from(err)),
            },
        }?;
        let (collateral_vault_pub, _collateral_vault_nonce) =
            Pubkey::find_program_address(&[b"collateral_vault"], &self.program_id());
        let (collateral_vault_authority, _) =
            Pubkey::find_program_address(&[&collateral_vault_pub.to_bytes()], &self.program_id());
        let (insurance_vault_pub, _insurance_vault_nonce) =
            Pubkey::find_program_address(&[b"insurance_vault"], &self.program_id());
        let (insurance_vault_authority, _) =
            Pubkey::find_program_address(&[&insurance_vault_pub.to_bytes()], &self.program_id());
        let (clearing_house_state_pub, _clearing_house_nonce) =
            Pubkey::find_program_address(&[b"clearing_house"], &self.program_id());

        let markets = Keypair::new();
        let deposit_history = Keypair::new();
        let funding_rate_history = Keypair::new();
        let funding_payment_history = Keypair::new();
        let trade_history = Keypair::new();
        let liquidation_history = Keypair::new();
        let curve_history = Keypair::new();
        let initialize_ix = ix(
            &self.program_id(),
            instruction::Initialize {
                _clearing_house_nonce,
                _collateral_vault_nonce,
                _insurance_vault_nonce,
                admin_controls_prices,
            },
            Context {
                accounts: &accounts::Initialize {
                    admin: self.wallet().pubkey(),
                    state: clearing_house_state_pub,
                    collateral_mint: usdc_mint.clone(),
                    collateral_vault: collateral_vault_pub,
                    collateral_vault_authority,
                    insurance_vault: insurance_vault_pub,
                    insurance_vault_authority,
                    markets: markets.pubkey(),
                    rent: solana_sdk::sysvar::rent::id(),
                    system_program: solana_sdk::system_program::id(),
                    token_program: TOKEN_PROGRAM_ID,
                },
                remaining_accounts: vec![],
            },
        );

        let initialize_tx_sig = self.send_tx(
            vec![&markets],
            &[
                self.create_account_ix(MARKETS_SIZE, &markets),
                initialize_ix,
            ],
        )?;
        
        let initialize_history_ix = ix(
            &self.program_id(),
            instruction::InitializeHistory {},
            Context {
                accounts: &accounts::InitializeHistory {
                    admin: self.wallet().pubkey(),
                    state: clearing_house_state_pub,
                    funding_payment_history: funding_payment_history.pubkey(),
                    trade_history: trade_history.pubkey(),
                    liquidation_history: liquidation_history.pubkey(),
                    deposit_history: deposit_history.pubkey(),
                    funding_rate_history: funding_rate_history.pubkey(),
                    curve_history: curve_history.pubkey(),
                },
                remaining_accounts: vec![],
            },
        );
        let initialize_history_tx_sig = self.send_tx(
            vec![
                &deposit_history,
                &funding_payment_history,
                &trade_history,
                &liquidation_history,
                &funding_rate_history,
                &curve_history,
            ],
            &[
                self.create_account_ix(FUNDING_RATE_HISTORY_SIZE, &funding_rate_history),
                self.create_account_ix(FUNDING_PAYMENT_HISTORY_SIZE, &funding_payment_history),
                self.create_account_ix(TRADE_HISTORY_SIZE, &trade_history),
                self.create_account_ix(LIQUIDATION_HISTORY_SIZE, &liquidation_history),
                self.create_account_ix(DEPOSIT_HISTORY_SIZE, &deposit_history),
                self.create_account_ix(CURVE_HISTORY_SIZE, &curve_history),
                initialize_history_ix,
            ],
        )?;
        Ok((initialize_tx_sig, initialize_history_tx_sig))
        
    }

    fn send_initialize_clearing_market(
        &self,
        market_index: u64,
        price_oracle: &Pubkey,
        base_asset_reserve: u128,
        quote_asset_reserve: u128,
        periodicity: i64,
        peg_multiplier: u128,
    ) -> DriftResult<Signature> {
        let state = self
            .client()
            .get_account_data::<State>(&get_state_pubkey())?;
        let markets = self
            .client()
            .get_account_data::<Markets>(&state.markets)?
            .markets;
        let market_is_initialized = markets[Markets::index_from_u64(market_index)].initialized;
        if market_is_initialized {
            return Err(DriftError::AccountCannotBeInitialized {
                name: format!("Markets[{}]", market_index),
                reason: String::from("Already initialized"),
            });
        }
        let initialize_market_ix = ix(
            &self.program_id(),
            instruction::InitializeMarket {
                market_index,
                amm_base_asset_reserve: base_asset_reserve,
                amm_quote_asset_reserve: quote_asset_reserve,
                amm_periodicity: periodicity,
                amm_peg_multiplier: peg_multiplier,
            },
            Context {
                accounts: &accounts::InitializeMarket {
                    admin: self.wallet().pubkey(),
                    state: get_state_pubkey(),
                    markets: state.markets,
                    oracle: price_oracle.clone(),
                },
                remaining_accounts: vec![],
            },
        );

        self.send_tx(vec![], &[initialize_market_ix])
    }
}
