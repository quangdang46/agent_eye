use thiserror::Error;

pub type Result<T> = std::result::Result<T, AeError>;

#[derive(Error, Debug)]
pub enum AeError {
    #[error("decode failed: {0}")]
    Decode(String),

    #[error("invalid dimensions: {0}")]
    InvalidDimensions(String),

    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("resource limit exceeded: {0}")]
    ResourceLimit(String),

    #[error("rendering failed: {0}")]
    Rendering(String),

    #[error("serialization failed: {0}")]
    Serialization(String),
}

pub fn decode_failed(msg: impl Into<String>) -> AeError {
    AeError::Decode(msg.into())
}

pub fn invalid_dimensions(msg: impl Into<String>) -> AeError {
    AeError::InvalidDimensions(msg.into())
}

pub fn unsupported_format(msg: impl Into<String>) -> AeError {
    AeError::UnsupportedFormat(msg.into())
}

pub fn resource_limit(msg: impl Into<String>) -> AeError {
    AeError::ResourceLimit(msg.into())
}

pub fn rendering(msg: impl Into<String>) -> AeError {
    AeError::Rendering(msg.into())
}

pub fn serialization(msg: impl Into<String>) -> AeError {
    AeError::Serialization(msg.into())
}
