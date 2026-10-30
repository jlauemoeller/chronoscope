//! The theme model (SPEC §8.1).
//!
//! A [`Theme`] has two layers: the base roles of a [`Palette`] and optional
//! component-token overrides. Every [`Token`] resolves to a Ratatui
//! [`Style`], derived from a base [`Role`] unless overridden. Components take a
//! resolved theme as a prop and ask it for styles and role colours; there is no
//! way to read a colour from a theme other than through [`Theme::role`] and
//! [`Theme::style`].
//!
//! Colour parsing lives in [`color`], theme-file parsing in [`parse`] and the
//! compiled-in themes in [`builtin`]. The list of available themes is the
//! [`registry`], and [`resolve`] picks the active one for the terminal.

pub mod builtin;
pub mod color;
pub mod parse;
pub mod registry;
pub mod resolve;

use std::collections::BTreeMap;

use ratatui::style::{Color, Modifier, Style};

pub use color::{ColorParseError, parse_color};
pub use parse::ThemeError;
pub use registry::{ThemeEntry, ThemeRegistry};
pub use resolve::{ResolveError, TerminalEnv, resolve, resolve_by_id};

/// A base colour role (every §8.1 base role except `name`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Foreground,
    MutedForeground,
    Background,
    Surface,
    Field,
    Primary,
    PrimaryForeground,
    Secondary,
    SecondaryForeground,
    Accent,
    Destructive,
    DestructiveForeground,
    Warning,
    Border,
    Ring,
    Cursor,
}

impl Role {
    /// Every role, in the order of the §8.1 list.
    pub const ALL: [Role; 16] = [
        Role::Foreground,
        Role::MutedForeground,
        Role::Background,
        Role::Surface,
        Role::Field,
        Role::Primary,
        Role::PrimaryForeground,
        Role::Secondary,
        Role::SecondaryForeground,
        Role::Accent,
        Role::Destructive,
        Role::DestructiveForeground,
        Role::Warning,
        Role::Border,
        Role::Ring,
        Role::Cursor,
    ];

    /// The key naming this role in a theme file.
    pub const fn key(self) -> &'static str {
        match self {
            Role::Foreground => "foreground",
            Role::MutedForeground => "muted_foreground",
            Role::Background => "background",
            Role::Surface => "surface",
            Role::Field => "field",
            Role::Primary => "primary",
            Role::PrimaryForeground => "primary_foreground",
            Role::Secondary => "secondary",
            Role::SecondaryForeground => "secondary_foreground",
            Role::Accent => "accent",
            Role::Destructive => "destructive",
            Role::DestructiveForeground => "destructive_foreground",
            Role::Warning => "warning",
            Role::Border => "border",
            Role::Ring => "ring",
            Role::Cursor => "cursor",
        }
    }
}

/// The colours of the 16 base roles (everything in a theme but its name).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub foreground: Color,
    pub muted_foreground: Color,
    pub background: Color,
    pub surface: Color,
    pub field: Color,
    pub primary: Color,
    pub primary_foreground: Color,
    pub secondary: Color,
    pub secondary_foreground: Color,
    pub accent: Color,
    pub destructive: Color,
    pub destructive_foreground: Color,
    pub warning: Color,
    pub border: Color,
    pub ring: Color,
    pub cursor: Color,
}

impl Palette {
    fn get(&self, role: Role) -> Color {
        match role {
            Role::Foreground => self.foreground,
            Role::MutedForeground => self.muted_foreground,
            Role::Background => self.background,
            Role::Surface => self.surface,
            Role::Field => self.field,
            Role::Primary => self.primary,
            Role::PrimaryForeground => self.primary_foreground,
            Role::Secondary => self.secondary,
            Role::SecondaryForeground => self.secondary_foreground,
            Role::Accent => self.accent,
            Role::Destructive => self.destructive,
            Role::DestructiveForeground => self.destructive_foreground,
            Role::Warning => self.warning,
            Role::Border => self.border,
            Role::Ring => self.ring,
            Role::Cursor => self.cursor,
        }
    }

    fn set(&mut self, role: Role, color: Color) {
        match role {
            Role::Foreground => self.foreground = color,
            Role::MutedForeground => self.muted_foreground = color,
            Role::Background => self.background = color,
            Role::Surface => self.surface = color,
            Role::Field => self.field = color,
            Role::Primary => self.primary = color,
            Role::PrimaryForeground => self.primary_foreground = color,
            Role::Secondary => self.secondary = color,
            Role::SecondaryForeground => self.secondary_foreground = color,
            Role::Accent => self.accent = color,
            Role::Destructive => self.destructive = color,
            Role::DestructiveForeground => self.destructive_foreground = color,
            Role::Warning => self.warning = color,
            Role::Border => self.border = color,
            Role::Ring => self.ring = color,
            Role::Cursor => self.cursor = color,
        }
    }

