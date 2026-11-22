use std::fmt;
use thiserror::Error;

/// Stable error categories used across NEXORA services and SDK contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    Config,
    Protocol,
    Service,
    Audio,
    InvalidInput,
    Io,
    Internal,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::Config => "config",
            Self::Protocol => "protocol",
            Self::Service => "service",
            Self::Audio => "audio",
            Self::InvalidInput => "invalid_input",
            Self::Io => "io",
            Self::Internal => "internal",
        };
        f.write_str(text)
    }
}

/// Structured NEXORA error. Messages are static or caller-supplied context
/// that must never contain credentials or secret material.
#[derive(Debug, Error)]
pub enum NexoraError {
    #[error("configuration error: {message}")]
    Config { message: String },
    #[error("protocol error: {message}")]
    Protocol { message: String },
    #[error("service error: {message}")]
    Service { message: String },
    #[error("audio error: {message}")]
    Audio { message: String },
    #[error("invalid input: {message}")]
    InvalidInput { message: String },
    #[error("io error: {message}")]
    Io { message: String },
    #[error("internal error: {message}")]
    Internal { message: String },
}

impl NexoraError {
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }
    pub fn protocol(message: impl Into<String>) -> Self {
        Self::Protocol {
            message: message.into(),
        }
    }
    pub fn service(message: impl Into<String>) -> Self {
        Self::Service {
            message: message.into(),
        }
    }
    pub fn audio(message: impl Into<String>) -> Self {
        Self::Audio {
            message: message.into(),
        }
    }
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }
    pub fn io(message: impl Into<String>) -> Self {
        Self::Io {
            message: message.into(),
        }
    }
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Config { .. } => ErrorCode::Config,
            Self::Protocol { .. } => ErrorCode::Protocol,
            Self::Service { .. } => ErrorCode::Service,
            Self::Audio { .. } => ErrorCode::Audio,
            Self::InvalidInput { .. } => ErrorCode::InvalidInput,
            Self::Io { .. } => ErrorCode::Io,
            Self::Internal { .. } => ErrorCode::Internal,
        }
    }
}

pub type NexoraResult<T> = Result<T, NexoraError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(NexoraError::config("bad").code(), ErrorCode::Config);
        assert_eq!(NexoraError::protocol("bad").code(), ErrorCode::Protocol);
        assert_eq!(NexoraError::service("bad").code(), ErrorCode::Service);
        assert_eq!(NexoraError::audio("bad").code(), ErrorCode::Audio);
        assert_eq!(
            NexoraError::invalid_input("bad").code(),
            ErrorCode::InvalidInput
        );
        assert_eq!(NexoraError::io("bad").code(), ErrorCode::Io);
        assert_eq!(NexoraError::internal("bad").code(), ErrorCode::Internal);
    }

    #[test]
    fn display_contains_context_without_secrets() {
        let err = NexoraError::config("missing section");
        assert_eq!(err.to_string(), "configuration error: missing section");
        assert_eq!(ErrorCode::InvalidInput.to_string(), "invalid_input");
    }
}
