//! Loads [`Config`] as defaults → TOML file → environment overrides.
//!
//! Each stage overlays the previous one, so later sources win field by field:
//! [`Config::default`] provides the baseline, an optional `config.toml` is
//! merged over it (a missing file is not an error), and `CALCLI_*` variables
//! overlay the result. Merging is per field, not per section: setting one
//! colour leaves the others alone.
//!
//! Unknown TOML keys are rejected so a typo surfaces instead of being silently
//! ignored; a single out-of-range value (e.g. an unsupported decimal separator)
//! is logged and the default kept, so one typo never blocks startup.
//!
//! # Backwards compatibility
//!
//! calcli 0.2 wrote a flat `[theme]` table and top-level `glyphs`. Those keys
//! are still accepted and mapped onto the new shape: the accent and the three
//! chrome colours become `[appearance.colors]` overrides, and the eight token
//! colours become `[highlight]`. The new sections win where both are present.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use serde::Deserialize;

use crate::config::appearance::Appearance;
use crate::config::highlight::HighlightColors;
use crate::config::{Config, ConfigError};
use crate::domain::format::{AngleMode, Notation};
use crate::theme::{
    Color, GlyphVariant, Palette, ThemeColors, parse_color as parse_theme_color,
};
use crate::util::paths;

/// A colour-override table (`name -> value`), used for `[appearance.colors]`
/// and each `[themes.<name>]`.
type ColorMap = BTreeMap<String, String>;

/// The on-disk shape: every field optional so partial files merge cleanly, and
/// wide enough to cover both the 0.2 layout and the current one.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawConfig {
    notation: Option<Notation>,
    decimals: Option<usize>,
    angle_mode: Option<AngleMode>,
    decimal_separator: Option<String>,
    thousands_separator: Option<String>,
    trim_trailing_zeros: Option<bool>,
    max_history: Option<usize>,
    restore_last_settings: Option<bool>,
    live_feedback: Option<bool>,
    history_spacing: Option<usize>,
    history_separator: Option<bool>,
    input_max_lines: Option<usize>,
    confirm_delete: Option<bool>,
    confirm_quit: Option<bool>,

    /// Legacy (0.2) top-level glyph set; superseded by `[appearance].glyphs`.
    glyphs: Option<GlyphVariant>,
    /// Legacy (0.2) flat `[theme]` colour table. Not to be confused with the
    /// theme *name*, which is the scalar `theme` inside `[appearance]`.
    theme: Option<RawLegacyTheme>,

    appearance: Option<RawAppearance>,
    /// Syntax-highlight token colours (`[highlight]`).
    highlight: Option<RawHighlight>,
    /// User-defined themes, each a `[themes.<name>]` colour table.
    themes: BTreeMap<String, ColorMap>,
    /// Per-action key overrides (`[keys]`), each a string or a list of strings.
    keys: BTreeMap<String, KeyBinding>,
}

/// The `[appearance]` table, all optional.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawAppearance {
    theme: Option<String>,
    glyphs: Option<GlyphVariant>,
    /// Per-colour overrides (`[appearance.colors]`), keyed by palette colour.
    colors: ColorMap,
}

/// The `[highlight]` table, all optional.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawHighlight {
    function: Option<String>,
    constant: Option<String>,
    operator: Option<String>,
    number: Option<String>,
    variable: Option<String>,
    ans: Option<String>,
    comment: Option<String>,
    unit: Option<String>,
}

/// The legacy `[theme]` table written by calcli 0.2, all optional.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawLegacyTheme {
    accent_color: Option<String>,
    function_color: Option<String>,
    constant_color: Option<String>,
    operator_color: Option<String>,
    number_color: Option<String>,
    variable_color: Option<String>,
    ans_color: Option<String>,
    comment_color: Option<String>,
    unit_color: Option<String>,
    settings_bar_bg: Option<String>,
    /// Tinted every second history entry, a feature that no longer exists.
    /// Read by nothing, but still named here: `deny_unknown_fields` would
    /// otherwise reject every 0.2 config that sets it.
    #[allow(dead_code, reason = "accepted for compatibility, without effect")]
    history_alt_bg: Option<String>,
    history_separator_color: Option<String>,
}