    fn map(mut self, f: impl Fn(Color) -> Color) -> Self {
        for role in Role::ALL {
            self.set(role, f(self.get(role)));
        }
        self
    }
}

/// A component token (§8.1 table plus the `syntax.*` tokens).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Token {
    UserTitle,
    AssistantTitle,
    SystemTitle,
    Timestamp,
    ToolDescription,
    Label,
    Command,
    Output,
    ErrorOutput,
    DiffAdded,
    DiffRemoved,
    Thinking,
    SelectionMarker,
    SearchMatch,
    HelpKey,
    HelpText,
    Toast,
    SyntaxKeyword,
    SyntaxType,
    SyntaxFunction,
    SyntaxString,
    SyntaxNumber,
    SyntaxComment,
    SyntaxPlain,
}

impl Token {
    /// Every token, in the order of the §8.1 table.
    pub const ALL: [Token; 24] = [
        Token::UserTitle,
        Token::AssistantTitle,
        Token::SystemTitle,
        Token::Timestamp,
        Token::ToolDescription,
        Token::Label,
        Token::Command,
        Token::Output,
        Token::ErrorOutput,
        Token::DiffAdded,
        Token::DiffRemoved,
        Token::Thinking,
        Token::SelectionMarker,
        Token::SearchMatch,
        Token::HelpKey,
        Token::HelpText,
        Token::Toast,
        Token::SyntaxKeyword,
        Token::SyntaxType,
        Token::SyntaxFunction,
        Token::SyntaxString,
        Token::SyntaxNumber,
        Token::SyntaxComment,
        Token::SyntaxPlain,
    ];

    /// The key naming this token in a theme file, e.g. `user_title` or
    /// `syntax.keyword`.
    pub const fn key(self) -> &'static str {
        match self {
            Token::UserTitle => "user_title",
            Token::AssistantTitle => "assistant_title",
            Token::SystemTitle => "system_title",
            Token::Timestamp => "timestamp",
            Token::ToolDescription => "tool_description",
            Token::Label => "label",
            Token::Command => "command",
            Token::Output => "output",
            Token::ErrorOutput => "error_output",
            Token::DiffAdded => "diff_added",
            Token::DiffRemoved => "diff_removed",
            Token::Thinking => "thinking",
            Token::SelectionMarker => "selection_marker",
            Token::SearchMatch => "search_match",
            Token::HelpKey => "help_key",
            Token::HelpText => "help_text",
            Token::Toast => "toast",
            Token::SyntaxKeyword => "syntax.keyword",
            Token::SyntaxType => "syntax.type",
            Token::SyntaxFunction => "syntax.function",
            Token::SyntaxString => "syntax.string",
            Token::SyntaxNumber => "syntax.number",
            Token::SyntaxComment => "syntax.comment",
            Token::SyntaxPlain => "syntax.plain",
        }
    }

    /// The base roles the token takes its foreground and background from when
    /// not overridden.
    const fn default_roles(self) -> (Role, Option<Role>) {
        match self {
            Token::UserTitle => (Role::Primary, None),
            Token::AssistantTitle => (Role::Accent, None),
            Token::SystemTitle => (Role::MutedForeground, None),
            Token::Timestamp => (Role::MutedForeground, None),
            Token::ToolDescription => (Role::Foreground, None),
            Token::Label => (Role::MutedForeground, None),
            Token::Command => (Role::Foreground, None),
            Token::Output => (Role::MutedForeground, None),
            Token::ErrorOutput => (Role::Destructive, None),
            Token::DiffAdded => (Role::Primary, None),
            Token::DiffRemoved => (Role::Destructive, None),
            Token::Thinking => (Role::MutedForeground, None),
            Token::SelectionMarker => (Role::Primary, None),
            Token::SearchMatch => (Role::PrimaryForeground, Some(Role::Primary)),
            Token::HelpKey => (Role::Primary, None),
            Token::HelpText => (Role::MutedForeground, None),
            Token::Toast => (Role::Foreground, Some(Role::Surface)),
            Token::SyntaxKeyword => (Role::Accent, None),
            Token::SyntaxType => (Role::Accent, None),
            Token::SyntaxFunction => (Role::Primary, None),
            Token::SyntaxString => (Role::Warning, None),
            Token::SyntaxNumber => (Role::Warning, None),
            Token::SyntaxComment => (Role::MutedForeground, None),
            Token::SyntaxPlain => (Role::Foreground, None),
        }
    }
}

