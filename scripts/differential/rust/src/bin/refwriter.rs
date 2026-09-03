//! The port's twin of `scripts/gen-reference/RefWriter.java` — Plan 9 Task 0.
//!
//! `scripts/gen-reference.sh --from-port` drives this, with **the same three arguments** the jar
//! lane gives `RefWriter`: `<in.dsn> <out.dsn> <out.ses>`. Nothing else in the port can produce
//! those two files: the CLI has no DSN-write path (Java's `-de/-do` job path cannot write DSN
//! headlessly, and the port reproduces that), and the round-trip references were until now cut
//! only by the jar. So the family-G lane switch (survey §7.1 — "regenerated from the port") needs
//! this binary to exist, in the same way the family-R lane switch needs `p6t1`.
//!
//! It is a **twin, not a re-implementation**: the two steps below are `RefWriter.main`'s two
//! steps, and `crates/fr-dsn/tests/parity_dsn.rs` already asserts that this exact pair of calls
//! reproduces the jar's bytes on all seven stems. Keeping the driver here rather than in a crate
//! keeps it out of the shipped surface — it is harness code, and `scripts/differential/rust` is
//! where the port's harness code lives.
//!
//! ```text
//! refwriter <in.dsn> <out.dsn> <out.ses>
//! ```
//!
//! Exit codes follow `RefWriter.java`: `2` for a usage error, `1` for a parse or I/O error on the
//! input, `0` on success.

use std::io::Write;
use std::path::Path;

use fr_board::Board;
use fr_dsn::parser::scope_parameter::DsnReadOptions;
use fr_dsn::{dsn_writer, ses_writer, BoardReadResult, CoordinateTransform};

/// `Path.of(args[0]).getFileName().toString().replaceAll("\\.dsn$", "")` (RefWriter.java:26).
///
/// The Java is a **regex replace on the whole name**, not a suffix strip, and it is anchored with
/// `$`, so `a.dsn` becomes `a` and `a.dsn.b` is left alone. `strip_suffix` is that, exactly.
/// `crates/fr-dsn/tests/parity_dsn.rs` carries the same helper with the same citation.
fn design_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .strip_suffix(".dsn")
        .unwrap_or_default()
        .to_string()
}

/// `RefWriter.main`'s read half: `DsnReader.readBoard(in, null, null, designName)`, and the
/// `switch` over its four results. `OutlineMissing` yields its board exactly as the Java arm
/// does — a board with no outline still round-trips, and one of the seven stems is that case.
fn read_board(path: &Path) -> Result<(Board, CoordinateTransform), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("io error: {e}"))?;
    let stem = design_name(path);
    match fr_dsn::read_board(file, None, Some(&stem), &DsnReadOptions::default()) {
        BoardReadResult::Success {
            board,
            coordinate_transform,
            ..
        }
        | BoardReadResult::OutlineMissing {
            board,
            coordinate_transform,
            ..
        } => {
            let board = *board.ok_or_else(|| "the reader produced no board".to_string())?;
            let ct = coordinate_transform
                .ok_or_else(|| "the reader produced no coordinate transform".to_string())?;
            Ok((board, ct))
        }
        BoardReadResult::ParseError { location, detail } => {
            Err(format!("parse error at {location}: {detail}"))
        }
        BoardReadResult::IoError(e) => Err(format!("io error: {e}")),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        eprintln!("usage: refwriter <in.dsn> <out.dsn> <out.ses>");
        std::process::exit(2);
    }
    let input = Path::new(&args[0]);
    let name = design_name(input);

    let (board, ct) = match read_board(input) {
        Ok(pair) => pair,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    };

    // `DsnWriter.write(board, out, designName, false)` — the `false` is Java's own
    // `compatMode` argument at RefWriter.java:48.
    let mut dsn: Vec<u8> = Vec::new();
    dsn_writer::write(&board, &ct, &mut dsn, &name, false).expect("write into a Vec cannot fail");
    // `SesWriter.write(board, out, designName)`.
    let mut ses: Vec<u8> = Vec::new();
    ses_writer::write(&board, &ct, &mut ses, &name).expect("write into a Vec cannot fail");

    // Both buffers are complete before either file is opened, so a failure in the second write
    // cannot leave a stale `.dsn` beside a truncated `.ses`. The Java driver writes straight
    // through two `FileOutputStream`s and does not have this property; the generator's callers
    // treat the pair as one artefact, so the port's driver gives it to them.
    for (path, bytes) in [(&args[1], &dsn), (&args[2], &ses)] {
        let mut file = match std::fs::File::create(path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("cannot create {path}: {e}");
                std::process::exit(1);
            }
        };
        if let Err(e) = file.write_all(bytes) {
            eprintln!("cannot write {path}: {e}");
            std::process::exit(1);
        }
    }
}
