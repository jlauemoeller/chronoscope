VERDICT: PASS

## Summary
The keymap (`src/keymap/`) implements every planner-notes rule: the full default table with groups and descriptions, the key syntax parser, event normalisation (SHIFT ignored on characters, `BackTab` == `shift-tab`, `Char(' ')` == `space`), overrides with warnings that keep the default, same-context key moves, chord-prefix exclusivity, `KeyResolver` chords, and labels and help entries that follow remappings. The code is idiomatic: no panics on user input, no needless clones, a clear split into `defaults` / `key` / `mod`. Every acceptance criterion has a test that would fail if the behaviour were wrong. The test gate is green; only nits remain.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (147 unit tests + integration tests, 0 failed)

## Findings
1. [nit] `src/keymap/key.rs` `KeySpec::to_event` — the doc comment says "A key event for this key, for subscriptions that need one". tuirealm's `EventClause::Keyboard(k)` matches exactly (`tuirealm-4.1.0/src/core/subscription.rs:162`: `Some(k) == ev.as_keyboard()`). A subscription built from the normalised `?` or `G` (no SHIFT) would therefore miss terminals that report SHIFT, which is exactly the quirk the keymap normalises away. Change: say in the doc that exact-match subscriptions do not get the keymap's normalisation, and that `GlobalEvents` should subscribe broadly (e.g. `EventClause::Any`/keyboard plus `SubClause`) and call `Keymap::resolve`. Or drop the "for subscriptions" wording until a caller needs it.
2. [nit] `src/keymap/key.rs` `KeySpec::parse` — `rest.chars().count() == 1` followed by `rest.chars().next().unwrap_or(' ')` uses a dummy fallback that can never happen. Change: `let mut chars = rest.chars(); let code = match (chars.next(), chars.next()) { (Some(c), None) => Key::Char(c), _ => named_key(..)... };`.
3. [nit] `src/keymap/key.rs` `KeySpec::new` / `parse` — `ctrl-D` (CONTROL plus an uppercase ASCII letter) parses into a binding that legacy terminals never send: Ctrl-Shift-d arrives as `Char('d')` + CONTROL. `shift-1` silently becomes `1`. Neither gets a warning. Consider lowercasing ASCII letters that carry CONTROL, or warning about such bindings. The test `parser_accepts_every_documented_form` accepts `"Ctrl-D"`, so align it with whichever you choose.
4. [nit] `src/keymap/key.rs` `KeySpec::is_ctrl_c` — it uses `modifiers.contains(CONTROL)`, so `ctrl-alt-c` is also rejected with the message "Ctrl-c always quits". This is harmless, but either compare `modifiers == KeyModifiers::CONTROL` or mention it in the doc comment.
5. [nit] `src/keymap/mod.rs` `Keymap::help_entries` — it repeats the ` / ` join logic of `Keymap::labels`. Change: pull out a private `fn join_labels(keys: &[KeySeq]) -> String` and use it in both.
6. [nit] `src/keymap/mod.rs` `KeyResolver` — the struct derives `Copy`, but it is stateful (it holds the pending chord prefix). A later component could write `let mut r = self.resolver; r.feed(..)` and silently lose the prefix, with no borrow error to catch it. Change: derive `Clone` only, not `Copy`.
