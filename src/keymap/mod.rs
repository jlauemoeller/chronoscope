//! The shared keymap (SPEC §1.4, §6, §8.5).
//!
//! Every component turns key events into [`Action`]s through a [`Keymap`], for
//! its own [`Context`]. The keymap holds the default bindings of SPEC §6 with
//! the user's `[keys.<context>]` overrides applied, and the help metadata
//! (group + description) from which the help line and the help overlay are
//! generated.
//!
//! `Ctrl-c` is not part of the keymap: it always quits and is handled
//! literally by the global event handler and by `Prompt`. The keymap never
//! resolves it, and a user binding of it is rejected.

mod defaults;
mod key;

use tuirealm::event::KeyEvent;

use crate::config::{ConfigWarning, RawKeys};
use defaults::DEFAULTS;
pub use key::{KeyParseError, KeySeq, KeySpec};

#[cfg(test)]
mod tests;

/// Where a key is resolved. Each context has a `[keys.<name>]` config table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Context {
    /// Quit, help, theme selector, session selector. Handled by `GlobalEvents`.
    Global,
    /// The session browser.
    Browser,
    /// The session selector.
    Selector,
    /// The help overlay and the theme selector.
    Overlay,
}

impl Context {
    /// All contexts, `Global` first.
    pub const ALL: [Context; 4] = [
        Context::Global,
        Context::Browser,
        Context::Selector,
        Context::Overlay,
    ];

    /// The name used in `[keys.<name>]`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Browser => "browser",
            Self::Selector => "selector",
            Self::Overlay => "overlay",
        }
    }

    /// The context with the config name `name`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }

    /// Whether user config may bind two-key chords here.
    pub fn supports_chords(self) -> bool {
        matches!(self, Self::Browser | Self::Selector)
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// Something a key can do. An action is identified by its context and itself:
/// `Search`, `Up`, `Down` and `Resume` exist in more than one context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    // Global.
    Quit,
    Help,
    Theme,
    OpenSelector,
    // Browser.
    Prev,
    Next,
    PrevTurn,
    NextTurn,
    HalfPageUp,
    HalfPageDown,
    PageUp,
    PageDown,
    Top,
    Bottom,
    FoldToggle,
    Fold,
    Unfold,
    FoldAll,
    UnfoldAll,
    ToggleSystem,
    Copy,
    CopyFull,
    Save,
    SaveFull,
    Details,
    Search,
    SearchNext,
    SearchPrev,
    ClearSearch,
    OpenSubagent,
    Back,
    Resume,
    Export,
    ExportFull,
    // Selector.
    Harness1,
    Harness2,
    Harness3,
    Harness4,
    NextPane,
    PrevPane,
    FocusProjects,
    FocusSessions,
    Up,
    Down,
    Open,
    Cancel,
    // Overlay (plus `Up` and `Down`).
    Select,
    Close,
}

/// One action of a context with its current keys and help metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub action: Action,
    /// The action name used in `[keys.<context>]`.
    pub name: &'static str,
    /// The active keys, first = the one shown in hints. Empty when unbound.
    pub keys: Vec<KeySeq>,
    pub group: &'static str,
    pub description: &'static str,
}

/// One row of the help overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpEntry {
    pub group: &'static str,
    /// All keys of the action joined with ` / `.
    pub keys: String,
    pub description: &'static str,
}

/// The active key bindings of every context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    contexts: [Vec<Binding>; 4],
}

impl Default for Keymap {
    /// The default keymap of SPEC §6.
    fn default() -> Self {
        let mut contexts: [Vec<Binding>; 4] = Default::default();
        for d in DEFAULTS {
            let keys = d
                .keys
                .iter()
                .map(|k| KeySeq::parse(k).unwrap_or_else(|e| panic!("bad default key: {e}")))
                .collect();
            contexts[d.context.index()].push(Binding {
                action: d.action,
                name: d.name,
                keys,
                group: d.group,
                description: d.description,
            });
        }
        Self { contexts }
    }
}

impl Keymap {
    /// The default keymap with the overrides from `[keys.*]` applied.
    ///
    /// Bad entries produce a warning (also logged) and keep the default:
    /// - an unknown context or action name;
    /// - an unparsable key, or `Ctrl-c`, which always quits;
    /// - a chord in the `global` or `overlay` context;
    /// - a key in another context that collides with a `global` key.
    ///
    /// A user key that is already bound to another action of the same context
    /// is taken from that action, with a warning: the user binding wins. The
    /// same happens to a default binding of another context that collides
    /// with a user-set `global` key. A key that starts a chord in a context is
    /// only a prefix there, so single-key bindings of it are dropped with a
    /// warning.
    pub fn from_config(raw: &RawKeys) -> (Self, Vec<ConfigWarning>) {
        let mut keymap = Self::default();
        let mut warnings = Vec::new();
        for name in raw.keys() {
            if Context::from_name(name).is_none() {
                warnings.push(warning(
                    &format!("keys.{name}"),
                    format!(
                        "unknown context \"{name}\" (known: global, browser, selector, overlay); ignoring it"
                    ),
                ));
            }
        }
        for context in Context::ALL {
            if let Some(actions) = raw.get(context.name()) {
                for (action, keys) in actions {
                    keymap.apply_override(context, action, keys, &mut warnings);
                }
            }
            keymap.drop_chord_prefix_singles(context, &mut warnings);
            if context == Context::Global {
                keymap.drop_global_collisions(&mut warnings);
            }
        }
        for w in &warnings {
            log::warn!("keymap: {w}");
        }
        (keymap, warnings)
    }

