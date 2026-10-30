//! What the transcript shows: the turns, how they are folded and which
//! section is selected, plus the options that decide how blocks look.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use super::cache::Measure;
use crate::components::turn_block::{BlockLayout, FoldState, SectionId, TurnBlock};
use crate::model::{Item, Turn, TurnId};
use crate::theme::Theme;
use crate::timestamp::TimestampFormatter;

/// The turns, the presentation state tied to them and the display options.
#[derive(Debug, Clone)]
pub struct Content {
    pub theme: Theme,
    pub formatter: TimestampFormatter,
    pub dim_unselected: bool,
    pub show_system: bool,
    pub truncate_output_lines: usize,
    /// The label of the key that opens a subagent transcript.
    pub open_subagent_key: String,
    pub turns: Vec<Arc<Turn>>,
    /// The position of every turn in `turns`.
    pub index: HashMap<TurnId, usize>,
    pub folds: FoldState,
    pub selected: Option<SectionId>,
}

/// What a batch of turns did to the existing ones.
#[derive(Debug, Default)]
pub struct Changes {
    /// How many turns were added at the end.
    pub appended: usize,
    /// The indices of turns replaced in place.
    pub replaced: Vec<usize>,
}

impl Content {
    pub fn new(theme: Theme, formatter: TimestampFormatter) -> Self {
        Self {
            theme,
            formatter,
            dim_unselected: true,
            show_system: true,
            truncate_output_lines: crate::config::DEFAULT_TRUNCATE_OUTPUT_LINES,
            open_subagent_key: crate::components::turn_block::renderers::DEFAULT_OPEN_KEY
                .to_owned(),
            turns: Vec::new(),
            index: HashMap::new(),
            folds: FoldState::new(),
            selected: None,
        }
    }

    /// The widget for block `index`, as currently configured.
    pub fn block(&self, index: usize) -> TurnBlock<'_> {
        self.block_for(&self.turns[index])
    }

    fn block_for<'a>(&'a self, turn: &'a Turn) -> TurnBlock<'a> {
        TurnBlock::new(turn, &self.theme, &self.formatter)
            .folds(&self.folds)
            .selected(self.selected.as_ref())
            .show_system(self.show_system)
            .dim_unselected(self.dim_unselected)
            .truncate_output_lines(self.truncate_output_lines)
            .open_subagent_key(&self.open_subagent_key)
    }

    /// The selectable sections of `turn` with the rows each covers, counted
    /// from the top border, at `width`.
    pub fn section_rows(&self, turn: &Turn, width: u16) -> Vec<(SectionId, Range<usize>)> {
        self.block_for(turn).layout(width).section_rows().to_vec()
    }

    /// Adds `turns` at the end; a turn whose id is already present replaces
    /// the existing one instead.
    pub fn append(&mut self, turns: &[Arc<Turn>]) -> Changes {
        let mut changes = Changes::default();
        for turn in turns {
            match self.index.get(turn.id()) {
                Some(&at) => {
                    self.turns[at] = Arc::clone(turn);
                    changes.replaced.push(at);
                }
                None => {
                    self.index.insert(turn.id().clone(), self.turns.len());
                    self.turns.push(Arc::clone(turn));
                    changes.appended += 1;
                }
            }
        }
        changes
    }

    /// Replaces the turns that have the same id as one of `turns`; the
    /// others are ignored.
    pub fn update(&mut self, turns: &[Arc<Turn>]) -> Changes {
        let mut changes = Changes::default();
        for turn in turns {
            if let Some(&at) = self.index.get(turn.id()) {
                self.turns[at] = Arc::clone(turn);
                changes.replaced.push(at);
            }
        }
        changes
    }

    /// Drops every turn, fold and the selection.
    pub fn reset(&mut self) {
        self.turns.clear();
        self.index.clear();
        self.folds = FoldState::new();
        self.selected = None;
    }

    /// The indices of the turns that hold injected items, whose sections
    /// appear or disappear with `show_system`.
    pub fn system_blocks(&self) -> Vec<usize> {
        let has_injected =
            |items: &[Item]| items.iter().any(|i| matches!(i, Item::Injected { .. }));
        self.turns
            .iter()
            .enumerate()
            .filter(|(_, turn)| match turn.as_ref() {
                Turn::User(user) => has_injected(&user.items),
                Turn::Assistant(assistant) => has_injected(&assistant.items),
                Turn::Compaction(_) => false,
            })
            .map(|(i, _)| i)
            .collect()
    }
}

/// Measures blocks of a [`Content`] for the layout cache, counting the
/// layouts it runs.
pub struct Measurer<'a> {
    content: &'a Content,
    width: u16,
    /// How many layouts were run.
    pub calls: usize,
    /// The layout of the block measured last, so that drawing it right after
    /// does not lay it out a second time.
    last: Option<(usize, BlockLayout)>,
    /// Layouts from the previous frame that are still valid.
    reuse: HashMap<usize, BlockLayout>,
}

impl<'a> Measurer<'a> {
    pub fn new(content: &'a Content, width: u16) -> Self {
        Self {
            content,
            width,
            calls: 0,
            last: None,
            reuse: HashMap::new(),
        }
    }

    /// Hands out `layouts` (by block index) instead of laying those blocks out
    /// again. The caller guarantees they are valid for the content and width.
    pub fn reusing(mut self, layouts: HashMap<usize, BlockLayout>) -> Self {
        self.reuse = layouts;
        self
    }

    /// The layout of block `index`: the one measured last if that is it.
    pub fn layout(&mut self, index: usize) -> BlockLayout {
        match self.last.take() {
            Some((at, layout)) if at == index => layout,
            _ => match self.reuse.remove(&index) {
                Some(layout) => layout,
                None => self.run(index),
            },
        }
    }

    fn run(&mut self, index: usize) -> BlockLayout {
        self.calls += 1;
        self.content.block(index).layout(self.width)
    }
}

impl Measure for Measurer<'_> {
    fn measure(&mut self, index: usize) -> usize {
        let layout = match self.reuse.remove(&index) {
            Some(layout) => layout,
            None => self.run(index),
        };
        let height = layout.height();
        self.last = Some((index, layout));
        height
    }
}
