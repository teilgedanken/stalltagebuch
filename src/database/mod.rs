//! Database module - re-exports from stalltagebuch-database crate
//!
//! This module provides backward compatibility by re-exporting the database
//! functionality from the standalone stalltagebuch-database crate, with app-specific
//! schema migrations.

pub mod schema;

// Re-export from the database crate (except init_database which we override)
#[allow(unused_imports)]
pub use stalltagebuch_database::{
    Connection, DatabaseError, get_app_directory, get_database_path, test_connection,
};

// Convert DatabaseError to AppError for compatibility
use crate::error::AppError;

/// Initialize database with complete schema including app-specific migrations
pub fn init_database() -> Result<Connection, DatabaseError> {
    // First initialize core database
    let conn = stalltagebuch_database::init_database()?;

    // Then apply app-specific schema migrations
    crate::database::schema::init_schema(&conn).map_err(|e| DatabaseError::Sqlite(e))?;

    Ok(conn)
}

impl From<stalltagebuch_database::DatabaseError> for AppError {
    fn from(err: stalltagebuch_database::DatabaseError) -> Self {
        match err {
            stalltagebuch_database::DatabaseError::Sqlite(e) => AppError::Database(e),
            stalltagebuch_database::DatabaseError::Io(e) => AppError::Filesystem(e),
            stalltagebuch_database::DatabaseError::Jni(msg) => AppError::Other(msg),
            stalltagebuch_database::DatabaseError::Other(msg) => AppError::Other(msg),
        }
    }
}
