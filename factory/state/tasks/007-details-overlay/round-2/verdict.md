VERDICT: REWORK

## Assessment

One bug remains, and it is what is left of round 1's must-fix: a `details` key bound to a chord that uses an overlay key opens the overlay, but the same chord does not close it. Everything else in round 2 holds. The fix is small, so this is a rework and not a human choice.

**Code review: PASS**, seven nits.
- Finding 1 (nit), `char_wrap("abcd ", 4)` gives a blank continuation row: to be fixed. I confirmed it by reading the code. Tasks 008 and 009 pass raw harness strings, so it becomes reachable. The note also covers two trailing spaces, which the reviewer's check would miss.
- Finding 2 (nit), a chord whose second key is an overlay key does not close: to be fixed. This is the same bug as the feature and UI must-fix. See Conflicts.
- Finding 3 (nit), `widest_row` measures other text than is drawn: to be fixed, together with UI finding 2.
- Finding 4 (nit), `open_details` works out the size twice: to be fixed. It takes a few lines and I tried it.
- Finding 5 (nit), `DESIRED_SIZE_ATTR` answers with `interior_value`: to be fixed. One word.
- Finding 6 (nit), a test name claims more than the test asserts: to be fixed by adding the assertion.
- Finding 7 (nit), three long doc lines: to be fixed. Comments only.

**Feature review: FAIL**, one must-fix and two nits.
- Finding 1 (must-fix), the chord does not close: to be fixed, but not with the snippet in the review. See Conflicts.
- Finding 2 (nit), `help_overlay/mod.rs:62` still clones the theme, although the build notes say this was done: to be fixed. One line.
- Finding 3 (nit), long doc lines: the same as code finding 7.

**UI review: FAIL**, one must-fix and one nit.
- Finding 1 (must-fix), the same bug as feature finding 1: to be fixed.
- Finding 2 (nit), `\r` widens the overlay: the same as code finding 3.

None of the nits would justify a rework alone. They are included because the rework is happening anyway, each is at most a few lines, and each was tried in a scratch copy.

**Why the must-fix is not minor.** `Esc` still closes the overlay, and a chord like `z j` is an unusual binding, so I weighed approving. I decided against it:
- SPEC.md §5.1 says "The `details` key also closes it", and an acceptance criterion says a remapped key opens and closes the overlay. `details = ["z j"]` loads with no warning, so the app accepts this as a valid key and then does not honour it.
- `z k` is worse to the user than `z j`: at the top of the content, neither key does anything.
- Round 1 raised this same requirement as a must-fix, and round 2 fixed only part of it.
- The fix is about ten lines, it is verified, and no REWORK has been used.

**Why this is not an A/B choice.** All three reviewers point to the same behaviour: a completed chord closes the overlay, and a single overlay key keeps its overlay meaning. The fix changes nothing else a user can see.

