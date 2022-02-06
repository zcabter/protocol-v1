mod common;

use crate::common::*;
use anchor_client::Cluster;
use clearing_house::state::{
    history::{funding_rate::FundingRateHistory, trade::TradeHistory},
    market::Markets,
    state::State,
};
use drift_sdk::sdk_core::{
    admin::{ClearingHouseAdmin, SimpleClearingHouseAdmin},
    ClearingHouse,
};
use solana_sdk::{pubkey::Pubkey, signer::Signer};

// const USDC_AMOUNT: u64 = 10 * 10 ^ 6;
// fn mock() -> (Keypair, Keypair) {
//     let usdc_mint = create_mock_mint();
//     let usdc_user_account = create_mock_user_token_account(&usdc_mint, USDC_AMOUNT as u64, None);
//     (usdc_mint, usdc_user_account)
// }

// fn after() {

// }

#[test]
fn ch_integration_initialize_state() {
    let admin = SimpleClearingHouseAdmin::default(Cluster::Localnet);
    let usdc_mint = create_mock_mint();
    admin
        .send_initialize_clearing_house(&usdc_mint.pubkey(), true)
        .unwrap();
    let state = admin
        .client
        .get_account_data::<State>(&admin.get_state_pubkey())
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
}

// fn ch_integration_init_user_account_and_deposit_collateral_atomically() {
//     let clearing_house = ClearingHouse::default(Cluster::Devnet);
//     let usdc_mint = create_mock_mint();
//     let usdc_user_account = create_mock_user_token_account(&usdc_mint, USDC_AMOUNT as u64, None);
//     let (_, user_acc_pub) = clearing_house
//         .send_initialize_user_account_and_deposit_collateral(
//             USDC_AMOUNT,
//             &usdc_user_account.pubkey(),
//         )
//         .unwrap();
//     let user: User = clearing_house
//         .client
//         .get_account_data(&user_acc_pub)
//         .unwrap();
//     assert_eq!(clearing_house.wallet.pubkey(), user.authority);
// }
