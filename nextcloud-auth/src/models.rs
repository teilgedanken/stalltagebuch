use serde::{Deserialize, Serialize};

/// Response from the Nextcloud login flow initialization endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginFlowInit {
    pub poll: PollEndpoint,
    pub login: String,
}

/// Polling endpoint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollEndpoint {
    pub token: String,
    pub endpoint: String,
}

/// Response from successful login polling
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginFlowResult {
    pub server: String,
    pub login_name: String,
    pub app_password: String,
}

/// Credentials returned by successful authentication
#[derive(Debug, Clone, PartialEq)]
pub struct NextcloudCredentials {
    pub server_url: String,
    pub username: String,
    pub app_password: String,
}

/// State of the login process
#[derive(Debug, Clone, PartialEq)]
pub enum LoginState {
    /// Not started
    NotStarted,
    /// Initiating the login flow
    InitiatingFlow,
    /// Waiting for user to complete login in browser
    WaitingForUser {
        login_url: String,
        poll_url: String,
        token: String,
    },
    /// Login successful
    Success(NextcloudCredentials),
    /// Login failed with error message
    Error(String),
}