/// An override of one token. Unset parts keep the token's derived default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenOverride {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    /// Modifiers added to the token's style.
    pub modifiers: Modifier,
}

impl TokenOverride {
    /// An override of the foreground only.
    pub fn fg(color: Color) -> Self {
        Self {
            fg: Some(color),
            ..Self::default()
        }
    }

    /// An override of foreground and background.
    pub fn fg_bg(fg: Color, bg: Color) -> Self {
        Self {
            fg: Some(fg),
            bg: Some(bg),
            ..Self::default()
        }
    }

    fn map_colors(mut self, f: impl Fn(Color) -> Color) -> Self {
        self.fg = self.fg.map(&f);
        self.bg = self.bg.map(&f);
        self
    }
}

/// A resolved theme: a name, the base roles, and the token overrides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    name: String,
    transparent_background: bool,
    palette: Palette,
    overrides: BTreeMap<Token, TokenOverride>,
}

impl Theme {
    /// A theme with the given display name and base roles, and no token
    /// overrides.
    pub fn new(name: impl Into<String>, palette: Palette) -> Self {
        Self {
            name: name.into(),
            transparent_background: false,
            palette,
            overrides: BTreeMap::new(),
        }
    }

    /// The display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether `background` is left unpainted so the terminal's own shows
    /// through.
    pub fn transparent_background(&self) -> bool {
        self.transparent_background
    }

    /// Sets `transparent_background`.
    #[must_use]
    pub fn with_transparent_background(mut self, transparent: bool) -> Self {
        self.transparent_background = transparent;
        self
    }

    /// Replaces the colour of one base role. Tokens derived from it follow.
    #[must_use]
    pub fn with_role(mut self, role: Role, color: Color) -> Self {
        self.palette.set(role, color);
        self
    }

    /// Overrides one token, leaving every other token untouched.
    #[must_use]
    pub fn with_token(mut self, token: Token, over: TokenOverride) -> Self {
        self.overrides.insert(token, over);
        self
    }

    /// The colour of a base role.
    pub fn role(&self, role: Role) -> Color {
        self.palette.get(role)
    }

    /// The resolved style of a component token.
    ///
    /// The foreground is always set. The background is set only for
    /// `search_match` and `toast` (or where an override gives one).
    pub fn style(&self, token: Token) -> Style {
        let (fg_role, bg_role) = token.default_roles();
        let over = self.overrides.get(&token).copied().unwrap_or_default();
        let fg = over.fg.unwrap_or_else(|| self.role(fg_role));
        let bg = over.bg.or_else(|| bg_role.map(|r| self.role(r)));
        let mut style = Style::default().fg(fg).add_modifier(over.modifiers);
        if let Some(bg) = bg {
            style = style.bg(bg);
        }
        style
    }

    /// Parses a theme from a TOML document. See [`parse`] for the format.
    pub fn from_toml(source: &str) -> Result<Self, ThemeError> {
        parse::from_toml(source)
    }

    /// A copy with every `Color::Rgb` replaced by the nearest xterm-256 colour
    /// (cube 16-231 or grey ramp 232-255, never 0-15). Named, indexed and
    /// reset colours are unchanged.
    #[must_use]
    pub fn to_ansi256(&self) -> Self {
        self.map_colors(color::to_ansi256)
    }

    /// A copy for `NO_COLOR`: every role and token colour is `Color::Reset`,
    /// and `search_match` is reverse video so hits stay visible.
    #[must_use]
    pub fn without_colors(&self) -> Self {
        let mut theme = self.map_colors(|_| Color::Reset);
        theme.overrides.clear();
        theme.overrides.insert(
            Token::SearchMatch,
            TokenOverride {
                modifiers: Modifier::REVERSED,
                ..TokenOverride::default()
            },
        );
        theme
    }

    fn map_colors(&self, f: impl Fn(Color) -> Color + Copy) -> Self {
        Self {
            name: self.name.clone(),
            transparent_background: self.transparent_background,
            palette: self.palette.map(f),
            overrides: self
                .overrides
                .iter()
                .map(|(token, over)| (*token, over.map_colors(f)))
                .collect(),
        }
    }
}