**Independent checks**, on a clean export of `HEAD` with its own target directory and `INSTA_UPDATE=no`:
- With the new tests on the round-2 code, three of them fail: `z j` at app level, `x j` and `j i`. This reproduces the finding.
- With the snippet from feature finding 1 and the UI review's equivalent steps, those three pass, but the new `the_prefix_of_another_chord_...` test fails. So the snippet is not safe to use as written.
- With the note below applied: `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass, and `cargo test` gives 2773 passed, 0 failed, 2 ignored. No `.snap` file changes, and no `.snap.new` file is written.
- I did not rerun the budgets. The builder and all three reviewers report them within budget, nobody disputes that, and the note touches no parsing or per-frame cost beyond removing one theme clone.

## Conflicts

1. **Severity.** The code reviewer rates the chord a nit because the binding planner note resolves `Context::Overlay` first. The feature and UI reviewers rate it a must-fix. These are not opposing demands: the code reviewer offers the same kind of change "if wanted" and does not ask to keep the behaviour. The planner note says "as in `HelpOverlay::on`", whose context has no chords, so it decides what one key with two meanings does. That single-key precedence stays, and `the_overlay_keys_come_first` still pins it. A pending chord that the next key completes is not that case. The spec sentence decides it.

2. **Scope of the fix.** The code reviewer would feed the resolver only while a prefix is pending, and says only the second key is affected. The feature and UI reviewers feed every key. Code outranks feature, but the narrower fix does not work: in `"j i"`, the `j` goes to the frame and the resolver never sees it, so `i` answers nothing. Both the feature and UI probes show this, and so does my test. The warning that `"j i"` gives at load only removes `j` from `next`; the chord still opens the overlay. The note feeds every key, and drops the narrower fix.

3. **The reviews' snippet.** Feature finding 1 and UI finding 1 close when a prefix was pending and `feed` answers `Action(Details)`. That is not the same as completing a chord, because `feed` drops a prefix the key does not complete and then resolves the key alone. With `details = "j"` and `top = "g g"`, pressing `g` then `j` closes the overlay instead of scrolling, against the planner's precedence. I confirmed this with a test. The note uses a check on the chord itself, and drops the snippet.

## Rework budget

Earlier REWORK verdicts: 0 of 2. Round 1 has no `verdict.md`, so its fix round does not count. This verdict is the first REWORK.

## Rework note

Round 2's resize, too-small guard and wrapping work stays. This round makes a `details` chord that uses an overlay key close the overlay, and clears the remaining round-2 review nits. This note replaces the fix suggested in the round-2 reviews. **Do not use the snippet in feature-review finding 1 or the steps in ui-review finding 1**, and do not feed the resolver only while a prefix is pending. Both are wrong; the reasons are in steps 1 and 2.

**1. Add a chord check to `KeyResolver`, in `src/keymap/mod.rs`, above `feed`.** `was pending && feed answered Action(Details)` is not enough, because `feed` drops a prefix the key does not complete and then resolves the key alone. With `details = "j"` and `top = "g g"`, `g` then `j` would close the overlay instead of scrolling.

```rust
/// Whether `event`, as the next key, completes a chord bound to `action`:
/// the pending prefix and this key together, not this key on its own.
pub fn completes(&self, keymap: &Keymap, event: &KeyEvent, action: Action) -> bool {
    self.pending.is_some_and(|first| {
        keymap.chord_action(self.context, first, KeySpec::from_event(event)) == Some(action)
    })
}
```

Add this test to the chords section of `src/keymap/tests.rs`:

```rust
#[test]
fn completes_is_about_the_chord_and_not_about_the_key_alone() {
    let (km, warnings) = keymap("[keys.browser]\nsearch = \"z x\"\ntop = \"v\"\n");
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut r = KeyResolver::new(Browser);
    assert!(!r.completes(&km, &ch('x'), Search), "nothing is pending");
    assert_eq!(feed(&mut r, &km, 'z'), Resolved::Pending);
    assert!(r.completes(&km, &ch('x'), Search));
    assert!(!r.completes(&km, &ch('x'), Top), "another action");
    assert!(!r.completes(&km, &ch('v'), Top), "a single key");
    assert!(r.is_pending(), "asking changes nothing");
}
```

**2. Change `DetailsOverlay::on`, in `src/components/details_overlay/mod.rs`.** Every key goes to the resolver exactly once, before the frame, so that an overlay key can start a chord (`j i`) as well as end one (`z j`). Remove the `reset()` call, because `feed` already drops a prefix. Replace the function and its doc comment with:

```rust
/// A `details` chord that this key completes closes the overlay, whatever
/// the key means in the overlay. Otherwise the overlay keys come first
/// (`Esc` closes, the up, down and page keys scroll), so a single `details`
/// key that is also an overlay key does what the overlay says; then the
/// `details` key closes it too. Every other key does nothing: the keys of
/// the transcript and the global keys are not for the overlay.
fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
    let key = match ev {
        Event::Keyboard(key) => key,
        // Only what the `App` routed here, in the overlay's coordinates.
        Event::User(UserEvent::Mouse(mouse)) => return self.on_mouse(mouse),
        _ => return None,
    };
    // The `details` key has a resolver of its own, because it can be a
    // chord. It sees every key once, so an overlay key can start or end a
    // chord, and a key that does not complete one drops the prefix.
    let keymap = self.frame.keymap();
    let completes_chord = self.resolver.completes(keymap, key, Action::Details);
    let resolved = self.resolver.feed(keymap, key);
    if completes_chord {
        return Some(Msg::CloseDetails);
    }
    let total = self.total();
    match self.frame.on_key(key, total) {
        FrameKey::Close => Some(Msg::CloseDetails),
        FrameKey::Scrolled => Some(Msg::DetailsScrolled),
        FrameKey::Unmoved => None,
        FrameKey::Unbound => {
            (resolved == Resolved::Action(Action::Details)).then_some(Msg::CloseDetails)
        }
    }
}
```

Add these tests to the `tests` module of the same file:

```rust
/// An overlay whose browser keys are `bindings` (action, key).
fn overlay_bound(bindings: &[(&str, &str)]) -> DetailsOverlay {
    let mut raw = crate::config::RawKeys::new();
    let browser = raw.entry("browser".to_owned()).or_default();
    for (action, key) in bindings {
        browser.insert((*action).to_owned(), vec![(*key).to_owned()]);
    }
    let (keymap, _) = Keymap::from_config(&raw);
    overlay_with(keymap, 10)
}

