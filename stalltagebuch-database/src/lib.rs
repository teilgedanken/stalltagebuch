//! Stalltagebuch Database Crate
//!
//! This crate provides database schema, migrations, and connection management
//! for the Stalltagebuch (Quail Diary) app.
//!
//! It is designed to be shared between the main app and the photo-gallery crate
//! to avoid circular dependencies.

pub mod connection;
pub mod error;
pub mod schema;

pub use connection::{get_app_directory, get_database_path, init_database, test_connection};
pub use error::DatabaseError;
pub use rusqlite::Connection;
