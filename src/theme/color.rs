//! Colour parsing and the xterm-256 mapping.

use std::fmt;

use ratatui::style::Color;

/// A colour string that is not `#rrggbb`, an ANSI name, an index or `reset`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorParseError {
    value: String,
}

impl ColorParseError {
    /// The offending input.
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for ColorParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid colour {:?}: expected #rrggbb, an ANSI name, an index 0-255 or \"reset\"",
            self.value
        )
    }
}

impl std::error::Error for ColorParseError {}

/// ANSI colour names with their Ratatui colours, in ANSI index order 0-15.
const NAMED: [(&str, Color); 16] = [
    ("black", Color::Black),
    ("red", Color::Red),
    ("green", Color::Green),
    ("yellow", Color::Yellow),
    ("blue", Color::Blue),
    ("magenta", Color::Magenta),
    ("cyan", Color::Cyan),
    ("white", Color::Gray),
    ("bright_black", Color::DarkGray),
    ("bright_red", Color::LightRed),
    ("bright_green", Color::LightGreen),
    ("bright_yellow", Color::LightYellow),
    ("bright_blue", Color::LightBlue),
    ("bright_magenta", Color::LightMagenta),
    ("bright_cyan", Color::LightCyan),
    ("bright_white", Color::White),
];

/// Parses a colour: `#rrggbb` (any case), an ANSI name (any case), an index
/// `0`-`255`, or `reset` (the terminal default).
pub fn parse_color(input: &str) -> Result<Color, ColorParseError> {
    let err = || ColorParseError {
        value: input.to_string(),
    };
    if let Some(hex) = input.strip_prefix('#') {
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(err());
        }
        let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| err());
        return Ok(Color::Rgb(channel(0)?, channel(2)?, channel(4)?));
    }
    if !input.is_empty() && input.bytes().all(|b| b.is_ascii_digit()) {
        return input.parse::<u8>().map(Color::Indexed).map_err(|_| err());
    }
    let lower = input.to_ascii_lowercase();
    if lower == "reset" {
        return Ok(Color::Reset);
    }
    NAMED
        .iter()
        .find(|(name, _)| *name == lower)
        .map(|(_, color)| *color)
        .ok_or_else(err)
}

/// The six channel levels of the xterm 6x6x6 colour cube.
const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

fn dist_sq(a: (u8, u8, u8), b: (u8, u8, u8)) -> u32 {
    let d = |x: u8, y: u8| u32::from(x.abs_diff(y)).pow(2);
    d(a.0, b.0) + d(a.1, b.1) + d(a.2, b.2)
}

fn nearest_level(channel: u8) -> usize {
    (0..CUBE_LEVELS.len())
        .min_by_key(|&i| CUBE_LEVELS[i].abs_diff(channel))
        .unwrap_or(0)
}

/// The nearest xterm-256 colour index (16-231 cube, 232-255 grey ramp) to an
/// RGB colour. Indices 0-15 are never returned: terminals redefine them.
pub fn nearest_ansi256(r: u8, g: u8, b: u8) -> u8 {
    let (ri, gi, bi) = (nearest_level(r), nearest_level(g), nearest_level(b));
    let cube_rgb = (CUBE_LEVELS[ri], CUBE_LEVELS[gi], CUBE_LEVELS[bi]);
    let cube_index = 16 + 36 * ri + 6 * gi + bi;

    let mean = (u32::from(r) + u32::from(g) + u32::from(b)) / 3;
    let grey_step = ((mean.saturating_sub(8) + 5) / 10).min(23);
    let grey_level = u8::try_from(8 + 10 * grey_step).unwrap_or(238);
    let grey_dist = dist_sq((r, g, b), (grey_level, grey_level, grey_level));

    let index = if grey_dist < dist_sq((r, g, b), cube_rgb) {
        232 + usize::try_from(grey_step).unwrap_or(0)
    } else {
        cube_index
    };
    u8::try_from(index).unwrap_or(231)
}

/// Replaces an RGB colour by its nearest xterm-256 colour; other colours are
/// unchanged.
pub fn to_ansi256(color: Color) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Indexed(nearest_ansi256(r, g, b)),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_colours() {
        let cases: &[(&str, Color)] = &[
            ("#ff0000", Color::Rgb(255, 0, 0)),
            ("#FF00aA", Color::Rgb(255, 0, 170)),
            ("#000000", Color::Rgb(0, 0, 0)),
            ("black", Color::Black),
            ("red", Color::Red),
            ("green", Color::Green),
            ("yellow", Color::Yellow),
            ("blue", Color::Blue),
            ("magenta", Color::Magenta),
            ("cyan", Color::Cyan),
            ("white", Color::Gray),
            ("bright_black", Color::DarkGray),
            ("bright_red", Color::LightRed),
            ("bright_green", Color::LightGreen),
            ("bright_yellow", Color::LightYellow),
            ("bright_blue", Color::LightBlue),
            ("bright_magenta", Color::LightMagenta),
            ("bright_cyan", Color::LightCyan),
            ("bright_white", Color::White),
            ("Bright_Blue", Color::LightBlue),
            ("RED", Color::Red),
            ("0", Color::Indexed(0)),
            ("7", Color::Indexed(7)),
            ("196", Color::Indexed(196)),
            ("255", Color::Indexed(255)),
            ("reset", Color::Reset),
            ("Reset", Color::Reset),
        ];
        for (input, expected) in cases {
            assert_eq!(parse_color(input), Ok(*expected), "input {input:?}");
        }
    }

    #[test]
    fn rejects_malformed_colours_naming_the_value() {
        for input in [
            "#12345",
            "#1234567",
            "#gggggg",
            "#",
            "256",
            "-1",
            "+5",
            "purple",
            "",
            " red",
            "#ééé",
            "1.5",
            "bright",
            "rgb(1,2,3)",
        ] {
            let err = parse_color(input).expect_err(input);
            assert_eq!(err.value(), input);
            assert!(
                err.to_string().contains(&format!("{input:?}")),
                "message {err} should name {input:?}"
            );
        }
    }

    #[test]
    fn ansi256_mapping_of_known_colours() {
        let cases = [
            ((255, 0, 0), 196),
            ((0, 0, 0), 16),
            ((255, 255, 255), 231),
            ((128, 128, 128), 244),
            ((8, 8, 8), 232),
            ((238, 238, 238), 255),
            ((0, 255, 0), 46),
            ((0, 0, 255), 21),
        ];
        for ((r, g, b), expected) in cases {
            assert_eq!(nearest_ansi256(r, g, b), expected, "({r},{g},{b})");
        }
    }

    #[test]
    fn ansi256_never_uses_the_first_sixteen_and_only_touches_rgb() {
        for r in (0..=255).step_by(5) {
            for g in (0..=255).step_by(5) {
                for b in (0..=255).step_by(5) {
                    assert!(nearest_ansi256(r as u8, g as u8, b as u8) >= 16);
                }
            }
        }
        for color in [Color::Red, Color::Reset, Color::Indexed(3), Color::White] {
            assert_eq!(to_ansi256(color), color);
        }
        assert_eq!(to_ansi256(Color::Rgb(255, 0, 0)), Color::Indexed(196));
    }
}
