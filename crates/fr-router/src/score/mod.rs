//! `fr_router::score` — the score-relevant subset of `core/scoring/BoardStatistics.java`
//! (controller ruling AG).
//!
//! # Why the router owns the score
//!
//! `getNormalizedScore` is the batch pipeline's control flow, not a report number:
//! `AutorouteBatchLoop` compares it pass to pass (`:425`), `BoardHistory` ranks boards by it, and
//! `BatchOptimizer` gates an accepted optimisation on it. Ruling AG therefore pulls this subset
//! forward from Plan 8 and lands it here; Plan 8's `fr-core` **re-exports** these types and adds
//! the Gson-compatible JSON surface, `BoardScoreBreakdown`, `ScoringWeightComparison` and the
//! `byte[]`/`FileFormat` constructor. Plan 7 adds no crate.
//!
//! # What is here, and what is not
//!
//! Ported: the field block (`:37-79`), the two delegating constructors with live callers
//! (`:84-86`, `:100-102`), the computing constructor (`:110-427`), `isPinEscaped` (`:555-576`),
//! `calculateScore` (`:597-616`), `getMaximumScore` (`:619-621`), `getNormalizedScore`
//! (`:624-635`), the nested `BoardStatisticsFanout` (`:638-647`) and the ten sibling DTOs.
//! (Member bodies, not their javadoc — the same ranges the audit map and the `// Java bug:`
//! markers cite.)
//!
//! The eleventh DTO, `BoardStatisticsClearanceViolations`, is **not** redeclared: Plan 5
//! delivered it and the block of this constructor that fills it, and
//! [`BoardStatistics::clearance_violations`] is `fr_drc`'s type. `fr-drc` moves from a
//! dev-dependency to a real dependency of this crate for it and for the two
//! `DesignRulesChecker`s the constructor builds (ruling 3).
//!
//! The deferral roster for the rest of `core/scoring` is at the foot of this file.

pub mod dtos;
pub mod normalized;
pub mod statistics;

pub use dtos::{
    BoardStatisticsBends, BoardStatisticsBoard, BoardStatisticsComponents,
    BoardStatisticsConnections, BoardStatisticsItems, BoardStatisticsLayers, BoardStatisticsNets,
    BoardStatisticsPads, BoardStatisticsTraces, BoardStatisticsVias, Rectangle2DFloat,
};
pub use statistics::{
    BoardStatistics, BoardStatisticsFanout, java_double_stream_sum, unescape_unicode,
};

/// `core.scoring.BoardStatisticsClearanceViolations`, defined in `fr-drc` (plan-5 ruling 5) and
/// re-exported here so a reader of this module finds all twelve DTOs in one place.
pub use fr_drc::BoardStatisticsClearanceViolations;

// ---------------------------------------------------------------------------------------------
// The `core/scoring` roster (ruling 4). `scripts/audit-port.sh core/scoring` reads these markers,
// so each is a checked obligation rather than a silence, and each names the reader that makes it
// Plan 8's.
// ---------------------------------------------------------------------------------------------

// NOTE for whoever edits the three `renamed:` lines below: `scripts/audit-port.sh` matches
// a `renamed:` marker against `\b<Method>\b` on the SAME line, and `BoardScoreBreakdown` has a
// one-word method whose name is the English preposition — so writing that word inside one of
// these lines silently reports that method as ported. Keep the three lines below free from it.
// (The `not ported:` rows further down are the opposite case: that method *is* rostered, so a
// line of theirs carrying the word says only what the roster already says.)
//
// The three markers below were Plan-8 deferrals until Plan 8 Task 2 consumed them.
// The methods are ported — in `fr-core`, because their one reader is the result manifest and
// `fr-core` is the crate that owns it — so each is now a `renamed:` row naming its Rust home. The
// range in the first was `:436-554` while it was a deferral (pre-flight scan note N3); the
// constructor's body ends at `:552`, which is what the file says.
//
// renamed: BoardStatistics.BoardStatistics(byte[], FileFormat) (core/scoring/BoardStatistics.java:436-552) — `fr_core::stats_from_bytes::BoardStatisticsExt::from_bytes`, an extension trait on this type: the SES/DSN/KiCad-JSON **text-scraping** twin, which never builds a board. `RoutingJob.setInput`/`setOutput` build a `BoardStatistics` from the file when no board object exists (spec §10); nothing in `autoroute/pipeline/**` calls it.
// renamed: BoardStatistics.countOccurrences (core/scoring/BoardStatistics.java:578-586) — `fr_core::count_occurrences`, the `private static` helper that constructor alone uses, made public because the port cannot hide it behind a trait method.
// renamed: BoardStatistics.toString (core/scoring/BoardStatistics.java:589-591) — `fr_core::to_gson_string`, with `fr_core::stats_json::GsonBoardStatistics` as its embeddable form. `GsonProvider.GSON.toJson(this)`, i.e. the JSON surface itself; Plan 8 owns the Gson-compatible serialisation for this whole family (ruling AG).
//
// The five markers below were Plan-8 deferrals, on the theory that the CLI/manifest surface Plan 8
// builds would be their reader. **Controller ruling AS, closed by Plan 8 Task 14: it is not.** The
// pair is dead in the jar, on two independent grounds, and the full evidence with both greps is in
// `crates/fr-core/src/lib.rs` §9 beside the `not ported:` rows that gate the `core/scoring` audit
// against `fr-core`. In short: (a) `grep -rn "ScoringWeightComparison|BoardScoreBreakdown"
// src/main/java src/test/java`, with the two declarations excluded, answers **only**
// `src/test/java/.../ScoringWeightComparisonTest.java` — no caller in `src/main` at all, so Plan
// 8's `route`, `drc`, `info` and the four MCP tools could not have reached them; and (b) quirk
// #236, the unit divergence that makes a breakdown fail to add up to the score it explains. Plan
// ruling 12 declines to port the JUnit suite for the same reason: it is the pair's sole
// reachability, and porting it would manufacture the caller the roster says does not exist.
//
// not ported: BoardScoreBreakdown.of (core/scoring/BoardScoreBreakdown.java:125-164) — the factory that re-derives `calculateScore`'s seven terms one by one for display. It duplicates the arithmetic this module owns rather than adding to it, and quirk #236 is where the duplicate disagrees.
// not ported: ScoringWeightComparison.compare (core/scoring/ScoringWeightComparison.java:41-56) — builds two `BoardScoreBreakdown`s and subtracts them term by term, for a weight-sweep report nothing in `src/main/java` asks for. Static, stateless, and constructed nowhere in `autoroute/pipeline/**`.
// not ported: BoardScoreBreakdown.toSummaryString (core/scoring/BoardScoreBreakdown.java) — the per-term breakdown rendering. A *presentation* of `calculateScore`'s terms, with no reader inside the routing loop and none on any CLI or MCP path Plan 8 built.
// not ported: ScoringWeightComparison.isCandidateBetter (core/scoring/ScoringWeightComparison.java) — the weight-sweep comparator; `autoroute/pipeline/**` compares `getNormalizedScore` directly and never constructs one.
// not ported: ScoringWeightComparison.toReportString (core/scoring/ScoringWeightComparison.java) — its report rendering, same absent reader.