impl Default for Theme {
    /// The built-in default theme.
    fn default() -> Self {
        builtin::default_theme()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A palette whose 16 roles are all distinct, so a token resolving to the
    /// wrong role is caught.
    fn distinct() -> Theme {
        let c = |i: u8| Color::Rgb(i, 100 + i, 200);
        Theme::new(
            "Distinct",
            Palette {
                foreground: c(1),
                muted_foreground: c(2),
                background: c(3),
                surface: c(4),
                field: c(5),
                primary: c(6),
                primary_foreground: c(7),
                secondary: c(8),
                secondary_foreground: c(9),
                accent: c(10),
                destructive: c(11),
                destructive_foreground: c(12),
                warning: c(13),
                border: c(14),
                ring: c(15),
                cursor: c(16),
            },
        )
    }

    /// (token, default fg role, default bg role) straight from the §8.1 table.
    const DEFAULTS: [(Token, Role, Option<Role>); 24] = [
        (Token::UserTitle, Role::Primary, None),
        (Token::AssistantTitle, Role::Accent, None),
        (Token::SystemTitle, Role::MutedForeground, None),
        (Token::Timestamp, Role::MutedForeground, None),
        (Token::ToolDescription, Role::Foreground, None),
        (Token::Label, Role::MutedForeground, None),
        (Token::Command, Role::Foreground, None),
        (Token::Output, Role::MutedForeground, None),
        (Token::ErrorOutput, Role::Destructive, None),
        (Token::DiffAdded, Role::Primary, None),
        (Token::DiffRemoved, Role::Destructive, None),
        (Token::Thinking, Role::MutedForeground, None),
        (Token::SelectionMarker, Role::Primary, None),
        (
            Token::SearchMatch,
            Role::PrimaryForeground,
            Some(Role::Primary),
        ),
        (Token::HelpKey, Role::Primary, None),
        (Token::HelpText, Role::MutedForeground, None),
        (Token::Toast, Role::Foreground, Some(Role::Surface)),
        (Token::SyntaxKeyword, Role::Accent, None),
        (Token::SyntaxType, Role::Accent, None),
        (Token::SyntaxFunction, Role::Primary, None),
        (Token::SyntaxString, Role::Warning, None),
        (Token::SyntaxNumber, Role::Warning, None),
        (Token::SyntaxComment, Role::MutedForeground, None),
        (Token::SyntaxPlain, Role::Foreground, None),
    ];

    #[test]
    fn defaults_table_covers_every_token_once() {
        let mut tokens: Vec<Token> = DEFAULTS.iter().map(|d| d.0).collect();
        tokens.sort();
        let mut all = Token::ALL.to_vec();
        all.sort();
        assert_eq!(tokens, all);
    }

    #[test]
    fn every_token_resolves_to_its_default_role() {
        let theme = distinct();
        for (token, fg, bg) in DEFAULTS {
            let style = theme.style(token);
            assert_eq!(style.fg, Some(theme.role(fg)), "{} fg", token.key());
            assert_eq!(style.bg, bg.map(|r| theme.role(r)), "{} bg", token.key());
            assert_eq!(style.add_modifier, Modifier::empty(), "{}", token.key());
        }
    }

    #[test]
    fn search_match_and_toast_have_their_explicit_backgrounds() {
        let theme = distinct();
        assert_eq!(
            theme.style(Token::SearchMatch),
            Style::default()
                .fg(theme.role(Role::PrimaryForeground))
                .bg(theme.role(Role::Primary))
        );
        assert_eq!(
            theme.style(Token::Toast),
            Style::default()
                .fg(theme.role(Role::Foreground))
                .bg(theme.role(Role::Surface))
        );
    }

    #[test]
    fn token_and_role_keys_are_unique() {
        let mut keys: Vec<&str> = Token::ALL.iter().map(|t| t.key()).collect();
        keys.extend(Role::ALL.iter().map(|r| r.key()));
        let n = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), n);
    }

    #[test]
    fn overriding_a_token_changes_only_that_token() {
        let base = distinct();
        let red = Color::Rgb(255, 0, 0);
        let changed = base
            .clone()
            .with_token(Token::UserTitle, TokenOverride::fg(red));
        for token in Token::ALL {
            if token == Token::UserTitle {
                assert_eq!(changed.style(token), Style::default().fg(red));
            } else {
                assert_eq!(changed.style(token), base.style(token), "{}", token.key());
            }
        }
        for role in Role::ALL {
            assert_eq!(changed.role(role), base.role(role));
        }
    }

    #[test]
    fn overriding_one_part_of_a_two_colour_token_keeps_the_other() {
        let base = distinct();
        let red = Color::Rgb(255, 0, 0);
        let changed = base.clone().with_token(
            Token::SearchMatch,
            TokenOverride {
                bg: Some(red),
                ..TokenOverride::default()
            },
        );
        assert_eq!(
            changed.style(Token::SearchMatch),
            Style::default()
                .fg(base.role(Role::PrimaryForeground))
                .bg(red)
        );
    }

    #[test]
    fn changing_a_role_changes_every_derived_token_that_is_not_overridden() {
        let green = Color::Rgb(0, 255, 0);
        let blue = Color::Rgb(0, 0, 255);
        // `help_key` is overridden, so it must not follow `primary`.
        let base = distinct().with_token(Token::HelpKey, TokenOverride::fg(blue));
        for role in Role::ALL {
            let changed = base.clone().with_role(role, green);
            assert_eq!(changed.role(role), green);
            for (token, fg, bg) in DEFAULTS {
                let before = base.style(token);
                let after = changed.style(token);
                let overridden = token == Token::HelpKey;
                let mut want = before;
                if !overridden && fg == role {
                    want.fg = Some(green);
                }
                if bg == Some(role) {
                    want.bg = Some(green);
                }
                assert_eq!(after, want, "role {} token {}", role.key(), token.key());
            }
        }
    }

    #[test]
    fn to_ansi256_maps_rgb_in_roles_and_overrides() {
        let theme = distinct()
            .with_role(Role::Primary, Color::Rgb(0xff, 0, 0))
            .with_role(Role::Background, Color::Rgb(0, 0, 0))
            .with_role(Role::Foreground, Color::Rgb(0xff, 0xff, 0xff))
            .with_role(Role::Border, Color::Rgb(0x80, 0x80, 0x80))
            .with_role(Role::Accent, Color::Magenta)
            .with_role(Role::Warning, Color::Reset)
            .with_role(Role::Cursor, Color::Indexed(5))
            .with_token(Token::Label, TokenOverride::fg(Color::Rgb(0xff, 0, 0)));
        let mapped = theme.to_ansi256();
        assert_eq!(mapped.role(Role::Primary), Color::Indexed(196));
        assert_eq!(mapped.role(Role::Background), Color::Indexed(16));
        assert_eq!(mapped.role(Role::Foreground), Color::Indexed(231));
        assert_eq!(mapped.role(Role::Border), Color::Indexed(244));
        assert_eq!(mapped.role(Role::Accent), Color::Magenta);
        assert_eq!(mapped.role(Role::Warning), Color::Reset);
        assert_eq!(mapped.role(Role::Cursor), Color::Indexed(5));
        assert_eq!(mapped.style(Token::Label).fg, Some(Color::Indexed(196)));
        assert_eq!(mapped.style(Token::UserTitle).fg, Some(Color::Indexed(196)));
        assert_eq!(mapped.style(Token::SyntaxKeyword).fg, Some(Color::Magenta));
        assert_eq!(mapped.name(), theme.name());
        for token in Token::ALL {
            let s = mapped.style(token);
            for c in [s.fg, s.bg].into_iter().flatten() {
                assert!(!matches!(c, Color::Rgb(..)), "{}", token.key());
            }
        }
    }

    #[test]
    fn to_ansi256_keeps_the_terminal_theme_unchanged() {
        let theme = builtin::get("terminal").unwrap();
        assert_eq!(theme.to_ansi256(), theme);
    }

    #[test]
    fn without_colors_resets_everything_and_reverses_search_match() {
        for entry in builtin::all() {
            let theme = entry
                .theme
                .with_token(Token::Label, TokenOverride::fg(Color::Red))
                .with_transparent_background(true)
                .without_colors();
            for role in Role::ALL {
                assert_eq!(
                    theme.role(role),
                    Color::Reset,
                    "{} {}",
                    entry.id,
                    role.key()
                );
            }
            for token in Token::ALL {
                let style = theme.style(token);
                assert_eq!(style.fg, Some(Color::Reset), "{} {}", entry.id, token.key());
                assert!(style.bg.is_none_or(|c| c == Color::Reset));
                let reversed = style.add_modifier.contains(Modifier::REVERSED);
                assert_eq!(reversed, token == Token::SearchMatch, "{}", token.key());
            }
            assert!(theme.transparent_background());
        }
    }

    #[test]
    fn transparent_background_defaults_to_false_and_can_be_set() {
        let theme = distinct();
        assert!(!theme.transparent_background());
        assert!(
            theme
                .with_transparent_background(true)
                .transparent_background()
        );
    }
}
