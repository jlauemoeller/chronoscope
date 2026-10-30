//! The seven themes compiled into the binary (SPEC §8.3).
//!
//! The palettes are ported from [ratcn](https://crates.io/crates/ratcn) 0.0.6
//! (MIT, Copyright (c) 2026 Kristofer Lund; see `THIRD-PARTY-NOTICES.md`),
//! including its `dim` arithmetic for the derived `ring` and `surface` roles.
//! The one deliberate difference is the Terminal theme, which uses only named
//! ANSI colours and `reset` (§9.4): ratcn's RGB `surface`, `field` and
//! `secondary` become `black`, `black` and `bright_black`.

use ratatui::style::Color;

use super::{Palette, Theme};

/// The id of the default theme.
pub const DEFAULT_ID: &str = "default";

/// A compiled-in theme with the id used in `config.toml` and `--theme`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinTheme {
    pub id: &'static str,
    pub theme: Theme,
}

/// The ids of the built-in themes, in display order.
pub const IDS: [&str; 7] = [
    "default",
    "terminal",
    "catppuccin",
    "gruvbox",
    "nord",
    "tokyo-night",
    "solarized",
];

/// All built-in themes, in the order of [`IDS`].
pub fn all() -> Vec<BuiltinTheme> {
    IDS.iter()
        .filter_map(|id| get(id).map(|theme| BuiltinTheme { id, theme }))
        .collect()
}

/// The built-in theme with the given id.
pub fn get(id: &str) -> Option<Theme> {
    Some(match id {
        "default" => default_dark(),
        "terminal" => terminal(),
        "catppuccin" => catppuccin(),
        "gruvbox" => gruvbox(),
        "nord" => nord(),
        "tokyo-night" => tokyo_night(),
        "solarized" => solarized(),
        _ => return None,
    })
}

/// The default built-in theme (id [`DEFAULT_ID`]).
pub fn default_theme() -> Theme {
    default_dark()
}

/// ratcn's `RING_DIM`: how far `ring` is dimmed from `primary` toward
/// `background`.
const RING_DIM: u16 = 10;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

/// ratcn's `dim` for RGB colours: blends `color` toward `toward` by `amount`
/// percent, rounding to nearest. Other colours pass `color` through unchanged.
fn dim(color: Color, toward: Color, amount: u16) -> Color {
    match (color, toward) {
        (Color::Rgb(r, g, b), Color::Rgb(tr, tg, tb)) => {
            let keep = 100 - amount;
            let mix = |a: u8, b: u8| ((a as u16 * keep + b as u16 * amount + 50) / 100) as u8;
            Color::Rgb(mix(r, tr), mix(g, tg), mix(b, tb))
        }
        _ => color,
    }
}

fn default_dark() -> Theme {
    let primary = rgb(229, 229, 229);
    let background = rgb(10, 10, 10);
    Theme::new(
        "Default",
        Palette {
            foreground: rgb(250, 250, 250),
            muted_foreground: rgb(161, 161, 161),
            background,
            surface: rgb(18, 18, 18),
            field: rgb(31, 31, 31),
            primary,
            primary_foreground: rgb(23, 23, 23),
            secondary: rgb(38, 38, 38),
            secondary_foreground: rgb(250, 250, 250),
            accent: rgb(139, 92, 246),
            destructive: rgb(255, 100, 103),
            destructive_foreground: rgb(23, 23, 23),
            warning: rgb(250, 204, 21),
            border: rgb(94, 94, 94),
            ring: dim(primary, background, RING_DIM),
            cursor: rgb(115, 115, 115),
        },
    )
}

fn terminal() -> Theme {
    let primary = Color::White;
    let background = Color::Reset;
    Theme::new(
        "Terminal",
        Palette {
            foreground: Color::Reset,
            muted_foreground: Color::Gray,
            background,
            surface: Color::Black,
            field: Color::Black,
            primary,
            primary_foreground: Color::Black,
            secondary: Color::DarkGray,
            secondary_foreground: Color::Reset,
            accent: Color::LightMagenta,
            destructive: Color::LightRed,
            destructive_foreground: Color::Black,
            warning: Color::Yellow,
            border: Color::DarkGray,
            // `dim` toward `reset` passes `primary` through unchanged.
            ring: dim(primary, background, RING_DIM),
            cursor: Color::LightBlue,
        },
    )
}

