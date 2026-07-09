//! The shared app frame: the tinted, borderless header panel with the tab bar,
//! the raised content surface, the tinted status band and the backgroundless
//! key-hint band.
//!
//! Every view renders through [`render_frame`], so the frame is identical
//! wherever the user is. The frame itself draws no border lines; rounded
//! borders belong to individual widgets (the input box, modals).
//!
//! The hint band's height comes from [`ratada::shortcut_hints::height`], never
//! from a constant, so hiding the hints with `F1` reclaims the hint rows *and*
//! the blank separator line above them.

use ratada::shortcut_hints::{self, HintGroup, HintStyle};
use ratada::theme::Skin;
use ratada::{help, statusbar, style, tabs};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::widgets::{Block, Padding};

/// The brand shown at the start of the tab bar.
const BRAND: &str = crate::APP_NAME;

/// How much a locked tab bar darkens its inactive tabs.
const LOCKED_DIM: f32 = 0.25;

/// The status band is always exactly one row.
const STATUS_HEIGHT: u16 = 1;

/// Rows the content surface keeps for itself before the hint band may grow:
/// the panel's own padding plus the input box and one history line. Grouped
/// hints wrap to many rows on a narrow terminal, and content wins over them
/// (the same rule `ratada::list::render_counted` applies to its badge).
const MIN_CONTENT_HEIGHT: u16 = 6;

/// Named hint groups in display order, owned so the borrowed
/// [`HintGroup`]s and [`help::HelpSection`]s can point into them.
pub type HintGroups = Vec<(String, Vec<(String, String)>)>;

/// Everything the app frame needs for one draw.
pub struct FrameContext<'a> {
    /// The active skin (palette plus glyphs).
    pub skin: &'a Skin,
    /// The `(key, label)` tab entries.
    pub tabs: &'a [(&'a str, &'a str)],
    /// Index of the active tab.
    pub active_tab: usize,
    /// Whether tab switching is currently disabled (an in-place edit), which
    /// darkens the inactive tabs so the bar reads as disabled.
    pub tabs_locked: bool,
    /// The left side of the status band (the active settings).
    pub status_left: &'a str,
    /// The right side of the status band (the transient status).
    pub status_right: &'a str,
    /// The grouped shortcut hints; the last group is always `Global`.
    pub hints: &'a HintGroups,
}

/// Renders the app frame and returns the content [`Rect`] the caller fills.
///
/// Top to bottom: the header panel (brand plus tab bar), the raised content
/// surface, the status band and the backgroundless hints. Each region is set
/// apart by its own background colour and inset by one cell.
pub fn render_frame(frame: &mut Frame, ctx: &FrameContext<'_>) -> Rect {
    let palette = &ctx.skin.palette;
    let area = frame.area();
    let width = area.width as usize;

    // The outer fill: the terminal's own background, so the hint band below the
    // status band shows through untinted.
    frame.render_widget(
        Block::default()
            .style(style::base(palette.foreground, palette.background)),
        area,
    );

    let groups = to_hint_groups(ctx.hints);
    let hints = hint_style(ctx.skin);
    let header_height = header_height(ctx.tabs, width);
    let hints_height = hints_height(
        shortcut_hints::height(&groups, width, hints.top_margin),
        area.height.saturating_sub(header_height + STATUS_HEIGHT),
    );

    let rows = Layout::vertical([
        Constraint::Length(header_height),
        Constraint::Min(1),
        Constraint::Length(STATUS_HEIGHT),
        Constraint::Length(hints_height),
    ])
    .split(area);

    render_header(frame, rows[0], ctx);
    let content = fill_surface(frame, rows[1], ctx.skin);
    render_status(frame, rows[2], ctx);
    render_hints(frame, rows[3], &groups, &hints);
    content
}

/// The rows the hint band may take: what it wants, capped so the content keeps
/// [`MIN_CONTENT_HEIGHT`]. Below that the band drops out entirely rather than
/// showing a stump, and the blank separator goes with it.
///
/// `wanted` is already `0` while the hints are hidden, so `F1` still reclaims
/// the whole band through this path.
fn hints_height(wanted: u16, available: u16) -> u16 {
    let spare = available.saturating_sub(MIN_CONTENT_HEIGHT);
    // One row would render the top margin and no hints at all.
    if spare < 2 { 0 } else { wanted.min(spare) }
}

