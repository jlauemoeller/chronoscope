//! The panic hook restores the terminal before it delegates to the previous
//! hook. The panic hook is process-global, so this binary holds a single test.

use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

use chronoscope::terminal::install_panic_hook_with;

#[test]
fn restore_runs_before_the_previous_hook_on_every_panic() {
    let calls: Arc<Mutex<Vec<String>>> = Arc::default();

    let original = panic::take_hook();
    let for_restore = Arc::clone(&calls);
    let for_previous = Arc::clone(&calls);
    install_panic_hook_with(
        move || for_restore.lock().unwrap().push("restore".to_owned()),
        Box::new(move |info| {
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .copied()
                .unwrap_or("?");
            for_previous
                .lock()
                .unwrap()
                .push(format!("previous: {message}"));
        }),
    );

    let first = panic::catch_unwind(AssertUnwindSafe(|| panic!("boom")));
    let second = panic::catch_unwind(AssertUnwindSafe(|| panic!("again")));
    panic::set_hook(original);

    assert!(first.is_err() && second.is_err());
    assert_eq!(
        *calls.lock().unwrap(),
        vec!["restore", "previous: boom", "restore", "previous: again"]
    );
}
