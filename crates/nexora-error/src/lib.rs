use std::error::Error;
use std::fmt;

use thiserror::Error;

const REDACTED_CONTEXT: &str = "<dynamic context omitted>";

/// Display-safe error context.
///
/// The inner value is private so downstream crates cannot bypass redaction by
/// constructing a [`NexoraError`] variant directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeErrorMessage(String);

impl SafeErrorMessage {
    fn from_static(message: &'static str) -> Self {
        Self(message.to_owned())
    }
}

impl fmt::Display for SafeErrorMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl AsRef<str> for SafeErrorMessage {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&'static str> for SafeErrorMessage {
    fn from(message: &'static str) -> Self {
        Self::from_static(message)
    }
}

/// A boxed error that can be preserved as the source of a [`NexoraError`].
pub type NexoraSource = Box<dyn Error + Send + Sync + 'static>;

/// The result type used across NEXORA crates.
pub type NexoraResult<T> = Result<T, NexoraError>;

mod sealed {
    pub trait Sealed {}
}

impl sealed::Sealed for &'static str {}
impl sealed::Sealed for String {}
impl sealed::Sealed for &String {}

/// Converts caller-provided context into a message safe for error output.
///
/// Static strings are retained. Owned runtime strings are redacted so secrets,
/// tokens, and other sensitive values cannot enter an error message through
/// formatting.
#[allow(private_bounds)]
pub trait IntoErrorMessage: sealed::Sealed {
    /// Converts the value into a display-safe message.
    fn into_error_message(self) -> SafeErrorMessage;
}

impl IntoErrorMessage for &'static str {
    fn into_error_message(self) -> SafeErrorMessage {
        SafeErrorMessage::from_static(self)
    }
}

impl IntoErrorMessage for String {
    fn into_error_message(self) -> SafeErrorMessage {
        SafeErrorMessage(REDACTED_CONTEXT.to_owned())
    }
}

impl IntoErrorMessage for &String {
    fn into_error_message(self) -> SafeErrorMessage {
        SafeErrorMessage(REDACTED_CONTEXT.to_owned())
    }
}

/// Errors produced by NEXORA components.
///
/// Static context is safe to display. Runtime-owned context is redacted, and
/// underlying source errors remain available through [`Error::source`] without
/// being interpolated into the outer error's `Display` or `Debug` output.
#[derive(Error)]
pub enum NexoraError {
    /// A configuration value or source is invalid.
    #[error("configuration error: {message}")]
    Config {
        /// Display-safe description of the failed operation.
        message: SafeErrorMessage,
        /// Optional underlying error.
        #[source]
        source: Option<NexoraSource>,
    },
    /// A protocol message or exchange is invalid.
    #[error("protocol error: {message}")]
    Protocol {
        /// Display-safe description of the failed operation.
        message: SafeErrorMessage,
        /// Optional underlying error.
        #[source]
        source: Option<NexoraSource>,
    },
    /// A service operation failed.
    #[error("service error: {message}")]
    Service {
        /// Display-safe description of the failed operation.
        message: SafeErrorMessage,
        /// Optional underlying error.
        #[source]
        source: Option<NexoraSource>,
    },
    /// An audio operation failed.
    #[error("audio error: {message}")]
    Audio {
        /// Display-safe description of the failed operation.
        message: SafeErrorMessage,
        /// Optional underlying error.
        #[source]
        source: Option<NexoraSource>,
    },
    /// An I/O operation failed.
    #[error("io error: {message}")]
    Io {
        /// Display-safe description of the failed operation.
        message: SafeErrorMessage,
        /// Optional underlying error.
        #[source]
        source: Option<NexoraSource>,
    },
    /// Input supplied by a caller is invalid.
    #[error("invalid input: {message}")]
    InvalidInput {
        /// Display-safe description of the failed operation.
        message: SafeErrorMessage,
        /// Optional underlying error.
        #[source]
        source: Option<NexoraSource>,
    },
    /// An unexpected internal failure occurred.
    #[error("internal error: {message}")]
    Internal {
        /// Display-safe description of the failed operation.
        message: SafeErrorMessage,
        /// Optional underlying error.
        #[source]
        source: Option<NexoraSource>,
    },
}