/// The header-panel height for `tabs` at `width`: the tab rows plus the
/// one-cell panel padding above and below.
fn header_height(tabs: &[(&str, &str)], width: usize) -> u16 {
    let inset = width.saturating_sub(2);
    tabs::height(BRAND, tabs, inset) + 2
}

/// Renders the tinted, borderless header panel carrying only the brand and the
/// tab bar. No banner ever sits here; everything status-like lives in the
/// status band.
fn render_header(frame: &mut Frame, area: Rect, ctx: &FrameContext<'_>) {
    let inner =
        fill_panel(frame, area, ctx.skin.palette.header, Padding::uniform(1));
    // A locked bar darkens the inactive tabs, which `ratada::tabs` styles from
    // `foreground_dim`; the active tab keeps its bold "you are here" marker.
    let skin = if ctx.tabs_locked {
        let mut dimmed = *ctx.skin;
        dimmed.palette.foreground_dim =
            ctx.skin.palette.foreground_dim.darken(LOCKED_DIM);
        dimmed
    } else {
        *ctx.skin
    };
    tabs::render(frame, inner, &skin, BRAND, ctx.tabs, ctx.active_tab);
}

/// Fills `area` with the raised content surface and returns the padded inside.
/// The uniform padding supplies the blank row between the header and the first
/// content line.
fn fill_surface(frame: &mut Frame, area: Rect, skin: &Skin) -> Rect {
    fill_panel(frame, area, skin.palette.surface, Padding::uniform(1))
}

/// Renders the status band: the active settings on the left, the transient
/// status on the right.
fn render_status(frame: &mut Frame, area: Rect, ctx: &FrameContext<'_>) {
    let footer = ctx.skin.palette.footer;
    let inner = fill_panel(frame, area, footer, Padding::horizontal(1));
    statusbar::render(
        frame,
        inner,
        ctx.skin,
        footer,
        ctx.status_left,
        ctx.status_right,
    );
}

/// Renders the backgroundless grouped hints, inset by one cell so they line up
/// with the padded panels above.
fn render_hints(
    frame: &mut Frame,
    area: Rect,
    groups: &[HintGroup<'_, String>],
    opts: &HintStyle,
) {
    let inner = Rect {
        x: area.x + 1,
        width: area.width.saturating_sub(2),
        ..area
    };
    shortcut_hints::render(frame, inner, groups, opts);
}

/// Fills `area` with `bg` and returns the region inset by `padding`.
fn fill_panel(
    frame: &mut Frame,
    area: Rect,
    bg: ratada::theme::Color,
    padding: Padding,
) -> Rect {
    let block = Block::default().style(style::bg(bg)).padding(padding);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}

/// The hint-band style: keys in the accent (bold), labels and descriptions dim,
/// one blank row above and no background, so the hints sit on the terminal's
/// own colour.
fn hint_style(skin: &Skin) -> HintStyle {
    HintStyle {
        key: style::fg(skin.palette.accent).add_modifier(Modifier::BOLD),
        label: style::secondary(&skin.palette),
        description: style::secondary(&skin.palette),
        ..HintStyle::default()
    }
}

/// Borrows owned groups as toolkit [`HintGroup`]s. The owned value must outlive
/// the result.
fn to_hint_groups(owned: &HintGroups) -> Vec<HintGroup<'_, String>> {
    owned
        .iter()
        .map(|(label, hints)| HintGroup {
            label: label.as_str(),
            hints: hints.as_slice(),
        })
        .collect()
}

