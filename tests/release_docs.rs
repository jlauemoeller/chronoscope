//! The README and the release workflow (SPEC §9.3, §9.4, §6.8, §8.5, §11).
//!
//! The README is documentation, but parts of it are checked against the code
//! so that it cannot drift: the config example must load without warnings,
//! the keybinding tables must match the default keymap, and the user theme
//! example must parse.

use std::collections::BTreeMap;
use std::path::Path;

use chronoscope::config::Config;
use chronoscope::keymap::{Context, KeySeq, Keymap};
use chronoscope::theme::{Theme, builtin};

fn read(path: &str) -> String {
    let full = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("reading {}: {e}", full.display()))
}

fn readme() -> String {
    read("README.md")
}

/// The contents of the fenced code blocks tagged `lang` (empty: untagged).
fn fenced_blocks(text: &str, lang: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    // The open block: whether it is wanted, and its lines.
    let mut current: Option<(bool, Vec<&str>)> = None;
    for line in text.lines() {
        let line = line.trim_end();
        match current.as_mut() {
            None => {
                if let Some(tag) = line.strip_prefix("```") {
                    current = Some((tag == lang, Vec::new()));
                }
            }
            Some((wanted, lines)) => {
                if line == "```" {
                    if *wanted {
                        blocks.push(lines.join("\n"));
                    }
                    current = None;
                } else {
                    lines.push(line);
                }
            }
        }
    }
    blocks
}

fn block_containing(text: &str, lang: &str, needle: &str) -> String {
    fenced_blocks(text, lang)
        .into_iter()
        .find(|b| b.contains(needle))
        .unwrap_or_else(|| panic!("no ```{lang} block containing {needle:?}"))
}

fn home() -> &'static Path {
    Path::new("/home/test")
}

#[test]
fn readme_has_every_planned_section() {
    let text = readme();
    let headings: Vec<&str> = text.lines().filter(|l| l.starts_with('#')).collect();
    let has = |needle: &str| headings.iter().any(|h| h.contains(needle));
    for section in [
        "Install",
        "Usage",
        "Keybindings",
        "Configuration",
        "Themes",
        "User themes",
        "Mouse",
        "Terminal multiplexers",
        "Data sources",
        "Open items",
        "Acknowledgements",
    ] {
        assert!(
            has(section),
            "missing a heading with {section:?}: {headings:?}"
        );
    }
    assert!(text.contains("Claude Code") && text.contains("OpenCode"));
    assert!(text.contains("cargo install --path ."));
    assert!(text.contains("releases"));
}

#[test]
fn readme_config_example_loads_without_warnings() {
    let text = readme();
    let example = block_containing(&text, "toml", "[harness.claude]");
    let (config, warnings) = Config::from_toml_str(&example, home());
    assert!(warnings.is_empty(), "config warnings: {warnings:?}");

    assert_eq!(config.theme, "tokyo-night");
    assert_eq!(config.transparent_background, Some(false));
    assert!(config.mouse && config.show_system && config.dim_unselected);
    assert!(!config.utc);
    assert_eq!(config.timestamp_format, "%Y-%m-%d %H:%M:%S");
    assert_eq!(config.truncate_output_lines, 200);
    assert_eq!(
        config.harness.claude.data_dir.as_deref(),
        Some(Path::new("/home/test/.claude/projects"))
    );
    assert_eq!(
        config.harness.opencode.resume_command,
        "opencode --session {id}"
    );
    assert_eq!(config.projects.aliases.len(), 2);

    // The key bindings in the example are valid as well.
    let (_, key_warnings) = Keymap::from_config(&config.keys);
    assert!(key_warnings.is_empty(), "keymap warnings: {key_warnings:?}");
}

#[test]
fn readme_config_example_is_the_one_in_the_spec() {
    let normalise = |s: &str| -> Vec<String> {
        s.lines()
            .map(|l| l.trim_end().to_owned())
            .filter(|l| !l.is_empty())
            .collect()
    };
    let readme = readme();
    let spec = read("doc/SPEC.md");
    let in_readme = block_containing(&readme, "toml", "[harness.claude]");
    let in_spec = block_containing(&spec, "toml", "[harness.claude]");
    assert_eq!(normalise(&in_readme), normalise(&in_spec));
}

