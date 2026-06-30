use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
pub enum RibError {
    #[error("parse error: {0}")]
    Parse(String),
    #[error("invalid range: {0}")]
    Range(String),
    #[error("invalid action: {0}")]
    Action(String),
    #[error("solver error: {0}")]
    Solver(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
}