/// Borrows the same owned groups as help sections, so the footer and the help
/// overlay can never drift apart.
pub fn to_help_sections(
    owned: &HintGroups,
) -> Vec<help::HelpSection<'_, String>> {
    owned
        .iter()
        .map(|(title, bindings)| help::HelpSection {
            title: title.as_str(),
            bindings: bindings.as_slice(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use ratada::theme::{
        ColorOverrides, GlyphVariant, Glyphs, Palette, ThemeRegistry,
    };
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    fn skin() -> Skin {
        let base = ThemeRegistry::builtin().resolve("default");
        Skin::new(
            Palette::resolve(base, &ColorOverrides::default()),
            Glyphs::new(GlyphVariant::Unicode),
        )
    }

    const TABS: &[(&str, &str)] = &[("1", "Calc"), ("2", "Vars")];

    fn groups() -> HintGroups {
        vec![(
            "Global".to_string(),
            vec![("f12".to_string(), "help".to_string())],
        )]
    }

    fn context<'a>(skin: &'a Skin, hints: &'a HintGroups) -> FrameContext<'a> {
        FrameContext {
            skin,
            tabs: TABS,
            active_tab: 0,
            tabs_locked: false,
            status_left: "RAD",
            status_right: "",
            hints,
        }
    }

    /// Draws the frame and returns `(screen text, content rect)`.
    fn draw(width: u16, height: u16) -> (String, Rect) {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let skin = skin();
        let hints = groups();
        let mut content = Rect::default();
        terminal
            .draw(|frame| {
                content = render_frame(frame, &context(&skin, &hints))
            })
            .expect("draw");
        let buffer = terminal.backend().buffer().clone();
        let text: String =
            buffer.content().iter().map(|cell| cell.symbol()).collect();
        (text, content)
    }

    #[test]
    fn the_frame_draws_the_brand_the_tabs_and_the_hints() {
        shortcut_hints::set_visible(true);
        let (screen, _) = draw(60, 16);
        assert!(screen.contains("calcli"));
        assert!(screen.contains("Calc"));
        assert!(screen.contains("RAD"));
        assert!(screen.contains("help"));
    }

    #[test]
    fn hiding_the_hints_reclaims_the_band_and_its_blank_separator() {
        let hints = groups();
        let borrowed = to_hint_groups(&hints);

        shortcut_hints::set_visible(true);
        let shown = shortcut_hints::height(&borrowed, 60, 1);
        shortcut_hints::set_visible(false);
        let hidden = shortcut_hints::height(&borrowed, 60, 1);
        shortcut_hints::set_visible(true);

        assert!(shown >= 2, "a hint row plus its top margin");
        assert_eq!(hidden, 0, "hiding must reclaim the margin too");
    }

    #[test]
    fn the_content_rect_grows_when_the_hints_are_hidden() {
        shortcut_hints::set_visible(true);
        let (_, with_hints) = draw(60, 16);
        shortcut_hints::set_visible(false);
        let (_, without_hints) = draw(60, 16);
        shortcut_hints::set_visible(true);
        assert!(without_hints.height > with_hints.height);
    }

    #[test]
    fn the_frame_survives_a_tiny_terminal() {
        shortcut_hints::set_visible(true);
        let (_, content) = draw(8, 4);
        assert!(content.height <= 4);
    }

    #[test]
    fn the_hint_band_never_starves_the_content() {
        // A band that wants more than the screen leaves nothing to calculate
        // in; the content keeps its minimum and the band takes what is left.
        assert_eq!(hints_height(15, 20), 14);
        assert_eq!(hints_height(4, 20), 4, "a band that fits is untouched");
    }

    #[test]
    fn a_band_with_no_room_left_disappears_entirely() {
        assert_eq!(hints_height(15, MIN_CONTENT_HEIGHT + 1), 0);
        assert_eq!(hints_height(15, MIN_CONTENT_HEIGHT), 0);
        assert_eq!(hints_height(15, 0), 0);
    }

    #[test]
    fn hidden_hints_stay_hidden_whatever_the_room() {
        // `shortcut_hints::height` already returns 0 while hidden, and the cap
        // must not resurrect the band.
        assert_eq!(hints_height(0, 100), 0);
    }

    #[test]
    fn a_narrow_terminal_still_leaves_room_for_the_content() {
        shortcut_hints::set_visible(true);
        let (_, content) = draw(24, 24);
        assert!(content.height >= 4, "content collapsed to {content:?}");
    }

    #[test]
    fn help_sections_and_hint_groups_share_one_owned_source() {
        let owned = groups();
        let hints = to_hint_groups(&owned);
        let sections = to_help_sections(&owned);
        assert_eq!(hints.len(), sections.len());
        assert_eq!(hints[0].label, sections[0].title);
        assert_eq!(hints[0].hints, sections[0].bindings);
    }
}
