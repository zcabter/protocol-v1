use anchor_client::Cluster;
use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use solana_account_decoder::UiAccountEncoding;
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{read_keypair_file, Keypair};
use std::rc::Rc;
use std::{env, error};

use super::error::DriftResult;

pub struct DriftRpcClient {
    pub c: RpcClient,
    pub conn: Rc<ConnectionConfig>
}

impl DriftRpcClient {
    pub fn new(rpc_client: RpcClient, conn: Rc<ConnectionConfig>) -> Self {
        DriftRpcClient { c: rpc_client, conn }
    }
    pub fn get_account_data<T: AccountDeserialize + 'static>(
        &self,
        account_pubkey: &Pubkey,
    ) -> DriftResult<Box<T>> {
        let data = {
            let mut retry = 0;
            let mut bytes: Option<solana_client::client_error::Result<Vec<u8>>> = Option::None;
            while retry < 3 {
                bytes = Some(self.c.get_account_data(account_pubkey));
                if let Some(Ok(_)) = bytes {
                    break;
                }
                retry += 1;
                println!("retry getting account data for {}: [{}]", account_pubkey, retry); // todo use logger instead
                std::thread::sleep(std::time::Duration::from_secs(4));
            }
            bytes.unwrap()
        }?;
        let mut data: &[u8] = &data;
        Ok(Box::new(T::try_deserialize(&mut data)?))
    }
}

pub struct ConnectionConfig {
    rpc_url: String,
    ws_url: String,
    commitment_config: CommitmentConfig,
    ws_acc_data_encoding: UiAccountEncoding,
}

impl ConnectionConfig {
    pub fn from(cluster: Cluster, commitment_config: CommitmentConfig) -> ConnectionConfig {
        ConnectionConfig {
            rpc_url: String::from(cluster.url()),
            ws_url: String::from(cluster.ws_url()),
            commitment_config,
            ws_acc_data_encoding: UiAccountEncoding::Base64,
        }
    }

    pub fn get_rpc_url(&self) -> String {
        self.rpc_url.clone()
    }

    pub fn get_ws_url(&self) -> String {
        self.ws_url.clone()
    }

    pub fn get_commitment_config(&self) -> &CommitmentConfig {
        &self.commitment_config
    }

    pub fn get_ws_data_encoding(&self) -> &UiAccountEncoding {
        &self.ws_acc_data_encoding
    }
}

pub fn read_wallet_from_default() -> Result<Keypair, Box<dyn error::Error>> {
    let path = env::var(environment_variables::WALLET)
        .unwrap_or(String::from(shellexpand::tilde("~/.config/solana/id.json")));
    read_wallet_from(path)
}

pub fn read_wallet_from(path: String) -> Result<Keypair, Box<dyn error::Error>> {
    read_keypair_file(path)
}

pub fn get_state_pubkey() -> Pubkey {
    Pubkey::find_program_address(&["clearing_house".as_bytes()], &clearing_house::id()).0
}

pub fn ix(program_id: &Pubkey, data: impl InstructionData, accounts: Context) -> Instruction {
    Instruction {
        program_id: program_id.clone(),
        accounts: accounts.to_account_metas(),
        data: data.data(),
    }
}

// Mimic anchor-ts's accounts part of the Context
pub struct Context<'a> {
    pub accounts: &'a dyn ToAccountMetas,
    pub remaining_accounts: Vec<AccountMeta>,
}

impl<'a> Context<'a> {
    pub fn to_account_metas(self: Self) -> Vec<AccountMeta> {
        vec![
            self.accounts.to_account_metas(None),
            self.remaining_accounts,
        ]
        .concat()
    }
}

pub mod environment_variables {
    pub const WALLET: &str = "WALLET";
}

pub mod size {
    /* todo: find better way to get account sizes
    console.log(`Markets size: ${chProgram.account.markets.size}`)
    console.log(`FundingRateHistory size: ${chProgram.account.fundingRateHistory.size}`)
    console.log(`FundingPaymentHistory size: ${chProgram.account.fundingPaymentHistory.size}`)
    console.log(`TradeHistory size: ${chProgram.account.tradeHistory.size}`)
    console.log(`LiquidationHistory size: ${chProgram.account.liquidationHistory.size}`)
    console.log(`DepositHistory size: ${chProgram.account.depositHistory.size}`)
    console.log(`CurveHistory size: ${chProgram.account.curveHistory.size}`)
    ===========================================================================================
    Markets size: 33416
    FundingRateHistory size: 114704
    FundingPaymentHistory size: 188432
    TradeHistory size: 247824
    LiquidationHistory size: 254992
    DepositHistory size: 132112
    CurveHistory size: 8720 */
    pub const MARKETS_SIZE: usize = 33_416;
    pub const FUNDING_RATE_HISTORY_SIZE: usize = 114_704;
    pub const FUNDING_PAYMENT_HISTORY_SIZE: usize = 188_432;
    pub const TRADE_HISTORY_SIZE: usize = 247_824;
    pub const LIQUIDATION_HISTORY_SIZE: usize = 254_992;
    pub const DEPOSIT_HISTORY_SIZE: usize = 132_112;
    pub const CURVE_HISTORY_SIZE: usize = 8720;
}


