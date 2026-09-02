//! `check_drc` — spec §13's `{ dsn_path | dsn_text, ses_path?, rules_path? } → KiCad report`.
//!
//! **The same document `freerouting drc` writes**, because it is the same sequence:
//! `Freerouting.initializeDrc`'s steps 5-12 (`Freerouting.java:262-354`), reached through
//! `commands::drc`'s own [`load_rules_file`](crate::commands::drc::load_rules_file),
//! [`load_session_file`](crate::commands::drc::load_session_file) and
//! [`quality_score`](crate::commands::drc::quality_score) rather than through a second copy of
//! them. `api/v1/JobOutputResource.getDrcReport` (`:589-661`) makes the same `fr-drc` call from
//! the jar's REST API, so this tool is that endpoint's counterpart — minus its job store.
//!
//! What is deliberately **not** here is the CLI's step 13: there is no file to write, so
//! `write_report`'s file-vs-stdout fork (quirk #275) has no counterpart and the document is the
//! result.
//!
//! # The load order is DSN → `.rules` → session, and it is load-bearing
//!
//! Quirk #273, argued at length in `commands::drc`'s module docs: a via created by the session
//! import takes the clearance class the `.rules` file installed, so the two orders give different
//! violation counts on the same three files (15 versus 0, measured). The order below is the
//! CLI's, which is Java's.

use super::super::jsonrpc::RpcError;
use super::super::server::{ProgressWriter, State};
use crate::commands::drc::{load_rules_file, load_session_file, quality_score, report_date};
use fr_core::CancelToken;
use fr_drc::DesignRulesChecker;
use fr_drc::report::{DrcCoordinates, DrcJsonFlavor, DrcReportOptions};
use serde_json::Value;
use std::path::PathBuf;

/// Loads the board, applies the optional `.rules` and session, and answers the KiCad DRC report.
///
/// **Reports no progress and does not poll the token**: a DRC pass is a load and one checker run.
/// See `tools`' module docs.
///
/// # Errors
///
/// [`RpcError::invalid_params`] for a bad argument, an unreadable input or a board that will not
/// load; [`RpcError::internal`] if the report will not serialize — which is Java's one
/// non-finite-`qualityScore` throw (`Freerouting.java:354` is outside the score's own `try`), and
/// is the CLI's exit 1 on the same input.
pub fn run(
    state: &State,
    args: Value,
    _progress: &ProgressWriter,
    _cancel: &CancelToken,
) -> Result<Value, RpcError> {
    let ses = super::optional_string(&args, "ses_path")?.map(PathBuf::from);
    let rules = super::optional_string(&args, "rules_path")?.map(PathBuf::from);
    let mut job = super::board_input(&args)?;

    // Step 6 — `BoardLoader.loadBoardIfNeeded`, the **whole** loader. See `commands::drc`'s step
    // 6 for why the parse half would be the same board today and a divergence tomorrow.
    let loaded = fr_core::load_board_if_needed(&mut job)
        .map_err(|error| RpcError::invalid_params(error.to_string()))?;
    let mut board = loaded.board;
    let transform = loaded.transform;

    // Steps 7 and 8, in Java's order — see the module docs.
    load_rules_file(rules.as_deref(), &job, &mut board, &transform);
    load_session_file(ses.as_deref(), &mut board, &transform);

    // Steps 9-10. `coordinate_unit` is `"mm"`, hard-coded, because `Freerouting.java:335-336`
    // hard-codes it (quirk #151) — **this tool exposes no unit option**, which is Task 7's
    // recorded decision and not an omission: the other four arms of
    // `DesignRulesChecker.convertCoordinate` (`:512-525`) are unreachable from `-drc` too.
    let coords = DrcCoordinates {
        board_unit: board.communication.unit,
        transform,
    };
    let options = DrcReportOptions {
        // `:339` — `new File(initialInputFile).getName()`. The job's own filename **is** that
        // base name (`BoardFileDetails.setFilename` keeps the name only), and it is what a
        // `dsn_text` call has instead of a path.
        source: job.get_input().map_or_else(
            || "board.dsn".to_string(),
            |details| details.get_filename().to_string(),
        ),
        coordinate_unit: "mm".to_string(),
        date: report_date(std::time::SystemTime::now()),
        // Ruling AT/5: one of exactly three FILE-FORMAT fields that carry `PARITY_VERSION`.
        freerouting_version: fr_core::PARITY_VERSION.to_string(),
        quality_score: None,
    };
    let mut checker = DesignRulesChecker::new(&mut board);
    let mut report = checker.generate_report(&coords, &options);

    // Step 11 — quirk #272's separate merge, **after** `generateReport`, because that call
    // mutates the board and `:348` reads the post-report statistics. The argv is the server's own
    // (see `State::settings_argv`), which is what makes this tool and the CLI answer the same
    // score on the same machine.
    report.quality_score = job
        .get_input()
        .and_then(|details| quality_score(&mut board, details, &state.settings_argv))
        .map(f64::from);

    // Step 12, with the flavor passed **by name**: ruling W's `KiCad` is the CLI's default and
    // the spelling the document's own `$schema` promises (quirk #154). `DrcJsonFlavor::default()`
    // is the other one, and is the *parity* choice `fr-drc`'s tests pin against the jar — passing
    // it by name here is what stops a change to that `Default` moving this tool silently.
    let json = report.to_json(DrcJsonFlavor::KiCad).map_err(|error| {
        RpcError::internal(format!("Couldn't serialise the DRC report: {error}"))
    })?;
    serde_json::from_str(&json).map_err(|error| {
        RpcError::internal(format!("the DRC report is not readable as JSON: {error}"))
    })
}
