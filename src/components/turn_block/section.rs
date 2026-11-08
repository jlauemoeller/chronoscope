//! Sections: the unit of selection, folding, copying and saving (product §4.2).
//!
//! [`derive_sections`] derives the sections of a turn; [`FoldState`] answers whether
//! a section is folded, starting from the defaults of the §5.2 table.

use std::collections::{HashMap, HashSet};

use crate::model::{
    InjectedEntry, Item, ItemId, NoticeLevel, Thinking, ToolCall, Turn, TurnId, TurnStats,
};

/// Which part of a turn a section shows.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SectionKey {
    /// The prompt of a User turn.
    Prompt,
    /// The summary of a Compaction turn.
    Summary,
    /// An item of the turn.
    Item(ItemId),
    /// The stats footer of an Assistant turn.
    Footer,
    /// The `… N more lines` row below a truncated tool call (the tool call is
    /// the item with this id). It is a selection stop of its own, but it is
    /// never listed by [`derive_sections`]: it exists only in a layout.
    More(ItemId),
}

/// The stable identity of a section: its turn and its part of the turn.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SectionId {
    /// The turn the section belongs to.
    pub turn: TurnId,
    /// The part of the turn.
    pub key: SectionKey,
}

impl SectionId {
    /// The id of the truncation row below the tool call `self`, when `self`
    /// is an item section.
    pub fn more_row(&self) -> Option<SectionId> {
        match &self.key {
            SectionKey::Item(item) => Some(SectionId {
                turn: self.turn.clone(),
                key: SectionKey::More(item.clone()),
            }),
            _ => None,
        }
    }

    /// The tool call section a truncation row belongs to; `None` for any
    /// other section.
    pub fn more_parent(&self) -> Option<SectionId> {
        match &self.key {
            SectionKey::More(item) => Some(SectionId {
                turn: self.turn.clone(),
                key: SectionKey::Item(item.clone()),
            }),
            _ => None,
        }
    }
}

/// The kinds of section (§5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SectionKind {
    /// The prompt of a User turn.
    UserPrompt,
    /// Markdown text of the assistant.
    AssistantText,
    /// Readable thinking.
    Thinking,
    /// A tool call.
    ToolCall,
    /// The `model → …` note.
    ModelChange,
    /// An error or warning about how the turn ended.
    Notice,
    /// The turn stats footer.
    Stats,
    /// A compaction summary.
    Compaction,
    /// System / injected content.
    System,
}

impl SectionKind {
    /// Whether sections of this kind can be folded.
    pub fn foldable(self) -> bool {
        !matches!(
            self,
            SectionKind::AssistantText
                | SectionKind::ModelChange
                | SectionKind::Notice
                | SectionKind::Stats
        )
    }

    /// Whether sections of this kind start folded.
    pub fn default_folded(self) -> bool {
        self.foldable()
    }
}