#[test]
fn a_chord_that_ends_in_an_overlay_key_closes_it() {
    let mut overlay = overlay_bound(&[("details", "x j")]);
    assert_eq!(press(&mut overlay, Key::Char('x')), None);
    assert_eq!(press(&mut overlay, Key::Char('j')), Some(Msg::CloseDetails));
    assert_eq!(offset(&overlay), 0);
}

#[test]
fn the_overlay_key_of_such_a_chord_scrolls_without_the_prefix() {
    let mut overlay = overlay_bound(&[("details", "x j")]);
    assert_eq!(
        press(&mut overlay, Key::Char('j')),
        Some(Msg::DetailsScrolled)
    );
    assert_eq!(offset(&overlay), 1);
}

#[test]
fn a_chord_that_starts_with_an_overlay_key_scrolls_and_then_closes_it() {
    let mut overlay = overlay_bound(&[("details", "j i")]);
    assert_eq!(press(&mut overlay, Key::Char('i')), None);
    assert_eq!(
        press(&mut overlay, Key::Char('j')),
        Some(Msg::DetailsScrolled)
    );
    assert_eq!(offset(&overlay), 1);
    assert_eq!(press(&mut overlay, Key::Char('i')), Some(Msg::CloseDetails));
}

#[test]
fn the_prefix_of_another_chord_does_not_make_a_single_overlay_key_close_it() {
    let mut overlay = overlay_bound(&[("details", "j"), ("top", "g g")]);
    assert_eq!(press(&mut overlay, Key::Char('g')), None);
    assert_eq!(
        press(&mut overlay, Key::Char('j')),
        Some(Msg::DetailsScrolled)
    );
    assert_eq!(offset(&overlay), 1);
}
```

The `j i` test takes the place of the `k i` test that feature-review finding 1 asks for; it covers the same path.

Add this test to `src/app/browser_tests/details.rs`, next to `a_details_key_bound_to_a_chord_opens_and_closes_the_overlay`:

```rust
#[test]
fn a_details_chord_that_ends_in_an_overlay_key_opens_and_closes_the_overlay() {
    let mut rig = rig_with_keymap(keymap_with("browser", "details", "z j"));
    rig.open_loaded();
    rig.press(Key::Char('z'));
    rig.press(Key::Char('j'));
    rig.pump_until(open);
    assert_eq!(rig.focus(), Some(Id::DetailsOverlay));
    rig.press(Key::Char('z'));
    rig.press(Key::Char('j'));
    rig.pump_until(|app| !open(app));
    assert_eq!(rig.focus(), Some(Id::Transcript));
}
```

**3. Clear the remaining round-2 nits.**

In `src/components/details_overlay/content.rs`:
- `widest_row` measures what is drawn. Replace the `.flat_map(|row| row.value.split('\n'))` and `.map(UnicodeWidthStr::width)` lines with `.flat_map(|row| display_lines(&row.value))` and `.map(|line| columns(&line))`, and add this next to `char_wrap`:
  ```rust
  /// The columns `text` takes, counted as [`char_wrap`] counts them.
  fn columns(text: &str) -> usize {
      text.chars().map(|ch| ch.width().unwrap_or(0)).sum()
  }
  ```
- `char_wrap` leaves no blank row at the end. Replace the final `pieces.push(current);` with:
  ```rust
  // Only spaces after the last cut are not a row.
  if pieces.is_empty() || !current.trim().is_empty() {
      pieces.push(current);
  }
  ```
- At the end of `a_space_the_cut_falls_on_does_not_start_the_next_row`, add:
  ```rust
  // Spaces at the end after a cut are not a row of their own.
  assert_eq!(char_wrap("abcd ", 4), ["abcd"]);
  assert_eq!(char_wrap("abcd  ", 4), ["abcd"]);
  // A value always has a row.
  assert_eq!(char_wrap("", 4), [""]);
  assert_eq!(char_wrap("  ", 4), ["  "]);
  ```
- At the end of `a_wrapped_value_with_spaces_loses_only_the_spaces_at_the_cuts`, add:
  ```rust
  // Joined up, only the space the cut fell on is gone.
  let value: String = texts[3..texts.len() - 1]
      .iter()
      .map(|line| &line[7..])
      .collect();
  assert_eq!(value, "one two three fourfive six");
  ```
- Add this test:
  ```rust
  #[test]
  fn what_is_not_drawn_does_not_widen_the_overlay() {
      let wanted = |value: &str| {
          let rows = vec![row("k", value)];
          DetailsContent::new(vec![DetailsGroup::new("G", rows)]).desired_size(u16::MAX)
      };
      let long = "x".repeat(60);
      let plain = wanted(&format!("{long}\n{long}"));
      assert_eq!(wanted(&format!("{long}\r\n{long}\r\n")), plain);
      assert_eq!(wanted(&format!("{long}\u{1b}\u{7}\n{long}")), plain);
  }
  ```

In `src/components/details_overlay/mod.rs`:
- Answer `DESIRED_SIZE_ATTR` with `payload_value(size)`, not `interior_value(size)`.

In `src/components/help_overlay/mod.rs`:
- In `view`, use `let theme = self.frame.theme();` and pass `theme` to `lines`, with no clone.
- Rewrap the `on_mouse` doc comment.

In `src/app/layout.rs` and `src/app/mouse.rs`:
- Rewrap the `help_overlay_rect` and `outside_overlay` doc comments to the width of the comments around them. Comments only.

In `src/app/details.rs`, give `open_details` one path to the size:
- Import `Component` too: `use tuirealm::component::{AppComponent, Component};`.
- Build the overlay like this, and delete the `self.refresh_details_size();` call in `open_details` (`push_viewports` keeps its call):
  ```rust
  let mut overlay = DetailsOverlay::new(content, self.theme.clone(), self.keymap.clone());
  let max_width = overlay_max_width(self.size.0);
  overlay.attr(MAX_WIDTH_ATTR, max_width_value(max_width));
  // Wrapped at the width the screen leaves it, the content is as high
  // as it will be drawn.
  let wanted = overlay.desired_size(max_width);
  ```

**Leave as is.** The planner's single-key precedence: with `details = "j"`, `j` scrolls and does not close, and `the_overlay_keys_come_first` keeps passing unchanged. Also `an_overlay_key_drops_a_pending_prefix`, the round-2 chord, resize and too-small tests, the wrapping tests apart from the two additions in step 3, and all snapshots.

**Done when.**
- The 7 new tests and the two additions to existing tests in step 3 pass, and every other existing test passes unchanged.
- In the real binary or a scratch crate, `details = ["z j"]`, `["z k"]` and `["j i"]` each open and close the overlay with the same chord.
- Each of these temporary edits fails the named tests. Make each edit, check it, restore the code, and say so in the build notes:
  - In `on`, set `completes_chord` to `self.resolver.is_pending()` before the `feed`, and close only if it is true and `resolved == Resolved::Action(Action::Details)`: `the_prefix_of_another_chord_does_not_make_a_single_overlay_key_close_it`.
  - Delete the `if completes_chord` early return and feed the resolver only in the `FrameKey::Unbound` arm: `a_chord_that_ends_in_an_overlay_key_closes_it`, `a_chord_that_starts_with_an_overlay_key_scrolls_and_then_closes_it` and the new app test.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` pass, with 2773 tests passing (2766 plus the 7 new ones).
- `INSTA_UPDATE=no cargo test --release -- --ignored budgets --nocapture` stays within budget.
- No `.snap` file changes in this round, and no `.snap.new` file is left behind. A changed snapshot is a failure, so do not accept one.
