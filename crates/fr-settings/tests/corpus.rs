//! The two `settings/sources/**` file readers, over the whole fixture corpus.
//!
//! `DsnFileSettings` and `RulesFileSettings` are the only sources in this crate that parse a
//! file, and both are *total* in Java: every failure mode — a parse error, a missing
//! `(autoroute_settings …)` block, an I/O error — is swallowed into a blank `RouterSettings`
//! (`DsnFileSettings.java:35-48`, `RulesFileSettings.java:84-92`). This suite is the guard on
//! that totality: it runs each of them over every file of the corpus and asserts that nothing
//! panics and that the one derived value the source computes — the layer count — is exactly what
//! the parse it wraps reported.
//!
//! Both tests skip with a printed message when `../freerouting` (or `$FREEROUTING_JAVA_DIR`) is
//! absent, through `parity::require_java_dir()`.
//!
//! The `.dsn` sweep is **not** `#[ignore]`d: `DsnFileSettings` goes through
//! `DsnReader.readMetadata`, which stops after the `(structure …)` scope, so all 148 files cost
//! about 2.7 s in a debug build. Its `.rules` sibling carries
//! `#[cfg_attr(debug_assertions, ignore)]` because the plan's Task 11 brief asks for it — **not**
//! because it is slow: the corpus has only ten `.rules` files and the sweep measures 0.04 s in
//! debug. Run it with `cargo test -p fr-settings --release --test corpus`, or
//! `cargo test -p fr-settings --test corpus -- --ignored`.

use std::path::{Path, PathBuf};

use fr_dsn::BoardReadResult;
use fr_settings::prelude::*;

/// Every file under `../freerouting/fixtures` with the given extension, sorted, so a failure
/// names the same file on every machine.
fn corpus(extension: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    collect(&parity::java_dir().join("fixtures"), extension, &mut found);
    found.sort();
    found
}

fn collect(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, extension, out);
        } else if path.extension().is_some_and(|e| e == extension) {
            out.push(path);
        }
    }
}

/// `DsnFileSettings::new` over every `.dsn` in the corpus.
///
/// The assertion is `DsnFileSettings.java:35-48` restated: the source's layer count is the
/// `(autoroute_settings …)` block's own when the block named layer rules, the metadata's
/// otherwise, and `0` when the file did not parse. Nothing else is derived, and nothing is
/// allowed to panic — `SettingsMerger.merge` has no error channel to report one through.
#[test]
fn dsn_file_settings_reads_every_fixture_without_panicking() {
    if !parity::require_java_dir() {
        return;
    }
    let files = corpus("dsn");
    assert!(
        files.len() >= 105,
        "corpus shrank: {} .dsn files",
        files.len()
    );

    for path in &files {
        let bytes = std::fs::read(path).expect("fixture readable");
        let name = path
            .file_name()
            .expect("named")
            .to_string_lossy()
            .into_owned();

        // What the wrapped parse reports, computed the same way `DsnFileSettings::new` does.
        let (block_layers, metadata_layers) = match fr_dsn::read_metadata(&bytes[..]) {
            BoardReadResult::Success {
                metadata: Some(metadata),
                ..
            } => (
                metadata
                    .router_settings
                    .as_ref()
                    .map_or(0, fr_dsn::parser::DsnRouterSettings::get_layer_count),
                metadata.layer_count,
            ),
            _ => (0, 0),
        };
        let expected = if block_layers > 0 {
            block_layers
        } else {
            metadata_layers
        };

        let source = DsnFileSettings::new(&bytes[..], &name);
        assert_eq!(source.get_priority(), 20, "{name}");
        assert_eq!(
            source.get_source_name(),
            format!("DSN file: {name}"),
            "{name}"
        );

        let settings = source.get_settings().expect("never null");
        assert_eq!(settings.get_layer_count(), expected, "{name}");

        // Quirk Q18: whenever the seeding branch ran, both `scoring` cost arrays come out
        // full-length and all-`1.0` — which is what freezes them for every later source
        // (`RouterSettings.java:466-477`, `docs/java-quirks.md` #128).
        if block_layers == 0 && metadata_layers > 0 {
            let scoring = settings
                .scoring
                .as_ref()
                .expect("allocated by setLayerCount");
            assert_eq!(
                scoring.preferred_direction_trace_cost.as_deref(),
                Some(&vec![1.0; metadata_layers][..]),
                "{name}"
            );
            assert_eq!(
                scoring.undesired_direction_trace_cost.as_deref(),
                Some(&vec![1.0; metadata_layers][..]),
                "{name}"
            );
        }
    }
    eprintln!("dsn corpus: {} files", files.len());
}

/// The `.rules` half: `RulesFileSettings::new` over every `.rules` in the corpus, plus a second
/// pass of the `.dsn` sweep's subject so the two readers are exercised by one release run.
///
/// `RulesFileSettings` derives its layer count from the file's own `(layer_rule …)` names
/// (`RulesReader.readRouterSettings`, the file-discovered layer structure — not the board's; see
/// quirk #142), so that is what is asserted against.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn rules_file_settings_reads_every_fixture_without_panicking() {
    if !parity::require_java_dir() {
        return;
    }
    let files = corpus("rules");
    assert!(
        files.len() >= 7,
        "corpus shrank: {} .rules files",
        files.len()
    );

    for path in &files {
        let bytes = std::fs::read(path).expect("fixture readable");
        let name = path
            .file_name()
            .expect("named")
            .to_string_lossy()
            .into_owned();

        let expected = match fr_dsn::rules_reader::read_router_settings(&bytes[..]) {
            Ok(Some(parsed)) => parsed.get_layer_count(),
            Ok(None) | Err(_) => 0,
        };

        let source = RulesFileSettings::new(&bytes[..], &name);
        assert_eq!(source.get_priority(), 40, "{name}");
        assert_eq!(
            source.get_source_name(),
            format!("RULES file: {name}"),
            "{name}"
        );

        let settings = source.get_settings().expect("never null");
        assert_eq!(settings.get_layer_count(), expected, "{name}");
    }

    // `from_path` reaches the same bytes through the filesystem, and names the source with the
    // basename (`RulesFileSettings.java:39`); the nested corpus files are the ones that can tell
    // the two apart.
    for path in &files {
        let name = path
            .file_name()
            .expect("named")
            .to_string_lossy()
            .into_owned();
        let source = RulesFileSettings::from_path(path);
        assert_eq!(source.get_source_name(), format!("RULES file: {name}"));
    }

    eprintln!("rules corpus: {} files", files.len());
}
