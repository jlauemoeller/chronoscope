STATUS: DONE

## Changes
- `src/keymap/mod.rs` (new, `pub mod keymap` in `src/lib.rs`): `Context` (global/browser/selector/overlay), `Action` enum, `Binding`, `HelpEntry`, `Keymap` (`default()`, `from_config(&RawKeys) -> (Keymap, Vec<ConfigWarning>)`, `resolve`, `keys`, `label`, `labels`, `help_entries`, `bindings`), `KeyResolver` + `Resolved {Action, Pending, Unbound}` for chords. Warnings are returned and logged.
- `src/keymap/key.rs` (new): key syntax parser (`KeySeq::parse`, `KeySpec`), event normalisation (SHIFT ignored on chars, BackTab == shift-tab, Char(' ') == space), labels, `KeyParseError` naming the string.
- `src/keymap/defaults.rs` (new): the default table (keys, group, description) in help order.
- `src/keymap/tests.rs` (new): 50 tests. UI: none, so no snapshots.

Decisions beyond the task text:
- `Action` is one enum; `Search`, `Up`, `Down`, `Resume` are shared by several contexts, an action is identified by (context, action).
- `Ctrl-c` never resolves, and binding it (alone or in a chord) is rejected with a warning (it always quits).
- `[keys.x] action = []` unbinds the action without warning; unbound actions return `None` from `label`/`labels` and are omitted from `help_entries`.
- A user-set `global` key that collides with a default of another context removes it from that action with a warning (user wins; otherwise both would fire). A chord is rejected if either of its keys is a global key.
- Overrides are applied in context order global, browser, selector, overlay, actions alphabetically within a context.
- `Keymap::resolve` considers single keys only; chords need `KeyResolver::new(context).feed(&keymap, &event)`.

## Self-check
- AC: full default keymap — `default_keymap_matches_the_planner_table`, `defaults_do_not_collide_within_or_across_contexts`
- AC: no chord / modifier+Enter defaults — `no_default_is_a_chord_or_modifier_enter`
- AC: parser accepts/rejects — `parser_accepts_every_documented_form`, `parser_rejects_bad_keys_naming_the_string`
- AC: resolution — `capital_g_resolves_to_bottom_with_or_without_shift`, `back_tab_resolves_to_prev_pane`, `space_resolves_to_fold_toggle`, `ctrl_d_resolves_to_half_page_down`, `ctrl_c_resolves_to_nothing_in_every_context`
- AC: overrides replace / keep defaults — `override_replaces_the_default_keys_entirely`, `the_spec_example_keeps_the_defaults`
- AC: warnings keep defaults — `unknown_action_warns_and_keeps_defaults`, `unknown_context_warns_and_keeps_defaults`, `unparsable_key_warns_naming_it_and_keeps_the_default`, `chord_in_global_or_overlay_warns_and_keeps_the_default`, `collision_with_a_global_key_warns_and_keeps_the_default`
- AC: same-context collision — `same_context_collision_moves_the_key_and_warns`
- AC: chords — `chord_completes_with_its_second_key`, `a_key_that_does_not_complete_the_chord_is_resolved_afresh`, `a_chord_prefix_is_only_a_prefix`
- AC: label/labels — `labels_use_the_documented_formats`, `every_named_key_has_its_documented_label`, `labels_reflect_remappings`
- AC: help_entries — `help_entries_follow_table_order_and_groups`, `help_entries_show_remapped_keys`
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (0 failed)
