//! Colors used by the UI, following the active Omarchy theme when there is one.
//!
//! Omarchy writes the active palette to `~/.local/state/omarchy/current/theme/colors.toml` and
//! swaps it on `omarchy-theme-set`. Without that file every color is the named terminal color the
//! app always used, so nothing changes outside Omarchy.

use ratatui::style::Color;
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Hue colors mirror the ANSI names they replace (`LightGreen` is `bright_green`), the rest are
/// roles the old code expressed with grays and fixed RGB values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,
    pub bright_red: Color,
    pub bright_green: Color,
    pub bright_yellow: Color,
    pub bright_blue: Color,
    pub bright_magenta: Color,
    pub bright_cyan: Color,
    pub orange: Color,
    /// Panel borders and headings.
    pub accent: Color,
    /// Regular text.
    pub text: Color,
    /// Secondary text, one step dimmer than `text`.
    pub subtle: Color,
    /// Hints, placeholders and empty values.
    pub muted: Color,
    /// Text drawn on a `bright_green` button.
    pub on_accent: Color,
    /// Table headers and neutral buttons.
    pub header_bg: Color,
    /// Popup backgrounds.
    pub panel_bg: Color,
    /// Grouping rows and highlighted cells inside a table.
    pub band_bg: Color,
    /// Totals rows.
    pub total_bg: Color,
    /// The selected month once its spending is over target.
    pub over_budget_bg: Color,
}

const DEFAULT: Theme = Theme {
    red: Color::Red,
    green: Color::Green,
    yellow: Color::Yellow,
    blue: Color::Blue,
    magenta: Color::Magenta,
    cyan: Color::Cyan,
    bright_red: Color::LightRed,
    bright_green: Color::LightGreen,
    bright_yellow: Color::LightYellow,
    bright_blue: Color::LightBlue,
    bright_magenta: Color::LightMagenta,
    bright_cyan: Color::LightCyan,
    orange: Color::Rgb(255, 165, 0),
    accent: Color::LightBlue,
    text: Color::White,
    subtle: Color::Gray,
    muted: Color::DarkGray,
    on_accent: Color::Black,
    header_bg: Color::DarkGray,
    panel_bg: Color::Black,
    band_bg: Color::Rgb(20, 20, 20),
    total_bg: Color::Rgb(10, 10, 10),
    over_budget_bg: Color::Rgb(45, 10, 10),
};

impl Default for Theme {
    fn default() -> Self {
        DEFAULT
    }
}

impl Theme {
    /// Reads an Omarchy `colors.toml`. Themes installed from git are untrusted input, so any key
    /// that is missing or not a `#rrggbb` color keeps its default instead of failing the theme.
    pub fn from_omarchy(text: &str) -> Theme {
        let Ok(table) = text.parse::<toml::Table>() else {
            return DEFAULT;
        };
        let get = |keys: &[&str]| {
            keys.iter()
                .find_map(|key| table.get(*key)?.as_str().and_then(parse_hex))
        };
        let pick = |keys: &[&str], default: Color| get(keys).map_or(default, Color::from);

        Theme {
            red: pick(&["red"], DEFAULT.red),
            green: pick(&["green"], DEFAULT.green),
            yellow: pick(&["yellow"], DEFAULT.yellow),
            blue: pick(&["blue"], DEFAULT.blue),
            magenta: pick(&["magenta"], DEFAULT.magenta),
            cyan: pick(&["cyan"], DEFAULT.cyan),
            bright_red: pick(&["bright_red", "red"], DEFAULT.bright_red),
            bright_green: pick(&["bright_green", "green"], DEFAULT.bright_green),
            bright_yellow: pick(&["bright_yellow", "yellow"], DEFAULT.bright_yellow),
            bright_blue: pick(&["bright_blue", "blue"], DEFAULT.bright_blue),
            bright_magenta: pick(&["bright_magenta", "magenta"], DEFAULT.bright_magenta),
            bright_cyan: pick(&["bright_cyan", "cyan"], DEFAULT.bright_cyan),
            orange: pick(&["orange"], DEFAULT.orange),
            accent: pick(&["accent", "blue"], DEFAULT.accent),
            text: pick(&["foreground"], DEFAULT.text),
            subtle: pick(&["light_foreground", "foreground"], DEFAULT.subtle),
            muted: pick(&["dark_foreground", "muted"], DEFAULT.muted),
            on_accent: pick(&["background"], DEFAULT.on_accent),
            header_bg: pick(&["lighter_background", "selection"], DEFAULT.header_bg),
            panel_bg: pick(&["dark_background", "background"], DEFAULT.panel_bg),
            band_bg: pick(&["dark_background", "background"], DEFAULT.band_bg),
            total_bg: pick(
                &["darker_background", "dark_background", "background"],
                DEFAULT.total_bg,
            ),
            over_budget_bg: match (get(&["red"]), get(&["background"])) {
                (Some(red), Some(background)) => Color::from(mix(background, red, 0.2)),
                _ => DEFAULT.over_budget_bg,
            },
        }
    }

