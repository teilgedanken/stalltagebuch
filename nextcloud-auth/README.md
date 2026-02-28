# Nextcloud Auth

A reusable Nextcloud authentication library implementing Login Flow v2.

## Features

- **Nextcloud Login Flow v2**: OAuth-like authentication flow
- **Dioxus UI components**: Ready-to-use authentication component
- **Async polling**: Efficient polling for login completion
- **Credential callbacks**: Returns credentials via callback, leaving storage to the application
- **Reusable**: Can be used by multiple applications

## Separation of Concerns

This crate focuses solely on authentication. It does **not**:

- Store credentials (handled by the application)
- Set up WebDAV clients (handled by the application)
- Manage sync settings (handled by the application)

## Usage

### Programmatic Usage

```rust
use nextcloud_auth::NextcloudAuthService;

let service = NextcloudAuthService::new("https://cloud.example.com".to_string());

// Authenticate with automatic polling (60 attempts, 5 seconds between attempts)
let credentials = service.authenticate_with_polling(60, 5).await?;

println!("Server: {}", credentials.server_url);
println!("Username: {}", credentials.username);
println!("App Password: {}", credentials.app_password);
```

### UI Component Usage

```rust
use nextcloud_auth::{NextcloudAuthComponent, AuthLabels};
use dioxus::prelude::*;

fn MyAuthScreen() -> Element {
    rsx! {
        NextcloudAuthComponent {
            server_url: "https://cloud.example.com".to_string(),
            on_success: move |creds| {
                // Save credentials and set up sync
                log::info!("Authenticated as: {}", creds.username);
            },
            on_error: move |error| {
                // Handle authentication error
                log::error!("Auth failed: {}", error);
            },
            labels: Some(AuthLabels {
                login_button: "🔐 Login to Nextcloud".to_string(),
                connecting: "🔄 Connecting...".to_string(),
                waiting: "Waiting for login...".to_string(),
                // ... other labels
                ..Default::default()
            }),
        }
    }
}
```

### Custom Labels (i18n)

The authentication component supports custom labels for internationalization:

```rust
use nextcloud_auth::AuthLabels;

let german_labels = AuthLabels {
    login_button: "🔐 Bei Nextcloud anmelden".to_string(),
    connecting: "🔄 Verbinde...".to_string(),
    waiting: "Warte auf Anmeldung...".to_string(),
    polling_background: "Polling im Hintergrund...".to_string(),
    instructions: "Bitte klicken Sie auf den Button, um Ihren Browser zu öffnen und sich anzumelden.".to_string(),
    open_browser: "🌐 Browser öffnen zum Anmelden".to_string(),
    login_success: "✅ Anmeldung erfolgreich!".to_string(),
    error_title: "❌ Anmeldefehler".to_string(),
    retry_button: "🔄 Erneut versuchen".to_string(),
    info_title: "ℹ️ Wie die Anmeldung funktioniert".to_string(),
    step1: "Klicken Sie auf den Anmelde-Button".to_string(),
    step2: "Öffnen Sie den Browser-Link".to_string(),
    step3: "Melden Sie sich bei Ihrer Nextcloud-Instanz an".to_string(),
    step4: "Autorisieren Sie den Zugriff".to_string(),
    step5: "Kehren Sie zur App zurück".to_string(),
};
```

## How It Works

### Login Flow v2

1. **Initiate**: The app sends a POST request to `/index.php/login/v2` on the Nextcloud server
2. **Receive**: Server responds with a login URL and polling endpoint
3. **User Login**: User opens the login URL in their browser and authenticates
4. **Poll**: App polls the endpoint until the user completes authentication
5. **Credentials**: Server returns server URL, username, and app password

### Architecture

- `models.rs`: Data structures (LoginFlowInit, LoginFlowResult, NextcloudCredentials, LoginState)
- `service.rs`: Authentication service with polling logic
- `component.rs`: Dioxus UI component for authentication

## Error Handling

The library provides detailed error types:

```rust
pub enum AuthError {
    NetworkError(String),
    JsonError(String),
    TimeoutError,
    ServerError(String),
}
```

## Advanced Usage

### Manual Polling Control

```rust
let service = NextcloudAuthService::new(server_url);

// Initiate login flow
let flow = service.initiate_login().await?;

println!("Please visit: {}", flow.login);

// Poll manually
loop {
    match service.poll_login(&flow.poll.endpoint, &flow.poll.token).await? {
        Some(credentials) => {
            // Login complete!
            break;
        }
        None => {
            // Still waiting, check again later
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}
```

## Integration with Applications

### Storing Credentials

After authentication, the application should store the credentials securely:

```rust
NextcloudAuthComponent {
    server_url: server_url.clone(),
    on_success: move |creds| {
        // Store in database or secure storage
        let settings = SyncSettings {
            server_url: creds.server_url,
            username: creds.username,
            app_password: creds.app_password,
            // ... other fields
        };
        save_sync_settings(&conn, &settings).unwrap();
    },
}
```

### Setting Up WebDAV

After authentication, set up WebDAV client separately:

```rust
use reqwest_dav::{ClientBuilder, Auth};

let client = ClientBuilder::new()
    .set_host(format!("{}/remote.php/dav/files/{}", 
        credentials.server_url, credentials.username))
    .set_auth(Auth::Basic(credentials.username, credentials.app_password))
    .build()?;
```

## License

MIT OR Apache-2.0
