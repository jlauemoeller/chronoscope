VERDICT: PASS

## Summary
The theme model, colour parsing, TOML parsing, the seven built-in themes and the `to_ansi256()` / `without_colors()` transforms are implemented cleanly as pure library code, and every acceptance criterion is met and covered by a test that would fail if the behaviour were wrong. I checked all seven palettes value by value against ratcn 0.0.6 `src/theme.rs`, and checked `dim` against `src/color.rs`. They match, including Terminal's `ring` (ratcn's `resolve_rgb(Reset)` is `None`, so `White` passes through) and the planner's Terminal substitutions. Only nits remain.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (40 unit + 5 other tests, 0 failed)

## Findings
1. [nit] `src/theme/parse.rs:71-96`: the `Palette` is built from `colors[0]`..`colors[15]` by position, which ties it silently to the order of `Role::ALL`. If someone reorders `Role::ALL`, roles get swapped. `complete_document_parses_to_the_expected_theme` would catch it, but the coupling is easy to miss. Change: in the `Role::ALL` loop, fill the palette with the existing private `Palette::set` (`parse` is a child module of `theme`, so it can already see it; no visibility change is needed), i.e. `palette.set(role, color_from(key, value)?)`, starting from a private all-`Reset` `Palette` constructor, and remove the `colors` vector and the index list.
2. [nit] `src/theme/parse.rs:38-41,57`: `ThemeError::InvalidColor` has a field named `source`, but `impl std::error::Error for ThemeError {}` does not override `source()`, so error-chain reporters lose the `ColorParseError`. Change: implement `fn source(&self) -> Option<&(dyn std::error::Error + 'static)>`, returning `Some(source)` for `InvalidColor` and `None` otherwise.
3. [nit] `src/theme/parse.rs:125-131`: bare TOML integers are accepted as colour indices (`accent = 99`). That goes beyond the planner's format ("Simple tokens take a colour string") and is not in the module's format doc. The `WrongType` message also still says "a colour string". Change: either document bare integer indices in the `//!` format block at the top of `parse.rs` and reword `expected` to "a colour string or index", or drop the `Value::Integer` arm. Separately, `Value::String(s) => s.clone()` allocates for no reason: call `parse_color` on the borrowed `&str` directly and convert only the integer case.
4. [nit] `src/theme/builtin.rs:36-40`: `all()` uses `filter_map`, so an id in `IDS` that `get` doesn't know would be dropped silently at runtime. The length check in `seven_builtins_in_order_with_ids_and_display_names` catches it in tests. Change: map each id with `get(id).expect("every id in IDS has a built-in")`, or have `get` and `all` share one `(id, fn() -> Theme)` table so they can't drift apart.
5. [nit] `src/theme/builtin.rs:71-80`: unlike ratcn, the ported `dim` does not clamp `amount` to 100, so `100 - amount` would underflow (and panic in debug builds) for `amount > 100`. Every call site passes a constant ≤ 50, so this is latent only. Change: add `let amount = amount.min(100);`, or `debug_assert!(amount <= 100)`, at the top of `dim`.
