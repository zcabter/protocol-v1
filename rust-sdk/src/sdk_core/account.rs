use std::{cell::Ref, rc::Rc, time::Duration};

use clearing_house::{
    state::{
        history::{
            curve::CurveHistory, deposit::DepositHistory, funding_payment::FundingPaymentHistory,
            funding_rate::FundingRateHistory, liquidation::LiquidationHistory, trade::TradeHistory,
        },
        market::Markets,
        state::State,
    },
};
use solana_client::{rpc_config::RpcAccountInfoConfig, pubsub_client::PubsubClientError};
use solana_sdk::pubkey::Pubkey;

use super::{error::DriftResult, util::{ConnectionConfig, DriftRpcClient}};

pub trait ClearingHouseAccount {
    fn subscribe(self: Self, consumer: AccountConsumer) -> Self;
    fn unsubscribe(self: Self) -> Self;
    fn state(&self) -> &dyn DriftAccount<State>;
    fn markets(&self) -> &dyn DriftAccount<Markets>;
    fn trade_history(&self) -> &dyn DriftAccount<TradeHistory>;
    fn deposit_history(&self) -> &dyn DriftAccount<DepositHistory>;
    fn funding_payment_history(&self) -> &dyn DriftAccount<FundingPaymentHistory>;
    fn funding_rate_history(&self) -> &dyn DriftAccount<FundingRateHistory>;
    fn curve_history(&self) -> &dyn DriftAccount<CurveHistory>;
    fn liquidation_history(&self) -> &dyn DriftAccount<LiquidationHistory>;
}

pub trait DriftAccount<T> {
    fn subscribe(
        &self,
        url: &str,
        config: Option<RpcAccountInfoConfig>,
        consumer: fn(T) -> (),
    ) -> Result<(), PubsubClientError>;
    fn unsubscribe(&self) -> Result<(), PubsubClientError>;
    fn is_subscribed(&self) -> bool;
    fn pubkey(&self) -> Pubkey;
    fn get_data(&self, force: bool) -> DriftResult<Ref<Box<T>>>;
    fn get_name(&self) -> String;
}

pub enum AccountConsumer {
    StateConsumer(fn(State) -> ()),
    MarketsConsumer(fn(Markets) -> ()),
    TradeHistoryConsumer(fn(TradeHistory) -> ()),
    DepositHistoryConsumer(fn(DepositHistory) -> ()),
    FundingPaymentHistoryConsumer(fn(FundingPaymentHistory) -> ()),
    FundingRateHistoryConsumer(fn(FundingRateHistory) -> ()),
    CurveHistoryConsumer(fn(CurveHistory) -> ()),
    LiquidationHistoryConsumer(fn(LiquidationHistory) -> ()),
}

pub struct DefaultClearingHouseAccount {
    connection: Rc<ConnectionConfig>,
    state: Box<dyn DriftAccount<State>>,
    markets: Box<dyn DriftAccount<Markets>>,
    trade_history: Box<dyn DriftAccount<TradeHistory>>,
    deposit_history: Box<dyn DriftAccount<DepositHistory>>,
    funding_payment_history: Box<dyn DriftAccount<FundingPaymentHistory>>,
    funding_rate_history: Box<dyn DriftAccount<FundingRateHistory>>,
    curve_history: Box<dyn DriftAccount<CurveHistory>>,
    liquidation_history: Box<dyn DriftAccount<LiquidationHistory>>,
}

