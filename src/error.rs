use std::fmt;

/// Central error types for the Quail Diary app
#[derive(Debug)]
pub enum AppError {
    /// Database error (rusqlite)
    Database(rusqlite::Error),
    /// Filesystem error
    Filesystem(std::io::Error),
    /// Validation error (e.g. invalid inputs)
    Validation(String),
    /// Resource not found
    NotFound(String),
    /// Permission denied (e.g. camera)
    PermissionDenied(String),
    /// Image processing error
    ImageProcessing(String),
    /// General error
    #[allow(dead_code)]
    Other(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::Database(e) => write!(f, "Database error: {}", e),
            AppError::Filesystem(e) => write!(f, "Filesystem error: {}", e),
            AppError::Validation(msg) => write!(f, "Validation error: {}", msg),
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            AppError::ImageProcessing(msg) => write!(f, "Image processing error: {}", msg),
            AppError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for AppError {}

// Conversions from other error types
impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Database(e)
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Filesystem(e)
    }
}

/// User-friendly error messages for UI (can be translated via i18n)
impl AppError {
    #[allow(dead_code)]
    pub fn user_message(&self) -> String {
        match self {
            AppError::Database(_) => "A database error occurred. Please try again.".to_string(),
            AppError::Filesystem(_) => {
                "Error accessing files. Please check app permissions.".to_string()
            }
            AppError::Validation(msg) => msg.clone(),
            AppError::NotFound(msg) => format!("{} was not found.", msg),
            AppError::PermissionDenied(msg) => format!("Permission required: {}", msg),
            AppError::ImageProcessing(_) => "Error processing image.".to_string(),
            AppError::Other(msg) => msg.clone(),
        }
    }
}
