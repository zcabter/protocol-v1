use crate::clearing_house::{ClearingHouse, DriftRpcClient};
use crate::default::account::AccountSubscriber;
use crate::sdk_core::constants;
use crate::{
    clearing_house::{ClearingHouseAccount, DriftAccountConsumer},
    sdk_core::connection::Connection,
};

use clearing_house::state::{market::Markets, state::State};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcAccountInfoConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::read_keypair_file;
use std::env;
use std::rc::Rc;
use std::str::FromStr;

impl ClearingHouse<DefaultClearingHouseAccount> {
    pub fn default_from_env() -> ClearingHouse<DefaultClearingHouseAccount> {
        // todo: refactor configs into ClearingHouseConfig
        let program_id = Pubkey::from_str(&env::var(constants::DRIFT_PROGRAM_ID).unwrap()).unwrap();
        let state_pubkey = Pubkey::from_str(&env::var(constants::envvar::DRFIT_STATE_PUBKEY).unwrap()).unwrap();
        let markets_pubkey = Pubkey::from_str(&env::var(constants::envvar::DRIFT_MARKETS_PUBKEY).unwrap()).unwrap();
        let wallet = read_keypair_file(env::var(constants::envvar::WALLET_JSON_PATH).unwrap()).unwrap();
        let commitment_config = CommitmentConfig::finalized();
        let conn = Rc::new(Connection::from_str(&env::var(constants::envvar::TARGET_NET).unwrap(), commitment_config.clone()));
        let rpc_client =  Rc::new(RpcClient::new_with_commitment(conn.get_rpc_url(), commitment_config));
        let rpc_client =  DriftRpcClient::new(rpc_client.clone());
        ClearingHouse {
            program_id,
            wallet: Box::new(wallet), 
            connection: conn.clone(), 
            client: rpc_client,
            accounts: DefaultClearingHouseAccount {
                connection: conn.clone(),
                state_account: account::state_account(state_pubkey),
                markets_account: account::markets_account(markets_pubkey),
                
            }
        }
    }
}

pub struct DefaultClearingHouseAccount {
    connection: Rc<Connection>,
    state_account: AccountSubscriber<State>,
    markets_account: AccountSubscriber<Markets>,
}

impl DefaultClearingHouseAccount {
    fn get_config_pair_for_subscribe(self: &Self) -> (String, RpcAccountInfoConfig) {
        let url = self.connection.get_ws_url();
        let config = RpcAccountInfoConfig {
            encoding: Some(self.connection.get_ws_data_encoding().clone()),
            data_slice: None,
            commitment: Some(self.connection.get_commitment_config().clone()),
        };
        (url, config)
    }
}

impl ClearingHouseAccount for DefaultClearingHouseAccount {
    fn subscribe(self: Self, consumer: DriftAccountConsumer) -> Self {
        match consumer {
            DriftAccountConsumer::StateAccountConsumer(f) => {
                if !self.state_account.is_subscribed() {
                    let (url, config) = self.get_config_pair_for_subscribe();
                    self.state_account.subscribe(&url, Some(config), f).unwrap();
                }
                self
            }
            DriftAccountConsumer::MarketsAccountConsumer(f) => {
                if !self.markets_account.is_subscribed() {
                    let (url, config) = self.get_config_pair_for_subscribe();
                    self.markets_account
                        .subscribe(&url, Some(config), f)
                        .unwrap();
                }
                self
            }
        }
    }

    fn unsubscribe(self: Self) -> Self {
        let unsubscribing = vec![
            self.state_account.unsubscribe(),
            self.markets_account.unsubscribe(),
        ];
        self
    }

    fn state_pubkey(self: &Self) -> &Pubkey {
        self.state_account.get_pubkey()
    }
    fn markets_pubkey(self: &Self) -> &Pubkey {
        self.markets_account.get_pubkey()
    }
}


