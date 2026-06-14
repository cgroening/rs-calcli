//! Central colour palette and styling helpers.
//!
//! One muted accent (resolved from config) carries meaning — borders, the
//! active mode, labels — while fixed dim/tint colours mark selection, focus and
//! the block cursor. Keeping them here (not scattered across views) is the
//! single source of truth the style guide asks for.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

/// Background tint for a selected list row.
pub const SELECTION_BG: Color = Color::Rgb(45, 55, 70);

/// Background tint for the focused input field.
pub const FOCUS_BG: Color = Color::Rgb(34, 42, 54);

/// Block-cursor colour (a muted red), distinct from the accent and selection.
pub const INPUT_CURSOR: Color = Color::Rgb(214, 92, 92);

/// Colour for an error message.
pub const ERROR: Color = Color::Rgb(224, 108, 108);

/// The fallback accent when the configured colour cannot be parsed.
const FALLBACK_ACCENT: Color = Color::Rgb(109, 208, 255);

/// Resolves a config colour string (`#rrggbb` or a few names) to a [`Color`],
/// falling back to the muted accent on anything unrecognized.
pub fn parse_color(value: &str) -> Color {
    if let Some(color) = parse_hex(value) {
        return color;
    }
    match value.trim().to_lowercase().as_str() {
        "cyan" => Color::Rgb(109, 208, 255),
        "green" => Color::Rgb(140, 200, 140),
        "magenta" => Color::Rgb(200, 140, 200),
        "yellow" => Color::Rgb(214, 196, 120),
        "blue" => Color::Rgb(120, 160, 220),
        _ => FALLBACK_ACCENT,
    }
}

/// Parses a `#rrggbb` hex string into a [`Color`], or `None` when malformed.
fn parse_hex(value: &str) -> Option<Color> {
    let hex = value.trim().strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(red, green, blue))
}

/// A dim style for secondary text.
pub fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

/// The block-cursor span (`█`) painted past the end of an input field.
pub fn cursor_block_span() -> Span<'static> {
    Span::styled("\u{2588}", Style::default().fg(INPUT_CURSOR))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_colours() {
        assert_eq!(parse_color("#6dd0ff"), Color::Rgb(109, 208, 255));
        assert_eq!(parse_color("#000000"), Color::Rgb(0, 0, 0));
    }

    #[test]
    fn falls_back_on_malformed_input() {
        assert_eq!(parse_color("not-a-color"), FALLBACK_ACCENT);
        assert_eq!(parse_color("#12"), FALLBACK_ACCENT);
    }
}
