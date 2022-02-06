use anchor_client::Cluster;
use drift_sdk::sdk_core::util::read_wallet_from_default;
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::system_instruction as sol_sys_ix;
use solana_sdk::transaction::Transaction;
use solana_sdk::{program_pack::Pack, signature::Keypair, signer::Signer};
use spl_token::instruction as spl_ix;
use spl_token::state::{Account, Mint};

pub fn create_mock_mint() -> Keypair {
    let wallet = read_wallet_from_default().unwrap();
    let fake_usd_mint = Keypair::new();
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
) -> Keypair {
    let wallet = read_wallet_from_default().unwrap();
    let wallet_pubkey = wallet.pubkey();
    let owner = owner.unwrap_or(&wallet_pubkey);
    let user_token_account = Keypair::new();
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
    user_token_account
}
