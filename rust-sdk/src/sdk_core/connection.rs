use solana_account_decoder::UiAccountEncoding;
use solana_sdk::commitment_config::CommitmentConfig;
use anchor_client::Cluster;
use std::str::FromStr;

pub struct Connection {
    rpc_url: String,
    ws_url: String, 
    commitment_config: CommitmentConfig,
    ws_acc_data_encoding: UiAccountEncoding,
}

impl Connection {
    pub fn from_str(cluster: &str, commitment_config: CommitmentConfig) -> Connection {
        let cluster = Cluster::from_str(cluster).unwrap();
        Self::from_with_commitment(cluster, commitment_config)
    }

    pub fn from_with_commitment(cluster: Cluster, commitment_config: CommitmentConfig) -> Connection {
        Connection {
            rpc_url: String::from(cluster.url()),
            ws_url: String::from(cluster.ws_url()),
            commitment_config,
            ws_acc_data_encoding: UiAccountEncoding::Base64
        }
    }

    pub fn get_rpc_url(self: &Self) -> String {
        self.rpc_url.clone()
    }

    pub fn get_ws_url(self: &Self) -> String {
        self.ws_url.clone()
    }

    pub fn get_commitment_config(self: &Self) -> &CommitmentConfig {
        &self.commitment_config
    }

    pub fn get_ws_data_encoding(self: &Self) -> &UiAccountEncoding {
        &self.ws_acc_data_encoding
    }

}