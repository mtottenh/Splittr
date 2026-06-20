//! Application-layer errors.

use splittr_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    /// The command's inputs were invalid (e.g. splits don't sum to the total).
    #[error("validation: {0}")]
    Validation(String),
    /// The actor is not permitted to perform this command (e.g. not a member).
    #[error("not authorized: {0}")]
    NotAuthorized(String),
    /// A referenced entity does not exist.
    #[error("not found: {0}")]
    NotFound(String),
    /// A privileged action (device enrol/revoke) needs the identity (root) key,
    /// which is sealed; unlock it via the app lock first (#34/ADR-0005).
    #[error("identity (root) key is locked")]
    RootLocked,
    /// The storage backend failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

pub type Result<T> = std::result::Result<T, AppError>;
