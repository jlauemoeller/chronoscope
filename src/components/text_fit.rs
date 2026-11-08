//! Fitting text into a number of columns, shared by the components and the
//! `App`: cut at the end with `…` after it ([`truncate_with_ellipsis`]), or
//! cut at the start with `…` before it, so the end of a path stays visible
//! ([`truncate_start`]). Widths are display columns, so a wide character
//! counts two.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// `text` cut to at most `width` columns, ending in `…` when it was cut.
pub fn truncate_with_ellipsis(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = c.width().unwrap_or(0);
        if used + w + 1 > width {
            break;
        }
        out.push(c);
        used += w;
    }
    if width > 0 {
        out.push('…');
    }
    out
}

/// `text` cut to at most `width` columns from the left, starting with `…`
/// when it was cut, so the end of a path stays visible; the text itself when
/// it fits.
pub fn truncate_start(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let mut tail: Vec<char> = Vec::new();
    let mut used = 1;
    for c in text.chars().rev() {
        let w = c.width().unwrap_or(0);
        if used + w > width {
            break;
        }
        tail.push(c);
        used += w;
    }
    tail.push('…');
    tail.into_iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A path of 25 columns: six narrow characters, three wide ones, a narrow
    /// one and six wide ones.
    const WIDE_PATH: &str = "/home/日本語/プロジェクト";

    #[test]
    fn truncation_keeps_the_result_within_the_width() {
        assert_eq!(truncate_with_ellipsis("hello", 5), "hello");
        assert_eq!(truncate_with_ellipsis("hello!", 5), "hell…");
        assert_eq!(truncate_with_ellipsis("日本語です", 6), "日本…");
        assert_eq!(truncate_with_ellipsis("hello", 0), "");
        assert_eq!(truncate_with_ellipsis("hello", 1), "…");
    }

    // The next three tests come from `WorkingDir` and `Prompt`, which had a
    // start-cutting function each (`truncate_start` and `tail_fitting`, the
    // same algorithm). The ones after them pin down the edge cases for every
    // caller.

    #[test]
    fn truncate_start_keeps_the_end() {
        assert_eq!(truncate_start("/a/b/c", 6), "/a/b/c");
        assert_eq!(truncate_start("/a/b/c/d", 6), "…b/c/d");
        assert_eq!(truncate_start("/a/b/c/d", 1), "…");
        assert_eq!(truncate_start("/a/b", 0), "");
    }

    #[test]
    fn truncate_start_never_exceeds_the_width_with_wide_characters() {
        for width in 1..12 {
            let cut = truncate_start(WIDE_PATH, width);
            assert!(cut.width() <= width, "{cut:?} in {width}");
        }
        assert!(truncate_start(WIDE_PATH, 6).starts_with('…'));
    }

    #[test]
    fn truncate_start_keeps_the_end_of_narrow_and_wide_text() {
        assert_eq!(truncate_start("abcdef", 6), "abcdef");
        assert_eq!(truncate_start("abcdef", 4), "…def");
        assert_eq!(truncate_start("abcdef", 1), "…");
        assert_eq!(truncate_start("abcdef", 0), "");
        assert_eq!(truncate_start("日本語", 5), "…本語");
        assert_eq!(truncate_start("日本語", 4), "…語");
    }

    #[test]
    fn truncate_start_of_an_empty_string_is_empty_at_any_width() {
        for width in 0..3 {
            assert_eq!(truncate_start("", width), "", "width {width}");
        }
    }

    #[test]
    fn truncate_start_at_widths_0_and_1_is_nothing_and_the_ellipsis() {
        for text in ["/a/b", "ab", "日本語", WIDE_PATH] {
            assert_eq!(truncate_start(text, 0), "", "{text:?}");
            assert_eq!(truncate_start(text, 1), "…", "{text:?}");
        }
        // A text that fits is never cut, even at width 1.
        assert_eq!(truncate_start("a", 1), "a");
        assert_eq!(truncate_start("a", 0), "");
    }

    #[test]
    fn truncate_start_keeps_a_text_that_fits_exactly() {
        assert_eq!(truncate_start("abcdef", 6), "abcdef");
        assert_eq!(truncate_start("日本語", 6), "日本語");
        assert_eq!(truncate_start(WIDE_PATH, 25), WIDE_PATH);
        assert_eq!(truncate_start(WIDE_PATH, 26), WIDE_PATH);
    }

    #[test]
    fn truncate_start_never_splits_a_wide_character() {
        // A wide character that does not fit beside the `…` leaves a column
        // empty rather than being cut.
        assert_eq!(truncate_start("日本語", 1), "…");
        assert_eq!(truncate_start("日本語", 2), "…");
        assert_eq!(truncate_start("日本語", 3), "…語");
        assert_eq!(truncate_start("日本語", 4), "…語");
        assert_eq!(truncate_start("日本語", 5), "…本語");
    }

    #[test]
    fn truncate_start_of_a_wide_path_at_every_width() {
        let expected = [
            "",
            "…",
            "…",
            "…ト",
            "…ト",
            "…クト",
            "…クト",
            "…ェクト",
            "…ェクト",
            "…ジェクト",
            "…ジェクト",
            "…ロジェクト",
            "…ロジェクト",
            "…プロジェクト",
            "…/プロジェクト",
            "…/プロジェクト",
            "…語/プロジェクト",
            "…語/プロジェクト",
            "…本語/プロジェクト",
            "…本語/プロジェクト",
            "…日本語/プロジェクト",
            "…/日本語/プロジェクト",
            "…e/日本語/プロジェクト",
            "…me/日本語/プロジェクト",
            "…ome/日本語/プロジェクト",
            WIDE_PATH,
            WIDE_PATH,
        ];
        for (width, want) in expected.iter().enumerate() {
            let cut = truncate_start(WIDE_PATH, width);
            assert_eq!(cut, *want, "width {width}");
            assert!(cut.width() <= width, "{cut:?} in {width}");
        }
    }
}