impl NexoraError {
    /// Creates a configuration error.
    pub fn config(message: impl IntoErrorMessage) -> Self {
        Self::Config {
            message: message.into_error_message(),
            source: None,
        }
    }

    /// Creates a configuration error with an underlying source.
    pub fn config_with_source(
        message: impl IntoErrorMessage,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::Config {
            message: message.into_error_message(),
            source: Some(Box::new(source)),
        }
    }

    /// Creates a configuration error and attaches a source to it.
    pub fn config_with_context(
        message: impl IntoErrorMessage,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::config_with_source(message, source)
    }

    /// Creates a protocol error.
    pub fn protocol(message: impl IntoErrorMessage) -> Self {
        Self::Protocol {
            message: message.into_error_message(),
            source: None,
        }
    }

    /// Creates a protocol error with an underlying source.
    pub fn protocol_with_source(
        message: impl IntoErrorMessage,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::Protocol {
            message: message.into_error_message(),
            source: Some(Box::new(source)),
        }
    }

    /// Creates a service error.
    pub fn service(message: impl IntoErrorMessage) -> Self {
        Self::Service {
            message: message.into_error_message(),
            source: None,
        }
    }

    /// Creates a service error with an underlying source.
    pub fn service_with_source(
        message: impl IntoErrorMessage,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::Service {
            message: message.into_error_message(),
            source: Some(Box::new(source)),
        }
    }

    /// Creates an audio error.
    pub fn audio(message: impl IntoErrorMessage) -> Self {
        Self::Audio {
            message: message.into_error_message(),
            source: None,
        }
    }

    /// Creates an audio error with an underlying source.
    pub fn audio_with_source(
        message: impl IntoErrorMessage,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::Audio {
            message: message.into_error_message(),
            source: Some(Box::new(source)),
        }
    }

    /// Creates an I/O error.
    pub fn io(message: impl IntoErrorMessage) -> Self {
        Self::Io {
            message: message.into_error_message(),
            source: None,
        }
    }

    /// Creates an I/O error with an underlying source.
    pub fn io_with_source(
        message: impl IntoErrorMessage,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::Io {
            message: message.into_error_message(),
            source: Some(Box::new(source)),
        }
    }

    /// Creates an invalid-input error.
    pub fn invalid_input(message: impl IntoErrorMessage) -> Self {
        Self::InvalidInput {
            message: message.into_error_message(),
            source: None,
        }
    }

    /// Creates an invalid-input error with an underlying source.
    pub fn invalid_input_with_source(
        message: impl IntoErrorMessage,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::InvalidInput {
            message: message.into_error_message(),
            source: Some(Box::new(source)),
        }
    }

    /// Creates an internal error.
    pub fn internal(message: impl IntoErrorMessage) -> Self {
        Self::Internal {
            message: message.into_error_message(),
            source: None,
        }
    }

    /// Creates an internal error with an underlying source.
    pub fn internal_with_source(
        message: impl IntoErrorMessage,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::Internal {
            message: message.into_error_message(),
            source: Some(Box::new(source)),
        }
    }

