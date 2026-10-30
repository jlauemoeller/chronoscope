//! The Port that turns the open session's [`ModelUpdate`]s into user events
//! (SPEC §1.1, §1.4).
//!
//! A harness streams a session over a plain `Sender<ModelUpdate>`. Every
//! load gets a number, its *generation*, and a small forwarding thread
//! ([`forward_to`]) tags what the harness sends with it and passes it on to
//! one long-lived channel, which the [`SessionPort`] reads. The application
//! drops the updates of a generation that has been replaced.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::Duration;

use tuirealm::event::Event;
use tuirealm::listener::{Poll, PortResult};

use super::UserEvent;
use crate::model::ModelUpdate;

/// How often the session port is polled.
pub const SESSION_POLL_INTERVAL: Duration = Duration::from_millis(10);
/// The most updates one poll takes from the channel, so the parse of a big
/// session is drained quickly without starving the other ports.
pub const SESSION_MAX_PER_POLL: usize = 16;

/// An update of the session being loaded, tagged with the load it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionEvent {
    /// The number of the load that produced the update.
    pub generation: u64,
    /// What the harness reported.
    pub update: ModelUpdate,
}

/// A [`ModelUpdate`] on its way to the application.
///
/// User events must be `Eq + Clone` and the payload is large and cheap to
/// share, so it is `Arc`-wrapped and two updates are equal when they have the
/// same sequence number, which the [`SessionPort`] gives each update once.
#[derive(Debug, Clone)]
pub struct SessionUpdate {
    seq: u64,
    update: Arc<ModelUpdate>,
}

impl SessionUpdate {
    /// An update with sequence number `seq`.
    pub fn new(seq: u64, update: ModelUpdate) -> Self {
        Self {
            seq,
            update: Arc::new(update),
        }
    }

    /// The sequence number.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// What the harness reported.
    pub fn update(&self) -> &ModelUpdate {
        &self.update
    }

    /// The shared update, to hand on without copying it.
    pub fn shared(&self) -> Arc<ModelUpdate> {
        Arc::clone(&self.update)
    }
}

impl PartialEq for SessionUpdate {
    fn eq(&self, other: &Self) -> bool {
        self.seq == other.seq
    }
}

impl Eq for SessionUpdate {}

/// A user event standing for the kind "session update", to subscribe to by
/// discriminant (the value is never delivered).
pub fn session_event_kind() -> UserEvent {
    UserEvent::Session {
        generation: 0,
        update: SessionUpdate::new(0, ModelUpdate::Loaded),
    }
}

/// The channel the forwarding threads write to and the [`SessionPort`] reads.
pub fn session_channel() -> (Sender<SessionEvent>, Receiver<SessionEvent>) {
    channel()
}

/// Returns the sender to give to `Harness::load` for load number
/// `generation`. A thread tags everything sent to it with `generation` and
/// passes it to `sink`. The thread ends when the harness drops the sender
/// (its worker stopped) or when the port is gone.
pub fn forward_to(generation: u64, sink: Sender<SessionEvent>) -> Sender<ModelUpdate> {
    let (tx, rx) = channel::<ModelUpdate>();
    let spawned = std::thread::Builder::new()
        .name("session-forward".to_owned())
        .spawn(move || {
            for update in rx {
                if sink.send(SessionEvent { generation, update }).is_err() {
                    break;
                }
            }
        });
    if let Err(err) = spawned {
        // The sender is useless without the thread: its receiver is gone and
        // the harness sees the hang-up.
        log::error!("could not start the session forwarding thread: {err}");
    }
    tx
}

/// Reads the session channel without blocking and emits
/// [`UserEvent::Session`] events, numbered from 1.
#[derive(Debug)]
pub struct SessionPort {
    events: Receiver<SessionEvent>,
    seq: u64,
}

impl SessionPort {
    /// A port reading `events`.
    pub fn new(events: Receiver<SessionEvent>) -> Self {
        Self { events, seq: 0 }
    }
}

impl Poll<UserEvent> for SessionPort {
    fn poll(&mut self) -> PortResult<Option<Event<UserEvent>>> {
        match self.events.try_recv() {
            Ok(SessionEvent { generation, update }) => {
                self.seq += 1;
                Ok(Some(Event::User(UserEvent::Session {
                    generation,
                    update: SessionUpdate::new(self.seq, update),
                })))
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    fn poll_all(port: &mut SessionPort) -> Vec<(u64, u64, ModelUpdate)> {
        let mut out = Vec::new();
        while let Some(Event::User(UserEvent::Session { generation, update })) =
            port.poll().unwrap()
        {
            out.push((generation, update.seq(), update.update().clone()));
        }
        out
    }

    #[test]
    fn updates_come_out_in_order_with_generation_and_increasing_sequence_numbers() {
        let (tx, rx) = session_channel();
        let mut port = SessionPort::new(rx);
        tx.send(SessionEvent {
            generation: 3,
            update: ModelUpdate::Progress(0.5),
        })
        .unwrap();
        tx.send(SessionEvent {
            generation: 3,
            update: ModelUpdate::Loaded,
        })
        .unwrap();
        assert_eq!(
            poll_all(&mut port),
            [
                (3, 1, ModelUpdate::Progress(0.5)),
                (3, 2, ModelUpdate::Loaded)
            ]
        );
    }

    #[test]
    fn an_empty_or_closed_channel_gives_nothing() {
        let (tx, rx) = session_channel();
        let mut port = SessionPort::new(rx);
        assert!(port.poll().unwrap().is_none());
        drop(tx);
        assert!(port.poll().unwrap().is_none());
    }

    #[test]
    fn updates_are_equal_when_their_sequence_numbers_are() {
        let a = SessionUpdate::new(1, ModelUpdate::Loaded);
        assert_eq!(a, a.clone());
        assert_eq!(a, SessionUpdate::new(1, ModelUpdate::Reset));
        assert_ne!(a, SessionUpdate::new(2, ModelUpdate::Loaded));
    }

    #[test]
    fn the_forwarder_tags_what_the_harness_sends_with_its_generation() {
        let (sink, rx) = session_channel();
        let tx = forward_to(7, sink);
        tx.send(ModelUpdate::Progress(0.25)).unwrap();
        tx.send(ModelUpdate::Loaded).unwrap();
        drop(tx);
        let mut port = SessionPort::new(rx);
        let mut got = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while got.len() < 2 && Instant::now() < deadline {
            got.extend(poll_all(&mut port));
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            got,
            [
                (7, 1, ModelUpdate::Progress(0.25)),
                (7, 2, ModelUpdate::Loaded)
            ]
        );
    }

    #[test]
    fn two_loads_are_told_apart_by_their_generation() {
        let (sink, rx) = session_channel();
        let old = forward_to(1, sink.clone());
        let new = forward_to(2, sink);
        old.send(ModelUpdate::Reset).unwrap();
        // Wait for the old one so the order is fixed.
        let mut port = SessionPort::new(rx);
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut got = Vec::new();
        while got.is_empty() && Instant::now() < deadline {
            got.extend(poll_all(&mut port));
        }
        new.send(ModelUpdate::Loaded).unwrap();
        while got.len() < 2 && Instant::now() < deadline {
            got.extend(poll_all(&mut port));
        }
        let generations: Vec<u64> = got.iter().map(|(g, ..)| *g).collect();
        assert_eq!(generations, [1, 2]);
    }
}
