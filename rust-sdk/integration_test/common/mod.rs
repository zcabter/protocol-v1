use std::convert::TryInto;

use anchor_client::Cluster;
use drift_sdk::sdk_core::util::{ix, read_wallet_from, read_wallet_from_default, Context};
use lazy_static::lazy_static;
use solana_client::{rpc_client::RpcClient, rpc_config::RpcSendTransactionConfig};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel}, program_pack::Pack, pubkey::Pubkey, signature::Keypair,
    signer::Signer, system_instruction as sol_sys_ix, transaction::Transaction,
};
use spl_token::{
    instruction as spl_ix,
    state::{Account, Mint},
};

pub const MAX_LEVERAGE: u64 = 5;
lazy_static! {
    pub static ref MOCK_MINT_KEYPAIR: Keypair = Keypair::new();
    pub static ref MOCK_USER_TOKEN_ACCOUNT_KEYPAIR: Keypair = Keypair::new();
    pub static ref MARK_PRICE_PRECISION: f64 = f64::powf(10.0, 10.0);
    pub static ref MANTISSA_SQRT_SCALA: f64 = f64::sqrt(*MARK_PRICE_PRECISION);
    pub static ref AMM_INITIAL_QUOTE_ASSET_AMOUNT: f64 = (5.0 * f64::powf(10.0, 12.0)) * *MANTISSA_SQRT_SCALA;
    pub static ref AMM_INITIAL_BASE_ASSET_AMOUNT: f64 = (5.0 * f64::powf(10.0, 12.0)) * *MANTISSA_SQRT_SCALA;

}

pub fn create_mock_mint() -> &'static Keypair {
    let wallet = read_wallet_from_default().unwrap();
    let fake_usd_mint = &*MOCK_MINT_KEYPAIR;
    let space = Mint::LEN;
    let client = RpcClient::new_with_commitment(
        Cluster::default().url().to_string(),
        CommitmentConfig::processed(),
    );
    let min_balance_for_rent_exempt_mint = client
        .get_minimum_balance_for_rent_exemption(space.clone())
        .unwrap();
    let create_usd_mint_account_ix = sol_sys_ix::create_account(
        &wallet.pubkey(),
        &fake_usd_mint.pubkey(),
        min_balance_for_rent_exempt_mint,
        space as u64,
        &spl_token::ID,
    );
    let init_collateral_mint_ix = spl_ix::initialize_mint(
        &spl_token::ID,
        &fake_usd_mint.pubkey(),
        &wallet.pubkey(),
        None,
        6,
    )
    .unwrap();
    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[create_usd_mint_account_ix, init_collateral_mint_ix],
        Some(&wallet.pubkey()),
        &[&wallet, &fake_usd_mint],
        recent_blockhash,
    );
    client.send_and_confirm_transaction(&tx).unwrap();
    fake_usd_mint
}

pub fn create_mock_user_token_account(mint: &Keypair, amount: u64, owner: Option<&Pubkey>) -> () {
    let wallet = read_wallet_from_default().unwrap();
    let wallet_pubkey = wallet.pubkey();
    let user_token_account = &*MOCK_USER_TOKEN_ACCOUNT_KEYPAIR;
    let owner = owner.unwrap_or(&wallet_pubkey);
    let space: usize = Account::LEN;
    let client = RpcClient::new_with_commitment(
        Cluster::default().url().to_string(),
        CommitmentConfig::processed(),
    );
    let min_balance_for_rent_exempt_mint = client
        .get_minimum_balance_for_rent_exemption(space)
        .unwrap();
    let create_user_token_account_ix = sol_sys_ix::create_account(
        &owner,
        &user_token_account.pubkey(),
        min_balance_for_rent_exempt_mint,
        space as u64,
        &spl_token::ID,
    );
    let create_init_token_account_ix = spl_ix::initialize_account(
        &spl_token::ID,
        &user_token_account.pubkey(),
        &mint.pubkey(),
        &owner,
    )
    .unwrap();
    let create_mint_to_user_account_ix = spl_ix::mint_to(
        &spl_token::ID,
        &mint.pubkey(),
        &user_token_account.pubkey(),
        &owner,
        &[],
        amount,
    )
    .unwrap();
    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[
            create_user_token_account_ix,
            create_init_token_account_ix,
            create_mint_to_user_account_ix,
        ],
        Some(&owner),
        &[&wallet, &user_token_account],
        recent_blockhash,
    );
    client.send_and_confirm_transaction(&tx).unwrap();
}

pub fn mock_oracle(price: i64, expo: i32) -> Pubkey {
    let oracle_program = read_wallet_from(String::from("target/deploy/pyth-keypair.json")).unwrap();
    let payer = read_wallet_from_default().unwrap();
    let space = 3312;
    let client = RpcClient::new_with_commitment(
        Cluster::default().url().to_string(),
        CommitmentConfig::processed(),
    );
    let collateral_token_feed = Keypair::new();
    let conf: u64 = ((price / 10) * 10_i64.pow((-expo).try_into().unwrap()))
        .try_into()
        .unwrap();
    let price = price * (10_i64.pow((-expo).try_into().unwrap()));
    let initialize_oracle_ix = ix(
        &pyth::id(),
        pyth::instruction::Initialize {
            _conf: conf,
            expo,
            price,
        },
        Context {
            accounts: &pyth::accounts::Initialize {
                price: collateral_token_feed.pubkey(),
            },
            remaining_accounts: vec![],
        },
    );
    let min_balance_for_rent_exempt_mint = client
        .get_minimum_balance_for_rent_exemption(space)
        .unwrap();
    let create_collateral_token_feed_account_ix = sol_sys_ix::create_account(
        &payer.pubkey(),
        &collateral_token_feed.pubkey(),
        min_balance_for_rent_exempt_mint,
        space.try_into().unwrap(),
        &oracle_program.pubkey(),
    );
    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[
            create_collateral_token_feed_account_ix,
            initialize_oracle_ix,
        ],
        Some(&payer.pubkey()),
        &[&collateral_token_feed, &payer],
        recent_blockhash,
    );
    client.send_transaction_with_config(&tx, RpcSendTransactionConfig {
        skip_preflight: true,
        ..RpcSendTransactionConfig::default()
    }).unwrap();
    collateral_token_feed.pubkey()
}

pub fn calculate_trade_amount(collateral_amount: u64) -> u128 {
    let ONE_MANTISSA = 100000;
    let fee = ONE_MANTISSA / 1000;
    let trade_amount =
        collateral_amount * MAX_LEVERAGE * (ONE_MANTISSA - (MAX_LEVERAGE * fee)) / ONE_MANTISSA;
    trade_amount as u128
}