    /// Colors given to a year's months in order, shared by every per-month chart.
    pub fn months(&self) -> [Color; 12] {
        [
            self.bright_red,
            self.bright_green,
            self.bright_blue,
            self.bright_yellow,
            self.bright_magenta,
            self.bright_cyan,
            self.red,
            self.green,
            self.blue,
            self.yellow,
            self.magenta,
            self.cyan,
        ]
    }
}

type Rgb = (u8, u8, u8);

fn parse_hex(value: &str) -> Option<Rgb> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some((channel(0)?, channel(2)?, channel(4)?))
}

/// `amount` of `over` blended onto `base`.
fn mix(base: Rgb, over: Rgb, amount: f32) -> Rgb {
    let blend = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount).round() as u8;
    (
        blend(base.0, over.0),
        blend(base.1, over.1),
        blend(base.2, over.2),
    )
}

// Rendering helpers are free functions far from `App`, so the palette lives here rather than
// being threaded through every one of them. Only the main thread writes it.
static CURRENT: RwLock<Theme> = RwLock::new(DEFAULT);

/// The palette to draw with right now.
pub fn current() -> Theme {
    *CURRENT.read().unwrap_or_else(|e| e.into_inner())
}

fn set(theme: Theme) {
    *CURRENT.write().unwrap_or_else(|e| e.into_inner()) = theme;
}

const CHECK_INTERVAL: Duration = Duration::from_secs(1);

/// Rereads the Omarchy palette about once a second so a theme switch shows up live.
pub struct ThemeWatcher {
    path: Option<PathBuf>,
    last: Option<String>,
    next_check: Instant,
}

impl ThemeWatcher {
    pub fn new() -> Self {
        ThemeWatcher {
            path: dirs::state_dir().map(|dir| dir.join("omarchy/current/theme/colors.toml")),
            last: None,
            next_check: Instant::now(),
        }
    }

    /// Applies the palette if it changed since the last check; true when the UI needs a redraw.
    /// The file is under a kilobyte, so comparing contents is cheaper to trust than mtimes
    /// across Omarchy's directory swap.
    pub fn poll(&mut self) -> bool {
        let now = Instant::now();
        if now < self.next_check {
            return false;
        }
        self.next_check = now + CHECK_INTERVAL;

        let contents = self
            .path
            .as_ref()
            .and_then(|path| std::fs::read_to_string(path).ok());
        if contents == self.last {
            return false;
        }
        set(contents.as_deref().map_or(DEFAULT, Theme::from_omarchy));
        self.last = contents;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"
mode = "dark"
accent = "#112233"
background = "#000000"
dark_background = "#010101"
foreground = "#eeeeee"
dark_foreground = "#777777"
red = "#ff0000"
green = "#00ff00"
bright_green = "#44ff44"
"##;

    #[test]
    fn omarchy_colors_fill_their_roles() {
        let theme = Theme::from_omarchy(SAMPLE);
        assert_eq!(theme.accent, Color::Rgb(0x11, 0x22, 0x33));
        assert_eq!(theme.text, Color::Rgb(0xee, 0xee, 0xee));
        assert_eq!(theme.muted, Color::Rgb(0x77, 0x77, 0x77));
        assert_eq!(theme.panel_bg, Color::Rgb(1, 1, 1));
        assert_eq!(theme.bright_green, Color::Rgb(0x44, 0xff, 0x44));
        assert_eq!(theme.over_budget_bg, Color::Rgb(51, 0, 0));
    }

    #[test]
    fn bright_colors_fall_back_to_the_base_hue() {
        let theme = Theme::from_omarchy(SAMPLE);
        assert_eq!(theme.bright_red, Color::Rgb(0xff, 0, 0));
    }

    #[test]
    fn missing_or_invalid_keys_keep_defaults() {
        let theme = Theme::from_omarchy("red = \"#12345\"\nblue = 7\ncyan = \"cyan\"");
        assert_eq!(theme, DEFAULT);
        assert_eq!(Theme::from_omarchy("not toml ["), DEFAULT);
    }

    #[test]
    fn light_theme_uses_its_own_foreground() {
        let theme = Theme::from_omarchy("mode = \"light\"\nforeground = \"#202020\"");
        assert_eq!(theme.text, Color::Rgb(0x20, 0x20, 0x20));
        assert_eq!(theme.subtle, Color::Rgb(0x20, 0x20, 0x20));
    }
}
