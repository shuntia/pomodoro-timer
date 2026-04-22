// Generated automatically by iced_lucide at build time.
// Do not edit manually.
// 4cfdae1a24a79f67eb0a1d905f8308d61bf2461ecd223c2e7b1ef2b85faba8d2
use iced::Font;
use iced::widget::{Text, text};

pub const FONT: &[u8] = include_bytes!("../fonts/lucide.ttf");

/// All icons as `(name, codepoint_str)` pairs.
/// Use this to populate an icon-picker widget.
#[allow(dead_code)]
pub const ALL_ICONS: &[(&str, &str)] = &[
    ("settings", "\u{E154}"),
    ("shuffle", "\u{E15E}"),
    ("skip_back", "\u{E15F}"),
    ("skip_forward", "\u{E160}"),
];

pub fn settings<'a>() -> Text<'a> {
    icon("\u{E154}")
}

pub fn shuffle<'a>() -> Text<'a> {
    icon("\u{E15E}")
}

pub fn skip_back<'a>() -> Text<'a> {
    icon("\u{E15F}")
}

pub fn skip_forward<'a>() -> Text<'a> {
    icon("\u{E160}")
}

/// Render any Lucide icon by its codepoint string.
/// Use this together with [`ALL_ICONS`] to display icons dynamically:
/// ```ignore
/// for (name, cp) in ALL_ICONS {
///     button(render(cp)).on_press(Msg::Pick(name.to_string()))
/// }
/// ```
pub fn render(codepoint: &str) -> Text<'_> {
    text(codepoint).font(Font::with_name("lucide"))
}

fn icon(codepoint: &str) -> Text<'_> {
    render(codepoint)
}
