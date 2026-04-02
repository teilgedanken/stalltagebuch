use dioxus::prelude::*;
use dioxus_i18n::tid;

#[derive(Clone, PartialEq)]
enum NetworkStatus {
    Checking,
    Online,
    Offline(String),
}

#[component]
pub fn NetworkCheckCard() -> Element {
    let mut network_status = use_signal_sync(|| NetworkStatus::Checking);
    let error_network_label = tid!("error-network").to_string();
    let error_client_label = tid!("error-client").to_string();

    // Check network connectivity on mount
    use_effect(move || {
        let error_network_label = error_network_label.clone();
        let error_client_label = error_client_label.clone();
        spawn(async move {
            // Try to connect to a reliable service
            match reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
            {
                Ok(client) => {
                    match client
                        .get("https://www.google.com/generate_204")
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.status().is_success() || response.status().as_u16() == 204 {
                                network_status.set(NetworkStatus::Online);
                            } else {
                                network_status.set(NetworkStatus::Offline(format!(
                                    "HTTP Status: {}",
                                    response.status()
                                )));
                            }
                        }
                        Err(e) => {
                            network_status.set(NetworkStatus::Offline(format!(
                                "{}: {}",
                                error_network_label, e
                            )));
                        }
                    }
                }
                Err(e) => {
                    network_status.set(NetworkStatus::Offline(format!(
                        "{}: {}",
                        error_client_label, e
                    )));
                }
            }
        });
    });

    let recheck = move |_| {
        network_status.set(NetworkStatus::Checking);
        let error_network_label = tid!("error-network").to_string();
        let error_client_label = tid!("error-client").to_string();
        spawn(async move {
            match reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
            {
                Ok(client) => {
                    match client
                        .get("https://www.google.com/generate_204")
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.status().is_success() || response.status().as_u16() == 204 {
                                network_status.set(NetworkStatus::Online);
                            } else {
                                network_status.set(NetworkStatus::Offline(format!(
                                    "HTTP Status: {}",
                                    response.status()
                                )));
                            }
                        }
                        Err(e) => {
                            network_status.set(NetworkStatus::Offline(format!(
                                "{}: {}",
                                error_network_label, e
                            )));
                        }
                    }
                }
                Err(e) => {
                    network_status.set(NetworkStatus::Offline(format!(
                        "{}: {}",
                        error_client_label, e
                    )));
                }
            }
        });
    };

    rsx! {
        match network_status() {
            NetworkStatus::Checking => rsx! {
                div { class: "notification is-info is-light",
                    div { class: "is-flex is-align-items-center", style: "gap: 12px;",
                        span { class: "tag is-info is-light", "🔄" }
                        p { class: "mb-0 has-text-weight-semibold", {tid!("network-checking")} }
                    }
                }
            },
            NetworkStatus::Online => rsx! {},
            NetworkStatus::Offline(error) => rsx! {
                div { class: "notification is-danger is-light",
                    div { class: "is-flex is-align-items-center mb-3", style: "gap: 12px;",
                        span { class: "tag is-danger is-light", "❌" }
                        div {
                            p { class: "mb-0 has-text-weight-semibold", {tid!("network-offline")} }
                            p { class: "mb-0 is-size-7 has-text-grey", "{error}" }
                        }
                    }
                    button { class: "button is-primary is-fullwidth", onclick: recheck,
                        "🔄 "
                        {tid!("action-retry")}
                    }
                }
            },
        }
    }
}
