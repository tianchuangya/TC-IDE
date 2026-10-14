//! Shared colour palette and the contrast rules it has to satisfy.
//!
//! Both backends drew from the same design tokens — one background, one panel, a
//! quiet border, three text weights, an accent, a selection and three syntax
//! colours — but each kept its own copy of the numbers. The light theme drifted:
//! its syntax colours were still the dark ones, so a keyword was a light blue on
//! a near white background. The palette lives here once, and the contrast of
//! every pair the interface actually uses is asserted in a unit test instead of
//! being checked by eye.
//!
//! The backends adopt the pieces they need: both take the palette, the Windows
//! renderer also takes the syntax colours, and the Windows `--self-check` walks
//! the table with the contrast helpers, which the unit tests use as well.
#![allow(dead_code)]

/// Contrast a body text colour needs against its background.
pub const TEXT_MINIMUM: f64 = 4.5;
/// Contrast a decoration, an accent or a border needs.
pub const DECORATION_MINIMUM: f64 = 3.0;

/// The colours of one theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Window background behind everything.
    pub background: u32,
    /// Panel fill, one step above the background.
    pub panel: u32,
    /// Raised control fill, one step above the panel.
    pub raised: u32,
    /// Quiet separator between regions.
    pub border: u32,
    /// Body text.
    pub text: u32,
    /// Secondary text and hints.
    pub muted: u32,
    /// Emphasised text, focused values.
    pub bright: u32,
    /// Accent used for selection, links and progress.
    pub accent: u32,
    /// Selection fill.
    pub selection: u32,
    /// Text drawn on the selection fill.
    pub selected_text: u32,
    /// Syntax colour of a keyword.
    pub keyword: u32,
    /// Syntax colour of a literal.
    pub literal: u32,
    /// Syntax colour of a comment.
    pub comment: u32,
    /// Drop shadow behind overlays.
    pub shadow: u32,
}

impl Palette {
    /// Palette of the dark theme.
    pub fn dark() -> Self {
        Self {
            background: 0x000000,
            panel: 0x1c1c1e,
            raised: 0x2c2c2e,
            border: 0x38383a,
            text: 0xd1d1d6,
            // One step lighter than the system grey: on a raised panel the
            // original read at 4.27, just under the text minimum.
            muted: 0x94949a,
            bright: 0xf5f5f7,
            accent: 0x0a84ff,
            selection: 0x193451,
            selected_text: 0x0a84ff,
            keyword: 0x64d2ff,
            literal: 0xff9f0a,
            comment: 0x8e8e93,
            shadow: 0x050506,
        }
    }

    /// Palette of the light theme.
    ///
    /// The syntax colours are not the dark ones: a light blue keyword on a near
    /// white background reads at a contrast of 1.6, which is why this palette
    /// carries its own darker keyword, literal and comment colours.
    pub fn light() -> Self {
        Self {
            background: 0xf5f5f7,
            panel: 0xffffff,
            raised: 0xf2f2f7,
            border: 0xd1d1d6,
            text: 0x3a3a3c,
            muted: 0x6e6e73,
            bright: 0x1d1d1f,
            accent: 0x007aff,
            selection: 0xdbeafe,
            selected_text: 0x0a5aa8,
            keyword: 0x0b5394,
            literal: 0x8a4b08,
            comment: 0x5f6368,
            shadow: 0xc7c7cc,
        }
    }

    /// Palette of a theme, by name of the scheme the user picked.
    pub fn for_scheme(light: bool) -> Self {
        if light {
            Self::light()
        } else {
            Self::dark()
        }
    }

    /// `#rrggbb`, the form the X11 colour parser wants.
    pub fn hex(value: u32) -> String {
        format!("#{:06x}", value & 0x00ff_ffff)
    }