impl DefaultClearingHouseAccount {
    pub fn new(conn: Rc<ConnectionConfig>, client: Rc<DriftRpcClient>) -> DefaultClearingHouseAccount {
        let program_id = clearing_house::ID;
        let state_pubkey =
            Pubkey::find_program_address(&["clearing_house".as_bytes()], &program_id).0;
        let state_data = client.get_account_data::<State>(&state_pubkey).unwrap();
        let state = Box::new(account::WebSocketAccountSubscriber::<State>::new(
            state_pubkey,
            "State",
            client.clone(),
        ));
        let markets = Box::new(account::WebSocketAccountSubscriber::<Markets>::new(
            state_data.markets,
            "Markets",
            client.clone(),
        ));
        let trade_history = Box::new(account::WebSocketAccountSubscriber::<TradeHistory>::new(
            state_data.trade_history,
            "TradeHistory",
            client.clone(),
        ));
        let deposit_history = Box::new(account::WebSocketAccountSubscriber::<DepositHistory>::new(
            state_data.deposit_history,
            "DepositHistory",
            client.clone(),
        ));
        let funding_payment_history = Box::new(account::WebSocketAccountSubscriber::<
            FundingPaymentHistory,
        >::new(
            state_data.funding_payment_history,
            "FundingPaymentHistory",
            client.clone(),
        ));
        let funding_rate_history = Box::new(account::WebSocketAccountSubscriber::<
            FundingRateHistory,
        >::new(
            state_data.funding_payment_history,
            "FundingRateHistory",
            client.clone(),
        ));
        let curve_history = Box::new(account::WebSocketAccountSubscriber::<CurveHistory>::new(
            state_data.curve_history,
            "CurveHistory",
            client.clone(),
        ));
        let liquidation_history = Box::new(
            account::WebSocketAccountSubscriber::<LiquidationHistory>::new(
                state_data.liquidation_history,
                "LiquidationHistory",
                client.clone(),
            ),
        );
        DefaultClearingHouseAccount {
            connection: conn,
            state,
            markets,
            trade_history,
            deposit_history,
            funding_payment_history,
            funding_rate_history,
            curve_history,
            liquidation_history,
        }
    }

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
    fn subscribe(self: Self, consumer: AccountConsumer) -> Self {
        // could replace with macro?
        let (url, config) = self.get_config_pair_for_subscribe();
        match consumer {
            AccountConsumer::StateConsumer(f) => {
                if !self.state.is_subscribed() {
                    self.state.subscribe(&url, Some(config), f).unwrap();
                }
                self
            }
            AccountConsumer::MarketsConsumer(f) => {
                if !self.markets.is_subscribed() {
                    self.markets.subscribe(&url, Some(config), f).unwrap();
                }
                self
            }
            AccountConsumer::TradeHistoryConsumer(f) => {
                if !self.trade_history.is_subscribed() {
                    self.trade_history.subscribe(&url, Some(config), f).unwrap();
                }
                self
            }
            AccountConsumer::DepositHistoryConsumer(f) => {
                if !self.deposit_history.is_subscribed() {
                    self.deposit_history
                        .subscribe(&url, Some(config), f)
                        .unwrap();
                }
                self
            }
            AccountConsumer::FundingPaymentHistoryConsumer(f) => {
                if !self.funding_payment_history.is_subscribed() {
                    self.funding_payment_history
                        .subscribe(&url, Some(config), f)
                        .unwrap();
                }
                self
            }
            AccountConsumer::FundingRateHistoryConsumer(f) => {
                if !self.funding_rate_history.is_subscribed() {
                    self.funding_rate_history
                        .subscribe(&url, Some(config), f)
                        .unwrap();
                }
                self
            }
            AccountConsumer::CurveHistoryConsumer(f) => {
                if !self.curve_history.is_subscribed() {
                    self.curve_history.subscribe(&url, Some(config), f).unwrap();
                }
                self
            }
            AccountConsumer::LiquidationHistoryConsumer(f) => {
                if !self.liquidation_history.is_subscribed() {
                    self.liquidation_history
                        .subscribe(&url, Some(config), f)
                        .unwrap();
                }
                self
            }
        }
    }

    fn unsubscribe(self: Self) -> Self {
        fn retry_unsub<A>(a: &dyn DriftAccount<A>, n: u8) {
            let mut m = 0;
            while m <= n {
                match a.unsubscribe() {
                    Ok(_) => break,
                    Err(e) => {
                        println!(
                            "Unsubscribe from {} account failed with {}.",
                            a.get_name(),
                            e.to_string()
                        );
                        std::thread::sleep(Duration::from_secs(2));
                        m += 1;
                        if m > n {
                            break;
                        }
                        println!("Retry {}/{}", m, n);
                    }
                }
            }
            if m > n {
                println!("Failed to unsubscribe from {} account", a.get_name())
            }
        }
        retry_unsub(self.state.as_ref(), 2);
        retry_unsub(self.markets.as_ref(), 2);
        retry_unsub(self.curve_history.as_ref(), 2);
        retry_unsub(self.deposit_history.as_ref(), 2);
        retry_unsub(self.funding_payment_history.as_ref(), 2);
        retry_unsub(self.funding_rate_history.as_ref(), 2);
        retry_unsub(self.liquidation_history.as_ref(), 2);
        retry_unsub(self.trade_history.as_ref(), 2);
        self
    }