/// A key binding from config: either one key or a list of keys.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum KeyBinding {
    One(String),
    Many(Vec<String>),
}

impl KeyBinding {
    fn into_keys(self) -> Vec<String> {
        match self {
            KeyBinding::One(key) => vec![key],
            KeyBinding::Many(keys) => keys,
        }
    }
}

/// Loads the configuration from the default config path, then applies the
/// environment overrides.
///
/// A missing file is not an error; the defaults are used in that case.
///
/// # Errors
///
/// Returns [`ConfigError`] when the file exists but cannot be read or parsed.
pub fn load_config() -> Result<Config, ConfigError> {
    load_from(&paths::config_file())
}

/// Loads the configuration from an explicit path, then applies env overrides.
///
/// # Errors
///
/// Returns [`ConfigError`] when the file exists but cannot be read or parsed.
pub fn load_from(path: &Path) -> Result<Config, ConfigError> {
    let mut config = match std::fs::read_to_string(path) {
        Ok(content) => {
            log::info!("loaded config from {}", path.display());
            parse(&content, path)?
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            log::debug!("no config file at {}, using defaults", path.display());
            Config::default()
        }
        Err(source) => {
            return Err(ConfigError::Read {
                path: path.display().to_string(),
                source,
            });
        }
    };
    apply_env(&mut config);
    Ok(config)
}

/// Parses and merges a TOML config string over the defaults (no env applied).
///
/// # Errors
///
/// Returns [`ConfigError::Parse`] when the string is not valid config TOML.
pub fn config_from_str(content: &str) -> Result<Config, ConfigError> {
    parse(content, Path::new("<memory>"))
}

fn parse(content: &str, path: &Path) -> Result<Config, ConfigError> {
    let raw: RawConfig =
        toml::from_str(content).map_err(|source| ConfigError::Parse {
            path: path.display().to_string(),
            source,
        })?;
    Ok(merge(raw))
}

/// Merges a raw config over the defaults.
fn merge(raw: RawConfig) -> Config {
    let defaults = Config::default();
    report_unknown(&unknown_color_names(&raw));
    let legacy = raw.theme.unwrap_or_default();
    Config {
        notation: raw.notation.unwrap_or(defaults.notation),
        decimals: raw.decimals.unwrap_or(defaults.decimals),
        angle_mode: raw.angle_mode.unwrap_or(defaults.angle_mode),
        decimal_separator: raw
            .decimal_separator
            .and_then(|s| parse_separator(&s))
            .unwrap_or(defaults.decimal_separator),
        thousands_separator: raw
            .thousands_separator
            .unwrap_or(defaults.thousands_separator),
        trim_trailing_zeros: raw
            .trim_trailing_zeros
            .unwrap_or(defaults.trim_trailing_zeros),
        max_history: raw.max_history.unwrap_or(defaults.max_history).max(1),
        restore_last_settings: raw
            .restore_last_settings
            .unwrap_or(defaults.restore_last_settings),
        live_feedback: raw.live_feedback.unwrap_or(defaults.live_feedback),
        history_spacing: raw
            .history_spacing
            .unwrap_or(defaults.history_spacing),
        history_separator: raw
            .history_separator
            .unwrap_or(defaults.history_separator),
        input_max_lines: raw
            .input_max_lines
            .unwrap_or(defaults.input_max_lines)
            .max(1),
        confirm_delete: raw.confirm_delete.unwrap_or(defaults.confirm_delete),
        confirm_quit: raw.confirm_quit.unwrap_or(defaults.confirm_quit),
        appearance: merge_appearance(
            raw.appearance.unwrap_or_default(),
            raw.glyphs,
            &legacy,
            defaults.appearance,
        ),
        highlight: merge_highlight(
            raw.highlight.unwrap_or_default(),
            &legacy,
            defaults.highlight,
        ),
        themes: raw
            .themes
            .into_iter()
            .map(|(name, colors)| (name, theme_colors(&colors)))
            .collect(),
        keys: raw
            .keys
            .into_iter()
            .map(|(action, binding)| (action, binding.into_keys()))
            .collect(),
    }
}

