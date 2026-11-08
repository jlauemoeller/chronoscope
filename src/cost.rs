//! Cost display (product §4.2): one format for the turn stats footer and the
//! details overlay.

/// A cost in US dollars: `$0.42`.
pub fn format_cost(cost: f64) -> String {
    format!("${cost:.2}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn costs_are_dollars_with_two_decimals() {
        assert_eq!(format_cost(0.42), "$0.42");
        assert_eq!(format_cost(0.0), "$0.00");
        assert_eq!(format_cost(3.0), "$3.00");
        assert_eq!(format_cost(1234.5), "$1234.50");
    }

    #[test]
    fn costs_are_rounded_to_the_cent() {
        assert_eq!(format_cost(0.004), "$0.00");
        assert_eq!(format_cost(0.126), "$0.13");
        assert_eq!(format_cost(1.999), "$2.00");
    }
}