    /// Every colour of the palette, for a test that walks all of them.
    pub fn all(&self) -> [(&'static str, u32); 14] {
        [
            ("background", self.background),
            ("panel", self.panel),
            ("raised", self.raised),
            ("border", self.border),
            ("text", self.text),
            ("muted", self.muted),
            ("bright", self.bright),
            ("accent", self.accent),
            ("selection", self.selection),
            ("selected_text", self.selected_text),
            ("keyword", self.keyword),
            ("literal", self.literal),
            ("comment", self.comment),
            ("shadow", self.shadow),
        ]
    }
}

/// Relative luminance of an `0xrrggbb` colour, as WCAG defines it.
pub fn relative_luminance(rgb: u32) -> f64 {
    fn channel(value: u32) -> f64 {
        let value = value as f64 / 255.0;
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }
    let red = channel((rgb >> 16) & 0xff);
    let green = channel((rgb >> 8) & 0xff);
    let blue = channel(rgb & 0xff);
    0.2126 * red + 0.7152 * green + 0.0722 * blue
}

/// Contrast ratio between two colours, from 1.0 (identical) to 21.0.
pub fn contrast_ratio(first: u32, second: u32) -> f64 {
    let first = relative_luminance(first);
    let second = relative_luminance(second);
    let (lighter, darker) = if first > second {
        (first, second)
    } else {
        (second, first)
    };
    (lighter + 0.05) / (darker + 0.05)
}

/// Whether a foreground is readable on a background.
pub fn meets_contrast(foreground: u32, background: u32, minimum: f64) -> bool {
    contrast_ratio(foreground, background) >= minimum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_matches_the_known_extremes() {
        assert!((contrast_ratio(0x000000, 0xffffff) - 21.0).abs() < 0.01);
        assert!((contrast_ratio(0x000000, 0x000000) - 1.0).abs() < 1.0e-9);
        assert!((relative_luminance(0x000000) - 0.0).abs() < 1.0e-9);
        assert!((relative_luminance(0xffffff) - 1.0).abs() < 1.0e-9);
        // Order does not matter.
        assert!(
            (contrast_ratio(0xffffff, 0x000000) - contrast_ratio(0x000000, 0xffffff)).abs()
                < 1.0e-9
        );
        assert!(meets_contrast(0x000000, 0xffffff, TEXT_MINIMUM));
        assert!(!meets_contrast(0x8e8e93, 0xffffff, TEXT_MINIMUM));
    }

    #[test]
    fn both_themes_keep_their_text_readable() {
        for (name, palette) in [("dark", Palette::dark()), ("light", Palette::light())] {
            // Text and hints also sit on raised controls and cards.
            for background in [palette.background, palette.panel, palette.raised] {
                for (role, colour) in [
                    ("text", palette.text),
                    ("bright", palette.bright),
                    ("muted", palette.muted),
                ] {
                    let ratio = contrast_ratio(colour, background);
                    assert!(
                        ratio >= TEXT_MINIMUM,
                        "{name} theme: {role} {:06x} on {:06x} reads at {ratio:.2}",
                        colour,
                        background
                    );
                }
            }
            // Syntax colours only ever appear on the code background and the
            // panel the code sits in.
            for background in [palette.background, palette.panel] {
                for (role, colour) in [
                    ("keyword", palette.keyword),
                    ("literal", palette.literal),
                    ("comment", palette.comment),
                ] {
                    let ratio = contrast_ratio(colour, background);
                    assert!(
                        ratio >= TEXT_MINIMUM,
                        "{name} theme: {role} {:06x} on {:06x} reads at {ratio:.2}",
                        colour,
                        background
                    );
                }
            }
        }
    }

    #[test]
    fn accents_borders_and_selections_stay_visible() {
        for (name, palette) in [("dark", Palette::dark()), ("light", Palette::light())] {
            assert!(
                contrast_ratio(palette.accent, palette.background) >= DECORATION_MINIMUM,
                "{name} theme: the accent disappears into the background"
            );
            assert!(
                contrast_ratio(palette.border, palette.background) >= 1.2,
                "{name} theme: the border is invisible"
            );
            assert_ne!(
                palette.selection, palette.background,
                "{name} theme: the selection has no fill"
            );
            assert!(
                contrast_ratio(palette.text, palette.selection) >= DECORATION_MINIMUM,
                "{name} theme: selected text is hard to read"
            );
            assert!(
                contrast_ratio(palette.shadow, palette.background) < 3.0,
                "{name} theme: a shadow should be quiet"
            );
        }
    }

    #[test]
    fn the_light_theme_does_not_reuse_the_dark_syntax_colours() {
        let dark = Palette::dark();
        let light = Palette::light();
        for (role, dark_colour, light_colour) in [
            ("keyword", dark.keyword, light.keyword),
            ("literal", dark.literal, light.literal),
            ("comment", dark.comment, light.comment),
        ] {
            assert_ne!(
                dark_colour, light_colour,
                "the {role} colour has to change with the theme"
            );
        }
        // The colours that were wrong before are the ones this guards.
        assert!(!meets_contrast(0x64d2ff, light.background, TEXT_MINIMUM));
        assert!(meets_contrast(
            light.keyword,
            light.background,
            TEXT_MINIMUM
        ));
        assert!(meets_contrast(
            light.literal,
            light.background,
            TEXT_MINIMUM
        ));
        assert!(meets_contrast(
            light.comment,
            light.background,
            TEXT_MINIMUM
        ));
    }

    #[test]
    fn the_palette_names_its_colours_and_picks_by_scheme() {
        assert_eq!(Palette::hex(0x0a84ff), "#0a84ff");
        assert_eq!(Palette::hex(0x000000), "#000000");
        assert_eq!(Palette::for_scheme(true), Palette::light());
        assert_eq!(Palette::for_scheme(false), Palette::dark());
        let dark = Palette::dark();
        assert_eq!(dark.all().len(), 14);
        assert!(dark
            .all()
            .iter()
            .any(|(name, value)| *name == "background" && *value == 0x000000));
        assert_ne!(Palette::light().background, dark.background);
    }
}
