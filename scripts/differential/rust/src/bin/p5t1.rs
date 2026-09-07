//! Rust twin of `scripts/differential/java/P5T1.java` (Plan 5 Task 10), report level.
//!
//! Reads one DSN — optionally with a `.rules` file and a `.ses` session applied, in the CLI's
//! order — runs the port's `DesignRulesChecker::generate_report` + `KiCadDrcReport::to_json`
//! and prints the document under the same normalisation rules the Java side applies. See
//! `P5T1.java`'s class comment for what those rules are and why; the short version is that the
//! three values `Freerouting.initializeDrc` injects (`date`, `freeroutingVersion`, `qualityScore`
//! — plan-5 ruling 5) are pinned to fixed literals on both sides, and nothing else is touched.
//!
//! Three of the five rules are simply what this side passes in: [`FIXED_DATE`] and
//! [`FIXED_VERSION`] are `DrcReportOptions` fields and [`FIXED_QUALITY_SCORE`] is the injected
//! `Option<f64>`. The fourth — sort each `unconnectedItems` entry's `items` by numeric uuid — has
//! to be applied here too, and is: plan-5 ruling 3 fixes the *seed* order and each connected
//! group's order, but the entry's `items` is `connectedSets.get(0)` followed by
//! `connectedSets.get(1)` (`DesignRulesChecker.java:143-146`), so the port emits two ascending
//! runs concatenated, not one ascending list. Sorting is exactly what
//! [`parity::normalize_drc_json`] does to both sides in `crates/copper-drc/tests/reference_parity.rs`,
//! and this driver is the same comparison one layer lower. The fifth rule is "touch nothing else"
//! — in particular `violations` is left alone on both sides, array *and* per-entry `items`.
//!
//! Applying rule 4 is why this calls `generate_report` and `to_json` separately rather than
//! `report_to_json`: that method is literally those two calls (`report/json.rs`), and the sort has
//! to happen between them.
//!
//! Usage: `p5t1 <dsn> [rules|-] [ses|-]`.

use std::io::{BufWriter, Write};

use copper_drc::report::{DrcCoordinates, DrcJsonFlavor, DrcReportOptions};
use copper_drc::DesignRulesChecker;

#[path = "../drc_common.rs"]
mod drc_common;

/// `P5T1.FIXED_DATE`.
const FIXED_DATE: &str = "1970-01-01T00:00:00Z";

/// `P5T1.FIXED_VERSION` **without** the `"Freerouting "` prefix: `generate_report` adds it, exactly
/// as `DesignRulesChecker.java:212-213` does.
const FIXED_VERSION: &str = "p5t1";

/// `P5T1.FIXED_QUALITY_SCORE` (a `double` there; `f32` here because `DrcReportOptions` takes
/// `getNormalizedScore`'s `float` and performs `Freerouting.java:349`'s widening itself —
/// `-1.0` is exact in both widths).
const FIXED_QUALITY_SCORE: f32 = -1.0;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: p5t1 <dsn> [rules|-] [ses|-]");
        std::process::exit(2);
    }
    let inputs = drc_common::Inputs::from_args(&args);

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    drc_common::print_rust_provenance();
    drc_common::print_header(&mut out, &inputs, "");

    let (mut board, transform) = drc_common::load_board(&inputs);
    let options = DrcReportOptions {
        // `Freerouting.java:339`: `new File(globalSettings.initialInputFile).getName()`.
        source: drc_common::base_name(&inputs.dsn),
        // `Freerouting.java:335-336` hard-codes the unit (quirk #151).
        coordinate_unit: "mm".to_string(),
        date: FIXED_DATE.to_string(),
        router_version: FIXED_VERSION.to_string(),
        quality_score: Some(FIXED_QUALITY_SCORE),
    };
    let coords = DrcCoordinates {
        board_unit: board.communication.unit,
        transform,
    };
    let mut report = DesignRulesChecker::new(&mut board).generate_report(&coords, &options);
    // Rule 4, `unconnectedItems` only — `P5T1.main`'s `byUuid` sort, on the same lists. The uuids
    // are `String.valueOf(item.getId())` (`DesignRulesChecker.java:320-321`), so the comparison is
    // numeric: `"1000"` must sort after `"99"`.
    for violation in &mut report.unconnected_items {
        violation.items.sort_by_key(|item| {
            item.uuid
                .parse::<i64>()
                .unwrap_or_else(|_| panic!("uuid {} is not a number", item.uuid))
        });
    }
    let json = report
        .to_json(DrcJsonFlavor::Legacy)
        .expect("the report serialises");
    writeln!(out, "{json}").expect("write");
}
