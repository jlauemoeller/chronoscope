//! Key syntax: parsing, normalisation, matching and display labels.

use std::fmt;

use tuirealm::event::{Key, KeyEvent, KeyModifiers};

/// A key string that could not be parsed. The message names the string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyParseError {
    /// The full string that was rejected.
    pub input: String,
    /// What is wrong with it.
    pub reason: String,
}

impl fmt::Display for KeyParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid key {:?}: {}", self.input, self.reason)
    }
}

impl std::error::Error for KeyParseError {}

/// One key press in normalised form.
///
/// Normalisation makes terminal quirks irrelevant:
/// - `SHIFT` is dropped for character keys (terminals report `G` and `?`
///   with or without it); a lowercase letter that carries `SHIFT` becomes the
///   uppercase letter, so `shift-g` is `G`;
/// - `shift-tab`, `BackTab` and `Tab` + `SHIFT` are all `BackTab` without a
///   modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeySpec {
    code: Key,
    modifiers: KeyModifiers,
}

impl KeySpec {
    /// Normalises a key code and modifiers.
    fn new(code: Key, mut modifiers: KeyModifiers) -> Self {
        let mut code = code;
        match code {
            Key::Char(c) => {
                if modifiers.contains(KeyModifiers::SHIFT) {
                    let mut upper = c.to_uppercase();
                    if let (Some(u), None) = (upper.next(), upper.next()) {
                        code = Key::Char(u);
                    }
                }
                modifiers.remove(KeyModifiers::SHIFT);
            }
            Key::Tab if modifiers.contains(KeyModifiers::SHIFT) => {
                code = Key::BackTab;
                modifiers.remove(KeyModifiers::SHIFT);
            }
            Key::BackTab => modifiers.remove(KeyModifiers::SHIFT),
            _ => {}
        }
        Self { code, modifiers }
    }

    /// The key a tui-realm key event stands for.
    pub fn from_event(event: &KeyEvent) -> Self {
        Self::new(event.code, event.modifiers)
    }

    /// A key event for this key, for subscriptions that need one.
    pub fn to_event(self) -> KeyEvent {
        KeyEvent::new(self.code, self.modifiers)
    }

    /// Whether this is `Ctrl-c`, which is never part of the keymap.
    pub fn is_ctrl_c(&self) -> bool {
        self.code == Key::Char('c') && self.modifiers.contains(KeyModifiers::CONTROL)
    }

    /// Parses a single key such as `a`, `ctrl-d`, `shift-tab` or `f5`.
    fn parse(s: &str) -> Result<Self, String> {
        let mut rest = s;
        let mut modifiers = KeyModifiers::NONE;
        while let Some((prefix, tail)) = split_modifier(rest) {
            modifiers |= prefix;
            rest = tail;
        }
        if rest.is_empty() {
            return Err(format!("{s:?} has no key after the modifier"));
        }
        let code = if rest.chars().count() == 1 {
            Key::Char(rest.chars().next().unwrap_or(' '))
        } else {
            named_key(&rest.to_ascii_lowercase())
                .ok_or_else(|| format!("unknown key name {rest:?}"))?
        };
        Ok(Self::new(code, modifiers))
    }

    /// The label of this key, e.g. `y`, `Ctrl-d`, `Shift-TAB`, `PgDn`.
    pub fn label(&self) -> String {
        let mut out = String::new();
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            out.push_str("Ctrl-");
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            out.push_str("Alt-");
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            out.push_str("Shift-");
        }
        match self.code {
            Key::Char(' ') => out.push_str("SPACE"),
            Key::Char(c) => out.push(c),
            Key::Enter => out.push_str("ENTER"),
            Key::Esc => out.push_str("ESC"),
            Key::Tab => out.push_str("TAB"),
            Key::BackTab => out.push_str("Shift-TAB"),
            Key::Backspace => out.push_str("Backspace"),
            Key::Up => out.push_str("Up"),
            Key::Down => out.push_str("Down"),
            Key::Left => out.push_str("Left"),
            Key::Right => out.push_str("Right"),
            Key::Home => out.push_str("Home"),
            Key::End => out.push_str("End"),
            Key::PageUp => out.push_str("PgUp"),
            Key::PageDown => out.push_str("PgDn"),
            Key::Delete => out.push_str("Delete"),
            Key::Insert => out.push_str("Insert"),
            Key::Function(n) => out.push_str(&format!("F{n}")),
            other => out.push_str(&format!("{other:?}")),
        }
        out
    }
}

impl fmt::Display for KeySpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}

/// Splits a leading `ctrl-` / `alt-` / `shift-` (any case) off `s`.
fn split_modifier(s: &str) -> Option<(KeyModifiers, &str)> {
    for (name, modifier) in [
        ("ctrl-", KeyModifiers::CONTROL),
        ("alt-", KeyModifiers::ALT),
        ("shift-", KeyModifiers::SHIFT),
    ] {
        if let Some(head) = s.get(..name.len())
            && head.eq_ignore_ascii_case(name)
        {
            return Some((modifier, &s[name.len()..]));
        }
    }
    None
}

/// Looks up a lowercase key name.
fn named_key(name: &str) -> Option<Key> {
    Some(match name {
        "enter" => Key::Enter,
        "esc" => Key::Esc,
        "tab" => Key::Tab,
        "backspace" => Key::Backspace,
        "space" => Key::Char(' '),
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        "home" => Key::Home,
        "end" => Key::End,
        "pgup" => Key::PageUp,
        "pgdn" => Key::PageDown,
        "delete" => Key::Delete,
        "insert" => Key::Insert,
        _ => {
            let n: u8 = name.strip_prefix('f')?.parse().ok()?;
            if !(1..=12).contains(&n) || name.len() != 1 + n.to_string().len() {
                return None;
            }
            Key::Function(n)
        }
    })
}

/// A binding: one key, or a two-key chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeySeq {
    first: KeySpec,
    second: Option<KeySpec>,
}

impl KeySeq {
    /// A single-key binding.
    pub fn single(key: KeySpec) -> Self {
        Self {
            first: key,
            second: None,
        }
    }

    /// Parses `"y"`, `"ctrl-d"` or a chord such as `"g g"`.
    pub fn parse(s: &str) -> Result<Self, KeyParseError> {
        let err = |reason: String| KeyParseError {
            input: s.to_owned(),
            reason,
        };
        let parts: Vec<&str> = s.split(' ').collect();
        if parts.len() > 2 {
            return Err(err("a chord has at most two keys".to_owned()));
        }
        let mut keys = Vec::with_capacity(2);
        for part in parts {
            if part.is_empty() {
                return Err(err("empty key (keys are separated by one space)".to_owned()));
            }
            keys.push(KeySpec::parse(part).map_err(err)?);
        }
        Ok(Self {
            first: keys[0],
            second: keys.get(1).copied(),
        })
    }

    /// The first key (the only one unless this is a chord).
    pub fn first(&self) -> KeySpec {
        self.first
    }

    /// The second key of a chord.
    pub fn second(&self) -> Option<KeySpec> {
        self.second
    }

    /// Whether this is a two-key chord.
    pub fn is_chord(&self) -> bool {
        self.second.is_some()
    }

    /// The keys, first to last.
    pub fn keys(&self) -> impl Iterator<Item = KeySpec> {
        std::iter::once(self.first).chain(self.second)
    }

    /// The display label; a chord is `g g`.
    pub fn label(&self) -> String {
        match self.second {
            Some(second) => format!("{} {}", self.first.label(), second.label()),
            None => self.first.label(),
        }
    }
}

impl fmt::Display for KeySeq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}
