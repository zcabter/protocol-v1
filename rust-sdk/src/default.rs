use std::rc::Rc;

use anchor_client::Cluster;
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;

use crate::sdk_core::{
    account::DefaultClearingHouseAccount,
    user::ClearingHouseUser,
    util::{read_wallet_from_default, ConnectionConfig, DriftRpcClient},
};

impl ClearingHouseUser<DefaultClearingHouseAccount> {
    pub fn default(cluster: Cluster) -> Self {
        Self::default_with_commitment(cluster, CommitmentConfig::confirmed())
    }

    pub fn default_with_commitment(cluster: Cluster, commitment_config: CommitmentConfig) -> Self {
        let wallet = Box::new(read_wallet_from_default().unwrap());
        let conn = Rc::new(ConnectionConfig::from(cluster, commitment_config.clone()));
        let rpc_client = RpcClient::new_with_commitment(conn.get_rpc_url(), commitment_config);
        let rpc_client = Rc::new(DriftRpcClient::new(rpc_client));
        ClearingHouseUser::new(
            wallet,
            conn.clone(),
            rpc_client.clone(),
            DefaultClearingHouseAccount::new(conn, rpc_client),
        )
    }
}
