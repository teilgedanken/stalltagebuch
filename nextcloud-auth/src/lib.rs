//! # Nextcloud Auth
//!
//! A reusable Nextcloud authentication library implementing Login Flow v2.
//!
//! This crate provides:
//! - Nextcloud Login Flow v2 implementation
//! - Dioxus UI components for authentication
//! - Async polling for login completion
//! - Credential management through callbacks
//!
//! ## Separation of Concerns
//!
//! This crate focuses solely on authentication. It does **not**:
//! - Store credentials (handled by the application)
//! - Set up WebDAV clients (handled by the application)
//! - Manage sync settings (handled by the application)
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use nextcloud_auth::{NextcloudAuthService, NextcloudAuthComponent};
//!
//! // Programmatic usage
//! let service = NextcloudAuthService::new("https://cloud.example.com".to_string());
//! let credentials = service.authenticate_with_polling(60, 5).await?;
//!
//! // UI component usage
//! NextcloudAuthComponent {
//!     server_url: "https://cloud.example.com".to_string(),
//!     on_success: move |creds| {
//!         // Save credentials and set up sync
//!     },
//!     on_error: move |error| {
//!         // Handle error
//!     },
//! }
//! ```

pub mod component;
pub mod models;
pub mod service;

pub use component::{AuthLabels, NextcloudAuthComponent, NextcloudAuthProps};
pub use models::{LoginFlowInit, LoginFlowResult, LoginState, NextcloudCredentials, PollEndpoint};
pub use service::{AuthError, NextcloudAuthService};
