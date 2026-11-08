//! Deciding when to draw (architecture.md#data-flow, product §6.1).

use std::time::{Duration, Instant};

/// The least time between two draws caused by ordinary changes.
pub const MIN_REDRAW_INTERVAL: Duration = Duration::from_millis(50);

/// How urgently the screen needs to be drawn again.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum RedrawRequest {
    /// Nothing visible changed.
    #[default]
    None,
    /// Something visible changed; draw when the interval allows.
    Dirty,
    /// The terminal was resized; draw at once.
    Resized,
}

/// Limits how often the screen is drawn.
///
/// A draw happens only when a change was requested, and at least
/// [`MIN_REDRAW_INTERVAL`] after the previous draw. The exception is a
/// resize: the terminal contents are wrong until the next draw, so a resize
/// request is drawn at once, whatever the interval.
///
/// The limiter reads no clock; callers pass the current instant.
#[derive(Debug, Clone)]
pub struct RedrawLimiter {
    interval: Duration,
    last_draw: Option<Instant>,
    pending: RedrawRequest,
}

impl RedrawLimiter {
    /// A limiter with the given minimum interval. The first draw is pending,
    /// so the first frame is drawn without any change being requested.
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_draw: None,
            pending: RedrawRequest::Dirty,
        }
    }

    /// Records that a redraw is needed. The most urgent request wins until
    /// the next draw.
    pub fn request(&mut self, request: RedrawRequest) {
        self.pending = self.pending.max(request);
    }

    /// Whether the screen should be drawn at `now`.
    pub fn should_draw(&self, now: Instant) -> bool {
        match self.pending {
            RedrawRequest::None => false,
            RedrawRequest::Resized => true,
            RedrawRequest::Dirty => self
                .last_draw
                .is_none_or(|last| now.saturating_duration_since(last) >= self.interval),
        }
    }

    /// Records that the screen was drawn at `now`.
    pub fn record_draw(&mut self, now: Instant) {
        self.last_draw = Some(now);
        self.pending = RedrawRequest::None;
    }
}

impl Default for RedrawLimiter {
    fn default() -> Self {
        Self::new(MIN_REDRAW_INTERVAL)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(base: Instant, n: u64) -> Instant {
        base + Duration::from_millis(n)
    }

    #[test]
    fn first_frame_is_drawn_without_a_request() {
        let limiter = RedrawLimiter::default();
        assert!(limiter.should_draw(Instant::now()));
    }

    #[test]
    fn nothing_is_drawn_when_clean() {
        let t0 = Instant::now();
        let mut limiter = RedrawLimiter::default();
        limiter.record_draw(t0);
        assert!(!limiter.should_draw(ms(t0, 1_000)));
        limiter.request(RedrawRequest::None);
        assert!(!limiter.should_draw(ms(t0, 2_000)));
    }

    #[test]
    fn a_dirty_request_waits_for_the_interval() {
        let t0 = Instant::now();
        let mut limiter = RedrawLimiter::default();
        limiter.record_draw(t0);
        limiter.request(RedrawRequest::Dirty);
        assert!(!limiter.should_draw(ms(t0, 0)));
        assert!(!limiter.should_draw(ms(t0, 49)));
        assert!(limiter.should_draw(ms(t0, 50)));
        assert!(limiter.should_draw(ms(t0, 500)));
    }

    #[test]
    fn a_draw_clears_the_request() {
        let t0 = Instant::now();
        let mut limiter = RedrawLimiter::default();
        limiter.request(RedrawRequest::Dirty);
        limiter.record_draw(t0);
        assert!(!limiter.should_draw(ms(t0, 100)));
    }

    #[test]
    fn ordinary_draws_are_never_less_than_the_interval_apart() {
        // A change is requested every millisecond for a second.
        let t0 = Instant::now();
        let mut limiter = RedrawLimiter::default();
        let mut draws = Vec::new();
        for n in 0..1_000 {
            let now = ms(t0, n);
            limiter.request(RedrawRequest::Dirty);
            if limiter.should_draw(now) {
                limiter.record_draw(now);
                draws.push(n);
            }
        }
        assert!(draws.len() > 10, "{draws:?}");
        assert!(
            draws.windows(2).all(|w| w[1] - w[0] >= 50),
            "draws too close together: {draws:?}"
        );
    }

    #[test]
    fn a_resize_is_drawn_immediately() {
        let t0 = Instant::now();
        let mut limiter = RedrawLimiter::default();
        limiter.record_draw(t0);
        limiter.request(RedrawRequest::Resized);
        assert!(limiter.should_draw(ms(t0, 1)));
    }

    #[test]
    fn a_resize_stays_urgent_when_followed_by_a_dirty_request() {
        let t0 = Instant::now();
        let mut limiter = RedrawLimiter::default();
        limiter.record_draw(t0);
        limiter.request(RedrawRequest::Resized);
        limiter.request(RedrawRequest::Dirty);
        assert!(limiter.should_draw(ms(t0, 1)));
    }

    #[test]
    fn after_a_resize_draw_the_interval_applies_again() {
        let t0 = Instant::now();
        let mut limiter = RedrawLimiter::default();
        limiter.record_draw(t0);
        limiter.request(RedrawRequest::Resized);
        limiter.record_draw(ms(t0, 1));
        limiter.request(RedrawRequest::Dirty);
        assert!(!limiter.should_draw(ms(t0, 20)));
        assert!(limiter.should_draw(ms(t0, 51)));
    }
}
