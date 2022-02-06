use solana_client::client_error::ClientError;
use solana_sdk::program_error::ProgramError;
use thiserror::Error;

pub type DriftResult<T> = std::result::Result<T, DriftError>;

#[derive(Error, Debug)]
pub enum DriftError {
    #[error(transparent)]
    ClientError(#[from] ClientError),
    #[error(transparent)]
    AccountDeserializationError(#[from] ProgramError), // anchor AccountDeserialize::try_deserialize uses this error
    #[error("Account '{name}' cannot be initialized: {reason}")]
    AccountCannotBeInitialized { name: String, reason: String }, 
}
