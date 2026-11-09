//! What a `notify` event means to the watcher of a loaded session
//! (product §6.1). Both adapters' watchers read events this way and then
//! match the paths against their own files.

use std::path::PathBuf;

use notify::{Event, EventKind};

/// What a watcher makes of one `notify` event.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Notice {
    /// The watcher looks at its data whatever the paths are: on an error
    /// (possibly an overflow), when events were lost (a rescan is due), and
    /// for an event that names no path.
    pub(crate) wake: bool,
    /// The paths the event names, for the watcher to match against its own
    /// files. None for an access, which the watcher's own reads cause.
    pub(crate) paths: Vec<PathBuf>,
}

/// Reads `event` for a watcher; see [`Notice`]. An access never wakes it, so
/// the watcher's own reads do not cause more reads.
pub(crate) fn notice(event: notify::Result<Event>) -> Notice {
    let event = match event {
        Ok(event) => event,
        Err(err) => {
            log::debug!("file watch error: {err}");
            return Notice {
                wake: true,
                paths: Vec::new(),
            };
        }
    };
    if matches!(event.kind, EventKind::Access(_)) {
        return Notice::default();
    }
    Notice {
        wake: event.need_rescan() || event.paths.is_empty(),
        paths: event.paths,
    }
}

#[cfg(test)]
mod tests {
    use notify::event::{AccessKind, CreateKind, Flag, ModifyKind};

    use super::*;

    fn modified(path: &str) -> Event {
        Event::new(EventKind::Modify(ModifyKind::Any)).add_path(PathBuf::from(path))
    }

    #[test]
    fn an_error_wakes_the_watcher() {
        let got = notice(Err(notify::Error::generic("overflow")));
        assert_eq!(
            got,
            Notice {
                wake: true,
                paths: Vec::new()
            }
        );
    }

    #[test]
    fn an_access_is_skipped_whatever_it_names() {
        let access = Event::new(EventKind::Access(AccessKind::Any))
            .add_path(PathBuf::from("/d/s1.jsonl"))
            .set_flag(Flag::Rescan);
        assert_eq!(notice(Ok(access)), Notice::default());
        let unnamed = Event::new(EventKind::Access(AccessKind::Any));
        assert_eq!(notice(Ok(unnamed)), Notice::default());
    }

    #[test]
    fn a_rescan_wakes_the_watcher_and_keeps_the_paths() {
        let event = modified("/d/s1.jsonl").set_flag(Flag::Rescan);
        assert_eq!(
            notice(Ok(event)),
            Notice {
                wake: true,
                paths: vec![PathBuf::from("/d/s1.jsonl")]
            }
        );
    }

    #[test]
    fn an_event_without_paths_wakes_the_watcher() {
        let event = Event::new(EventKind::Create(CreateKind::Any));
        assert_eq!(
            notice(Ok(event)),
            Notice {
                wake: true,
                paths: Vec::new()
            }
        );
    }

    #[test]
    fn any_other_event_leaves_its_paths_to_the_watcher() {
        let event = modified("/d/s1.jsonl").add_path(PathBuf::from("/d/other.jsonl"));
        assert_eq!(
            notice(Ok(event)),
            Notice {
                wake: false,
                paths: vec![
                    PathBuf::from("/d/s1.jsonl"),
                    PathBuf::from("/d/other.jsonl")
                ]
            }
        );
    }
}