/// Merges `[appearance]` over the defaults, folding in the legacy `[theme]`
/// chrome colours and the legacy top-level `glyphs`. The new keys win.
fn merge_appearance(
    raw: RawAppearance,
    legacy_glyphs: Option<GlyphVariant>,
    legacy: &RawLegacyTheme,
    defaults: Appearance,
) -> Appearance {
    let mut colors = defaults.colors;
    for (name, value) in legacy_chrome_colors(legacy) {
        colors.insert(name.to_string(), value);
    }
    colors.extend(raw.colors);
    Appearance {
        theme: raw.theme.unwrap_or(defaults.theme),
        colors,
        glyphs: raw.glyphs.or(legacy_glyphs).unwrap_or(defaults.glyphs),
    }
}

/// The legacy `[theme]` colours that map onto a palette colour.
///
/// `history_alt_bg` is absent on purpose: it tinted the zebra stripe, which no
/// longer exists. Mapping it onto `panel` would point it at a colour calcli
/// never draws, which reads as support for something that is gone.
fn legacy_chrome_colors(
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

/// Merges `[highlight]` over the defaults, falling back to the legacy
/// `[theme].*_color` keys where the new section is silent.
fn merge_highlight(
    raw: RawHighlight,
    legacy: &RawLegacyTheme,
    defaults: HighlightColors,
) -> HighlightColors {
    let pick = |new: Option<String>, old: &Option<String>, fallback: Color| {
        new.or_else(|| old.clone())
            .and_then(|value| parse_highlight_color(&value))
            .unwrap_or(fallback)
    };
    HighlightColors {
        function: pick(raw.function, &legacy.function_color, defaults.function),
        constant: pick(raw.constant, &legacy.constant_color, defaults.constant),
        operator: pick(raw.operator, &legacy.operator_color, defaults.operator),
        number: pick(raw.number, &legacy.number_color, defaults.number),
        variable: pick(raw.variable, &legacy.variable_color, defaults.variable),
        ans: pick(raw.ans, &legacy.ans_color, defaults.ans),
        comment: pick(raw.comment, &legacy.comment_color, defaults.comment),
        unit: pick(raw.unit, &legacy.unit_color, defaults.unit),
    }
}

/// Parses a highlight colour, logging and ignoring an unusable value.
fn parse_highlight_color(value: &str) -> Option<Color> {
    let color = parse_theme_color(value);
    if color.is_none() {
        log::warn!("invalid highlight colour {value:?}, keeping the default");
    }
    color
}

/// Converts a raw custom theme colour table into [`ThemeColors`].
fn theme_colors(raw: &ColorMap) -> ThemeColors {
    ThemeColors::from_lookup(|name| {
        raw.get(name)
            .map(String::as_str)
            .and_then(parse_theme_color)
    })
}

/// Every colour the file names in a section that cannot carry it, as
/// `(section, name)` pairs.
///
/// The two sections take different colour sets, and mixing them up is silent:
/// `[appearance.colors]` overrides any palette colour, while a
/// `[themes.<name>]` contributes only the [`ThemeColors`] a palette is derived
/// *from*. Validating a theme against `Palette::KEYS` accepts the derived ones
/// (`selection`, `cursor`, `input_bg`, …) and then drops the value without a
/// word.
fn unknown_color_names(raw: &RawConfig) -> Vec<(String, String)> {
    let mut unknown = Vec::new();
    if let Some(appearance) = &raw.appearance {
        for name in unknown_colors(&appearance.colors, Palette::KEYS) {
            unknown.push(("appearance.colors".to_string(), name.to_string()));
        }
    }
    for (theme, colors) in &raw.themes {
        for name in unknown_colors(colors, ThemeColors::KEYS) {
            unknown.push((format!("themes.{theme}"), name.to_string()));
        }
    }
    unknown
}

/// The keys of `colors` that are not in `known`, in file order.
fn unknown_colors<'a>(colors: &'a ColorMap, known: &[&str]) -> Vec<&'a str> {
    colors
        .keys()
        .map(String::as_str)
        .filter(|name| !known.contains(name))
        .collect()
}

