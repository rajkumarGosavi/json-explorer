use thiserror::Error;

#[derive(Debug, Error)]
pub enum IndexError {
    #[error("invalid JSON at byte {byte_offset} (line {line}, col {col}): {message}")]
    Syntax {
        message: String,
        byte_offset: u64,
        line: u64,
        col: u64,
    },
    #[error("file is empty or contains only whitespace")]
    Empty,
}
