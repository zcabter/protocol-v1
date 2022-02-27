mod common;
use crate::common::*;
use anchor_client::Cluster;
use clearing_house::{
    controller::position::PositionDirection,
    state::{
        history::{
            funding_rate::FundingRateHistory, trade::TradeHistory,
        },
        market::Markets,
        state::State,
        user::{User, UserPositions},
    },
};
use drift_sdk::sdk_core::{
    account::ClearingHouseAccount,
    admin::{ClearingHouseAdmin, DefaultClearingHouseAdmin},
    user::{ClearingHouseUser, ClearingHouseUserTransactor},
    util::get_state_pubkey,
    ClearingHouse,
};
use solana_sdk::{pubkey::Pubkey, signer::Signer};

const USDC_AMOUNT: u64 = 10 * 10_u64.pow(6);


fn main() {
    println!("running clearing_house_test");
    test_initialize_state();
    test_initialize_markets();
    test_init_user_account_and_deposit_collateral_atomically();
    test_windraw_collateral();
    test_long_from_0_position();
}

fn test_initialize_state() {
    let admin = DefaultClearingHouseAdmin::default(Cluster::Localnet);
    create_mock_mint();
    admin
        .send_initialize_clearing_house(&(*MOCK_MINT_KEYPAIR).pubkey(), true)
        .unwrap();
    let state = admin
        .client
        .get_account_data::<State>(&get_state_pubkey())
        .unwrap();
    assert_eq!(admin.wallet.pubkey(), state.admin);

    let (collateral_account_authority, collateral_account_nonce) =
        Pubkey::find_program_address(&[&state.collateral_vault.to_bytes()], &admin.program_id);
    assert_eq!(
        collateral_account_authority,
        state.collateral_vault_authority
    );
    assert_eq!(collateral_account_nonce, state.collateral_vault_nonce);

    let (insurance_account_authority, insurance_account_nonce) =
        Pubkey::find_program_address(&[&state.insurance_vault.to_bytes()], &admin.program_id);
    assert_eq!(insurance_account_authority, state.insurance_vault_authority);
    assert_eq!(insurance_account_nonce, state.insurance_vault_nonce);

    let markets = admin
        .client
        .get_account_data::<Markets>(&state.markets)
        .unwrap();
    assert_eq!(64, markets.markets.len());

    let funding_rate_history = admin
        .client
        .get_account_data::<FundingRateHistory>(&state.funding_rate_history)
        .unwrap();
    assert_eq!(1, funding_rate_history.next_record_id());
    let trade_history = admin
        .client
        .get_account_data::<TradeHistory>(&state.trade_history)
        .unwrap();
    assert_eq!(1, trade_history.next_record_id());

    println!("test_initialize_state... ok")
}

fn test_initialize_markets() {
    let sol_usd_address = mock_oracle(1, -7);
    let admin = DefaultClearingHouseAdmin::default(Cluster::Localnet);
    admin.send_initialize_market(
        0, 
        &sol_usd_address,
        *AMM_INITIAL_BASE_ASSET_AMOUNT as u128, 
        *AMM_INITIAL_QUOTE_ASSET_AMOUNT as u128, 
        60 * 60, // periodicity
        1000 // peg precision
    ).unwrap();
    let clearing_house_user = ClearingHouseUser::default(Cluster::Localnet);
    let market_data = clearing_house_user.accounts.markets().get_account_data(true).unwrap().markets[0];
    assert_eq!(market_data.initialized, true);
    println!("test_initialize_markets... ok")

}

fn test_init_user_account_and_deposit_collateral_atomically() {
    let clearing_house_user = ClearingHouseUser::default(Cluster::Localnet);
    let usdc_mint = &*MOCK_MINT_KEYPAIR;
    create_mock_user_token_account(usdc_mint, USDC_AMOUNT as u64, None);
    let (_, user_acc_pub) = clearing_house_user
        .send_initialize_user_account_and_deposit_collateral(
            USDC_AMOUNT,
            &MOCK_USER_TOKEN_ACCOUNT_KEYPAIR.pubkey(),
        )
        .unwrap();
    let user: Box<User> = clearing_house_user
        .client
        .get_account_data(&user_acc_pub)
        .unwrap();
    assert_eq!(clearing_house_user.wallet.pubkey(), user.authority);
    assert_eq!(USDC_AMOUNT as u128, user.collateral);
    assert_eq!(USDC_AMOUNT as i128, user.cumulative_deposits);
    let user_acc = clearing_house_user.user_account(true).unwrap();
    assert_eq!(
        user_acc_pub,
        clearing_house_user.user_account_pubkey_and_nonce().0
    );
    assert_eq!(user.authority, user_acc.authority);

    let state = clearing_house_user
        .accounts
        .state()
        .get_account_data(true)
        .unwrap();
    let collateral_vault = clearing_house_user
        .get_token_account(&state.collateral_vault)
        .unwrap();
    assert_eq!(USDC_AMOUNT, collateral_vault.amount);

    let user_positions = clearing_house_user
        .client
        .get_account_data::<UserPositions>(&user.positions)
        .unwrap();
    assert_eq!(5, user_positions.positions.len());
    assert_eq!(user_acc_pub, user_positions.user);
    assert_eq!(0, user_positions.positions[0].base_asset_amount);
    assert_eq!(0, user_positions.positions[0].quote_asset_amount);
    assert_eq!(0, user_positions.positions[0].last_cumulative_funding_rate);

    let deposit_history = clearing_house_user
        .accounts
        .deposit_history()
        .get_account_data(true)
        .unwrap();
    assert_eq!(2, deposit_history.next_record_id()); // todo define getters on private fields to complete tests
    println!("test_init_user_account_and_deposit_collateral_atomically... ok")
}

