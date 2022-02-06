pub mod account;
pub mod admin;
pub mod error;
pub mod user;
pub mod util;

use solana_sdk::{
    instruction::Instruction, pubkey::Pubkey, signature::Signature, signer::Signer,
    transaction::Transaction,
};

use crate::sdk_core::{error::DriftResult, util::DriftRpcClient};

pub trait ClearingHouse {
    fn program_id(&self) -> Pubkey;

    fn wallet(&self) -> &dyn Signer;

    fn client(&self) -> &DriftRpcClient;

    fn send_tx(
        &self,
        additional_signers: Vec<&dyn Signer>,
        ixs: &[Instruction],
    ) -> DriftResult<Signature> {
        let mut signers = vec![self.wallet()];
        signers.extend_from_slice(&additional_signers);
        let tx = {
            let hash = self.client().c.get_latest_blockhash()?;
            Transaction::new_signed_with_payer(&ixs, Some(&self.wallet().pubkey()), &signers, hash)
        };
        Ok(self.client().c.send_and_confirm_transaction(&tx)?)
    }

    fn create_account_ix(&self, space: usize, signer: &dyn Signer) -> Instruction {
        let min_balance_for_rent_exempt_mint = self
            .client()
            .c
            .get_minimum_balance_for_rent_exemption(space.clone())
            .unwrap();
        solana_sdk::system_instruction::create_account(
            &self.wallet().pubkey(),
            &signer.pubkey(),
            min_balance_for_rent_exempt_mint,
            space as u64,
            &self.program_id(),
        )
    }

    fn get_state_pubkey(&self) -> Pubkey {
        Pubkey::find_program_address(&["clearing_house".as_bytes()], &self.program_id()).0
    }
}