#[test]
fn readme_config_example_sets_every_config_key() {
    let readme = readme();
    let example = block_containing(&readme, "toml", "[harness.claude]");
    let table: toml::Table = example.parse().unwrap();
    for key in [
        "theme",
        "transparent_background",
        "mouse",
        "show_system",
        "dim_unselected",
        "timestamp_format",
        "utc",
        "truncate_output_lines",
        "harness",
        "projects",
        "keys",
    ] {
        assert!(table.contains_key(key), "example lacks {key}");
    }
    for harness in ["claude", "opencode"] {
        for key in ["enabled", "resume_command"] {
            assert!(
                table["harness"][harness].get(key).is_some(),
                "{harness}.{key}"
            );
        }
    }
    assert!(table["harness"]["claude"].get("data_dir").is_some());
    assert!(table["harness"]["opencode"].get("db_path").is_some());
}

/// The rows of the README's keybinding tables: (context, action name) ->
/// the keys in the README.
fn readme_bindings(text: &str) -> BTreeMap<(String, String), Vec<KeySeq>> {
    let mut context: Option<String> = None;
    let mut out = BTreeMap::new();
    for line in text.lines() {
        if line.starts_with('#') {
            context = line
                .split_once("[keys.")
                .and_then(|(_, rest)| rest.split_once(']'))
                .map(|(name, _)| name.to_owned());
            continue;
        }
        let Some(context) = &context else { continue };
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        // `| action | keys | description |` gives ["", action, keys, description, ""].
        if cells.len() != 5 || !cells[1].starts_with('`') {
            continue;
        }
        let ticked = |cell: &str| -> Vec<String> {
            cell.split('`')
                .skip(1)
                .step_by(2)
                .map(str::to_owned)
                .collect()
        };
        let action = ticked(cells[1]).remove(0);
        let keys = ticked(cells[2])
            .iter()
            .map(|k| KeySeq::parse(k).unwrap_or_else(|e| panic!("README key {k:?}: {e}")))
            .collect();
        let previous = out.insert((context.clone(), action.clone()), keys);
        assert!(previous.is_none(), "{context}.{action} listed twice");
    }
    out
}

#[test]
fn readme_keybindings_match_the_default_keymap() {
    let documented = readme_bindings(&readme());
    let keymap = Keymap::default();

    let mut expected = 0;
    for context in Context::ALL {
        for binding in keymap.bindings(context) {
            expected += 1;
            let key = (context.name().to_owned(), binding.name.to_owned());
            let got = documented
                .get(&key)
                .unwrap_or_else(|| panic!("README lacks {}.{}", key.0, key.1));
            assert_eq!(got, &binding.keys, "keys of {}.{}", key.0, key.1);
        }
    }
    assert_eq!(
        documented.len(),
        expected,
        "the README lists bindings the keymap does not have"
    );
}

#[test]
fn readme_has_no_ctrl_c_copy_and_no_modifier_enter() {
    let text = readme();
    let documented = readme_bindings(&text);
    let ctrl_c = KeySeq::parse("ctrl-c").unwrap();
    for ((context, action), keys) in &documented {
        assert!(
            !keys.contains(&ctrl_c),
            "{context}.{action} is documented with ctrl-c"
        );
    }
    let copy = &documented[&("browser".to_owned(), "copy".to_owned())];
    assert_eq!(copy, &[KeySeq::parse("y").unwrap()]);

    let lower = text.to_lowercase();
    for banned in [
        "ctrl-enter",
        "shift-enter",
        "alt-enter",
        "ctrl+enter",
        "shift+enter",
    ] {
        assert!(!lower.contains(banned), "README mentions {banned}");
    }
    assert!(text.contains("`Ctrl-c` always quits"));
    assert!(text.contains("kitty keyboard protocol"));
}

#[test]
fn readme_help_key_is_documented() {
    let text = readme();
    assert!(text.contains("Press `?`"));
}

#[test]
fn readme_documents_the_tmux_settings_verbatim() {
    let text = readme();
    let tmux = block_containing(&text, "tmux", "set-clipboard");
    let lines: Vec<&str> = tmux.lines().collect();
    assert_eq!(
        lines,
        [
            "set -g set-clipboard on",
            "set -g allow-passthrough on",
            "set -sg escape-time 10",
        ]
    );
    // One-line explanations.
    assert!(text.contains("`set -g set-clipboard on`"));
    assert!(text.contains("`set -g allow-passthrough on`"));
    assert!(text.contains("`set -sg escape-time 10`"));
    assert!(text.contains("500 ms"));
}

#[test]
fn readme_says_herdr_osc52_is_unverified() {
    let text = readme();
    let herdr: Vec<&str> = text
        .lines()
        .filter(|l| l.contains("herdr") && l.to_lowercase().contains("unverified"))
        .collect();
    assert!(
        !herdr.is_empty(),
        "no line says herdr's behaviour is unverified"
    );
    assert!(text.contains("OSC52"));
}

