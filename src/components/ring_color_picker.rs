use crate::models::{RingColor, ring_color_preview_bg, ring_color_select_bg};
use dioxus::prelude::*;
use dioxus_i18n::tid;

#[component]
pub fn RingColorField(
    field_label: String,
    side_label: String,
    selected: Option<RingColor>,
    is_open: bool,
    disabled: bool,
    key_prefix: String,
    on_toggle: EventHandler<()>,
    on_select: EventHandler<Option<RingColor>>,
) -> Element {
    rsx! {
        div { class: "column",
            p { class: "help mb-2", {side_label.clone()} }
            RingColorTrigger {
                field_label: field_label.clone(),
                side_label: side_label.clone(),
                selected: selected.clone(),
                is_open,
                disabled,
                compact: false,
                on_toggle,
            }
            if is_open {
                RingColorPalette {
                    selected,
                    key_prefix,
                    container_style: "margin-top: 0.5rem;".to_string(),
                    on_select,
                }
            }
        }
    }
}

#[component]
pub fn RingColorTrigger(
    field_label: String,
    side_label: String,
    selected: Option<RingColor>,
    is_open: bool,
    disabled: bool,
    compact: bool,
    on_toggle: EventHandler<()>,
) -> Element {
    let trigger_label = format!("{field_label} {side_label}");
    let selection_label = selected
        .as_ref()
        .map(ring_color_option_label)
        .unwrap_or_else(|| tid!("ring-color-none"));

    rsx! {
        button {
            class: if compact { "button is-link is-light px-2" } else { "button is-light is-fullwidth" },
            style: if compact { ring_color_compact_trigger_style(selected.as_ref(), is_open) } else { ring_color_field_trigger_style(selected.as_ref(), is_open) },
            disabled,
            title: trigger_label.clone(),
            aria_label: trigger_label,
            onclick: move |_| on_toggle.call(()),
            if compact {
                if let Some(color) = selected.as_ref() {
                    span { style: ring_color_swatch_style(Some(color), is_open, "1.5rem") }
                } else {
                    span {
                        class: "icon is-small",
                        style: ring_color_palette_icon_style(is_open),
                        "🎨"
                    }
                }
            } else {
                span { style: "display: flex; align-items: center; gap: 0.25rem; min-width: 0;",
                    if let Some(color) = selected.as_ref() {
                        span { style: ring_color_swatch_style(Some(color), is_open, "1.25rem") }
                    } else {
                        span {
                            class: "icon is-small",
                            style: ring_color_palette_icon_style(is_open),
                            "🎨"
                        }
                    }
                    span { class: if selected.is_some() { "has-text-black" } else { "has-text-grey" },
                        "{selection_label}"
                    }
                }
            }
        }
    }
}