    /// Attaches additional static context while preserving the original source
    /// chain.
    pub fn with_context(self, context: &'static str) -> Self {
        match self {
            Self::Config {
                message: previous_message,
                source: previous_source,
            } => Self::Config {
                message: SafeErrorMessage::from_static(context),
                source: Some(Box::new(Self::Config {
                    message: previous_message,
                    source: previous_source,
                })),
            },
            Self::Protocol {
                message: previous_message,
                source: previous_source,
            } => Self::Protocol {
                message: SafeErrorMessage::from_static(context),
                source: Some(Box::new(Self::Protocol {
                    message: previous_message,
                    source: previous_source,
                })),
            },
            Self::Service {
                message: previous_message,
                source: previous_source,
            } => Self::Service {
                message: SafeErrorMessage::from_static(context),
                source: Some(Box::new(Self::Service {
                    message: previous_message,
                    source: previous_source,
                })),
            },
            Self::Audio {
                message: previous_message,
                source: previous_source,
            } => Self::Audio {
                message: SafeErrorMessage::from_static(context),
                source: Some(Box::new(Self::Audio {
                    message: previous_message,
                    source: previous_source,
                })),
            },
            Self::Io {
                message: previous_message,
                source: previous_source,
            } => Self::Io {
                message: SafeErrorMessage::from_static(context),
                source: Some(Box::new(Self::Io {
                    message: previous_message,
                    source: previous_source,
                })),
            },
            Self::InvalidInput {
                message: previous_message,
                source: previous_source,
            } => Self::InvalidInput {
                message: SafeErrorMessage::from_static(context),
                source: Some(Box::new(Self::InvalidInput {
                    message: previous_message,
                    source: previous_source,
                })),
            },
            Self::Internal {
                message: previous_message,
                source: previous_source,
            } => Self::Internal {
                message: SafeErrorMessage::from_static(context),
                source: Some(Box::new(Self::Internal {
                    message: previous_message,
                    source: previous_source,
                })),
            },
        }
    }

    /// Returns the stable error code for this error.
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Config { .. } => ErrorCode::Config,
            Self::Protocol { .. } => ErrorCode::Protocol,
            Self::Service { .. } => ErrorCode::Service,
            Self::Audio { .. } => ErrorCode::Audio,
            Self::Io { .. } => ErrorCode::Io,
            Self::InvalidInput { .. } => ErrorCode::InvalidInput,
            Self::Internal { .. } => ErrorCode::Internal,
        }
    }
}

impl fmt::Debug for NexoraError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (variant, has_source) = match self {
            Self::Config { source, .. } => ("Config", source.is_some()),
            Self::Protocol { source, .. } => ("Protocol", source.is_some()),
            Self::Service { source, .. } => ("Service", source.is_some()),
            Self::Audio { source, .. } => ("Audio", source.is_some()),
            Self::Io { source, .. } => ("Io", source.is_some()),
            Self::InvalidInput { source, .. } => ("InvalidInput", source.is_some()),
            Self::Internal { source, .. } => ("Internal", source.is_some()),
        };

        formatter
            .debug_struct(variant)
            .field("message", &REDACTED_CONTEXT)
            .field("has_source", &has_source)
            .finish()
    }
}

impl From<std::io::Error> for NexoraError {
    fn from(error: std::io::Error) -> Self {
        Self::io_with_source("I/O operation failed", error)
    }
}

/// Stable, machine-readable category for a [`NexoraError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    /// Configuration failure.
    Config,
    /// Protocol failure.
    Protocol,
    /// Service failure.
    Service,
    /// Audio failure.
    Audio,
    /// Invalid caller input.
    InvalidInput,
    /// I/O failure.
    Io,
    /// Unexpected internal failure.
    Internal,
}

impl ErrorCode {
    /// Returns the stable textual representation of the code.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Protocol => "protocol",
            Self::Service => "service",
            Self::Audio => "audio",
            Self::InvalidInput => "invalid_input",
            Self::Io => "io",
            Self::Internal => "internal",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<&NexoraError> for ErrorCode {
    fn from(error: &NexoraError) -> Self {
        error.code()
    }
}

impl From<NexoraError> for ErrorCode {
    fn from(error: NexoraError) -> Self {
        error.code()
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, ErrorCode, NexoraError};

