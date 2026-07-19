//! The calcli 0.2 compatibility shim.
//!
//! 0.2 wrote a flat `[theme]` table and a top-level `glyphs`. Both are still
//! accepted and mapped onto the current shape, because a `config.toml` that no
//! longer parses stops calcli from starting.
//!
//! Everything here exists only to keep those files loading; when 0.2 support is
//! finally dropped, this module goes as a whole.

use serde::Deserialize;

/// The legacy `[theme]` table written by calcli 0.2, all optional.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct RawLegacyTheme {
    pub(super) accent_color: Option<String>,
    pub(super) function_color: Option<String>,
    pub(super) constant_color: Option<String>,
    pub(super) operator_color: Option<String>,
    pub(super) number_color: Option<String>,
    pub(super) variable_color: Option<String>,
    pub(super) ans_color: Option<String>,
    pub(super) comment_color: Option<String>,
    pub(super) unit_color: Option<String>,
    pub(super) settings_bar_bg: Option<String>,
    /// Tinted every second history entry, a feature that no longer exists.
    /// Read by nothing, but still named here: `deny_unknown_fields` would
    /// otherwise reject every 0.2 config that sets it.
    #[allow(dead_code, reason = "accepted for compatibility, without effect")]
    pub(super) history_alt_bg: Option<String>,
    pub(super) history_separator_color: Option<String>,
}

/// The legacy `[theme]` colours that map onto a palette colour.
///
/// `history_alt_bg` is absent on purpose: it tinted the zebra stripe, which no
/// longer exists. Mapping it onto `panel` would point it at a colour calcli
/// never draws, which reads as support for something that is gone.
pub(super) fn chrome_colors(
    legacy: &RawLegacyTheme,
) -> impl Iterator<Item = (&'static str, String)> + '_ {
    [
        ("accent", &legacy.accent_color),
        ("footer", &legacy.settings_bar_bg),
        ("border", &legacy.history_separator_color),
    ]
    .into_iter()
    .filter_map(|(name, value)| Some((name, value.clone()?)))
}