#[component]
pub fn RingColorPalette(
    selected: Option<RingColor>,
    key_prefix: String,
    container_style: String,
    on_select: EventHandler<Option<RingColor>>,
) -> Element {
    rsx! {
        div { class: "box p-2 mb-0", style: "{container_style}",
            div {
                class: "buttons are-small mb-0",
                style: "display: flex; flex-wrap: wrap; gap: 0.35rem;",
                button {
                    class: "button is-light p-1",
                    style: ring_color_palette_button_style(None, selected.is_none()),
                    title: tid!("ring-color-none"),
                    aria_label: tid!("ring-color-none"),
                    onclick: move |_| on_select.call(None),
                    span { style: ring_color_none_swatch_style(selected.is_none()), "×" }
                }
                for color in RingColor::all().iter().cloned() {
                    {
                        let button_color = color.clone();
                        let button_label = ring_color_option_label(&color);
                        let button_key = format!("{key_prefix}-{}", color.as_str());

                        rsx! {
                            button {
                                key: "{button_key}",
                                class: "button is-light p-1",
                                style: ring_color_palette_button_style(Some(&color), selected.as_ref() == Some(&color)),
                                title: button_label.clone(),
                                aria_label: button_label,
                                onclick: move |_| on_select.call(Some(button_color.clone())),
                                span { style: ring_color_palette_swatch_style(&color) }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub(crate) fn ring_color_option_label(color: &RingColor) -> String {
    match color {
        RingColor::Lila => tid!("ring-color-purple"),
        RingColor::Rosa => tid!("ring-color-pink"),
        RingColor::Hellblau => tid!("ring-color-light-blue"),
        RingColor::Dunkelblau => tid!("ring-color-dark-blue"),
        RingColor::Rot => tid!("ring-color-red"),
        RingColor::Orange => tid!("ring-color-orange"),
        RingColor::Weiss => tid!("ring-color-white"),
        RingColor::Gelb => tid!("ring-color-yellow"),
        RingColor::Schwarz => tid!("ring-color-black"),
        RingColor::Gruen => tid!("ring-color-green"),
    }
}

fn ring_color_field_trigger_style(selected: Option<&RingColor>, is_open: bool) -> String {
    let border = if is_open {
        "2px solid #3273dc"
    } else {
        "1px solid #dbdbdb"
    };
    let background = selected
        .map(|selected| ring_color_select_bg(selected.as_str()))
        .unwrap_or("#ffffff");

    format!(
        "display: flex; align-items: center; justify-content: space-between; min-height: 2.5rem; padding: 0.6rem 0.75rem; border: {}; background: {};",
        border, background
    )
}

fn ring_color_compact_trigger_style(selected: Option<&RingColor>, is_open: bool) -> String {
    let border = if is_open {
        "2px solid #3273dc"
    } else {
        "1px solid #dbdbdb"
    };
    let background = selected
        .map(|selected| ring_color_select_bg(selected.as_str()))
        .unwrap_or("#ffffff");

    format!(
        "height: 2.5em; min-height: 2.5em; min-width: 2.5em; border: {}; background: {};",
        border, background
    )
}

fn ring_color_swatch_style(color: Option<&RingColor>, is_open: bool, size: &str) -> String {
    let border = if is_open {
        "2px solid #3273dc"
    } else if color.is_some() {
        "1px solid rgba(0, 0, 0, 0.35)"
    } else {
        "1px dashed #bbb"
    };
    let background = color
        .map(|selected| ring_color_preview_bg(selected.as_str()))
        .unwrap_or(
            "linear-gradient(135deg, #ffffff 0%, #ffffff 45%, #ececec 45%, #ececec 55%, #ffffff 55%, #ffffff 100%)",
        );
    let outer_background = color
        .map(|selected| ring_color_select_bg(selected.as_str()))
        .unwrap_or("#ffffff");

    format!(
        "display: inline-block; width: {}; height: {}; border-radius: 2px; border: {}; background: {}; box-shadow: inset 0 0 0 3px {}; flex-shrink: 0;",
        size, size, border, background, outer_background
    )
}

fn ring_color_palette_icon_style(is_open: bool) -> &'static str {
    if is_open {
        "color: #3273dc;"
    } else {
        "color: inherit;"
    }
}

fn ring_color_palette_button_style(color: Option<&RingColor>, is_selected: bool) -> String {
    let background = color
        .map(|selected| ring_color_select_bg(selected.as_str()))
        .unwrap_or("#ffffff");
    let border = if is_selected {
        "2px solid #3273dc"
    } else {
        "1px solid #dbdbdb"
    };

    format!("background: {}; border: {};", background, border)
}

fn ring_color_palette_swatch_style(color: &RingColor) -> String {
    format!(
        "display: inline-block; width: 1rem; height: 1rem; border-radius: 2px; border: 1px solid rgba(0, 0, 0, 0.2); background: {};",
        ring_color_preview_bg(color.as_str()),
    )
}

fn ring_color_none_swatch_style(is_selected: bool) -> String {
    let border = if is_selected {
        "2px solid #3273dc"
    } else {
        "1px dashed #bbb"
    };

    format!(
        "display: inline-flex; align-items: center; justify-content: center; width: 1rem; height: 1rem; border: {}; border-radius: 2px; color: #777; font-size: 0.75rem;",
        border
    )
}
