//! Error type for the storage layer.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    /// An op failed to (de)serialize to/from its on-disk form.
    #[error("serialization error: {0}")]
    Serialize(String),
    /// The underlying storage backend reported an error.
    #[error("storage backend error: {0}")]
    Backend(String),
    /// A stored op could not be decrypted — wrong key or tampered data (#22).
    #[error("decryption failed: wrong key or corrupted data")]
    Decrypt,
}

pub type Result<T> = std::result::Result<T, StoreError>;

impl From<postcard::Error> for StoreError {
    fn from(e: postcard::Error) -> Self {
        StoreError::Serialize(e.to_string())
    }
}

/// Helper to map any backend error into [`StoreError::Backend`].
pub(crate) fn backend<E: std::fmt::Display>(e: E) -> StoreError {
    StoreError::Backend(e.to_string())
}
