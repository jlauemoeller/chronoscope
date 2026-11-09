//! Double-clicks (product §5.9, cycle 5 spec §8.2), shared by the components
//! that have them: `ProjectList`, `SessionList`, `ThemeSelector` and
//! `TurnSidebar`. Each keeps its own [`LastPress`], measured with a
//! [`ClickClock`] that the `App` hands down when it mounts the component, and
//! through [`CLICK_CLOCK_ATTR`] to one already mounted.

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tuirealm::props::AttrValue;
use tuirealm::props::Attribute;

use super::{payload_from_value, payload_value};

/// Two left presses on the same row at most this far apart are a
/// double-click.
pub const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// The attribute that replaces the clock a component measures double-clicks
/// with (a [`ClickClock`] payload, made by [`ClickClock::value`]).
pub const CLICK_CLOCK_ATTR: Attribute = Attribute::Custom("click_clock");

/// The clock double-clicks are measured with: the system clock unless tests
/// inject another. Clones share the one clock; two clocks are equal when they
/// are clones of each other.
#[derive(Clone)]
pub struct ClickClock(Arc<dyn Fn() -> Instant + Send + Sync>);

impl ClickClock {
    /// A clock that reads `now`.
    pub fn new(now: impl Fn() -> Instant + Send + Sync + 'static) -> Self {
        Self(Arc::new(now))
    }

    /// The time now, by this clock.
    pub fn now(&self) -> Instant {
        (self.0)()
    }

    /// The attribute value for [`CLICK_CLOCK_ATTR`].
    pub fn value(&self) -> AttrValue {
        payload_value(self.clone())
    }

    /// The clock in an attribute value made by [`value`](Self::value).
    pub fn from_value(value: &AttrValue) -> Option<Self> {
        payload_from_value(value)
    }
}

impl Default for ClickClock {
    fn default() -> Self {
        Self::new(Instant::now)
    }
}

impl fmt::Debug for ClickClock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClickClock").finish_non_exhaustive()
    }
}

impl PartialEq for ClickClock {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// A component's last left press on a row: what the row shows (an id, so a
/// row that moved is still the same target) and when, by its clock.
#[derive(Debug, Clone)]
pub struct LastPress<T> {
    last: Option<(T, Instant)>,
    clock: ClickClock,
}

impl<T> Default for LastPress<T> {
    fn default() -> Self {
        Self {
            last: None,
            clock: ClickClock::default(),
        }
    }
}

impl<T: PartialEq> LastPress<T> {
    /// Measures with `clock` from now on.
    pub fn set_clock(&mut self, clock: ClickClock) {
        self.clock = clock;
    }

    /// Records a left press on the row of `target`. Returns whether it
    /// completes a double-click: the last press was on the same target at
    /// most [`DOUBLE_CLICK`] ago. A completed double-click is forgotten, so a
    /// third press starts a new one.
    pub fn press(&mut self, target: T) -> bool {
        let now = self.clock.now();
        let double = self.last.as_ref().is_some_and(|(last, at)| {
            *last == target && now.saturating_duration_since(*at) <= DOUBLE_CLICK
        });
        self.last = (!double).then_some((target, now));
        double
    }

    /// Forgets the last press: a press that is not on a row, of the
    /// component's own or elsewhere, ends a double-click under way.
    pub fn forget(&mut self) {
        self.last = None;
    }
}

/// A clock for tests that shows a fixed start plus the time it is
/// [`set`](Self::set) to, shared by its clones.
#[cfg(test)]
#[derive(Debug, Clone)]
pub struct TestClock {
    start: Instant,
    elapsed: Arc<std::sync::Mutex<Duration>>,
}

#[cfg(test)]
impl Default for TestClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl TestClock {
    /// A clock that shows its start until it is set.
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            elapsed: Arc::default(),
        }
    }

    /// Moves the clock to `elapsed` after its start.
    pub fn set(&self, elapsed: Duration) {
        *self.elapsed.lock().unwrap() = elapsed;
    }

    /// The time it shows.
    pub fn now(&self) -> Instant {
        self.start + *self.elapsed.lock().unwrap()
    }

    /// A [`ClickClock`] that reads this clock.
    pub fn click_clock(&self) -> ClickClock {
        let clock = self.clone();
        ClickClock::new(move || clock.now())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn presses() -> (LastPress<&'static str>, TestClock) {
        let clock = TestClock::new();
        let mut presses = LastPress::default();
        presses.set_clock(clock.click_clock());
        (presses, clock)
    }

    #[test]
    fn two_presses_on_the_same_target_within_400_ms_are_a_double_click() {
        for (ms, double) in [(0, true), (400, true), (401, false)] {
            let (mut presses, clock) = presses();
            assert!(!presses.press("a"));
            clock.set(Duration::from_millis(ms));
            assert_eq!(presses.press("a"), double, "{ms} ms");
        }
    }

    #[test]
    fn a_third_press_starts_over() {
        let (mut presses, clock) = presses();
        presses.press("a");
        clock.set(Duration::from_millis(100));
        assert!(presses.press("a"));
        clock.set(Duration::from_millis(200));
        assert!(!presses.press("a"));
        clock.set(Duration::from_millis(300));
        assert!(presses.press("a"));
    }

    #[test]
    fn another_target_or_forgetting_ends_it() {
        let (mut presses, _) = presses();
        presses.press("a");
        assert!(!presses.press("b"));
        assert!(!presses.press("a"));
        presses.forget();
        assert!(!presses.press("a"));
        assert!(presses.press("a"));
    }

    #[test]
    fn a_clock_survives_the_attribute_and_compares_by_identity() {
        let clock = ClickClock::default();
        let back = ClickClock::from_value(&clock.value()).unwrap();
        assert_eq!(back, clock);
        assert_ne!(ClickClock::default(), clock);
        assert_eq!(ClickClock::from_value(&AttrValue::Flag(true)), None);
    }
}
