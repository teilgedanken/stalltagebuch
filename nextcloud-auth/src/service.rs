use crate::models::{LoginFlowInit, LoginFlowResult, NextcloudCredentials};

/// Error type for authentication operations
#[derive(Debug)]
pub enum AuthError {
    NetworkError(String),
    JsonError(String),
    TimeoutError,
    ServerError(String),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            AuthError::JsonError(msg) => write!(f, "JSON error: {}", msg),
            AuthError::TimeoutError => write!(f, "Authentication timeout"),
            AuthError::ServerError(msg) => write!(f, "Server error: {}", msg),
        }
    }
}

impl std::error::Error for AuthError {}

/// Nextcloud authentication service
pub struct NextcloudAuthService {
    server_url: String,
}

impl NextcloudAuthService {
    /// Create a new authentication service
    pub fn new(server_url: String) -> Self {
        Self { server_url }
    }

    /// Initiate the Nextcloud Login Flow v2
    pub async fn initiate_login(&self) -> Result<LoginFlowInit, AuthError> {
        let url = format!(
            "{}/index.php/login/v2",
            self.server_url.trim_end_matches('/')
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .connect_timeout(std::time::Duration::from_secs(10))
            .tcp_keepalive(std::time::Duration::from_secs(30))
            .user_agent("NextcloudAuth/0.1.0")
            .build()
            .map_err(|e| AuthError::NetworkError(format!("Client build failed: {}", e)))?;

        let response = client
            .post(&url)
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(AuthError::ServerError(format!(
                "Server returned status: {}",
                response.status()
            )));
        }

        response
            .json::<LoginFlowInit>()
            .await
            .map_err(|e| AuthError::JsonError(format!("Failed to parse response: {}", e)))
    }

    /// Poll for login completion
    /// Returns Ok(Some(credentials)) if login completed
    /// Returns Ok(None) if still waiting
    /// Returns Err on error
    pub async fn poll_login(
        &self,
        poll_url: &str,
        token: &str,
    ) -> Result<Option<NextcloudCredentials>, AuthError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .tcp_keepalive(std::time::Duration::from_secs(30))
            .user_agent("NextcloudAuth/0.1.0")
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .pool_max_idle_per_host(4)
            .build()
            .map_err(|e| AuthError::NetworkError(format!("Client build failed: {}", e)))?;

        let response = client
            .post(poll_url)
            .form(&[("token", token)])
            .header("User-Agent", "NextcloudAuth/0.1.0")
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(format!("Poll request failed: {}", e)))?;

        match response.status().as_u16() {
            200 => {
                // Login completed
                let result = response
                    .json::<LoginFlowResult>()
                    .await
                    .map_err(|e| AuthError::JsonError(format!("Failed to parse result: {}", e)))?;

                Ok(Some(NextcloudCredentials {
                    server_url: result.server,
                    username: result.login_name,
                    app_password: result.app_password,
                }))
            }
            404 => {
                // Still waiting for user
                Ok(None)
            }
            status => Err(AuthError::ServerError(format!(
                "Unexpected status code: {}",
                status
            ))),
        }
    }

    /// Complete authentication flow with polling (async)
    /// Polls up to max_attempts times with specified delay between attempts
    pub async fn authenticate_with_polling(
        &self,
        max_attempts: u32,
        poll_delay_secs: u64,
    ) -> Result<NextcloudCredentials, AuthError> {
        // Initiate flow
        let flow = self.initiate_login().await?;

        log::info!("Login flow initiated. Please visit: {}", flow.login);

        // Initial delay to allow user to open browser
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Poll for completion
        let mut consecutive_errors = 0;
        for attempt in 0..max_attempts {
            log::debug!("Polling attempt {}/{}", attempt + 1, max_attempts);

            match self.poll_login(&flow.poll.endpoint, &flow.poll.token).await {
                Ok(Some(credentials)) => {
                    log::info!("Authentication successful!");
                    return Ok(credentials);
                }
                Ok(None) => {
                    // Still waiting, continue polling
                    consecutive_errors = 0;
                    tokio::time::sleep(std::time::Duration::from_secs(poll_delay_secs)).await;
                }
                Err(e) => {
                    consecutive_errors += 1;
                    log::warn!("Poll error (attempt {}): {}", consecutive_errors, e);

                    // Use exponential backoff for errors
                    let backoff = poll_delay_secs.saturating_mul(1 << consecutive_errors.min(2));
                    let wait_time = backoff.min(30);

                    tokio::time::sleep(std::time::Duration::from_secs(wait_time)).await;
                }
            }
        }

        Err(AuthError::TimeoutError)
    }
}