mod account {
    use anchor_lang::AccountDeserialize;
    use clearing_house::state::{market::Markets, state::State};
    use solana_account_decoder::{UiAccountData, UiAccountEncoding};
    use solana_client::{
        pubsub_client::{PubsubAccountClientSubscription, PubsubClient, PubsubClientError},
        rpc_config::RpcAccountInfoConfig,
    };
    use solana_sdk::pubkey::Pubkey;
    use std::{cell::RefCell, marker::PhantomData};

    pub fn state_account(pubkey: Pubkey) -> AccountSubscriber<State> {
        AccountSubscriber::new(pubkey, "State")
    }
    pub fn markets_account(pubkey: Pubkey) -> AccountSubscriber<Markets> {
        AccountSubscriber::new(pubkey, "State")
    }

    pub struct AccountSubscriber<T> {
        pubkey: Pubkey,
        name: &'static str,
        subscription: RefCell<Option<PubsubAccountClientSubscription>>,
        _marker: PhantomData<T>,
    }

    impl<T: 'static> AccountSubscriber<T> where T: AccountDeserialize {
        pub fn new(pubkey: Pubkey, name: &'static str) -> AccountSubscriber<T> {
            AccountSubscriber {
                pubkey,
                name,
                subscription: RefCell::new(None),
                _marker: PhantomData,
            }
        }

        pub fn subscribe(
            self: &Self,
            url: &str,
            config: Option<RpcAccountInfoConfig>,
            consumer: fn(T) -> (),
        ) -> Result<(), PubsubClientError> {
            let pubsub_subscription = WebSockerSubscriber::subscribe(
                String::from(self.name),
                url,
                &self.pubkey,
                config,
                consumer,
            )?;
            self.subscription.replace_with(|_| Some(pubsub_subscription));
            Ok(())
        }

        pub fn unsubscribe(self: &Self) -> Result<(), PubsubClientError> {
            match self.subscription.borrow().as_ref() {
                Some(subscription) => {
                    WebSockerSubscriber::unsubscribe(subscription).and_then(|ok| {
                        self.subscription.replace_with(|_| None);
                        Ok(ok)
                    })
                }
                None => Ok(()),
            }
        }

        pub fn is_subscribed(self: &Self) -> bool {
            self.subscription.borrow().is_some()
        }

        pub fn get_pubkey(self: &Self) -> &Pubkey {
            &self.pubkey
        }
    }
    
    struct WebSockerSubscriber {}

    impl WebSockerSubscriber {
        fn subscribe<T: AccountDeserialize + 'static>(
            account_name: String,
            url: &str,
            pubkey: &Pubkey,
            config: Option<RpcAccountInfoConfig>,
            consumer: fn(T) -> (),
        ) -> Result<PubsubAccountClientSubscription, PubsubClientError> {
            let (pubsub_client_subscription, rx) =
                PubsubClient::account_subscribe(url, pubkey, config)?;
            std::thread::spawn(move || loop {
                match rx.recv() {
                    Ok(response) => {
                        if let UiAccountData::Binary(data, UiAccountEncoding::Base64) =
                            response.value.data
                        {
                            let decoded = base64::decode(data).unwrap();
                            let mut decoded: &[u8] = &decoded;
                            match T::try_deserialize(&mut decoded) {
                                Ok(t) => consumer(t),
                                Err(err) => {
                                    println!(
                                        "While decoding data from {}: {}",
                                        account_name,
                                        err.to_string()
                                    )
                                }
                            }
                        }
                        return;
                    }
                    Err(err) => {
                        println!(
                            "While receving data from {}: {}",
                            account_name,
                            err.to_string()
                        );
                        return;
                    }
                }
            });
            Ok(pubsub_client_subscription)
        }

        fn unsubscribe(
            subscription: &PubsubAccountClientSubscription,
        ) -> Result<(), PubsubClientError> {
            // todo: cleaup thread?
            subscription.send_unsubscribe()
        }
    }
}
