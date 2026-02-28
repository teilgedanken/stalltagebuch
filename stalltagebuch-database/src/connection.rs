//! Database connection management
//!
//! Provides platform-specific database path resolution and connection initialization.

use crate::error::DatabaseError;
use crate::schema;
use rusqlite::Connection;
use std::path::PathBuf;

#[cfg(target_os = "android")]
use jni::objects::JObject;
#[cfg(target_os = "android")]
use jni::JNIEnv;
#[cfg(target_os = "android")]
use ndk_context::android_context;

/// Returns the app directory (for photos etc.)
#[cfg(target_os = "android")]
pub fn get_app_directory() -> Option<PathBuf> {
    android_files_dir().ok()
}

#[cfg(not(target_os = "android"))]
pub fn get_app_directory() -> Option<PathBuf> {
    std::env::current_dir().ok()
}

/// Returns the path to the database file
pub fn get_database_path() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        android_files_dir()
            .unwrap_or_else(|_| PathBuf::from("/data/local/tmp/stalltagebuch"))
            .join("stalltagebuch.db")
    }

    #[cfg(not(target_os = "android"))]
    {
        PathBuf::from("./data/stalltagebuch.db")
    }
}

#[cfg(target_os = "android")]
fn android_files_dir() -> Result<PathBuf, DatabaseError> {
    use jni::JavaVM;

    let vm_ptr = android_context().vm() as *mut jni::sys::JavaVM;

    let vm = unsafe { JavaVM::from_raw(vm_ptr) }
        .map_err(|e| DatabaseError::Jni(format!("JavaVM creation failed: {}", e)))?;

    let mut env = vm
        .attach_current_thread()
        .map_err(|e| DatabaseError::Jni(format!("Failed to attach thread: {}", e)))?;

    let context_ptr = android_context().context();
    let context = unsafe { JObject::from_raw(context_ptr as jni::sys::jobject) };

    get_files_dir(&mut env, &context)
}

#[cfg(target_os = "android")]
fn get_files_dir(env: &mut JNIEnv, context: &JObject) -> Result<PathBuf, DatabaseError> {
    let file = env
        .call_method(context, "getFilesDir", "()Ljava/io/File;", &[])
        .map_err(|e| DatabaseError::Jni(format!("getFilesDir failed: {}", e)))?;

    let file_obj = file
        .l()
        .map_err(|e| DatabaseError::Jni(format!("Failed to get file object: {}", e)))?;

    let path_jstring = env
        .call_method(file_obj, "getAbsolutePath", "()Ljava/lang/String;", &[])
        .map_err(|e| DatabaseError::Jni(format!("getAbsolutePath failed: {}", e)))?;

    let path_obj = path_jstring
        .l()
        .map_err(|e| DatabaseError::Jni(format!("Failed to get path object: {}", e)))?;

    let path_str: String = env
        .get_string(&path_obj.into())
        .map_err(|e| DatabaseError::Jni(format!("Failed to get string: {}", e)))?
        .into();

    Ok(PathBuf::from(path_str))
}

/// Initializes the database with complete schema
pub fn init_database() -> Result<Connection, DatabaseError> {
    let db_path = get_database_path();

    // Ensure directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(&db_path)?;

    // Initialize schema (triggers are created inside init_schema now)
    schema::init_schema(&conn)?;

    Ok(conn)
}

/// Tests the database connection
#[allow(dead_code)]
pub fn test_connection() -> Result<(), DatabaseError> {
    let conn = init_database()?;

    // Simple query to test
    let count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
        [],
        |row| row.get(0),
    )?;

    if count < 3 {
        return Err(DatabaseError::Other("Schema incomplete".into()));
    }

    Ok(())
}