fn catppuccin() -> Theme {
    let primary = rgb(137, 180, 250);
    let background = rgb(17, 17, 27);
    let field = rgb(30, 30, 46);
    Theme::new(
        "Catppuccin",
        Palette {
            foreground: rgb(205, 214, 244),
            muted_foreground: rgb(166, 173, 200),
            background,
            surface: dim(background, field, 30),
            field,
            primary,
            primary_foreground: rgb(17, 17, 27),
            secondary: rgb(49, 50, 68),
            secondary_foreground: rgb(205, 214, 244),
            accent: rgb(203, 166, 247),
            destructive: rgb(243, 139, 168),
            destructive_foreground: rgb(17, 17, 27),
            warning: rgb(250, 179, 135),
            border: rgb(108, 112, 134),
            ring: dim(primary, background, RING_DIM),
            cursor: rgb(137, 180, 250),
        },
    )
}

fn gruvbox() -> Theme {
    let primary = rgb(250, 189, 47);
    let background = rgb(40, 40, 40);
    Theme::new(
        "Gruvbox",
        Palette {
            foreground: rgb(235, 219, 178),
            muted_foreground: rgb(213, 196, 161),
            background,
            surface: rgb(50, 48, 47),
            field: rgb(60, 56, 54),
            primary,
            primary_foreground: rgb(40, 40, 40),
            secondary: rgb(80, 73, 69),
            secondary_foreground: rgb(235, 219, 178),
            accent: rgb(254, 128, 25),
            destructive: rgb(251, 73, 52),
            destructive_foreground: rgb(29, 32, 33),
            warning: rgb(250, 189, 47),
            border: rgb(124, 111, 100),
            ring: dim(primary, background, RING_DIM),
            cursor: rgb(250, 189, 47),
        },
    )
}

fn nord() -> Theme {
    let primary = rgb(136, 192, 208);
    let background = rgb(46, 52, 64);
    let field = rgb(59, 66, 82);
    Theme::new(
        "Nord",
        Palette {
            foreground: rgb(236, 239, 244),
            muted_foreground: rgb(216, 222, 233),
            background,
            surface: dim(background, field, 50),
            field,
            primary,
            primary_foreground: rgb(46, 52, 64),
            secondary: rgb(76, 86, 106),
            secondary_foreground: rgb(236, 239, 244),
            accent: rgb(180, 142, 173),
            destructive: rgb(191, 97, 106),
            destructive_foreground: rgb(236, 239, 244),
            warning: rgb(235, 203, 139),
            border: rgb(112, 126, 155),
            ring: dim(primary, background, RING_DIM),
            cursor: rgb(136, 192, 208),
        },
    )
}

fn tokyo_night() -> Theme {
    let primary = rgb(122, 162, 247);
    let background = rgb(26, 27, 38);
    let field = rgb(41, 46, 66);
    Theme::new(
        "Tokyo Night",
        Palette {
            foreground: rgb(192, 202, 245),
            muted_foreground: rgb(169, 177, 214),
            background,
            surface: dim(background, field, 50),
            field,
            primary,
            primary_foreground: rgb(26, 27, 38),
            secondary: rgb(40, 52, 87),
            secondary_foreground: rgb(192, 202, 245),
            accent: rgb(187, 154, 247),
            destructive: rgb(247, 118, 142),
            destructive_foreground: rgb(26, 27, 38),
            warning: rgb(224, 175, 104),
            border: rgb(115, 122, 162),
            ring: dim(primary, background, RING_DIM),
            cursor: rgb(122, 162, 247),
        },
    )
}

