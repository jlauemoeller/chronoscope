//! Fitting the top border's titles into the width of the header: which
//! agents of a deep breadcrumb are shown, how wide each name is, and how much
//! of the session id is kept. All pure, so each rule is tested on its own.

use unicode_width::UnicodeWidthStr;

use crate::components::working_dir::truncate_start;

/// A name keeps at least this many columns while the titles still fit that
/// way.
pub(super) const MIN_NAME_WIDTH: usize = 6;
/// What stands for the agents of the outer levels that do not fit.
pub(super) const COLLAPSED: &str = " › …";
/// The columns of the title that are not names: ` Project ` before the
/// project and a blank after the last name.
pub(super) const TITLE_CHROME: usize = 10;
/// The columns of ` › Agent: ` before every agent.
pub(super) const SEPARATOR_WIDTH: usize = 10;
/// A shown name keeps at least this many columns (`ab…`): with fewer the
/// breadcrumb would be a row of ellipses, so outer agents collapse instead.
const USEFUL_NAME_WIDTH: usize = 3;
/// The shortest cut of the session id worth showing: `…` and six characters.
const MIN_ID_WIDTH: usize = 7;

/// How many of `count` agents, counted from the innermost, are shown: all
/// that fit with at least [`USEFUL_NAME_WIDTH`] columns for the project and
/// for each agent. The ones
/// left out are replaced by one [`COLLAPSED`] segment. Never 0 for a
/// breadcrumb, so the agent on screen is always named.
pub(super) fn visible_agents(count: usize, room: usize) -> usize {
    (1..=count)
        .rev()
        .find(|&shown| {
            let collapsed = if shown < count { COLLAPSED.width() } else { 0 };
            let chrome = TITLE_CHROME + collapsed + shown * SEPARATOR_WIDTH;
            chrome + USEFUL_NAME_WIDTH * (1 + shown) <= room
        })
        .unwrap_or(count.min(1))
}

/// The columns of the title around the names, for `shown` agents out of
/// `count`.
pub(super) fn chrome_width(count: usize, shown: usize) -> usize {
    let collapsed = if shown < count { COLLAPSED.width() } else { 0 };
    TITLE_CHROME + collapsed + shown * SEPARATOR_WIDTH
}

/// Shrinks `widths` until they sum to at most `room`, never below `floors`:
/// the longest name loses a column at a time (the outermost of equals), so
/// short names stay whole. Stops at the floors when `room` is still too small.
pub(super) fn fit_widths(widths: &[usize], floors: &[usize], room: usize) -> Vec<usize> {
    let mut widths = widths.to_vec();
    while widths.iter().sum::<usize>() > room {
        let longest = (0..widths.len())
            .rev()
            .filter(|&n| widths[n] > floors[n])
            .max_by_key(|&n| widths[n]);
        match longest {
            Some(n) => widths[n] -= 1,
            None => break,
        }
    }
    widths
}

/// `names` cut with `…` to fit `room` columns: first down to
/// [`MIN_NAME_WIDTH`] each, and if that is not enough down to one column
/// each (`…`).
pub(super) fn cut_names(names: &[&str], room: usize) -> Vec<String> {
    let widths: Vec<usize> = names.iter().map(|name| name.width()).collect();
    let floors = |most: usize| widths.iter().map(|w| most.min(*w)).collect::<Vec<_>>();
    let mut fitted = fit_widths(&widths, &floors(MIN_NAME_WIDTH), room);
    if fitted.iter().sum::<usize>() > room {
        fitted = fit_widths(&widths, &floors(1), room);
    }
    names
        .iter()
        .zip(fitted)
        .map(|(name, width)| cut(name, width))
        .collect()
}

fn cut(name: &str, width: usize) -> String {
    if width == 0 {
        String::new()
    } else {
        crate::components::truncate_with_ellipsis(name, width)
    }
}

/// The columns the names need at least before the id has to be cut:
/// [`MIN_NAME_WIDTH`] each.
pub(super) fn names_floor(names: &[&str]) -> usize {
    names
        .iter()
        .map(|name| MIN_NAME_WIDTH.min(name.width()))
        .sum()
}

