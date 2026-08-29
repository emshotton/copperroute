//! The half of `p5t1`/`p5t2` that is not the comparison surface: argument handling, the shared
//! header line and the three-step board load `Freerouting.initializeDrc` performs.
//!
//! Included by both drivers with `#[path = "../drc_common.rs"] mod drc_common;`, the way
//! `p3t15.rs` includes `token_dump.rs`. The Java side shares the same code the same way: `P5T2`
//! calls `P5T1.loadBoard`, and `run.sh` compiles `P5T1.java` alongside it.

#![allow(dead_code)] // each driver uses a subset.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use fr_board::board::Board;
use fr_dsn::parser::scope_parameter::DsnReadOptions;
use fr_dsn::{BoardReadResult, CoordinateTransform};

/// The three files a driver run is about, resolved and canonicalised.
pub struct Inputs {
    pub dsn: PathBuf,
    pub rules: Option<PathBuf>,
    pub ses: Option<PathBuf>,
}

impl Inputs {
    /// `<dsn> [rules|-] [ses|-]` at `args[0..3]`, with a missing argument, an empty one and `-` all
    /// meaning "absent" — `P5T1.optionalPath`'s rule.
    pub fn from_args(args: &[String]) -> Inputs {
        Inputs {
            dsn: canonical(&args[0]),
            rules: optional_path(args, 1),
            ses: optional_path(args, 2),
        }
    }
}

/// Provenance for *this* side, on **stderr**: `run.sh` captures and diffs stdout only, so a line
/// here cannot become a spurious diff, and the header's jar identity says nothing about which Rust
/// binary produced the other half of the comparison (`p4t1`'s convention).
pub fn print_rust_provenance() {
    if let Ok(exe) = std::env::current_exe() {
        let stamp = std::fs::metadata(&exe)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_millis());
        eprintln!("rust-binary {} mtime={stamp}", exe.display());
    }
}

/// The header both sides print first: the jar the port is a port of (plan-5 ruling 1), its size and
/// mtime, and the three input files. Java derives the jar path from `DesignRulesChecker.class`'s
/// code source; this side takes it from the environment `run.sh` sets, so a mismatch is a real
/// finding rather than a shared assumption.
///
/// `tail` is the driver-specific remainder of the line (`p5t2` appends its mode).
pub fn print_header<W: Write>(out: &mut W, inputs: &Inputs, tail: &str) {
    let jar = std::fs::canonicalize(require_env("FREEROUTING_JAR")).expect("jar exists");
    let meta = std::fs::metadata(&jar).expect("jar metadata");
    let mtime = meta
        .modified()
        .expect("mtime")
        .duration_since(UNIX_EPOCH)
        .expect("after epoch")
        .as_millis();
    writeln!(
        out,
        "HEADER jar={} bytes={} mtime={mtime} fixture={} rules={} ses={}{tail}",
        jar.display(),
        meta.len(),
        base_name(&inputs.dsn),
        name(inputs.rules.as_deref()),
        name(inputs.ses.as_deref()),
    )
    .expect("write");
}

/// The board `Freerouting.initializeDrc` hands to `DesignRulesChecker`: the DSN, then the `.rules`
/// file (`Freerouting.java:277-292`), then the session file (`:297-329`) — the same three calls in
/// the same order as `P5T1.loadBoard` and as `crates/fr-drc/tests/reference_parity.rs`. The rules
/// come first because they can change the clearances the session's wires are then checked against.
pub fn load_board(inputs: &Inputs) -> (Board, CoordinateTransform) {
    let dsn = &inputs.dsn;
    let file = std::fs::File::open(dsn).unwrap_or_else(|e| panic!("cannot open {dsn:?}: {e}"));
    // The design name is a log-message hint only (`DsnReader.java:56-57`); both sides pass the
    // input file's base name.
    let design_name = base_name(dsn);
    let result = fr_dsn::read_board(file, None, Some(&design_name), &DsnReadOptions::default());
    let (board, transform) = match result {
        BoardReadResult::Success {
            board,
            coordinate_transform,
            ..
        }
        | BoardReadResult::OutlineMissing {
            board,
            coordinate_transform,
            ..
        } => (
            *board.unwrap_or_else(|| panic!("{design_name} produced no board")),
            coordinate_transform.unwrap_or_else(|| panic!("{design_name} produced no transform")),
        ),
        other => panic!("{design_name} did not read: {other:?}"),
    };
    let mut board = board;

    if let Some(rules) = &inputs.rules {
        let file =
            std::fs::File::open(rules).unwrap_or_else(|e| panic!("cannot open {rules:?}: {e}"));
        // `designName` is `drcJob.name` (`Freerouting.java:283`), which `RoutingJob.setInput`
        // fills from `input.getFilenameWithoutExtension()` (`RoutingJob.java:457`) — the base name
        // **without** `.dsn`, which is what `crates/fr-drc/tests/reference_parity.rs` passes too.
        // The port ignores the parameter; Java compares it against the `(rules PCB <name>` header
        // and takes the mismatch branch (`RulesReader.java:100-110`) when they differ, so the two
        // sides must agree on it for the log to be the reference run's.
        //
        // Java passes `drcJob.routerSettings` as the fourth argument (`Freerouting.java:284-285`);
        // that only receives the file's `(autoroute_settings …)`, which never reaches the board, so
        // the DRC path can pass `None`.
        let rules_design_name = design_name.strip_suffix(".dsn").unwrap_or(&design_name);
        let read =
            fr_dsn::rules_reader::read(file, rules_design_name, &mut board, &transform, None)
                .unwrap_or_else(|e| panic!("{rules:?} did not read: {e:?}"));
        assert!(read, "{rules:?} was rejected by the rules reader");
    }
    if let Some(ses) = &inputs.ses {
        let file = std::fs::File::open(ses).unwrap_or_else(|e| panic!("cannot open {ses:?}: {e}"));
        let summary = fr_dsn::ses_reader::read(file, &mut board, &transform)
            .unwrap_or_else(|e| panic!("{ses:?} did not read: {e:?}"));
        assert_eq!(
            summary.errors_encountered, 0,
            "{ses:?} imported with errors"
        );
    }

    (board, transform)
}

pub fn require_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("environment variable {name} is not set"))
}

pub fn canonical(raw: &str) -> PathBuf {
    std::fs::canonicalize(raw).unwrap_or_else(|e| panic!("cannot resolve {raw}: {e}"))
}

fn optional_path(args: &[String], i: usize) -> Option<PathBuf> {
    let raw = args.get(i)?;
    if raw.is_empty() || raw == "-" {
        return None;
    }
    Some(canonical(raw))
}

pub fn base_name(path: &Path) -> String {
    path.file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned()
}

fn name(path: Option<&Path>) -> String {
    path.map_or_else(|| "-".to_string(), base_name)
}
