use thiserror::Error;

#[derive(Debug, Error)]
pub enum F1Error {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON parsing failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Base64 decoding failed: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("I/O operation failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("Timestamp parsing failed: {0}")]
    TimeParse(#[from] chrono::ParseError),

    #[error("Unexpected data format: {0}")]
    UnexpectedFormat(String),

    #[error("Bincode serialization/deserialization failed: {0}")]
    Bincode(String),
}