/// The right title for `id` in `room` columns, with its blank on each side:
/// the whole id, else its end after a `…` (`…/subagents/agent-r2`, which
/// tells levels apart), else nothing when not even [`MIN_ID_WIDTH`] columns
/// fit.
pub(super) fn fit_id(id: &str, room: usize) -> String {
    let inner = room.saturating_sub(2);
    if id.is_empty() || inner < MIN_ID_WIDTH.min(id.width()) {
        return String::new();
    }
    format!(" {} ", truncate_start(id, inner))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_that_fit_are_left_alone() {
        assert_eq!(fit_widths(&[4, 12, 7], &[4, 6, 6], 23), [4, 12, 7]);
        assert_eq!(fit_widths(&[4, 12, 7], &[4, 6, 6], 100), [4, 12, 7]);
    }

    #[test]
    fn the_longest_name_shrinks_first() {
        assert_eq!(fit_widths(&[4, 12, 7], &[4, 6, 6], 20), [4, 9, 7]);
        assert_eq!(fit_widths(&[4, 12, 7], &[4, 6, 6], 17), [4, 6, 7]);
    }

    #[test]
    fn equal_names_shrink_from_the_outermost() {
        assert_eq!(fit_widths(&[10, 10, 10], &[1, 1, 1], 29), [9, 10, 10]);
        assert_eq!(fit_widths(&[10, 10, 10], &[1, 1, 1], 27), [9, 9, 9]);
    }

    #[test]
    fn nothing_goes_below_a_floor() {
        assert_eq!(fit_widths(&[12, 12], &[6, 6], 5), [6, 6]);
        assert_eq!(fit_widths(&[3, 12], &[3, 6], 4), [3, 6]);
    }

    #[test]
    fn no_room_leaves_the_floors() {
        assert_eq!(fit_widths(&[12, 12], &[1, 1], 0), [1, 1]);
        assert_eq!(fit_widths(&[], &[], 0), Vec::<usize>::new());
    }

    #[test]
    fn names_are_cut_to_six_columns_before_they_are_cut_further() {
        // 4 + 12 + 7 = 23 columns do not fit in 20: the longest loses three.
        assert_eq!(
            cut_names(&["BRIO", "story-writer", "Explore"], 20),
            ["BRIO", "story-wr…", "Explore"]
        );
    }

    #[test]
    fn names_that_still_do_not_fit_become_an_ellipsis_each() {
        let cut = cut_names(&["BRIO", "story-writer", "Explore", "general-purpose"], 10);
        assert_eq!(cut.iter().map(|n| n.width()).sum::<usize>(), 10);
        assert!(cut.iter().all(|name| !name.is_empty()), "{cut:?}");
        assert_eq!(cut[0], "B…");
    }

    #[test]
    fn every_agent_is_shown_when_they_fit() {
        assert_eq!(visible_agents(0, 55), 0);
        assert_eq!(visible_agents(1, 55), 1);
        assert_eq!(visible_agents(2, 55), 2);
        assert_eq!(visible_agents(4, 75), 4);
    }

    #[test]
    fn the_outer_agents_collapse_when_the_separators_alone_overflow() {
        // 10 + 5 * 10 + 4 would not fit in 55 columns.
        let shown = visible_agents(5, 55);
        assert!((1..5).contains(&shown), "{shown}");
        assert!(chrome_width(5, shown) + USEFUL_NAME_WIDTH * (1 + shown) <= 55);
        // The innermost agent is always named, however narrow.
        assert_eq!(visible_agents(5, 0), 1);
        assert_eq!(visible_agents(5, 20), 1);
    }

    #[test]
    fn the_whole_id_is_kept_when_it_fits() {
        assert_eq!(fit_id("agent-r2", 12), " agent-r2 ");
        assert_eq!(fit_id("agent-r2", 10), " agent-r2 ");
    }

    #[test]
    fn a_long_id_keeps_its_end() {
        let id = "85a86657-11a7-4318-a21a-0f4476babd5c/subagents/agent-r2";
        assert_eq!(fit_id(id, 22), " …/subagents/agent-r2 ");
        assert_eq!(fit_id(id, 12), " …/agent-r2 ");
    }

    #[test]
    fn an_id_that_cannot_keep_seven_columns_is_dropped() {
        let id = "85a86657-11a7-4318-a21a-0f4476babd5c/subagents/agent-r2";
        assert_eq!(fit_id(id, 9), " …ent-r2 ");
        assert_eq!(fit_id(id, 8), "");
        assert_eq!(fit_id("", 40), "");
    }
}