fn test_windraw_collateral() {
    let clearing_house_user = ClearingHouseUser::default(Cluster::Localnet);
    let user_account = clearing_house_user.user_account(true).unwrap();
    assert_eq!(USDC_AMOUNT as u128, user_account.collateral);

    clearing_house_user
        .send_withdraw_collateral(USDC_AMOUNT, &MOCK_USER_TOKEN_ACCOUNT_KEYPAIR.pubkey())
        .unwrap();
    let user_account = clearing_house_user.user_account(true).unwrap();
    assert_eq!(0, user_account.collateral);
    assert_eq!(0, user_account.cumulative_deposits);

    let state = clearing_house_user
        .accounts
        .state()
        .get_account_data(false)
        .unwrap();
    let collateral_vault = clearing_house_user
        .get_token_account(&state.collateral_vault)
        .unwrap();
    assert_eq!(0, collateral_vault.amount);

    let user_usdc_token_account = clearing_house_user
        .get_token_account(&MOCK_USER_TOKEN_ACCOUNT_KEYPAIR.pubkey())
        .unwrap();
    assert_eq!(USDC_AMOUNT, user_usdc_token_account.amount);
    /*
    let deposit_history = clearing_house_user
        .accounts
        .deposit_history()
        .get_account_data(true)
        .unwrap();
    // tests below do not compile because fields on DepositHistory are private
    let deposit_record = deposit_history.deposit_records[1];
    assert_eq!(2, deposit_history.head);
    assert_eq!(2, deposit_record.record_id);
    assert_eq!(
        clearing_house_user.wallet().pubkey(),
        deposit_record.user_authority
    );
    assert_eq!(
        clearing_house_user.user_account_pubkey_and_nonce().0,
        deposit_record.user
    );
    assert!(DepositDirection::WITHDRAW == deposit_record.direction);
    assert_eq!(10_000_000, deposit_record.amount);
    assert_eq!(10_000_000, deposit_record.collateral_before);
    assert_eq!(10_000_000, deposit_record.cumulative_deposits_before);

    */
    println!("test_windraw_collateral... ok")
}

fn test_long_from_0_position() {
    let clearing_house_user = ClearingHouseUser::default(Cluster::Localnet);
    clearing_house_user
        .send_deposit_collateral(
            USDC_AMOUNT,
            &MOCK_USER_TOKEN_ACCOUNT_KEYPAIR.pubkey(),
            None,
        )
        .unwrap();
    let market_index = 0;
    let incremental_usdc_notional_amount = calculate_trade_amount(USDC_AMOUNT);
    clearing_house_user
        .send_open_position(
            PositionDirection::Long,
            incremental_usdc_notional_amount,
            market_index,
            None,
            None,
            None,
        )
        .unwrap();
    let user_account = clearing_house_user.user_account(true).unwrap();
    assert_eq!(9950250, user_account.collateral);
    assert_eq!(49750, user_account.total_fee_paid);
    assert_eq!(USDC_AMOUNT as i128, user_account.cumulative_deposits);

    let user_positions_account = clearing_house_user
        .client
        .get_account_data::<UserPositions>(&user_account.positions)
        .unwrap();
    assert_eq!(
        49750000,
        user_positions_account.positions[0].quote_asset_amount
    );
    assert_eq!(
        497450503674885,
        user_positions_account.positions[0].base_asset_amount
    );
    let markets_account = clearing_house_user
        .accounts
        .markets()
        .get_account_data(true)
        .unwrap();
    let market = markets_account.markets[0];
    assert_eq!(497450503674885, market.base_asset_amount);
    assert_eq!(49750, market.amm.total_fee);
    assert_eq!(49750, market.amm.total_fee_minus_distributions);

    // can't test bc TradeHistory fields are private
    // let trade_history_account = clearing_house_user
    //     .accounts
    //     .trade_history()
    //     .get_account_data(true)
    //     .unwrap();

    println!("test_long_from_0_position... ok");
}
