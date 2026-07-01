//! Cross-cutting error type.

use crate::event_store::EventStoreError;
use thiserror::Error;

impl From<EventStoreError> for CoreError {
    fn from(e: EventStoreError) -> Self {
        Self::infra(e.to_string())
    }
}

impl From<crate::driver::DriverError> for CoreError {
    fn from(e: crate::driver::DriverError) -> Self {
        Self::infra(e.to_string())
    }
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("infrastructure error: {0}")]
    Infra(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("agent error: {0}")]
    Agent(String),
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

    #[must_use]
    pub fn agent(msg: impl Into<String>) -> Self {
        Self::Agent(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_error() {
        let err = CoreError::config("missing key");
        assert_eq!(err.to_string(), "configuration error: missing key");
    }

    #[test]
    fn infra_error() {
        let err = CoreError::infra("connection refused");
        assert_eq!(err.to_string(), "infrastructure error: connection refused");
    }

    #[test]
    fn serialization_error() {
        let err = CoreError::serialization("invalid format");
        assert_eq!(err.to_string(), "serialization error: invalid format");
    }
}