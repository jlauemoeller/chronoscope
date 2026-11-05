//! Token count display (SPEC §5.2): one format for the turn stats footer, the
//! details overlay and the session selector's Session Tokens panel.

/// A token count: `950`, `45.2k` or `1.2M`.
pub fn format_tokens(tokens: u64) -> String {
    if tokens < 1_000 {
        return tokens.to_string();
    }
    let thousands = tokens.saturating_add(50) / 100;
    if thousands < 10_000 {
        return format!("{}.{}k", thousands / 10, thousands % 10);
    }
    let millions = tokens.saturating_add(50_000) / 100_000;
    format!("{}.{}M", millions / 10, millions % 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_counts_are_formatted() {
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(950), "950");
        assert_eq!(format_tokens(999), "999");
        assert_eq!(format_tokens(1_000), "1.0k");
        assert_eq!(format_tokens(45_213), "45.2k");
        assert_eq!(format_tokens(999_949), "999.9k");
        assert_eq!(format_tokens(999_950), "1.0M");
        assert_eq!(format_tokens(1_234_567), "1.2M");
    }

    #[test]
    fn the_worked_example_reads_as_in_the_wireframe() {
        assert_eq!(format_tokens(15_800), "15.8k");
        assert_eq!(format_tokens(6_300_000), "6.3M");
        assert_eq!(format_tokens(2_200_000), "2.2M");
        assert_eq!(format_tokens(862_300_000), "862.3M");
        assert_eq!(format_tokens(30_700_000), "30.7M");
    }

    #[test]
    fn token_counts_near_u64_max_do_not_overflow() {
        assert!(format_tokens(u64::MAX).ends_with('M'));
    }
}
