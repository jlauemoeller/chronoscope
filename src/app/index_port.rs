//! The Port that turns the session index worker's events into user events
//! (SPEC §1.1, §1.4).

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use tuirealm::event::Event;
use tuirealm::listener::{Poll, PortResult};

use super::UserEvent;
use crate::index::IndexEvent;

/// How often the index port is polled.
pub const INDEX_POLL_INTERVAL: Duration = Duration::from_millis(25);
/// The most events one poll takes from the worker, so a burst of stats is
/// drained quickly without starving the other ports.
pub const INDEX_MAX_PER_POLL: usize = 64;

/// An [`IndexEvent`] on its way to the application.
///
/// User events must be `Eq + Clone`; the payload is large and cheap to share,
/// so it is `Arc`-wrapped and two updates are equal when they have the same
/// sequence number, which the [`IndexPort`] gives each event once.
#[derive(Debug, Clone)]
pub struct IndexUpdate {
    seq: u64,
    event: Arc<IndexEvent>,
}

impl IndexUpdate {
    /// An update with sequence number `seq`.
    pub fn new(seq: u64, event: IndexEvent) -> Self {
        Self {
            seq,
            event: Arc::new(event),
        }
    }

    /// The sequence number.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// What the worker reported.
    pub fn event(&self) -> &IndexEvent {
        &self.event
    }
}

impl PartialEq for IndexUpdate {
    fn eq(&self, other: &Self) -> bool {
        self.seq == other.seq
    }
}

impl Eq for IndexUpdate {}

/// A user event standing for the kind "index update", to subscribe to by
/// discriminant (the value is never delivered).
///
/// `EventClause::Discriminant` compares only the `UserEvent` variant, so the
/// payload (an empty Claude snapshot) is irrelevant: every index event
/// matches, whatever the harness or kind.
pub fn index_event_kind() -> UserEvent {
    use crate::model::HarnessId;
    UserEvent::Index(IndexUpdate::new(
        0,
        IndexEvent::Snapshot {
            harness: HarnessId::Claude,
            available: false,
            projects: Arc::default(),
            sessions: Arc::default(),
        },
    ))
}

/// Reads the index worker's channel without blocking and emits
/// [`UserEvent::Index`] events, numbered from 1.
///
/// When the worker has stopped (the channel is disconnected) it has nothing
/// more to report; the port reports no error, so the application keeps
/// running with the data it has.
#[derive(Debug)]
pub struct IndexPort {
    events: Receiver<IndexEvent>,
    seq: u64,
}

impl IndexPort {
    /// A port reading `events`.
    pub fn new(events: Receiver<IndexEvent>) -> Self {
        Self { events, seq: 0 }
    }
}

impl Poll<UserEvent> for IndexPort {
    fn poll(&mut self) -> PortResult<Option<Event<UserEvent>>> {
        match self.events.try_recv() {
            Ok(event) => {
                self.seq += 1;
                Ok(Some(Event::User(UserEvent::Index(IndexUpdate::new(
                    self.seq, event,
                )))))
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;
    use crate::model::{HarnessId, SessionId, SessionStats, TokenUsage};

    fn stats_event(n: usize) -> IndexEvent {
        IndexEvent::Stats {
            harness: HarnessId::Claude,
            session: SessionId::from("s"),
            stats: SessionStats {
                turns: n,
                agent_time: Duration::ZERO,
                run_time: Duration::ZERO,
                tokens: TokenUsage::default(),
            },
        }
    }

    #[test]
    fn events_come_out_in_order_with_increasing_sequence_numbers() {
        let (tx, rx) = mpsc::channel();
        let mut port = IndexPort::new(rx);
        tx.send(stats_event(1)).unwrap();
        tx.send(stats_event(2)).unwrap();
        let mut seqs = Vec::new();
        let mut turns = Vec::new();
        while let Some(Event::User(UserEvent::Index(update))) = port.poll().unwrap() {
            seqs.push(update.seq());
            if let IndexEvent::Stats { stats, .. } = update.event() {
                turns.push(stats.turns);
            }
        }
        assert_eq!(seqs, [1, 2]);
        assert_eq!(turns, [1, 2]);
    }

    #[test]
    fn an_empty_channel_gives_nothing() {
        let (_tx, rx) = mpsc::channel();
        assert!(IndexPort::new(rx).poll().unwrap().is_none());
    }

    #[test]
    fn a_stopped_worker_is_not_an_error() {
        let (tx, rx) = mpsc::channel();
        let mut port = IndexPort::new(rx);
        tx.send(stats_event(1)).unwrap();
        drop(tx);
        assert!(port.poll().unwrap().is_some(), "queued events still arrive");
        assert!(port.poll().unwrap().is_none());
        assert!(port.poll().unwrap().is_none());
    }

    #[test]
    fn updates_are_equal_when_their_sequence_numbers_are() {
        let a = IndexUpdate::new(1, stats_event(1));
        assert_eq!(a, a.clone());
        assert_eq!(a, IndexUpdate::new(1, stats_event(9)));
        assert_ne!(a, IndexUpdate::new(2, stats_event(1)));
    }
}