/// Warns about each unusable colour name, so a typo (or a colour in the wrong
/// section) surfaces instead of being silently ignored.
fn report_unknown(unknown: &[(String, String)]) {
    for (section, name) in unknown {
        log::warn!("unknown colour '{name}' in [{section}], ignoring");
    }
}

/// Parses a decimal-separator string, accepting only `.` or `,`.
fn parse_separator(value: &str) -> Option<char> {
    match value {
        "." => Some('.'),
        "," => Some(','),
        other => {
            log::warn!(
                "ignoring unsupported decimal_separator {other:?}; \
                 using the default"
            );
            None
        }
    }
}

/// Applies `CALCLI_*` environment overrides on top of the file/defaults.
pub fn apply_env(config: &mut Config) {
    if let Ok(value) = std::env::var("CALCLI_DECIMAL_SEPARATOR")
        && let Some(separator) = parse_separator(&value)
    {
        config.decimal_separator = separator;
    }
    if let Ok(value) = std::env::var("CALCLI_DECIMALS")
        && let Ok(decimals) = value.parse::<usize>()
    {
        config.decimals = decimals;
    }
    if let Ok(value) = std::env::var("CALCLI_ACCENT")
        && !value.is_empty()
    {
        config.appearance.colors.insert("accent".to_string(), value);
    }
    if let Ok(value) = std::env::var("CALCLI_THEME")
        && !value.is_empty()
    {
        // The name is validated when resolved against the theme registry.
        config.appearance.theme = value;
    }
    if let Ok(value) = std::env::var("CALCLI_GLYPHS") {
        match value.to_ascii_lowercase().as_str() {
            "ascii" => config.appearance.glyphs = GlyphVariant::Ascii,
            "unicode" => config.appearance.glyphs = GlyphVariant::Unicode,
            _ => {}
        }
    }
    if let Ok(value) = std::env::var("CALCLI_CONFIRM_QUIT")
        && let Ok(flag) = value.parse::<bool>()
    {
        config.confirm_quit = flag;
    }
    if let Ok(value) = std::env::var("CALCLI_CONFIRM_DELETE")
        && let Ok(flag) = value.parse::<bool>()
    {
        config.confirm_delete = flag;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CALCLI_THEME;

    /// The complete `config.toml` calcli 0.2 shipped as `examples/config.toml`.
    const LEGACY_CONFIG: &str =
        include_str!("../../tests/data/config-0.2.toml");

    /// The example config we ship today.
    const EXAMPLE_CONFIG: &str = include_str!("../../examples/config.toml");

    /// The example is documentation, and documentation that no longer parses is
    /// worse than none: it hands the user a file that stops calcli from
    /// starting. Removing a key from `RawConfig` has to reach this file too.
    #[test]
    fn the_shipped_example_config_parses_and_states_the_defaults() {
        let config = config_from_str(EXAMPLE_CONFIG)
            .expect("examples/config.toml must parse");
        let defaults = Config::default();

        assert_eq!(config.notation, defaults.notation);
        assert_eq!(config.decimals, defaults.decimals);
        assert_eq!(config.angle_mode, defaults.angle_mode);
        assert_eq!(config.max_history, defaults.max_history);
        assert_eq!(config.history_spacing, defaults.history_spacing);
        assert_eq!(config.history_separator, defaults.history_separator);
        assert_eq!(config.input_max_lines, defaults.input_max_lines);
        assert_eq!(config.confirm_delete, defaults.confirm_delete);
        assert_eq!(config.confirm_quit, defaults.confirm_quit);
        assert_eq!(config.highlight, defaults.highlight);
    }

    #[test]
    fn empty_raw_config_yields_defaults() {
        let config = merge(RawConfig::default());
        assert_eq!(config, Config::default());
    }

    #[test]
    fn partial_config_overrides_only_given_keys() {
        let raw: RawConfig =
            toml::from_str("decimals = 6\ndecimal_separator = \",\"\n")
                .unwrap();
        let config = merge(raw);
        assert_eq!(config.decimals, 6);
        assert_eq!(config.decimal_separator, ',');
        assert_eq!(config.notation, Config::default().notation);
        assert_eq!(config.thousands_separator, " ");
    }

    #[test]
    fn an_unsupported_separator_falls_back_to_the_default() {
        let raw: RawConfig =
            toml::from_str("decimal_separator = \";\"\n").unwrap();
        assert_eq!(merge(raw).decimal_separator, '.');
    }

    #[test]
    fn max_history_and_input_max_lines_are_at_least_one() {
        let raw: RawConfig =
            toml::from_str("max_history = 0\ninput_max_lines = 0\n").unwrap();
        let config = merge(raw);
        assert_eq!(config.max_history, 1);
        assert_eq!(config.input_max_lines, 1);
    }

    #[test]
    fn parses_notation_and_angle_mode_from_toml() {
        let raw: RawConfig =
            toml::from_str("notation = \"scientific\"\nangle_mode = \"deg\"\n")
                .unwrap();
        let config = merge(raw);
        assert_eq!(config.notation, Notation::Scientific);
        assert_eq!(config.angle_mode, AngleMode::Deg);
    }

    /// The 0.2 config minus the one key that no longer exists.
    fn legacy_config_without_zebra() -> String {
        LEGACY_CONFIG.replace("history_zebra = false\n", "")
    }

    /// Dropping `history_zebra` is a breaking change, and deliberately a loud
    /// one: `deny_unknown_fields` refuses the file rather than ignore the key,
    /// so the user is told to delete a line instead of wondering why nothing
    /// changed. `config.toml` may do this; `state.toml` may not, because an
    /// unreadable state file is silently treated as an empty session.
    ///
    /// The key is named by the TOML error, which `ConfigError::Parse` carries
    /// as its source. `main` prints the whole chain (`{error:#}`), so this is
    /// what the user reads.
    #[test]
    fn a_0_2_config_file_is_rejected_by_name_for_its_zebra_key() {
        let error = config_from_str(LEGACY_CONFIG)
            .expect_err("history_zebra is gone, so the 0.2 file is refused");
        let cause = std::error::Error::source(&error)
            .expect("a parse failure carries the TOML error")
            .to_string();
        assert!(
            cause.contains("history_zebra"),
            "the message must name the offending key: {cause}",
        );
    }

    #[test]
    fn a_legacy_0_2_config_file_still_loads() {
        let config = config_from_str(&legacy_config_without_zebra())
            .expect("the 0.2 config shape must keep loading");

        // Plain scalars survive untouched.
        assert_eq!(config.decimals, 3);
        assert_eq!(config.max_history, 500);
        assert_eq!(config.appearance.glyphs, GlyphVariant::Unicode);
        assert!(config.live_feedback);

        // The legacy accent becomes a palette override.
        assert_eq!(config.palette().accent, Color::hex("#6dd0ff"));
        // The legacy chrome colours land on their palette counterparts.
        assert_eq!(config.palette().footer, Color::hex("#252525"));
        assert_eq!(config.palette().border, Color::hex("#3e3e3e"));
        // The legacy token colours land in [highlight].
        assert_eq!(config.highlight.function, Color::hex("#78c2b3"));
        assert_eq!(config.highlight.unit, Color::hex("#ff79c6"));
    }

    /// `history_alt_bg` tinted the zebra stripe. The key is still accepted, but
    /// it no longer reaches `panel`: pointing it at a colour calcli never draws
    /// would look like support for a feature that is gone.
    #[test]
    fn the_legacy_zebra_colour_no_longer_reaches_the_palette() {
        let legacy = config_from_str(&legacy_config_without_zebra())
            .expect("the 0.2 config shape must keep loading");
        assert_eq!(legacy.palette().panel, Config::default().palette().panel);
    }

    #[test]
    fn the_new_highlight_section_wins_over_the_legacy_theme_table() {
        let raw = "\
[theme]
function_color = \"#111111\"
[highlight]
function = \"#222222\"
";
        let config = config_from_str(raw).unwrap();
        assert_eq!(config.highlight.function, Color::hex("#222222"));
    }

    #[test]
    fn the_new_appearance_glyphs_wins_over_the_legacy_top_level_key() {
        let config = config_from_str("glyphs = \"ascii\"\n").unwrap();
        assert_eq!(config.appearance.glyphs, GlyphVariant::Ascii);

        let config = config_from_str(
            "glyphs = \"ascii\"\n[appearance]\nglyphs = \"unicode\"\n",
        )
        .unwrap();
        assert_eq!(config.appearance.glyphs, GlyphVariant::Unicode);
    }

    #[test]
    fn an_appearance_colour_override_wins_over_the_legacy_accent() {
        let raw = "\
[theme]
accent_color = \"#111111\"
[appearance.colors]
accent = \"#222222\"
";
        let config = config_from_str(raw).unwrap();
        assert_eq!(config.palette().accent, Color::hex("#222222"));
    }

    #[test]
    fn overriding_one_colour_keeps_the_default_red_cursor() {
        let config =
            config_from_str("[appearance.colors]\naccent = \"#222222\"\n")
                .unwrap();
        assert_eq!(config.palette().cursor, Color::hex("#d65c5c"));
    }

    #[test]
    fn an_invalid_highlight_colour_keeps_the_default() {
        let config =
            config_from_str("[highlight]\nfunction = \"nope\"\n").unwrap();
        assert_eq!(
            config.highlight.function,
            HighlightColors::default().function,
        );
    }

    #[test]
    fn key_bindings_accept_a_single_key_or_a_list() {
        let config =
            config_from_str("[keys]\nquit = \"x\"\ncopy = [\"y\", \"c\"]\n")
                .unwrap();
        assert_eq!(config.keys["quit"], vec!["x".to_string()]);
        assert_eq!(config.keys["copy"], vec!["y".to_string(), "c".to_string()]);
    }

    #[test]
    fn a_custom_theme_is_registered_alongside_the_calcli_theme() {
        let config =
            config_from_str("[themes.solar]\naccent = \"#010203\"\n").unwrap();
        let registry = config.theme_registry();
        assert!(registry.contains("solar"));
        assert!(registry.contains(CALCLI_THEME));
        assert_eq!(registry.resolve("solar").accent, Color::Rgb(1, 2, 3));
    }

    #[test]
    fn a_theme_may_ship_its_own_focus_border() {
        let config =
            config_from_str("[themes.mine]\nborder_focus = \"#010203\"\n")
                .unwrap();
        let registry = config.theme_registry();
        assert_eq!(registry.resolve("mine").border_focus, Color::Rgb(1, 2, 3));
    }

    #[test]
    fn a_theme_that_sets_only_border_drags_its_focus_border_along() {
        let config =
            config_from_str("[themes.mine]\nborder = \"#4a4a4a\"\n").unwrap();
        let theme = config.theme_registry().resolve("mine");
        assert_eq!(theme.border, Color::hex("#4a4a4a"));
        assert!(theme.border_focus.luminance() > theme.border.luminance());
    }

    #[test]
    fn an_appearance_focus_border_override_reaches_the_palette() {
        let config = config_from_str(
            "[appearance.colors]\nborder_focus = \"#8a8a8a\"\n",
        )
        .unwrap();
        assert_eq!(config.palette().border_focus, Color::hex("#8a8a8a"));
        // The plain border keeps the theme's value.
        assert_eq!(config.palette().border, Color::hex("#3e3e3e"));
    }

    #[test]
    fn overriding_only_the_border_drags_the_focus_border_along() {
        let config =
            config_from_str("[appearance.colors]\nborder = \"#4a4a4a\"\n")
                .unwrap();
        let palette = config.palette();
        assert_eq!(palette.border, Color::hex("#4a4a4a"));
        assert!(
            palette.border_focus.luminance() > palette.border.luminance(),
            "the focused frame must not sink into the new border",
        );
    }

    // --- Which colour belongs in which section ---

    /// The `(section, name)` pairs `content` reports as unusable.
    fn unknown(content: &str) -> Vec<(String, String)> {
        let raw: RawConfig = toml::from_str(content).expect("valid toml");
        unknown_color_names(&raw)
    }

    #[test]
    fn a_theme_table_rejects_the_palettes_derived_colours() {
        // `cursor`, `selection` and the input fills are derived by the toolkit;
        // a theme cannot contribute them. Accepting them here would drop the
        // value without a word, which is what used to happen.
        let reported = unknown(
            "[themes.mine]\n\
             border = \"#4a4a4a\"\n\
             border_focus = \"#8a8a8a\"\n\
             cursor = \"#ff0000\"\n\
             input_bg = \"#0000ff\"\n\
             selection = \"#00ff00\"\n",
        );
        assert_eq!(
            reported,
            vec![
                ("themes.mine".to_string(), "cursor".to_string()),
                ("themes.mine".to_string(), "input_bg".to_string()),
                ("themes.mine".to_string(), "selection".to_string()),
            ],
        );
    }

    #[test]
    fn appearance_colours_may_name_any_palette_colour() {
        // The very colours a theme may not carry belong here.
        let reported = unknown(
            "[appearance.colors]\n\
             cursor = \"#ff0000\"\n\
             selection = \"#00ff00\"\n\
             input_bg = \"#0000ff\"\n",
        );
        assert!(reported.is_empty(), "{reported:?}");
    }

    #[test]
    fn a_theme_may_name_every_theme_colour() {
        let table =
            ThemeColors::KEYS
                .iter()
                .fold(String::new(), |mut table, name| {
                    table.push_str(name);
                    table.push_str(" = \"#010203\"\n");
                    table
                });
        assert!(unknown(&format!("[themes.mine]\n{table}")).is_empty());
    }

    #[test]
    fn both_sections_report_a_typo() {
        assert_eq!(
            unknown("[appearance.colors]\nbordr = \"#010203\"\n"),
            vec![("appearance.colors".to_string(), "bordr".to_string())],
        );
        assert_eq!(
            unknown("[themes.mine]\nbordr = \"#010203\"\n"),
            vec![("themes.mine".to_string(), "bordr".to_string())],
        );
    }

    #[test]
    fn a_clean_file_reports_nothing() {
        assert!(unknown("[appearance]\ntheme = \"calcli\"\n").is_empty());
    }

    #[test]
    fn every_theme_colour_name_is_also_a_palette_colour_name() {
        // A theme colour that the palette does not carry would be unreachable.
        for name in ThemeColors::KEYS {
            assert!(
                Palette::KEYS.contains(name),
                "{name} is in a theme but not in the palette",
            );
        }
    }

    #[test]
    fn an_unknown_top_level_key_is_rejected() {
        assert!(config_from_str("nonsense = 1\n").is_err());
    }
}
