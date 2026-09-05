use std::path::{Path, PathBuf};

use fr_dsn::BoardReadResult;
use fr_settings::prelude::*;

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