/// What a section shows, borrowed from the turn.
#[derive(Debug, Clone, Copy)]
pub enum SectionContent<'a> {
    /// The prompt text.
    Prompt(&'a str),
    /// Assistant markdown.
    Text(&'a str),
    /// Readable thinking text.
    Thinking(&'a str),
    /// A tool call.
    Tool(&'a ToolCall),
    /// The model the turn switched to.
    ModelChange(&'a str),
    /// An error or warning, with its text.
    Notice(NoticeLevel, &'a str),
    /// The footer statistics.
    Stats(&'a TurnStats),
    /// The compaction summary.
    Compaction(&'a str),
    /// Injected entries, collapsed into one section.
    System(&'a [InjectedEntry]),
}

/// One section of a turn.
#[derive(Debug, Clone)]
pub struct Section<'a> {
    /// The section's identity.
    pub id: SectionId,
    /// What it shows.
    pub content: SectionContent<'a>,
}

impl Section<'_> {
    /// The word for the section in the name of a saved file: `user`,
    /// `assistant` (also the `model → …` note), `error` or `notice` (a
    /// notice of level `Error` or `Warning`), `thinking`, the tool's kind
    /// (`shell`, `read`, `write`, `edit`, `grep`, `glob`, `webfetch`,
    /// `websearch`, `subagent`, or `tool` for any other), `system`,
    /// `compaction` or `stats`.
    pub fn file_kind(&self) -> &'static str {
        use crate::model::ToolKind;
        match self.content {
            SectionContent::Prompt(_) => "user",
            SectionContent::Text(_) | SectionContent::ModelChange(_) => "assistant",
            SectionContent::Notice(NoticeLevel::Error, _) => "error",
            SectionContent::Notice(NoticeLevel::Warning, _) => "notice",
            SectionContent::Thinking(_) => "thinking",
            SectionContent::Tool(call) => match call.tool {
                ToolKind::Shell => "shell",
                ToolKind::Read => "read",
                ToolKind::Write => "write",
                ToolKind::Edit => "edit",
                ToolKind::Grep => "grep",
                ToolKind::Glob => "glob",
                ToolKind::WebFetch => "webfetch",
                ToolKind::WebSearch => "websearch",
                ToolKind::Subagent => "subagent",
                ToolKind::Other(_) => "tool",
            },
            SectionContent::Stats(_) => "stats",
            SectionContent::Compaction(_) => "compaction",
            SectionContent::System(_) => "system",
        }
    }

    /// The kind of section.
    pub fn kind(&self) -> SectionKind {
        match self.content {
            SectionContent::Prompt(_) => SectionKind::UserPrompt,
            SectionContent::Text(_) => SectionKind::AssistantText,
            SectionContent::Thinking(_) => SectionKind::Thinking,
            SectionContent::Tool(_) => SectionKind::ToolCall,
            SectionContent::ModelChange(_) => SectionKind::ModelChange,
            SectionContent::Notice(..) => SectionKind::Notice,
            SectionContent::Stats(_) => SectionKind::Stats,
            SectionContent::Compaction(_) => SectionKind::Compaction,
            SectionContent::System(_) => SectionKind::System,
        }
    }
}

/// The sections of `turn`, in display order.
///
/// Redacted (and empty) thinking yields no section; injected items yield none
/// when `show_system` is false. A User turn lists its items before the
/// prompt, an Assistant turn ends with the stats footer when it has stats.
///
/// Derivation does not render, so a section whose markdown renders to no
/// rows at all (say, text that is only an HTML comment) is still listed here.
/// Such a section has no entry in
/// [`BlockLayout::section_rows`](super::BlockLayout::section_rows), which is
/// therefore the list of sections that can be selected.
pub fn derive_sections(turn: &Turn, show_system: bool) -> Vec<Section<'_>> {
    let mut out = Vec::new();
    let id = |key| SectionId {
        turn: turn.id().clone(),
        key,
    };
    match turn {
        Turn::User(user) => {
            item_sections(turn.id(), &user.items, show_system, &mut out);
            if !user.text.trim().is_empty() {
                out.push(Section {
                    id: id(SectionKey::Prompt),
                    content: SectionContent::Prompt(&user.text),
                });
            }
        }
        Turn::Assistant(assistant) => {
            item_sections(turn.id(), &assistant.items, show_system, &mut out);
            if let Some(stats) = &assistant.stats {
                out.push(Section {
                    id: id(SectionKey::Footer),
                    content: SectionContent::Stats(stats),
                });
            }
        }
        Turn::Compaction(compaction) => out.push(Section {
            id: id(SectionKey::Summary),
            content: SectionContent::Compaction(&compaction.summary),
        }),
    }
    out
}

fn item_sections<'a>(
    turn: &TurnId,
    items: &'a [Item],
    show_system: bool,
    out: &mut Vec<Section<'a>>,
) {
    for item in items {
        if let Some(content) = item_content(item, show_system) {
            out.push(Section {
                id: SectionId {
                    turn: turn.clone(),
                    key: SectionKey::Item(item.id().clone()),
                },
                content,
            });
        }
    }
}

fn item_content(item: &Item, show_system: bool) -> Option<SectionContent<'_>> {
    match item {
        Item::Text { text, .. } => (!text.trim().is_empty()).then_some(SectionContent::Text(text)),
        Item::Injected { entries, .. } => {
            (show_system && !entries.is_empty()).then_some(SectionContent::System(entries))
        }
        Item::Thinking {
            thinking: Thinking::Text(text),
            ..
        } => (!text.trim().is_empty()).then_some(SectionContent::Thinking(text)),
        Item::Thinking {
            thinking: Thinking::Redacted,
            ..
        } => None,
        Item::ToolCall(call) => Some(SectionContent::Tool(call)),
        Item::ModelChange { model, .. } => Some(SectionContent::ModelChange(model)),
        Item::Notice { level, text, .. } => {
            (!text.trim().is_empty()).then_some(SectionContent::Notice(*level, text))
        }
    }
}

/// Which sections the user has folded or unfolded, on top of the defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FoldState {
    overrides: HashMap<SectionId, bool>,
    /// The tool calls whose truncated output was expanded. Independent of
    /// folding: folding and unfolding a call keeps it.
    expanded: HashSet<SectionId>,
}

impl FoldState {
    /// A state in which every section has its default fold.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the section is folded. Sections that cannot be folded never
    /// are.
    pub fn is_folded(&self, section: &Section<'_>) -> bool {
        let kind = section.kind();
        kind.foldable()
            && self
                .overrides
                .get(&section.id)
                .copied()
                .unwrap_or_else(|| kind.default_folded())
    }

    /// Whether the output of the tool call `id` was expanded past
    /// `truncate_output_lines`.
    pub fn is_expanded(&self, id: &SectionId) -> bool {
        self.expanded.contains(id)
    }

    /// Marks the output of the tool call `id` as expanded.
    pub fn set_expanded(&mut self, id: SectionId) {
        self.expanded.insert(id);
    }

    /// Marks the output of the tool call `id` as not expanded again.
    pub fn clear_expanded(&mut self, id: &SectionId) {
        self.expanded.remove(id);
    }

    /// Folds or unfolds the section with the given id.
    pub fn set_folded(&mut self, id: SectionId, folded: bool) {
        self.overrides.insert(id, folded);
    }
}
