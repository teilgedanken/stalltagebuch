use crate::Screen;
use dioxus::prelude::*;
use dioxus_i18n::tid;

#[component]
pub fn NavigationBar(current_screen: Screen, on_navigate: EventHandler<Screen>) -> Element {
    rsx! {
        div { class: "is-fixed-bottom",
            div { class: "tabs is-toggle is-centered is-medium",
                ul {
                    li { class: if matches!(current_screen, Screen::Home) { "is-active" } else { "" },
                        a { onclick: move |_| on_navigate.call(Screen::Home),
                            span { "🏠" }
                            if matches!(current_screen, Screen::Home) {
                                span { {tid!("nav-home")} }
                            }
                        }
                    }

                    li { class: if matches!(current_screen, Screen::ProfileList) { "is-active" } else { "" },
                        a { onclick: move |_| on_navigate.call(Screen::ProfileList),
                            span { "🐦" }
                            if matches!(current_screen, Screen::ProfileList) {
                                span { {tid!("nav-profiles")} }
                            }
                        }
                    }

                    li { class: if matches!(current_screen, Screen::Statistics) { "is-active" } else { "" },
                        a { onclick: move |_| on_navigate.call(Screen::Statistics),
                            span { "🥚" }
                            if matches!(current_screen, Screen::Statistics) {
                                span { {tid!("nav-eggs")} }
                            }
                        }
                    }
                }
            }
        }
    }
}
