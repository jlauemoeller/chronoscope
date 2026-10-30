//! Parsing a theme from a TOML document.
//!
//! Format:
//!
//! ```toml
//! name = "My Theme"                 # required display name
//! transparent_background = false    # optional
//! foreground = "#fafafa"            # every base role is required
//! # ... muted_foreground, background, surface, field, primary, ...
//!
//! [tokens]                          # optional overrides
//! user_title = "bright_blue"
//! search_match = { fg = "#000000", bg = "yellow" }   # either key optional
//! toast = { bg = "#202020" }
//!
//! [tokens.syntax]
//! keyword = "#ff00ff"
//! ```
//!
//! Colours use [`parse_color`]. Unknown keys are ignored.

use std::fmt;

use toml::{Table, Value};

use super::{Palette, Role, Theme, Token, TokenOverride, parse_color};

/// Why a theme document could not be turned into a [`Theme`].
#[derive(Debug, Clone, PartialEq)]
pub enum ThemeError {
    /// The document is not valid TOML.
    Syntax(String),
    /// A required key (`name` or a base role) is missing.
    MissingKey(String),
    /// A key has a value of the wrong type.
    WrongType { key: String, expected: &'static str },
    /// A key holds an invalid colour.
    InvalidColor {
        key: String,
        source: super::ColorParseError,
    },
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThemeError::Syntax(msg) => write!(f, "invalid theme TOML: {msg}"),
            ThemeError::MissingKey(key) => write!(f, "missing required key `{key}`"),
            ThemeError::WrongType { key, expected } => {
                write!(f, "`{key}` must be {expected}")
            }
            ThemeError::InvalidColor { key, source } => write!(f, "`{key}`: {source}"),
        }
    }
}

impl std::error::Error for ThemeError {}

/// Parses a complete theme document.
pub fn from_toml(source: &str) -> Result<Theme, ThemeError> {
    let table: Table = source
        .parse()
        .map_err(|e: toml::de::Error| ThemeError::Syntax(e.to_string()))?;

    let name = match table.get("name") {
        None => return Err(ThemeError::MissingKey("name".into())),
        Some(Value::String(s)) => s.clone(),
        Some(_) => return Err(wrong_type("name", "a string")),
    };

    let mut colors = Vec::with_capacity(Role::ALL.len());
    for role in Role::ALL {
        let key = role.key();
        let value = table
            .get(key)
            .ok_or_else(|| ThemeError::MissingKey(key.into()))?;
        colors.push(color_from(key, value)?);
    }
    let palette = Palette {
        foreground: colors[0],
        muted_foreground: colors[1],
        background: colors[2],
        surface: colors[3],
        field: colors[4],
        primary: colors[5],
        primary_foreground: colors[6],
        secondary: colors[7],
        secondary_foreground: colors[8],
        accent: colors[9],
        destructive: colors[10],
        destructive_foreground: colors[11],
        warning: colors[12],
        border: colors[13],
        ring: colors[14],
        cursor: colors[15],
    };

    let mut theme = Theme::new(name, palette);
    match table.get("transparent_background") {
        None => {}
        Some(Value::Boolean(b)) => theme = theme.with_transparent_background(*b),
        Some(_) => return Err(wrong_type("transparent_background", "a boolean")),
    }

    if let Some(tokens) = table.get("tokens") {
        let tokens = tokens
            .as_table()
            .ok_or_else(|| wrong_type("tokens", "a table"))?;
        for token in Token::ALL {
            if let Some(over) = token_override(tokens, token)? {
                theme = theme.with_token(token, over);
            }
        }
    }
    Ok(theme)
}

fn wrong_type(key: &str, expected: &'static str) -> ThemeError {
    ThemeError::WrongType {
        key: key.into(),
        expected,
    }
}

fn color_from(key: &str, value: &Value) -> Result<ratatui::style::Color, ThemeError> {
    let text = match value {
        Value::String(s) => s.clone(),
        // Indices may be written as bare integers.
        Value::Integer(i) => i.to_string(),
        _ => return Err(wrong_type(key, "a colour string")),
    };
    parse_color(&text).map_err(|source| ThemeError::InvalidColor {
        key: key.into(),
        source,
    })
}

