//! Cross-cutting error type.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("infrastructure error: {0}")]
    Infra(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

impl CoreError {
    #[must_use]
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    #[must_use]
    pub fn infra(msg: impl Into<String>) -> Self {
        Self::Infra(msg.into())
    }

    #[must_use]
    pub fn serialization(msg: impl Into<String>) -> Self {
        Self::Serialization(msg.into())
    }
}