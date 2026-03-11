use std::fmt;

/// Central error types for the Quail Diary app
#[derive(Debug)]
pub enum AppError {
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

/// Extended error types including platform-specific errors
#[derive(Debug)]
pub enum StalltagebuchError {
    /// JNI/Android error
    JniError(String),
    /// Filesystem error
    Filesystem(std::io::Error),
    /// Validation error
    Validation(String),
    /// Resource not found
    NotFound(String),
    /// Permission denied
    PermissionDenied(String),
    /// Image processing error
    ImageProcessing(String),
    /// General error
    Other(String),
}

impl fmt::Display for StalltagebuchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StalltagebuchError::JniError(msg) => write!(f, "JNI error: {}", msg),
            StalltagebuchError::Filesystem(e) => write!(f, "Filesystem error: {}", e),
            StalltagebuchError::Validation(msg) => write!(f, "Validation error: {}", msg),
            StalltagebuchError::NotFound(msg) => write!(f, "Not found: {}", msg),
            StalltagebuchError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            StalltagebuchError::ImageProcessing(msg) => {
                write!(f, "Image processing error: {}", msg)
            }
            StalltagebuchError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for StalltagebuchError {}

impl From<std::io::Error> for StalltagebuchError {
    fn from(e: std::io::Error) -> Self {
        StalltagebuchError::Filesystem(e)
    }
}

impl From<AppError> for StalltagebuchError {
    fn from(e: AppError) -> Self {
        match e {
            AppError::Filesystem(io) => StalltagebuchError::Filesystem(io),
            AppError::Validation(msg) => StalltagebuchError::Validation(msg),
            AppError::NotFound(msg) => StalltagebuchError::NotFound(msg),
            AppError::PermissionDenied(msg) => StalltagebuchError::PermissionDenied(msg),
            AppError::ImageProcessing(msg) => StalltagebuchError::ImageProcessing(msg),
            AppError::Other(msg) => StalltagebuchError::Other(msg),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
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

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Filesystem(e)
    }
}

#[cfg(target_os = "android")]
impl From<jni::errors::Error> for AppError {
    fn from(e: jni::errors::Error) -> Self {
        AppError::PermissionDenied(format!("JNI error: {}", e))
    }
}

#[cfg(target_os = "android")]
impl From<jni::errors::Error> for StalltagebuchError {
    fn from(e: jni::errors::Error) -> Self {
        StalltagebuchError::JniError(format!("JNI error: {}", e))
    }
}

/// User-friendly error messages for UI (can be translated via i18n)
impl AppError {
    #[allow(dead_code)]
    pub fn user_message(&self) -> String {
        match self {
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