fn solarized() -> Theme {
    let primary = rgb(38, 139, 210);
    let background = rgb(0, 43, 54);
    let field = rgb(7, 54, 66);
    Theme::new(
        "Solarized",
        Palette {
            foreground: rgb(238, 232, 213),
            muted_foreground: rgb(147, 161, 161),
            background,
            surface: dim(background, field, 30),
            field,
            primary,
            primary_foreground: rgb(253, 246, 227),
            secondary: rgb(88, 110, 117),
            secondary_foreground: rgb(253, 246, 227),
            accent: rgb(211, 54, 130),
            destructive: rgb(220, 50, 47),
            destructive_foreground: rgb(253, 246, 227),
            warning: rgb(181, 137, 0),
            border: rgb(101, 123, 131),
            ring: dim(primary, background, RING_DIM),
            cursor: rgb(42, 161, 152),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Role, Token};

    /// `0xRRGGBB` as a colour.
    fn hex(v: u32) -> Color {
        Color::Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    /// Expected base roles in `Role::ALL` order, as literals. `ring` and the
    /// `surface` values that ratcn computes with `dim` are evaluated by hand.
    fn expected(id: &str) -> (&'static str, [Color; 16]) {
        match id {
            "default" => (
                "Default",
                [
                    hex(0xfafafa), // foreground
                    hex(0xa1a1a1), // muted_foreground
                    hex(0x0a0a0a), // background
                    hex(0x121212), // surface
                    hex(0x1f1f1f), // field
                    hex(0xe5e5e5), // primary
                    hex(0x171717), // primary_foreground
                    hex(0x262626), // secondary
                    hex(0xfafafa), // secondary_foreground
                    hex(0x8b5cf6), // accent
                    hex(0xff6467), // destructive
                    hex(0x171717), // destructive_foreground
                    hex(0xfacc15), // warning
                    hex(0x5e5e5e), // border
                    hex(0xcfcfcf), // ring
                    hex(0x737373), // cursor
                ],
            ),
            "terminal" => (
                "Terminal",
                [
                    Color::Reset,        // foreground
                    Color::Gray,         // muted_foreground
                    Color::Reset,        // background
                    Color::Black,        // surface
                    Color::Black,        // field
                    Color::White,        // primary
                    Color::Black,        // primary_foreground
                    Color::DarkGray,     // secondary
                    Color::Reset,        // secondary_foreground
                    Color::LightMagenta, // accent
                    Color::LightRed,     // destructive
                    Color::Black,        // destructive_foreground
                    Color::Yellow,       // warning
                    Color::DarkGray,     // border
                    Color::White,        // ring
                    Color::LightBlue,    // cursor
                ],
            ),
            "catppuccin" => (
                "Catppuccin",
                [
                    hex(0xcdd6f4),
                    hex(0xa6adc8),
                    hex(0x11111b),
                    hex(0x151521), // surface = dim(background, field, 30)
                    hex(0x1e1e2e),
                    hex(0x89b4fa),
                    hex(0x11111b),
                    hex(0x313244),
                    hex(0xcdd6f4),
                    hex(0xcba6f7),
                    hex(0xf38ba8),
                    hex(0x11111b),
                    hex(0xfab387),
                    hex(0x6c7086),
                    hex(0x7da4e4), // ring
                    hex(0x89b4fa),
                ],
            ),
            "gruvbox" => (
                "Gruvbox",
                [
                    hex(0xebdbb2),
                    hex(0xd5c4a1),
                    hex(0x282828),
                    hex(0x32302f),
                    hex(0x3c3836),
                    hex(0xfabd2f),
                    hex(0x282828),
                    hex(0x504945),
                    hex(0xebdbb2),
                    hex(0xfe8019),
                    hex(0xfb4934),
                    hex(0x1d2021),
                    hex(0xfabd2f),
                    hex(0x7c6f64),
                    hex(0xe5ae2e), // ring
                    hex(0xfabd2f),
                ],
            ),
            "nord" => (
                "Nord",
                [
                    hex(0xeceff4),
                    hex(0xd8dee9),
                    hex(0x2e3440),
                    hex(0x353b49), // surface = dim(background, field, 50)
                    hex(0x3b4252),
                    hex(0x88c0d0),
                    hex(0x2e3440),
                    hex(0x4c566a),
                    hex(0xeceff4),
                    hex(0xb48ead),
                    hex(0xbf616a),
                    hex(0xeceff4),
                    hex(0xebcb8b),
                    hex(0x707e9b),
                    hex(0x7fb2c2), // ring
                    hex(0x88c0d0),
                ],
            ),
            "tokyo-night" => (
                "Tokyo Night",
                [
                    hex(0xc0caf5),
                    hex(0xa9b1d6),
                    hex(0x1a1b26),
                    hex(0x222534), // surface = dim(background, field, 50)
                    hex(0x292e42),
                    hex(0x7aa2f7),
                    hex(0x1a1b26),
                    hex(0x283457),
                    hex(0xc0caf5),
                    hex(0xbb9af7),
                    hex(0xf7768e),
                    hex(0x1a1b26),
                    hex(0xe0af68),
                    hex(0x737aa2),
                    hex(0x7095e2), // ring
                    hex(0x7aa2f7),
                ],
            ),
            "solarized" => (
                "Solarized",
                [
                    hex(0xeee8d5),
                    hex(0x93a1a1),
                    hex(0x002b36),
                    hex(0x022e3a), // surface = dim(background, field, 30)
                    hex(0x073642),
                    hex(0x268bd2),
                    hex(0xfdf6e3),
                    hex(0x586e75),
                    hex(0xfdf6e3),
                    hex(0xd33682),
                    hex(0xdc322f),
                    hex(0xfdf6e3),
                    hex(0xb58900),
                    hex(0x657b83),
                    hex(0x2281c2), // ring
                    hex(0x2aa198),
                ],
            ),
            other => panic!("no expectations for {other}"),
        }
    }

    #[test]
    fn seven_builtins_in_order_with_ids_and_display_names() {
        let got: Vec<(&str, String)> = all()
            .into_iter()
            .map(|b| (b.id, b.theme.name().to_string()))
            .collect();
        let want = [
            ("default", "Default"),
            ("terminal", "Terminal"),
            ("catppuccin", "Catppuccin"),
            ("gruvbox", "Gruvbox"),
            ("nord", "Nord"),
            ("tokyo-night", "Tokyo Night"),
            ("solarized", "Solarized"),
        ];
        assert_eq!(got.len(), want.len());
        for ((id, name), (want_id, want_name)) in got.iter().zip(want) {
            assert_eq!(*id, want_id);
            assert_eq!(name, want_name);
        }
    }

    #[test]
    fn default_theme_is_the_default_id() {
        assert_eq!(DEFAULT_ID, "default");
        assert_eq!(default_theme(), get(DEFAULT_ID).unwrap());
        assert_eq!(Theme::default(), get("default").unwrap());
        assert_eq!(Theme::default().name(), "Default");
    }

    #[test]
    fn unknown_id_is_none() {
        assert!(get("nope").is_none());
        assert!(get("").is_none());
    }

    #[test]
    fn every_role_of_every_builtin_matches_ratcn() {
        for id in IDS {
            let theme = get(id).unwrap();
            let (name, roles) = expected(id);
            assert_eq!(theme.name(), name, "{id} name");
            assert!(!theme.transparent_background(), "{id} transparent");
            for (role, want) in Role::ALL.into_iter().zip(roles) {
                assert_eq!(theme.role(role), want, "{id}.{}", role.key());
            }
        }
    }

    #[test]
    fn terminal_theme_uses_only_ansi_colours_and_reset() {
        let theme = get("terminal").unwrap();
        let ansi_only = |c: Color| !matches!(c, Color::Rgb(..) | Color::Indexed(_));
        for role in Role::ALL {
            assert!(ansi_only(theme.role(role)), "role {}", role.key());
        }
        for token in Token::ALL {
            let style = theme.style(token);
            assert!(style.fg.is_some_and(ansi_only), "token {} fg", token.key());
            assert!(style.bg.is_none_or(ansi_only), "token {} bg", token.key());
        }
    }

    #[test]
    fn dim_matches_ratcn_rounding() {
        // Mirrors ratcn's `dim` unit tests (RGB endpoints).
        let black = rgb(0, 0, 0);
        let white = rgb(255, 255, 255);
        let gray = rgb(128, 128, 128);
        assert_eq!(dim(gray, black, 0), gray);
        assert_eq!(dim(gray, black, 100), black);
        assert_eq!(dim(gray, white, 100), white);
        assert_eq!(dim(rgb(1, 1, 1), black, 50), rgb(1, 1, 1));
        assert_eq!(dim(rgb(3, 3, 3), black, 50), rgb(2, 2, 2));
        assert_eq!(dim(rgb(254, 254, 254), white, 50), rgb(255, 255, 255));
        assert_eq!(dim(white, black, 1), rgb(252, 252, 252));
        assert_eq!(dim(Color::White, Color::Reset, 10), Color::White);
    }
}
