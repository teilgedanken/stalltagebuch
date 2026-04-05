use crate::camera;
use dioxus_i18n::prelude::*;

fn map_system_locale_to_supported(locale_tag: &str) -> unic_langid::LanguageIdentifier {
    let normalized = locale_tag.trim().replace('_', "-").to_ascii_lowercase();
    if normalized.starts_with("de") {
        unic_langid::langid!("de-DE")
    } else if normalized.starts_with("en") {
        unic_langid::langid!("en-US")
    } else {
        unic_langid::langid!("en-US")
    }
}

/// Initialize i18n configuration based on the Android system language.
/// Falls back to en-US for unsupported locales.
pub fn init_i18n() -> I18nConfig {
    let system_locale_tag = camera::get_system_language_tag();
    let active_locale = map_system_locale_to_supported(&system_locale_tag);
    log::info!(
        "Initialized i18n with locale '{}' (system locale '{}')",
        active_locale,
        system_locale_tag
    );

    I18nConfig::new(active_locale)
        .with_locale(Locale::new_static(
            unic_langid::langid!("de-DE"),
            include_str!("../locales/de-DE.ftl"),
        ))
        .with_locale(Locale::new_static(
            unic_langid::langid!("en-US"),
            include_str!("../locales/en-US.ftl"),
        ))
}