/// Looks up the override for `token` in the `[tokens]` table.
fn token_override(tokens: &Table, token: Token) -> Result<Option<TokenOverride>, ThemeError> {
    let key = token.key();
    let full_key = format!("tokens.{key}");
    let value = match key.strip_prefix("syntax.") {
        Some(name) => match tokens.get("syntax") {
            None => return Ok(None),
            Some(Value::Table(syntax)) => syntax.get(name),
            Some(_) => return Err(wrong_type("tokens.syntax", "a table")),
        },
        None => tokens.get(key),
    };
    let Some(value) = value else {
        return Ok(None);
    };
    if matches!(token, Token::SearchMatch | Token::Toast) {
        let table = value
            .as_table()
            .ok_or_else(|| wrong_type(&full_key, "a table like { fg = \"…\", bg = \"…\" }"))?;
        let part = |name: &str| -> Result<_, ThemeError> {
            table
                .get(name)
                .map(|v| color_from(&format!("{full_key}.{name}"), v))
                .transpose()
        };
        Ok(Some(TokenOverride {
            fg: part("fg")?,
            bg: part("bg")?,
            ..TokenOverride::default()
        }))
    } else {
        Ok(Some(TokenOverride::fg(color_from(&full_key, value)?)))
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::{Color, Style};

    use super::*;

    const BASE: &str = r##"
name = "Test Theme"
foreground = "#010101"
muted_foreground = "#020202"
background = "#030303"
surface = "#040404"
field = "#050505"
primary = "#060606"
primary_foreground = "#070707"
secondary = "#080808"
secondary_foreground = "#090909"
accent = "#0a0a0a"
destructive = "#0b0b0b"
destructive_foreground = "#0c0c0c"
warning = "#0d0d0d"
border = "#0e0e0e"
ring = "#0f0f0f"
cursor = "#101010"
"##;

    fn grey(v: u8) -> Color {
        Color::Rgb(v, v, v)
    }

    fn expected_palette() -> Palette {
        Palette {
            foreground: grey(1),
            muted_foreground: grey(2),
            background: grey(3),
            surface: grey(4),
            field: grey(5),
            primary: grey(6),
            primary_foreground: grey(7),
            secondary: grey(8),
            secondary_foreground: grey(9),
            accent: grey(10),
            destructive: grey(11),
            destructive_foreground: grey(12),
            warning: grey(13),
            border: grey(14),
            ring: grey(15),
            cursor: grey(16),
        }
    }

    #[test]
    fn complete_document_parses_to_the_expected_theme() {
        let theme = Theme::from_toml(BASE).unwrap();
        assert_eq!(theme, Theme::new("Test Theme", expected_palette()));
        assert!(!theme.transparent_background());
    }

    #[test]
    fn transparent_background_is_read() {
        let doc = format!("transparent_background = true\n{BASE}");
        assert!(Theme::from_toml(&doc).unwrap().transparent_background());
    }

    #[test]
    fn each_missing_base_role_or_name_is_named_in_the_error() {
        let keys: Vec<&str> = std::iter::once("name")
            .chain(Role::ALL.iter().map(|r| r.key()))
            .collect();
        for key in keys {
            let doc: String = BASE
                .lines()
                .filter(|l| !l.starts_with(&format!("{key} =")))
                .collect::<Vec<_>>()
                .join("\n");
            let err = Theme::from_toml(&doc).unwrap_err();
            assert_eq!(err, ThemeError::MissingKey(key.into()), "removed {key}");
            assert!(err.to_string().contains(&format!("`{key}`")));
        }
    }

    #[test]
    fn invalid_colour_names_the_key_and_value() {
        let doc = BASE.replace("accent = \"#0a0a0a\"", "accent = \"purple\"");
        let err = Theme::from_toml(&doc).unwrap_err();
        let msg = err.to_string();
        assert!(matches!(&err, ThemeError::InvalidColor { key, .. } if key == "accent"));
        assert!(msg.contains("accent") && msg.contains("purple"), "{msg}");
    }

    #[test]
    fn invalid_toml_and_wrong_types_are_errors() {
        assert!(matches!(
            Theme::from_toml("name = "),
            Err(ThemeError::Syntax(_))
        ));
        let doc = BASE.replace("name = \"Test Theme\"", "name = 3");
        assert!(matches!(
            Theme::from_toml(&doc),
            Err(ThemeError::WrongType { key, .. }) if key == "name"
        ));
        let doc = format!("transparent_background = \"yes\"\n{BASE}");
        assert!(matches!(
            Theme::from_toml(&doc),
            Err(ThemeError::WrongType { key, .. }) if key == "transparent_background"
        ));
    }

    #[test]
    fn indices_may_be_bare_integers() {
        let doc = BASE.replace("accent = \"#0a0a0a\"", "accent = 99");
        let theme = Theme::from_toml(&doc).unwrap();
        assert_eq!(theme.role(Role::Accent), Color::Indexed(99));
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let doc = format!("mystery = 1\n{BASE}\n[tokens]\nnot_a_token = \"red\"\n");
        assert_eq!(
            Theme::from_toml(&doc).unwrap(),
            Theme::new("Test Theme", expected_palette())
        );
    }

    #[test]
    fn simple_token_and_syntax_overrides_are_applied() {
        let doc = format!(
            "{BASE}\n[tokens]\nuser_title = \"bright_blue\"\ndiff_added = \"green\"\n\n\
             [tokens.syntax]\nkeyword = \"#ff00ff\"\ntype = \"3\"\n"
        );
        let theme = Theme::from_toml(&doc).unwrap();
        let plain = Theme::new("Test Theme", expected_palette());
        let expected = plain
            .with_token(Token::UserTitle, TokenOverride::fg(Color::LightBlue))
            .with_token(Token::DiffAdded, TokenOverride::fg(Color::Green))
            .with_token(
                Token::SyntaxKeyword,
                TokenOverride::fg(Color::Rgb(255, 0, 255)),
            )
            .with_token(Token::SyntaxType, TokenOverride::fg(Color::Indexed(3)));
        assert_eq!(theme, expected);
        assert_eq!(
            theme.style(Token::UserTitle),
            Style::default().fg(Color::LightBlue)
        );
        assert_eq!(
            theme.style(Token::SyntaxKeyword),
            Style::default().fg(Color::Rgb(255, 0, 255))
        );
        // Untouched tokens still derive from the base roles.
        assert_eq!(theme.style(Token::AssistantTitle).fg, Some(grey(10)));
        assert_eq!(theme.style(Token::DiffRemoved).fg, Some(grey(11)));
    }

    #[test]
    fn search_match_accepts_fg_and_bg_and_either_may_be_omitted() {
        let both =
            format!("{BASE}\n[tokens]\nsearch_match = {{ fg = \"#000000\", bg = \"yellow\" }}\n");
        let theme = Theme::from_toml(&both).unwrap();
        assert_eq!(
            theme.style(Token::SearchMatch),
            Style::default().fg(Color::Rgb(0, 0, 0)).bg(Color::Yellow)
        );

        let fg_only = format!("{BASE}\n[tokens]\nsearch_match = {{ fg = \"red\" }}\n");
        let style = Theme::from_toml(&fg_only)
            .unwrap()
            .style(Token::SearchMatch);
        assert_eq!(style, Style::default().fg(Color::Red).bg(grey(6)));

        let bg_only = format!("{BASE}\n[tokens]\ntoast = {{ bg = \"red\" }}\n");
        let style = Theme::from_toml(&bg_only).unwrap().style(Token::Toast);
        assert_eq!(style, Style::default().fg(grey(1)).bg(Color::Red));
    }

    #[test]
    fn invalid_token_colour_names_the_key() {
        let doc = format!("{BASE}\n[tokens]\nlabel = \"nope\"\n");
        let err = Theme::from_toml(&doc).unwrap_err();
        assert!(matches!(&err, ThemeError::InvalidColor { key, .. } if key == "tokens.label"));

        let doc = format!("{BASE}\n[tokens.syntax]\nstring = \"nope\"\n");
        let err = Theme::from_toml(&doc).unwrap_err();
        assert!(
            matches!(&err, ThemeError::InvalidColor { key, .. } if key == "tokens.syntax.string")
        );

        let doc = format!("{BASE}\n[tokens]\nsearch_match = {{ bg = \"nope\" }}\n");
        let err = Theme::from_toml(&doc).unwrap_err();
        assert!(
            matches!(&err, ThemeError::InvalidColor { key, .. } if key == "tokens.search_match.bg")
        );
    }

    #[test]
    fn search_match_must_be_a_table() {
        let doc = format!("{BASE}\n[tokens]\nsearch_match = \"red\"\n");
        assert!(matches!(
            Theme::from_toml(&doc),
            Err(ThemeError::WrongType { key, .. }) if key == "tokens.search_match"
        ));
    }
}