    #[test]
    fn display_uses_the_error_category_and_static_context() {
        assert_eq!(
            NexoraError::config("configuration source is unavailable").to_string(),
            "configuration error: configuration source is unavailable"
        );
        assert_eq!(
            NexoraError::protocol("message framing is invalid").to_string(),
            "protocol error: message framing is invalid"
        );
        assert_eq!(
            NexoraError::service("service request failed").to_string(),
            "service error: service request failed"
        );
        assert_eq!(
            NexoraError::audio("audio frame is invalid").to_string(),
            "audio error: audio frame is invalid"
        );
        assert_eq!(
            NexoraError::io("file read failed").to_string(),
            "io error: file read failed"
        );
        assert_eq!(
            NexoraError::invalid_input("request field is invalid").to_string(),
            "invalid input: request field is invalid"
        );
        assert_eq!(
            NexoraError::internal("unexpected state detected").to_string(),
            "internal error: unexpected state detected"
        );
    }

    #[test]
    fn source_chain_preserves_the_underlying_error() {
        let source = std::io::Error::new(std::io::ErrorKind::NotFound, "source detail");
        let error = NexoraError::config_with_source("configuration load failed", source);

        let source = error.source().expect("the error should have a source");
        let io_error = source
            .downcast_ref::<std::io::Error>()
            .expect("the source should be an I/O error");
        assert_eq!(io_error.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn attached_context_preserves_the_previous_error() {
        let error = NexoraError::invalid_input("input is invalid")
            .with_context("request validation failed");

        assert_eq!(
            error.to_string(),
            "invalid input: request validation failed"
        );

        let previous = error
            .source()
            .expect("the contextual error should have a source")
            .downcast_ref::<NexoraError>()
            .expect("the source should be a NEXORA error");
        assert_eq!(previous.to_string(), "invalid input: input is invalid");
    }

    #[test]
    fn attached_context_preserves_an_existing_source_chain() {
        let source = std::io::Error::new(std::io::ErrorKind::InvalidData, "source detail");
        let error = NexoraError::config_with_source("configuration load failed", source)
            .with_context("service startup failed");

        let previous = error
            .source()
            .expect("the contextual error should have a source")
            .downcast_ref::<NexoraError>()
            .expect("the source should be a NEXORA error");
        let original = previous
            .source()
            .expect("the previous error should preserve its source")
            .downcast_ref::<std::io::Error>()
            .expect("the original source should be an I/O error");

        assert_eq!(original.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn error_codes_map_to_the_expected_categories() {
        assert_eq!(NexoraError::config("failed").code(), ErrorCode::Config);
        assert_eq!(NexoraError::protocol("failed").code(), ErrorCode::Protocol);
        assert_eq!(NexoraError::service("failed").code(), ErrorCode::Service);
        assert_eq!(NexoraError::audio("failed").code(), ErrorCode::Audio);
        assert_eq!(
            NexoraError::invalid_input("failed").code(),
            ErrorCode::InvalidInput
        );
        assert_eq!(NexoraError::io("failed").code(), ErrorCode::Io);
        assert_eq!(NexoraError::internal("failed").code(), ErrorCode::Internal);

        assert_eq!(ErrorCode::Config.to_string(), "config");
        assert_eq!(ErrorCode::Protocol.to_string(), "protocol");
        assert_eq!(ErrorCode::Service.to_string(), "service");
        assert_eq!(ErrorCode::Audio.to_string(), "audio");
        assert_eq!(ErrorCode::InvalidInput.to_string(), "invalid_input");
        assert_eq!(ErrorCode::Io.to_string(), "io");
        assert_eq!(ErrorCode::Internal.to_string(), "internal");
    }

    #[test]
    fn runtime_context_and_source_details_are_redacted() {
        let runtime_message = String::from("token=secret-value");
        let error = NexoraError::internal(runtime_message);

        assert!(!error.to_string().contains("secret-value"));
        assert!(!format!("{error:?}").contains("secret-value"));
        assert!(error.source().is_none());
    }

    #[test]
    fn outer_display_and_debug_do_not_expose_source_details() {
        #[derive(Debug, Error)]
        #[error("sensitive source detail")]
        struct SensitiveSource;

        let error = NexoraError::internal_with_source("operation failed", SensitiveSource);

        assert!(!error.to_string().contains("sensitive source detail"));
        assert!(!format!("{error:?}").contains("sensitive source detail"));
        assert!(error.source().is_some());
    }
}
