//! Error types for the database crate

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JNI error: {0}")]
    Jni(String),

    #[error("Other error: {0}")]
    Other(String),
}