#[test]
fn readme_command_line_matches_the_spec() {
    let text = readme();
    let usage = block_containing(&text, "", "chronoscope --latest");
    for line in [
        "chronoscope <session-id | path>",
        "--harness <claude|opencode>",
        "--project <path|name>",
        "--theme <name>",
        "--config <path>",
    ] {
        assert!(usage.contains(line), "usage lacks {line:?}");
    }
}

#[test]
fn readme_lists_the_builtin_themes_and_the_colour_rules() {
    let text = readme();
    for id in builtin::IDS {
        assert!(
            text.contains(&format!("| `{id}` |")),
            "theme {id} not listed"
        );
    }
    for needle in ["NO_COLOR", "COLORTERM", "truecolor", "24bit", "xterm-256"] {
        assert!(text.contains(needle), "README lacks {needle}");
    }
    assert!(text.contains("themes/<name>.toml"));
}

#[test]
fn readme_user_theme_example_parses() {
    let text = readme();
    let example = block_containing(&text, "toml", "foreground = ");
    let theme = Theme::from_toml(&example).unwrap_or_else(|e| panic!("user theme example: {e}"));
    assert_eq!(theme.name(), "My Theme");
}

#[test]
fn readme_covers_mouse_and_native_selection() {
    let text = readme();
    assert!(text.contains("mouse = false"));
    assert!(text.contains("Shift"));
}

#[test]
fn readme_lists_the_open_items() {
    let text = readme();
    let section = text
        .split("## Open items to verify")
        .nth(1)
        .and_then(|rest| rest.split("\n## ").next())
        .expect("open items section");
    for needle in [
        "OpenCode data directory on macOS and Windows",
        "opencode --session <id>",
        "herdr OSC52",
        "Claude Code",
        "compaction",
        "model.variant",
    ] {
        assert!(section.contains(needle), "open items lack {needle:?}");
    }
}

#[test]
fn readme_credits_ratcn_and_points_to_the_notices() {
    let text = readme();
    let ack = text.split("## Acknowledgements").nth(1).expect("section");
    assert!(ack.contains("ratcn"));
    assert!(ack.contains("MIT"));
    assert!(ack.contains("THIRD-PARTY-NOTICES.md"));
    assert!(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("THIRD-PARTY-NOTICES.md")
            .is_file()
    );
}

#[test]
fn release_workflow_builds_and_uploads_all_five_targets_on_version_tags() {
    let text = read(".github/workflows/release.yml");
    let workflow: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text).expect("valid YAML");

    // `on` is a plain string key here (YAML 1.2), but accept a boolean key too.
    let on = workflow
        .get("on")
        .or_else(|| workflow.get(serde_yaml_ng::Value::Bool(true)))
        .expect("an `on` trigger");
    let tags: Vec<&str> = on["push"]["tags"]
        .as_sequence()
        .expect("push.tags")
        .iter()
        .filter_map(|t| t.as_str())
        .collect();
    assert_eq!(tags, ["v*"]);
    assert!(on["push"].get("branches").is_none(), "no branch trigger");
    assert_eq!(on.as_mapping().unwrap().len(), 1, "only the push trigger");

    let build = &workflow["jobs"]["build"];
    let mut targets: Vec<&str> = build["strategy"]["matrix"]["include"]
        .as_sequence()
        .expect("matrix.include")
        .iter()
        .map(|entry| entry["target"].as_str().expect("target"))
        .collect();
    targets.sort_unstable();
    assert_eq!(
        targets,
        [
            "aarch64-apple-darwin",
            "aarch64-unknown-linux-gnu",
            "x86_64-apple-darwin",
            "x86_64-pc-windows-msvc",
            "x86_64-unknown-linux-gnu",
        ]
    );

    let steps = serde_yaml_ng::to_string(&build["steps"]).unwrap();
    assert!(steps.contains("cargo build --release --locked --target"));
    assert!(steps.contains("README.md THIRD-PARTY-NOTICES.md"));
    assert!(steps.contains(".tar.gz") && steps.contains(".zip"));
    assert!(steps.contains("actions/upload-artifact"));

    let release = &workflow["jobs"]["release"];
    assert_eq!(release["needs"].as_str(), Some("build"));
    assert_eq!(release["permissions"]["contents"].as_str(), Some("write"));
    let steps = serde_yaml_ng::to_string(&release["steps"]).unwrap();
    assert!(steps.contains("gh release upload"));
}

#[test]
fn readme_rust_version_matches_the_manifest() {
    let manifest: toml::Table = read("Cargo.toml").parse().unwrap();
    let msrv = manifest["package"]["rust-version"].as_str().unwrap();
    let text = readme();
    assert!(
        text.contains(&format!("({msrv} or newer)")),
        "README must say Rust {msrv} or newer"
    );
}
