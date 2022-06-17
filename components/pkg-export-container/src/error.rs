use base64::DecodeError;
use rusoto_core::RusotoError;
use rusoto_ecr::GetAuthorizationTokenError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Base64DecodeError(DecodeError),
    #[error("Invalid registry type: {0}")]
    InvalidRegistryType(String),
    #[error("No ECR Tokens returned")]
    NoECRTokensReturned,
    #[error(transparent)]
    TokenFetchFailed(RusotoError<GetAuthorizationTokenError>),
    #[error("A primary service package could not be determined from: {0:?}. At least one \
             package with a run hook must be provided.")]
    PrimaryServicePackageNotFound(Vec<String>),
}
