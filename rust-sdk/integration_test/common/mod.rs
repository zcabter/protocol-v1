use anchor_client::Cluster;
use drift_sdk::sdk_core::util::read_wallet_from_default;
use lazy_static::lazy_static;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig, program_pack::Pack, pubkey::Pubkey, signature::Keypair,
    signer::Signer, system_instruction as sol_sys_ix, transaction::Transaction,
};
use spl_token::{
    instruction as spl_ix,
    state::{Account, Mint},
};

lazy_static! {
    pub static ref MOCK_MINT_KEYPAIR: Keypair = Keypair::new();
    pub static ref MOCK_USER_TOKEN_ACCOUNT_KEYPAIR: Keypair  = Keypair::new();
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

pub fn create_mock_user_token_account(
    mint: &Keypair,
    amount: u64,
    owner: Option<&Pubkey>,
) -> () {
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