    fn state(self: &Self) -> &dyn DriftAccount<State> {
        self.state.as_ref()
    }

    fn markets(self: &Self) -> &dyn DriftAccount<Markets> {
        self.markets.as_ref()
    }

    fn trade_history(self: &Self) -> &dyn DriftAccount<TradeHistory> {
        self.trade_history.as_ref()
    }

    fn deposit_history(self: &Self) -> &dyn DriftAccount<DepositHistory> {
        self.deposit_history.as_ref()
    }

    fn funding_payment_history(self: &Self) -> &dyn DriftAccount<FundingPaymentHistory> {
        self.funding_payment_history.as_ref()
    }

    fn funding_rate_history(self: &Self) -> &dyn DriftAccount<FundingRateHistory> {
        self.funding_rate_history.as_ref()
    }

    fn curve_history(self: &Self) -> &dyn DriftAccount<CurveHistory> {
        self.curve_history.as_ref()
    }

    fn liquidation_history(self: &Self) -> &dyn DriftAccount<LiquidationHistory> {
        self.liquidation_history.as_ref()
    }
}

pub mod account {
    use anchor_lang::AccountDeserialize;
    use solana_account_decoder::{UiAccountData, UiAccountEncoding};
    use solana_client::{
        pubsub_client::{PubsubAccountClientSubscription, PubsubClient, PubsubClientError},
        rpc_config::RpcAccountInfoConfig,
    };
    use solana_sdk::pubkey::Pubkey;
    use std::{
        cell::{Ref, RefCell},
        marker::PhantomData,
        rc::Rc,
    };

    use crate::sdk_core::{util::DriftRpcClient, account::DriftAccount, error::DriftResult};

    pub struct WebSocketAccountSubscriber<T> {
        pubkey: Pubkey,
        name: &'static str,
        subscription: RefCell<Option<PubsubAccountClientSubscription>>,
        client: Rc<DriftRpcClient>,
        data: Box<RefCell<Option<Box<T>>>>,
        _marker: PhantomData<T>,
    }

    impl<T: AccountDeserialize + 'static> DriftAccount<T> for WebSocketAccountSubscriber<T> {
        fn subscribe(
            self: &Self,
            url: &str,
            config: Option<RpcAccountInfoConfig>,
            consumer: fn(T) -> (),
        ) -> Result<(), PubsubClientError> {
            let subscription =
                Self::ws_sub(&self.pubkey, String::from(self.name), url, config, consumer)?;
            self.subscription.replace_with(|_| Some(subscription));
            Ok(())
        }

        fn unsubscribe(self: &Self) -> Result<(), PubsubClientError> {
            match self.subscription.borrow().as_ref() {
                Some(subscription) => subscription.send_unsubscribe().and_then(|ok| {
                    self.subscription.replace_with(|_| None);
                    Ok(ok)
                }),
                None => Ok(()),
            }
        }

        fn is_subscribed(self: &Self) -> bool {
            self.subscription.borrow().is_some()
        }

        fn pubkey(self: &Self) -> Pubkey {
            self.pubkey
        }

        fn get_data(self: &Self, force: bool) -> DriftResult<Ref<Box<T>>> {
            if force || self.data.borrow().is_none() {
                let t: Box<T> = self.client.get_account_data(&self.pubkey)?;
                self.data.replace(Some(t));
            }
            Ok(Ref::map(self.data.borrow(), |a| a.as_ref().unwrap()))
        }

        fn get_name(self: &Self) -> String {
            String::from(self.name)
        }
    }

    impl<T: AccountDeserialize + 'static> WebSocketAccountSubscriber<T> {
        pub fn new(
            pubkey: Pubkey,
            name: &'static str,
            client: Rc<DriftRpcClient>,
        ) -> WebSocketAccountSubscriber<T> {
            WebSocketAccountSubscriber {
                pubkey,
                name,
                subscription: RefCell::new(None),
                client,
                data: Box::new(RefCell::new(None)),
                _marker: PhantomData,
            }
        }

        fn ws_sub(
            pubkey: &Pubkey,
            name: String,
            url: &str,
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
                                        name,
                                        err.to_string()
                                    )
                                }
                            }
                        }
                        return;
                    }
                    Err(err) => {
                        println!("While receving data from {}: {}", name, err.to_string());
                        return;
                    }
                }
            });
            Ok(pubsub_client_subscription)
        }
    }
}