    /// The bindings of `context` in table (help) order.
    pub fn bindings(&self, context: Context) -> &[Binding] {
        &self.contexts[context.index()]
    }

    /// The binding of `action` in `context`.
    pub fn binding(&self, context: Context, action: Action) -> Option<&Binding> {
        self.bindings(context).iter().find(|b| b.action == action)
    }

    /// The active keys of `action` in `context`.
    pub fn keys(&self, context: Context, action: Action) -> &[KeySeq] {
        self.binding(context, action)
            .map_or(&[], |b| b.keys.as_slice())
    }

    /// Resolves a single key press to an action. Chords are not considered
    /// (use a [`KeyResolver`] for those). `Ctrl-c` never resolves.
    pub fn resolve(&self, context: Context, event: &KeyEvent) -> Option<Action> {
        let key = KeySpec::from_event(event);
        if key.is_ctrl_c() {
            return None;
        }
        self.single_action(context, key)
    }

    /// The label of the first key of `action`, for help-line hints
    /// (`None` when the action is unbound).
    pub fn label(&self, context: Context, action: Action) -> Option<String> {
        self.keys(context, action).first().map(KeySeq::label)
    }

    /// All keys of `action` joined with ` / `, for the help overlay
    /// (`None` when the action is unbound).
    pub fn labels(&self, context: Context, action: Action) -> Option<String> {
        let keys = self.keys(context, action);
        if keys.is_empty() {
            return None;
        }
        Some(
            keys.iter()
                .map(KeySeq::label)
                .collect::<Vec<_>>()
                .join(" / "),
        )
    }

    /// `(group, keys, description)` rows in table order, for the active
    /// bindings. Unbound actions are left out.
    pub fn help_entries(&self, context: Context) -> Vec<HelpEntry> {
        self.bindings(context)
            .iter()
            .filter(|b| !b.keys.is_empty())
            .map(|b| HelpEntry {
                group: b.group,
                keys: b
                    .keys
                    .iter()
                    .map(KeySeq::label)
                    .collect::<Vec<_>>()
                    .join(" / "),
                description: b.description,
            })
            .collect()
    }

    fn single_action(&self, context: Context, key: KeySpec) -> Option<Action> {
        let seq = KeySeq::single(key);
        self.bindings(context)
            .iter()
            .find(|b| b.keys.contains(&seq))
            .map(|b| b.action)
    }

    fn chord_action(&self, context: Context, first: KeySpec, second: KeySpec) -> Option<Action> {
        self.bindings(context)
            .iter()
            .find(|b| {
                b.keys
                    .iter()
                    .any(|k| k.first() == first && k.second() == Some(second))
            })
            .map(|b| b.action)
    }

    fn is_chord_prefix(&self, context: Context, key: KeySpec) -> bool {
        self.bindings(context)
            .iter()
            .flat_map(|b| &b.keys)
            .any(|k| k.is_chord() && k.first() == key)
    }

