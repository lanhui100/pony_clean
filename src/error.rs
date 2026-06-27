use thiserror::Error;

#[derive(Error, Debug)]
pub enum PonyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Process error: {0}")]
    Process(String),

    #[error("Cleanup error: {0}")]
    Cleanup(String),
}

pub type Result<T> = std::result::Result<T, PonyError>;
