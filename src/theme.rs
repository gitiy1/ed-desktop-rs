use gpui::{Rgba, WindowAppearance, rgb};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    #[default]
    System,
    Dark,
    Light,
}

impl ThemePreference {
    pub fn is_dark(self, appearance: WindowAppearance) -> bool {
        match self {
            Self::System => matches!(
                appearance,
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            ),
            Self::Dark => true,
            Self::Light => false,
        }
    }

    pub fn palette(self, appearance: WindowAppearance) -> Palette {
        if self.is_dark(appearance) {
            Palette::dark()
        } else {
            Palette::light()
        }
    }
}

/// Semantic colors shared by every page and widget.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub background: Rgba,
    pub surface: Rgba,
    pub inset: Rgba,
    pub selected: Rgba,
    pub border: Rgba,
    pub text: Rgba,
    pub secondary: Rgba,
    pub muted: Rgba,
    pub accent: Rgba,
    pub accent_hover: Rgba,
    pub on_accent: Rgba,
    pub focus: Rgba,
    pub button: Rgba,
    pub button_hover: Rgba,
    pub input: Rgba,
    pub progress_track: Rgba,
    pub paused: Rgba,
    pub sidebar: Rgba,
    pub sidebar_text: Rgba,
    pub sidebar_muted: Rgba,
    pub sidebar_border: Rgba,
    pub nav_selected: Rgba,
    pub nav_hover: Rgba,
}

impl Palette {
    pub fn light() -> Self {
        Self {
            background: rgb(0xf3f6f4),
            surface: rgb(0xffffff),
            inset: rgb(0xf8faf9),
            selected: rgb(0xf0f7f3),
            border: rgb(0xd8e2dc),
            text: rgb(0x253630),
            secondary: rgb(0x5e7066),
            muted: rgb(0x687970),
            accent: rgb(0x167d70),
            accent_hover: rgb(0x11685d),
            on_accent: rgb(0xffffff),
            focus: rgb(0x168576),
            button: rgb(0xe8eeeb),
            button_hover: rgb(0xd9e5de),
            input: rgb(0xffffff),
            progress_track: rgb(0xe5ebe8),
            paused: rgb(0x997326),
            sidebar: rgb(0x20332e),
            sidebar_text: rgb(0xf4f7f5),
            sidebar_muted: rgb(0xb8c8c0),
            sidebar_border: rgb(0x43564f),
            nav_selected: rgb(0x38564c),
            nav_hover: rgb(0x426257),
        }
    }

    pub fn dark() -> Self {
        Self {
            background: rgb(0x121b18),
            surface: rgb(0x1b2722),
            inset: rgb(0x17211d),
            selected: rgb(0x223c32),
            border: rgb(0x384c41),
            text: rgb(0xe4eee8),
            secondary: rgb(0xb2c4b9),
            muted: rgb(0x9bb1a3),
            accent: rgb(0x69c9aa),
            accent_hover: rgb(0x8ddbbf),
            on_accent: rgb(0x102b21),
            focus: rgb(0x69c9aa),
            button: rgb(0x2a3b32),
            button_hover: rgb(0x384f41),
            input: rgb(0x15201a),
            progress_track: rgb(0x35483c),
            paused: rgb(0xdeb768),
            sidebar: rgb(0x0f1914),
            sidebar_text: rgb(0xe4eee8),
            sidebar_muted: rgb(0x9eb5a6),
            sidebar_border: rgb(0x304438),
            nav_selected: rgb(0x294b3c),
            nav_hover: rgb(0x345b49),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_follows_all_platform_appearances_while_overrides_remain_fixed() {
        for appearance in [
            WindowAppearance::Light,
            WindowAppearance::VibrantLight,
            WindowAppearance::Dark,
            WindowAppearance::VibrantDark,
        ] {
            assert_eq!(ThemePreference::Dark.palette(appearance), Palette::dark());
            assert_eq!(ThemePreference::Light.palette(appearance), Palette::light());
            assert_eq!(
                ThemePreference::System.is_dark(appearance),
                matches!(
                    appearance,
                    WindowAppearance::Dark | WindowAppearance::VibrantDark
                )
            );
        }
    }

    fn luminance(color: Rgba) -> f32 {
        let linear = |channel: f32| {
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
    }

    #[test]
    fn both_palettes_keep_text_and_buttons_readable() {
        for palette in [Palette::light(), Palette::dark()] {
            for (foreground, background) in [
                (palette.text, palette.surface),
                (palette.secondary, palette.surface),
                (palette.muted, palette.input),
                (palette.text, palette.button),
                (palette.on_accent, palette.accent),
                (palette.on_accent, palette.accent_hover),
                (palette.sidebar_text, palette.sidebar),
                (palette.sidebar_muted, palette.sidebar),
            ] {
                let (a, b) = (luminance(foreground), luminance(background));
                let contrast = (a.max(b) + 0.05) / (a.min(b) + 0.05);
                assert!(
                    contrast >= 4.5,
                    "text contrast {contrast:.2} is too low: {foreground:?} on {background:?}"
                );
            }
        }
    }
}