    /// The name of the global action that uses any key of `seq`.
    fn global_collision(&self, seq: &KeySeq) -> Option<&'static str> {
        self.bindings(Context::Global)
            .iter()
            .find(|b| {
                b.keys
                    .iter()
                    .any(|g| seq.keys().any(|k| KeySeq::single(k) == *g))
            })
            .map(|b| b.name)
    }

    fn apply_override(
        &mut self,
        context: Context,
        action: &str,
        raw_keys: &[String],
        warnings: &mut Vec<ConfigWarning>,
    ) {
        let path = format!("keys.{}.{action}", context.name());
        let skip = |warnings: &mut Vec<ConfigWarning>, problem: String| {
            warnings.push(warning(
                &path,
                format!("invalid binding for \"{path}\": {problem}; keeping the default"),
            ));
        };
        let Some(index) = self.bindings(context).iter().position(|b| b.name == action) else {
            skip(
                warnings,
                format!(
                    "unknown action \"{action}\" in context \"{}\"",
                    context.name()
                ),
            );
            return;
        };
        let mut seqs: Vec<KeySeq> = Vec::new();
        for raw in raw_keys {
            match KeySeq::parse(raw) {
                Ok(seq) => {
                    if !seqs.contains(&seq) {
                        seqs.push(seq);
                    }
                }
                Err(err) => {
                    skip(warnings, err.to_string());
                    return;
                }
            }
        }
        for seq in &seqs {
            if seq.is_chord() && !context.supports_chords() {
                skip(
                    warnings,
                    format!(
                        "chord \"{seq}\" is not supported in the {} context (only in browser and selector)",
                        context.name()
                    ),
                );
                return;
            }
            if seq.keys().any(|k| k.is_ctrl_c()) {
                skip(
                    warnings,
                    format!("\"{seq}\" cannot be bound: Ctrl-c always quits"),
                );
                return;
            }
            if context != Context::Global
                && let Some(global) = self.global_collision(seq)
            {
                skip(
                    warnings,
                    format!("\"{seq}\" is already used by the global action \"{global}\""),
                );
                return;
            }
        }
        for (i, other) in self.contexts[context.index()].iter_mut().enumerate() {
            if i == index {
                continue;
            }
            let other_name = other.name;
            other.keys.retain(|k| {
                if seqs.contains(k) {
                    warnings.push(warning(
                        &path,
                        format!(
                            "\"{k}\" is now bound to \"{action}\" and removed from \"{other_name}\" in context \"{}\"",
                            context.name()
                        ),
                    ));
                    false
                } else {
                    true
                }
            });
        }
        self.contexts[context.index()][index].keys = seqs;
    }

    /// Drops single-key bindings of keys that start a chord in `context`.
    fn drop_chord_prefix_singles(&mut self, context: Context, warnings: &mut Vec<ConfigWarning>) {
        let prefixes: Vec<KeySpec> = self
            .bindings(context)
            .iter()
            .flat_map(|b| &b.keys)
            .filter(|k| k.is_chord())
            .map(KeySeq::first)
            .collect();
        for binding in &mut self.contexts[context.index()] {
            let name = binding.name;
            binding.keys.retain(|k| {
                let dropped = !k.is_chord() && prefixes.contains(&k.first());
                if dropped {
                    warnings.push(warning(
                        &format!("keys.{}.{name}", context.name()),
                        format!(
                            "\"{k}\" starts a chord in context \"{}\", so it cannot also be a single key; removed from \"{name}\"",
                            context.name()
                        ),
                    ));
                }
                !dropped
            });
        }
    }

    /// Removes keys of the other contexts that collide with a global key
    /// (both would fire). Only user-set global keys can cause this.
    fn drop_global_collisions(&mut self, warnings: &mut Vec<ConfigWarning>) {
        let globals: Vec<(&'static str, KeySeq)> = self
            .bindings(Context::Global)
            .iter()
            .flat_map(|b| b.keys.iter().map(|k| (b.name, *k)))
            .collect();
        for context in [Context::Browser, Context::Selector, Context::Overlay] {
            for binding in &mut self.contexts[context.index()] {
                let name = binding.name;
                binding.keys.retain(|k| {
                    let hit = globals
                        .iter()
                        .find(|(_, g)| k.keys().any(|key| KeySeq::single(key) == *g));
                    if let Some((global, _)) = hit {
                        warnings.push(warning(
                            &format!("keys.{}.{name}", context.name()),
                            format!(
                                "\"{k}\" is now used by the global action \"{global}\"; removed from \"{name}\" in context \"{}\"",
                                context.name()
                            ),
                        ));
                    }
                    hit.is_none()
                });
            }
        }
    }
}

fn warning(key: &str, message: String) -> ConfigWarning {
    ConfigWarning {
        key: key.to_owned(),
        message,
    }
}

/// What a [`KeyResolver`] made of a key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolved {
    /// The key (or chord) is bound to this action.
    Action(Action),
    /// The key starts a chord; the next key decides.
    Pending,
    /// The key is not bound.
    Unbound,
}

/// Resolves key presses of one context, remembering a pending chord prefix.
///
/// Each component owns one as presentation state. There is no timeout: a
/// pending prefix stays until the next key. If that key does not complete a
/// chord, the prefix is dropped and the key is resolved afresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyResolver {
    context: Context,
    pending: Option<KeySpec>,
}

impl KeyResolver {
    pub fn new(context: Context) -> Self {
        Self {
            context,
            pending: None,
        }
    }

    /// Whether a chord prefix is waiting for its second key.
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Drops a pending prefix.
    pub fn reset(&mut self) {
        self.pending = None;
    }

    /// Whether `event`, as the next key, completes a chord bound to `action`:
    /// the pending prefix and this key together, not this key on its own.
    pub fn completes(&self, keymap: &Keymap, event: &KeyEvent, action: Action) -> bool {
        self.pending.is_some_and(|first| {
            keymap.chord_action(self.context, first, KeySpec::from_event(event)) == Some(action)
        })
    }

    /// Feeds a key press. `Ctrl-c` is never bound: it drops a pending prefix
    /// and gives `Unbound`.
    pub fn feed(&mut self, keymap: &Keymap, event: &KeyEvent) -> Resolved {
        let key = KeySpec::from_event(event);
        let prefix = self.pending.take();
        if key.is_ctrl_c() {
            return Resolved::Unbound;
        }
        if let Some(first) = prefix
            && let Some(action) = keymap.chord_action(self.context, first, key)
        {
            return Resolved::Action(action);
        }
        if keymap.is_chord_prefix(self.context, key) {
            self.pending = Some(key);
            return Resolved::Pending;
        }
        match keymap.single_action(self.context, key) {
            Some(action) => Resolved::Action(action),
            None => Resolved::Unbound,
        }
    }
}
