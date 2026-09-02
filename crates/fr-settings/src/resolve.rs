//! Plan ruling 1: Java's real headless settings precedence, linearised into one pass.
//!
//! # What Java runs
//!
//! Spec §11 says `defaults → DSN → SES → .rules → env → CLI`. What the headless path actually
//! runs is two whole merges with a board-tuning pass wedged between them, and a `.rules` re-apply
//! after the second:
//!
//! | # | Java | what it does |
//! |---|---|---|
//! | 1 | `Freerouting.java:1408-1413` | the prototype merger: `DefaultSettings(0)`, `JsonFileSettings(10)`, `CliSettings(60)`, `EnvironmentVariablesSource(55)` |
//! | 2 | `Freerouting.java:125-136` | clone it, add `DsnFileSettings(20)` and — only for `-dr`/`-de …rules` — `RulesFileSettings(40)` |
//! | 3 | `Freerouting.java:146` → `SettingsMerger.java:133-193` | **merge #1**: sort ascending, `clone()` the first non-null source, `applyNewValuesFrom` the rest, `validate()` |
//! | 4 | `RoutingJobScheduler.java:93-96` → `HeadlessBoardManager.java:739-748` | on merge #1's result: `setLayerCount(board)` when it disagrees, then `applyBoardSpecificOptimizations(board)` |
//! | 5 | `RoutingJobScheduler.java:103-166` | clone the prototype again, re-add the DSN, add the rules the scheduler chose, add `new ApiSettings(job.routerSettings)` at **70** — the whole result of steps 3-4 |
//! | 6 | `RoutingJobScheduler.java:170` | **merge #2**, including its own `validate()` |
//! | 7 | `:173-184` → `RulesReader.java:153-157` | re-parse the `.rules` bytes and `applyNewValuesFrom` them onto the **merged** object |
//! | 8 | `:186` | `applyBoardSpecificOptimizations(board)` |
//!
//! Merge #1's result is *complete* — every field is non-null once `DefaultSettings` has run — so
//! at priority 70 it beats everything merge #2's own `0..60` chain contributes, **except where it
//! left a field null**. That is the only channel merge #2's own sources have, and it is what
//! [`RouterSettings::fill_absent_from`] expresses. Step 7 is a different matter: it is a plain
//! `applyNewValuesFrom` onto the finished object, so for every field an `(autoroute_settings)`
//! block can carry, the `.rules` file outranks the environment and the command line.
//!
//! # The linear form
//!
//! ```text
//! s = DefaultSettings                                  // 0
//! s.apply(dsn)                                         // 20
//! s.apply(parse(cli_rules))                            // 40   ( -dr / -de …rules only )
//! s.apply(env)                                         // 55
//! s.apply(cli)                                         // 60
//! s.validate()                                         // merge #1's validate, == the priority-70 payload
//! s.set_layer_count(board) if it disagrees             // HeadlessBoardManager.java:741-744
//! s.apply_board_specific_optimizations(board)          // HeadlessBoardManager.java:745
//! s.board_specific_trace_costs_applied = None          // the private flag never survives merge #2
//! s.fill_absent_from(parse(scheduler_rules))           // merge #2's own 0..60 chain, all of it
//! s.validate()                                         // merge #2's validate
//! s.apply(parse_against(scheduler_rules, board))       // RulesReader.java:112, :153-157
//! s.apply_board_specific_optimizations(board)          // RoutingJobScheduler.java:186
//! ```
//!
//! **The two lines this file does not carry.** `applyRouterSettingsForLoadedBoard` has four
//! steps, not two: after `:745` it calls `applyCopperToEdgeClearanceOverride` (`:746`) and
//! `applyHoleClearanceOverride` (`:747`). Those mutate the **board**, not the settings, so they
//! are not here — `resolve_headless` takes `&Board`. Plan 7 Task 15b ported them as
//! `fr_board::Board::apply_*` and gave them a seam, `fr_router::pipeline::prepare_board`, which
//! a caller runs on the loaded board after this function. Until then nothing in the port applied
//! them at all (Plan 8 survey §5.7), and `applyCopperToEdgeClearanceOverride` mutates 15 of the
//! 16 corpus boards at default settings (quirk #231).
//!
//! `parse` is `RulesReader.readRouterSettings` — the layer structure discovered from the file's
//! own `(layer_rule …)` names — and `parse_against` is `RulesReader.read`, whose layer structure
//! is the **board's**. They are two different readings of the same bytes and Java performs both,
//! which is why the rules slots of [`SettingsInputs`] are byte slices rather than parsed objects
//! (quirk #142).
//!
//! `crates/fr-settings/tests/precedence.rs` runs both forms over a 64-case matrix and asserts
//! they agree field for field; Task 9's `p4t1` runs the same matrix against the JVM.
//!
//! # Three things the plan's nine-step listing gets wrong, and Java wins
//!
//! 1. **The board pass runs twice.** Ruling 1 carries only the `setLayerCount` half of
//!    `HeadlessBoardManager.applyRouterSettingsForLoadedBoard`; `:745` calls
//!    `applyBoardSpecificOptimizations(board)` right after it, on merge #1's result, before
//!    merge #2 ever runs. Both are reproduced here. The first pass leaves no trace in the final
//!    answer — every layer field it fills, the last pass would fill identically, and the cost
//!    arrays it writes are re-initialised at the end — which is why the omission was harmless;
//!    `the_first_board_optimization_pass_leaves_no_trace_in_the_final_result` pins that rather
//!    than assuming it. Its neighbour at `:741-744` — the `setLayerCount` the plan does carry —
//!    is not unobservable at all: it is quirk #119's teeth
//!    (`the_first_board_layer_count_discards_a_mis_sized_layer_array`).
//! 2. **The private flag has to be cleared with it.** The first board pass sets
//!    `boardSpecificTraceCostsApplied = TRUE` (`RouterSettings.java:386`), and merge #2 builds a
//!    *fresh* object that `copyFields` can never write that `private` field into
//!    (`ReflectionUtil.java:226-228`) — so Java's merge #2 result always carries a null flag,
//!    which is exactly why step 8 re-initialises the costs (quirk #127). The linear pass keeps
//!    the same object, so it clears the flag by hand at the point where Java changes objects.
//!    Like the pass that sets it, this is state fidelity rather than a behaviour change: the two
//!    board passes compute *identical* arrays, because the penalty at `:365-373` is chosen by the
//!    board's running alternation and not by the `preferredDirectionHorizontal` a later `.rules`
//!    re-apply may have flipped, so whether the last pass recomputes them is unobservable today.
//!    Both are reproduced anyway — the next field added to `ScoringSettings` could make either
//!    one visible, and a port that had quietly dropped them would then be wrong with no test to
//!    say so.
//! 3. **`validate()` is not idempotent.** The plan expects the second call to be a no-op because
//!    `.rules` carries none of its three fields. It is a no-op for `max_threads` and
//!    `trace_pull_tight_accuracy`, but `max_passes == 0` becomes `Integer.MAX_VALUE` on the first
//!    call (`RouterSettings.java:937-940`) and `9999` on the second, because `MAX_VALUE > 9999`.
//!    Quirk #140.
//!
//! # Where the linearisation stops being exact
//!
//! `resolve_headless(.., None, ..)` — no board — is the merge alone, and there it can disagree
//! with Java's two merges in exactly one family: **no DSN source, and merge #1's `-dr` rules
//! different from merge #2's `job.rules`**. `copyFields` rule 5 is first-writer-wins for
//! primitive arrays (`ReflectionUtil.java:269-290`), and merge #2 restarts that race from a fresh
//! `DefaultSettings.clone()`, so it can pick a different winner for the two `scoring` cost arrays
//! than merge #1 did; a single linear pass carries merge #1's answer and cannot un-write it.
//! Java never sees it: the difference survives only while no board pass runs, and a job with no
//! board never reaches `RoutingJobScheduler.java:173` or `:186` at all — `:186` re-derives both
//! arrays from the board and the disagreement is gone. `tests/precedence.rs`'s
//! `a_split_rules_pair_restarts_the_cost_array_race` pins both halves of that.

use std::path::{Path, PathBuf};

use fr_board::Board;

use crate::sources::rules_file::apply_rules_file_against_board;
use crate::{HostEnvironment, RouterSettings, SettingsSource, sources::DefaultSettings};

/// Everything the headless path can feed the merge. Every field is optional; `None` means the
/// corresponding Java source was never registered.
#[derive(Debug, Clone, Copy, Default)]
pub struct SettingsInputs<'a> {
    /// `JsonFileSettings`, priority 10 (`Freerouting.java:1410`) — the `--settings <file>` the
    /// CLI names, else the working directory's `freerouting.json`
    /// ([`crate::sources::JsonFileSettings`], scan ruling R7). `None` is "no such file", which is
    /// what the source itself answers for a missing or unreadable document (`:55-62`).
    ///
    /// It feeds **both** chains, because Java registers it on the prototype merger
    /// (`Freerouting.java:1408-1413`) that merge #1 and merge #2 are each cloned from: merge #1's
    /// `applyNewValuesFrom` between `DefaultSettings` and the DSN, and merge #2's own `0..60`
    /// chain, where it can only reach a field merge #1 left absent
    /// ([`RouterSettings::fill_absent_from`]).
    pub json_file: Option<&'a RouterSettings>,
    /// `DsnFileSettings`, priority 20 (`Freerouting.java:126-127`,
    /// `RoutingJobScheduler.java:111-115`). `None` is a non-DSN input, where the scheduler
    /// registers no DSN source at all.
    pub dsn: Option<&'a RouterSettings>,
    /// The **bytes** of the `-dr` / `-de …rules` file, priority 40 — **merge #1 only**
    /// (`Freerouting.java:129-136`). Parsed here through `RulesReader.readRouterSettings`, which
    /// is the only way Java reaches this file.
    pub cli_rules: Option<&'a [u8]>,
    /// The **bytes** of the `.rules` the scheduler resolved (`job.rules ?? -dr ?? adjacent
    /// <design>.rules`, `RoutingJobScheduler.java:115-152` — see
    /// [`resolve_scheduler_rules_path`]).
    ///
    /// Bytes, not a parsed `RouterSettings`, because **Java parses this file twice with two
    /// different layer structures** and both results reach the answer:
    ///
    /// 1. at priority 40 in merge #2, through `RulesFileSettings` →
    ///    `RulesReader.readRouterSettings`, whose layer structure is *discovered from the file*
    ///    (`RulesReader.java:198`, `:238-274`);
    /// 2. after merge #2, through `RulesReader.read(…, job.board, job.routerSettings)`
    ///    (`RoutingJobScheduler.java:173-184`), whose layer structure is the **board's**
    ///    (`RulesReader.java:112`).
    ///
    /// A two-`layer_rule` file read against a four-layer board puts `B.Cu` at index 3 in the
    /// second parse and at index 1 in the first. Handing `resolve_headless` one parsed object
    /// therefore cannot be right for both steps — it was wrong on 13 of Task 9's 84 differential
    /// rows — so it takes the bytes and performs both parses itself. Quirk #142;
    /// [`crate::sources::rules_file::apply_rules_file_against_board`] is the second one.
    pub scheduler_rules: Option<&'a [u8]>,
    /// `EnvironmentVariablesSource`, priority 55.
    pub env: Option<&'a RouterSettings>,
    /// `CliSettings`, priority 60.
    pub cli: Option<&'a RouterSettings>,
}

/// Which of the merge's optional steps run. Java always runs all of them; the port's tests turn
/// one off at a time to show what each is worth.
#[derive(Debug, Clone, Copy)]
struct Steps {
    /// `HeadlessBoardManager.java:741-744` — the `setLayerCount` half of the between-merges pass.
    first_board_layer_count: bool,
    /// `HeadlessBoardManager.java:745` — its `applyBoardSpecificOptimizations` half.
    first_board_optimization: bool,
    /// `RulesReader.java:153-157`, after merge #2.
    post_merge_rules_reapply: bool,
    /// Merge #2's `validate()` (`SettingsMerger.java:189`).
    second_validate: bool,
}

impl Steps {
    /// What Java runs.
    const JAVA: Self = Self {
        first_board_layer_count: true,
        first_board_optimization: true,
        post_merge_rules_reapply: true,
        second_validate: true,
    };
}

/// Plan ruling 1: the linear form of Java's two merges plus the post-merge rules re-apply.
///
/// # It models the CLI-started path, and only that
///
/// The linearisation's premise is that merge #1's result is *complete* — every field non-null
/// after `DefaultSettings` — so that re-injecting it at priority 70 makes merge #2's own `0..60`
/// chain reachable only where merge #1 left a field absent. That holds for a job started by
/// `Freerouting.java:125-146`, which is the only entry point that runs merge #1.
///
/// It does **not** hold for an API job. There `job.routerSettings` is `new RouterSettings()`
/// (`core/RoutingJob.java:105`), the request body deserialised into one
/// (`api/v1/JobInputResource.java:203-211`), or a bare `setLayerCount` (`:337-339`, `:540-542`) —
/// a *sparse* payload at priority 70, with merge #2's own chain doing the real work. Feeding one
/// of those to `resolve_headless` as `SettingsInputs` would answer the wrong thing, because
/// `fill_absent_from` is not the same operation as "merge #2's sources, with a sparse override on
/// top".
///
// ~~obligation: RoutingJobScheduler.scheduleJob — Plan 8 owns the API surface (plan ruling 10) and
// must compose the API path separately: merge #2 alone, with `ApiSettings(job.routerSettings)`
// as a sparse priority-70 source, rather than calling `resolve_headless`.~~
//
//   **DISCHARGED — both users, by Plan 8 Tasks 7 and 12.** The marker had two independent
//   users, and each composes [`crate::SettingsMerger`] directly for the structural reason
//   recorded above. Neither calls this function, and neither is a variant of the other:
//
//   1. **Task 12, ruling AU's MCP path** —
//      `crates/freerouting/src/mcp/tools/route_board.rs`. `RoutingJobScheduler.scheduleJob`'s own
//      order (`RoutingJobScheduler.java:91-186`): the caller's sparse `settings` object becomes
//      `job.routerSettings` *before* the load (`api/v1/JobInputResource.java:203-211`), the load's
//      `applyRouterSettingsForLoadedBoard` pass writes the board's layer count and board-tuned
//      trace costs into it, and it is then registered as `ApiSettings` at priority 70 over a
//      merger holding `DefaultSettings(0)`, `JsonFileSettings(10)`, `DsnFileSettings(20)`,
//      `RulesFileSettings(40)`, `EnvironmentVariablesSource(55)` and `CliSettings(60)`.
//      **Measured against this function**: `route_board` on
//      `examples/tutorial_board/tutorial_board.dsn` answers a session byte-identical to `p8t1`'s
//      `tests/reference/cli-tutorial_board/route.ses`, and on
//      `fixtures/Issue143-rpi_splitter.dsn` byte-identical to what `freerouting route` writes —
//      two different compositions, one SES
//      (`crates/freerouting/tests/mcp_stdio.rs::the_four_tools_over_spawned_pipes`).
//   2. **Task 7, the DRC quality score (quirk #272)** —
//      `crates/freerouting/src/commands/drc.rs::quality_score_settings`.
//      `Freerouting.initializeDrc:342-352`: the prototype merger plus one `DsnFileSettings`, with
//      no `.rules` tier, no board pass and no merge #2. `p8t3 merge` pins it against the JVM
//      field by field. **Untouched by Task 12** — the two are different compositions and the
//      discharge of one is not the discharge of the other.
//
//   `docs/java-quirks.md` carries the same note. What survives the discharge is the *warning*
//   this function's premise section states: `resolve_headless` models the CLI-started path and
//   only that, and a future API-shaped caller must compose the merger rather than reach for it.
///
/// `board` is `None` for "there is no board" — the merge alone. Java reaches that shape nowhere in
/// the headless path, and each of its three board-facing sites answers it differently:
/// `HeadlessBoardManager.java:740` guards on `board != null` and skips its pass;
/// `RoutingJobScheduler.java:173` guards the post-merge `.rules` re-apply on
/// `rulesData != null && job.board != null`; `:186` guards nothing at all, so a null board there
/// is an `NPE` at `RouterSettings.java:267`. This port follows the first two exactly — with no
/// board there is no layer structure for the second parse of the `.rules` file to resolve against
/// (quirk #142), so the re-apply *cannot* run — and totalizes the third. See the module docs for
/// the one input family where the board-less answer then differs from Java's two merges.
///
// totalized: applyBoardSpecificOptimizations (RouterSettings.java:266-267) — Java dereferences
// `board.boundingBox` with no null check, so `RoutingJobScheduler.java:186`'s unguarded
// `job.routerSettings.applyBoardSpecificOptimizations(job.board)` throws a
// `NullPointerException` for a board-less job; the scheduler's `catch (Exception)` at `:243-249`
// logs it and sets the job `TERMINATED`. This port returns the merged settings instead. No
// reachable caller observes the difference — the scheduler only gets there once
// `HeadlessBoardManager` has produced a board — and quirk row "totalized" records it. It is the
// only one of the three sites this port does not follow literally; `:173`'s guard is reproduced.
///
/// # Panics
///
/// Through [`RouterSettings::validate`], if `DefaultSettings` did not run — which cannot happen
/// here, since this function starts from it. See quirk #125 for the merger-level panic.
#[must_use]
pub fn resolve_headless(
    inputs: &SettingsInputs<'_>,
    board: Option<&Board>,
    host: &HostEnvironment,
) -> RouterSettings {
    resolve_headless_steps(inputs, board, host, Steps::JAVA)
}

fn resolve_headless_steps(
    inputs: &SettingsInputs<'_>,
    board: Option<&Board>,
    host: &HostEnvironment,
    steps: Steps,
) -> RouterSettings {
    // --- merge #1 (`Freerouting.java:125-146`, `SettingsMerger.java:133-193`) -----------------
    // The base is the lowest-priority source, taken through `clone()` (`SettingsMerger.java:160`
    // → `RouterSettings.java:487-504`), not through the derived `Clone`: the two differ in
    // `resultJsonPath` and in how a null nested object comes back (quirk #114).
    let defaults = DefaultSettings::new(host);
    let mut settings = defaults
        .get_settings()
        .expect("DefaultSettings always has settings")
        .java_clone();

    // Every later source is `applyNewValuesFrom` (`SettingsMerger.java:171`), in ascending
    // priority order. `JsonFileSettings(10)` sits here, between `DefaultSettings` and the DSN,
    // and its history is worth keeping: Plan 4 wrote ~~"is out of scope (spec §2) and is a
    // no-op when the file is absent, which is the only shape the headless path has"~~ and left it
    // out of the chain; scan ruling R7 ported the source ([`crate::sources::JsonFileSettings`])
    // in Task 5 and gave the CLI `--settings <file>` and a working-directory `freerouting.json`;
    // **Task 6 threads it**, because `commands::route` is the first caller that can supply one.
    // `p4t1` proves the *absent*-file tier contributes nothing;
    // `tests/json.rs::a_json_file_tier_beats_the_defaults_and_loses_to_the_dsn` proves a present
    // one lands where Java puts it (above the defaults, below the DSN), and
    // `crates/freerouting/tests/cli_e2e.rs::a_settings_file_reaches_the_run` proves it through
    // the binary, which is the only caller that can supply one.
    if let Some(json_file) = inputs.json_file {
        settings.apply_new_values_from(json_file); // 10
    }
    if let Some(dsn) = inputs.dsn {
        settings.apply_new_values_from(dsn); // 20
    }
    if let Some(cli_rules) = parse_rules_file(inputs.cli_rules) {
        settings.apply_new_values_from(&cli_rules); // 40
    }
    if let Some(env) = inputs.env {
        settings.apply_new_values_from(env); // 55
    }
    if let Some(cli) = inputs.cli {
        settings.apply_new_values_from(cli); // 60
    }
    // `SettingsMerger.java:189`. This is the object `RoutingJobScheduler.java:163-166` re-injects
    // at priority 70.
    //
    // Java bug: validate (RouterSettings.java:932-941) — this call maps `maxPasses == 0` to
    // `Integer.MAX_VALUE` (`:937-940`) and the second one below maps that to `9999` (`:934-936`),
    // so the headless path's two merges turn `--router.max_passes=0` into 9999 where a single
    // merge answers `Integer.MAX_VALUE`. Quirk #140.
    settings.validate(host);

    // --- `HeadlessBoardManager.applyRouterSettingsForLoadedBoard` (`:739-748`) ----------------
    // It runs between the merges: `RoutingJobScheduler.java:93-96` builds the
    // `HeadlessBoardManager` around the job whose `routerSettings` merge #1 has already set.
    if let Some(board) = board {
        // :741-744 — quirk #119's teeth: a `--router.layers.*` or `FREEROUTING__ROUTER__LAYERS__*`
        // whose token count disagrees with the board's layer count sized `layers` from the
        // *tokens* (`ReflectionUtil.java:52-59`), and `setLayerCount` then discards the whole
        // array — silently.
        let board_layer_count = board.get_layer_count();
        if steps.first_board_layer_count && settings.get_layer_count() != board_layer_count {
            settings.set_layer_count(board_layer_count);
        }
        // :745.
        if steps.first_board_optimization {
            settings.apply_board_specific_optimizations(board);
        }
        // :746-747 — `applyCopperToEdgeClearanceOverride` / `applyHoleClearanceOverride` read
        // `routerSettings` and write the *board's* rules; they change no setting, so there is
        // nothing to reproduce here.
    }

    // --- merge #2 (`RoutingJobScheduler.java:103-170`) ----------------------------------------
    // Merge #2 builds a fresh `RouterSettings` from `DefaultSettings.clone()` and copies
    // everything else in with `copyFields`, which never writes the `private
    // boardSpecificTraceCostsApplied` (`ReflectionUtil.java:226-228`,
    // `RouterSettings.java:111`). So whatever the first board pass set, merge #2's result carries
    // a null flag — and that is precisely why step `:186` re-initialises the trace costs (quirk
    // #127). The linear pass keeps one object, so it clears the flag where Java changes objects.
    // State fidelity, not behaviour: both board passes compute the same arrays, so today nothing
    // observes whether the last one recomputes them (module docs, correction 2).
    settings.board_specific_trace_costs_applied = None;

    // Merge #2's own `0..60` chain is `DefaultSettings`, the DSN, the scheduler's `.rules`, the
    // environment and the CLI — the same sources that already fed merge #1, except the rules.
    // Since merge #1's result sits above all of them at priority 70, they can only reach a field
    // it left absent, which is `fill_absent_from` (plan ruling 1's Q1 channel: `layers[i]`'s
    // three nullable fields, `resultJsonPath` and the two `timeoutString`s).
    // The priority-10 tier is on the prototype merger, so merge #2's clone carries it too
    // (`Freerouting.java:1408-1413` → `RoutingJobScheduler.java:103`). It is applied before the
    // rules, in the merger's own ascending-priority order.
    //
    // **This arm is fidelity, not an observable channel of its own**, and the test above says so
    // rather than claiming otherwise: after merge #1 every field the document carries is
    // non-null, so `fill_absent_from` finds nothing of the json's to fill. Its only reachable
    // channel is a field the between-merges board pass *nulled* — `set_layer_count`'s
    // `preferred_direction_horizontal`/`bend_cost` wipe — which is the same Q1 channel
    // `adjacent_rules_reach_only_the_fields_merge_one_left_null` (below) pins for the `.rules`
    // tier through the identical call. It is written out because Java writes it out, and because
    // a future tier list in which merge #1 does *not* carry this source would need it.
    if let Some(json_file) = inputs.json_file {
        settings.fill_absent_from(json_file);
    }
    // The priority-40 parse — `RulesFileSettings`, layer structure discovered from the file
    // itself. **Not** the board-structured one below; see [`SettingsInputs::scheduler_rules`].
    if let Some(scheduler_rules) = parse_rules_file(inputs.scheduler_rules) {
        settings.fill_absent_from(&scheduler_rules);
    }
    if steps.second_validate {
        // Java bug: validate (RouterSettings.java:934-936) — the second half of quirk #140: it is
        // this call that sees the `Integer.MAX_VALUE` the first one wrote and clamps it to 9999.
        settings.validate(host); // `SettingsMerger.java:189`, again.
    }

    // --- the post-merge re-apply (`:173-184` → `RulesReader.java:153-157`) --------------------
    // `RulesReader.read(new ByteArrayInputStream(rulesData), designName, job.board,
    // job.routerSettings)`: the file parsed a *second* time, against the board's layer structure,
    // and `applyNewValuesFrom`'d onto the already-merged object — which is what puts
    // `(autoroute_settings)` above the environment and the command line (quirk Q2,
    // `docs/java-quirks.md` #142), and what
    // makes the two parses observably different (quirk #142).
    //
    // Java's guard is `rulesData != null && job.board != null` (`:173`), reproduced exactly: with
    // no board there is no layer structure to parse against, so the step cannot run at all.
    if let (true, Some(bytes), Some(board)) = (
        steps.post_merge_rules_reapply,
        inputs.scheduler_rules,
        board,
    ) {
        apply_rules_file_against_board(bytes, board, &mut settings);
    }

    // --- `RoutingJobScheduler.java:186` -------------------------------------------------------
    // `applyBoardSpecificOptimizations`, not the `IfNeeded` variant: unconditional.
    if let Some(board) = board {
        settings.apply_board_specific_optimizations(board);
    }

    settings
}

/// `RulesFileSettings`' parse (`sources/RulesFileSettings.java:81-93` →
/// `RulesReader.readRouterSettings`, `RulesReader.java:180-236`): the layer structure is
/// discovered from the file's own `(layer_rule …)` names, and every failure — an empty stream, a
/// bad header, no `(autoroute_settings …)` scope, an unreadable file — gives Java a blank
/// `RouterSettings` whose `applyNewValuesFrom` writes nothing. `None` here is that blank.
fn parse_rules_file(bytes: Option<&[u8]>) -> Option<RouterSettings> {
    match fr_dsn::rules_reader::read_router_settings(bytes?) {
        Ok(Some(parsed)) => Some(RouterSettings::from(parsed)),
        Ok(None) | Err(_) => None,
    }
}

/// How the scheduler picks the `.rules` file that feeds merge #2 and the post-merge re-apply
/// (`RoutingJobScheduler.java:115-152`): the job's own rules, else `-dr`, else an adjacent
/// `<design>.rules` beside the DSN.
///
/// The `else if` chain matters: a `-dr` that names a **missing** file does not fall through to
/// the adjacent probe (`:118-131` — the branch is taken on `initialRulesFile != null`, and the
/// inner `rf.exists()` only decides whether the bytes are read), so a typo'd `-dr` silently
/// disables the adjacent file a user would otherwise have got.
///
/// `job_rules` is not probed for existence: Java tests `job.rules.getData() != null` (`:115`),
/// i.e. bytes that are already in hand. The other two are `File.exists()` calls
/// (`RoutingJobScheduler.java:120`, `:140`), reproduced here as `Path::exists`; the injectable
/// form is [`resolve_scheduler_rules_path_with`].
#[must_use]
pub fn resolve_scheduler_rules_path(
    job_rules: Option<&Path>,
    cli_rules: Option<&Path>,
    dsn_path: Option<&Path>,
) -> Option<PathBuf> {
    resolve_scheduler_rules_path_with(job_rules, cli_rules, dsn_path, |path| path.exists())
}

/// [`resolve_scheduler_rules_path`] with the two `File.exists()` probes injected, so a caller —
/// or a test — can resolve against something other than the real filesystem.
#[must_use]
pub fn resolve_scheduler_rules_path_with(
    job_rules: Option<&Path>,
    cli_rules: Option<&Path>,
    dsn_path: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    // :115-117 — the job arrived with its own rules bytes.
    if let Some(job_rules) = job_rules {
        return Some(job_rules.to_path_buf());
    }
    // :118-131 — `globalSettings.initialRulesFile`, read only when it exists.
    if let Some(cli_rules) = cli_rules {
        return exists(cli_rules).then(|| cli_rules.to_path_buf());
    }
    // :132-152 — `new File(job.input.getDirectoryPath(), baseName + ".rules")`, gated on
    // `isDsn && job.input.getDirectoryPath() != null`.
    let dsn_path = dsn_path?;
    let file_name = dsn_path.file_name()?.to_string_lossy().into_owned();
    // :133-136 — `lastIndexOf('.') > 0`, so a leading dot is *not* an extension separator:
    // `.dsn` keeps its whole name and the probe looks for `.dsn.rules`.
    let base_name = match file_name.rfind('.') {
        Some(dot) if dot > 0 => &file_name[..dot],
        _ => &file_name[..],
    };
    let adjacent = dsn_path.with_file_name(format!("{base_name}.rules"));
    exists(&adjacent).then_some(adjacent)
}

#[cfg(test)]
mod tests {
    use fr_board::prelude::*;
    use fr_geometry::{IntBox, PolylineShapeRef, TileShape};

    use super::*;
    use crate::sources::{CliSettings, EnvironmentVariablesSource};

    fn host() -> HostEnvironment {
        HostEnvironment::with_processors(4)
    }

    /// `BProbe.java`'s synthetic board: an `IntBox` outline that is also the bounding box, and a
    /// stack of signal layers named `L0`, `L1`, … — the names [`rules_bytes`] writes into its
    /// `layer_rule`s, so that the board-structured parse of those bytes
    /// (`RulesReader.java:112`) resolves them.
    fn board(layer_count: usize) -> Board {
        named_board(
            &(0..layer_count)
                .map(|i| format!("L{i}"))
                .collect::<Vec<_>>(),
        )
    }

    /// The same board with the layer names spelled out — for the one test that feeds it a
    /// committed `.rules` fixture, whose `layer_rule`s name `F.Cu`/`B.Cu`.
    fn named_board(names: &[String]) -> Board {
        let layers = LayerStructure::new(
            names
                .iter()
                .map(|name| Layer::new(name.clone(), true))
                .collect(),
        );
        let clearance_matrix = ClearanceMatrix::get_default_instance(&layers, 10);
        let mut rules = BoardRules::new(layers.clone(), clearance_matrix);
        rules.create_default_net_class();
        let box_ = IntBox::from_coords(0, 0, 2_000_000, 1_000_000);
        Board::new(
            vec![PolylineShapeRef::Tile(TileShape::Box(box_))],
            0,
            box_,
            rules,
            BoardLibrary::new(Padstacks::new(layers), Packages::new()),
            Components::new(),
            Communication::default(),
        )
    }

    /// What `DsnFileSettings` produces for a file with no `(autoroute_settings)` block: a layer
    /// count, and the seeding `setLayerCount` does (`DsnFileSettings.java:46-48`).
    fn bare_dsn(layer_count: usize) -> RouterSettings {
        let mut settings = RouterSettings::new();
        settings.set_layer_count(layer_count);
        settings
    }

    /// A minimal `.rules` file: `(via_costs 99)` and one `layer_rule` naming the board's first
    /// layer horizontal. Only layer 0 is named, so every other layer's
    /// `preferredDirectionHorizontal` stays absent on both parses — which is what makes the Q1
    /// channel visible.
    ///
    /// Bytes rather than a `RouterSettings`, because `resolve_headless` parses the file twice
    /// itself (quirk #142).
    fn rules_bytes() -> Vec<u8> {
        b"(rules PCB unit\n  (autoroute_settings\n    (vias on)\n    (via_costs 99)\n    (layer_rule L0\n      (active on)\n      (preferred_direction horizontal)\n    )\n  )\n)\n"
            .to_vec()
    }

    /// Ruling 1's Q1 channel, and the correction to survey Q1: the scheduler's `.rules` reaches
    /// `layers[0].preferredDirectionHorizontal` — a field merge #1 left `null` — through
    /// `fill_absent_from`, while its `viaCosts` reaches the answer only through the post-merge
    /// re-apply, because `DefaultSettings` already wrote 50 there. Turning the re-apply off tells
    /// the two channels apart: the direction still lands, `viaCosts` falls back to 50.
    ///
    /// The first board pass is switched off for the same reason the board is switched *on*: with
    /// it, `HeadlessBoardManager.java:745` fills every `preferredDirectionHorizontal` before
    /// merge #2 runs and the fill channel has nothing left to carry
    /// (`the_first_board_pass_closes_the_direction_channel`); without the board, the post-merge
    /// re-apply cannot run at all, because Java guards it on `job.board != null`
    /// (`RoutingJobScheduler.java:173`) and the second parse needs the board's layer structure
    /// (quirk #142). Both switches are `#[cfg(test)]`; Java runs every step.
    #[test]
    fn adjacent_rules_reach_only_the_fields_merge_one_left_null() {
        let host = host();
        let board = board(2);
        let dsn = bare_dsn(2);
        let rules = rules_bytes();
        let inputs = SettingsInputs {
            json_file: None,
            dsn: Some(&dsn),
            scheduler_rules: Some(&rules),
            ..SettingsInputs::default()
        };
        let no_first_pass = Steps {
            first_board_optimization: false,
            ..Steps::JAVA
        };

        let resolved = resolve_headless_steps(&inputs, Some(&board), &host, no_first_pass);
        assert!(resolved.get_preferred_direction_is_horizontal(0));
        assert_eq!(resolved.get_via_costs(), 99);

        let without_reapply = resolve_headless_steps(
            &inputs,
            Some(&board),
            &host,
            Steps {
                post_merge_rules_reapply: false,
                ..no_first_pass
            },
        );
        assert!(
            without_reapply.get_preferred_direction_is_horizontal(0),
            "the direction comes through fill_absent_from, not the re-apply"
        );
        assert_eq!(
            without_reapply.get_via_costs(),
            50,
            "DefaultSettings.DEFAULT_VIA_COSTS — the fill cannot overwrite it"
        );
    }

    /// The other half of the same story: once the first board pass runs, it fills
    /// `preferredDirectionHorizontal` on every layer (`RouterSettings.java:361-363`), so the
    /// priority-70 payload is no longer null there and the `.rules` direction can only arrive
    /// through the post-merge re-apply. The end result is the same either way — which is why
    /// nobody noticed `:745` was missing from the plan's step list.
    #[test]
    fn the_first_board_pass_closes_the_direction_channel() {
        let host = host();
        let board = board(2);
        let dsn = bare_dsn(2);
        let rules = rules_bytes();
        let inputs = SettingsInputs {
            json_file: None,
            dsn: Some(&dsn),
            scheduler_rules: Some(&rules),
            ..SettingsInputs::default()
        };

        assert!(
            resolve_headless(&inputs, Some(&board), &host).get_preferred_direction_is_horizontal(0)
        );

        let without_reapply = resolve_headless_steps(
            &inputs,
            Some(&board),
            &host,
            Steps {
                post_merge_rules_reapply: false,
                ..Steps::JAVA
            },
        );
        // A 2 000 000 x 1 000 000 board starts the alternation at `false` and toggles on layer 0,
        // so layer 0 comes out horizontal here too — but from the board, not from the file.
        assert!(without_reapply.get_preferred_direction_is_horizontal(0));
        // Layer 1 is where the two differ: the file says nothing about it, and the alternation
        // makes it vertical.
        assert!(!without_reapply.get_preferred_direction_is_horizontal(1));
    }

    /// Quirk #142, in isolation: the same bytes, the same board, two answers.
    ///
    /// The file names two `layer_rule`s, `L0` and `L3`. Read against its own names
    /// (`RulesReader.readRouterSettings` → `discoverLayerStructure`, `:198`) they are layers 0 and
    /// **1** of a two-layer stack; read against a four-layer board (`RulesReader.read`, `:112`)
    /// they are layers 0 and **3**. Feeding `resolve_headless` a single pre-parsed
    /// `RouterSettings` — what `SettingsInputs` held before fix round 2 — could only ever be right
    /// for one of the two steps.
    #[test]
    fn a_rules_file_is_parsed_twice_against_two_layer_structures() {
        let bytes = b"(rules PCB unit\n  (autoroute_settings\n    (layer_rule L0\n      (active on)\n      (preferred_direction horizontal)\n    )\n    (layer_rule L3\n      (active off)\n      (preferred_direction horizontal)\n    )\n  )\n)\n";

        // 1. the file-discovered structure: two layers, `L3` at index 1.
        let discovered = parse_rules_file(Some(bytes)).expect("the scope parses");
        assert_eq!(discovered.get_layer_count(), 2);
        assert!(!discovered.get_layer_active(1), "`L3` landed at index 1");

        // 2. the board's structure: four layers, `L3` at index 3.
        let mut target = RouterSettings::new();
        target.set_layer_count(4);
        assert!(apply_rules_file_against_board(
            bytes,
            &board(4),
            &mut target
        ));
        assert!(
            target.get_layer_active(1),
            "index 1 is `L1`, which the file never names"
        );
        assert!(!target.get_layer_active(3), "`L3` landed at index 3");
    }

    /// The environment and the CLI sources every case in the pair of tests below shares.
    fn env_and_cli() -> (EnvironmentVariablesSource, CliSettings) {
        let cli = CliSettings::new(&[
            "--router.max_passes=88".to_string(),
            "--router.optimizer.enabled=false".to_string(),
        ]);
        let env = EnvironmentVariablesSource::new(
            &[
                (
                    "FREEROUTING__ROUTER__MAX_PASSES".to_string(),
                    "77".to_string(),
                ),
                (
                    "FREEROUTING__ROUTER__LAYERS__PREFERRED_DIRECTION_HORIZONTAL".to_string(),
                    "true,true".to_string(),
                ),
            ]
            .into_iter()
            .collect(),
        );
        (env, cli)
    }

    /// The correction to ruling 1, pinned: running `applyBoardSpecificOptimizations` between the
    /// merges (`HeadlessBoardManager.java:745`) changes nothing about the final answer. Every
    /// per-layer field it fills, the last pass at `RoutingJobScheduler.java:186` fills
    /// identically from the same board; the cost arrays it writes lose to the DSN's seeding at
    /// priority 20 in merge #2 and are re-initialised at the end anyway (quirk #127).
    ///
    /// It is reproduced regardless, because "unobservable" is a property of the current field
    /// set, not a licence to leave a step out — and its neighbour at `:741-744` is *not*
    /// unobservable at all, as the next test shows.
    #[test]
    fn the_first_board_optimization_pass_leaves_no_trace_in_the_final_result() {
        let host = host();
        let (env, cli) = env_and_cli();
        let rules = rules_bytes();
        let scheduler_rules: &[u8] = &rules;

        for layer_count in [2usize, 4] {
            let board = board(layer_count);
            let dsn = bare_dsn(layer_count);
            for (dsn_input, cli_rules) in [
                (None, None),
                (Some(&dsn), None),
                (Some(&dsn), Some(scheduler_rules)),
            ] {
                let inputs = SettingsInputs {
                    json_file: None,
                    dsn: dsn_input,
                    cli_rules,
                    scheduler_rules: Some(scheduler_rules),
                    env: env.get_settings(),
                    cli: cli.get_settings(),
                };
                let with = resolve_headless(&inputs, Some(&board), &host);
                let without = resolve_headless_steps(
                    &inputs,
                    Some(&board),
                    &host,
                    Steps {
                        first_board_optimization: false,
                        ..Steps::JAVA
                    },
                );
                assert_eq!(with, without, "layers={layer_count}");
            }
        }
    }

    /// Quirk #119's other half, and the reason the controller asked for `:741-744` in both forms:
    /// the `setLayerCount` between the merges is *very* observable.
    ///
    /// `FREEROUTING__ROUTER__LAYERS__PREFERRED_DIRECTION_HORIZONTAL=true,true` sizes `layers`
    /// from its **token count** (`ReflectionUtil.java:52-59`). With no DSN source to seed a layer
    /// count, merge #1's result therefore carries a 2-element `layers` — and on a 4-layer board
    /// `HeadlessBoardManager.java:742-743` fires, and `setLayerCount(4)` resets every element's
    /// `preferredDirectionHorizontal` to null (`RouterSettings.java:456-477`). The user's two
    /// directions are gone, silently. Skip that one call and they survive into the answer.
    #[test]
    fn the_first_board_layer_count_discards_a_mis_sized_layer_array() {
        let host = host();
        let (env, cli) = env_and_cli();
        let board = board(4);
        let inputs = SettingsInputs {
            env: env.get_settings(),
            cli: cli.get_settings(),
            ..SettingsInputs::default()
        };

        // Java: the board's alternation wins on every layer, because `setLayerCount` wiped the
        // environment's array first. 2 000 000 x 1 000 000 starts the running flag at `false`.
        let resolved = resolve_headless(&inputs, Some(&board), &host);
        let directions: Vec<bool> = (0..4)
            .map(|i| resolved.get_preferred_direction_is_horizontal(i))
            .collect();
        assert_eq!(directions, vec![true, false, true, false]);

        // Without that one call, the environment's `true,true` reaches layer 1.
        let kept = resolve_headless_steps(
            &inputs,
            Some(&board),
            &host,
            Steps {
                first_board_layer_count: false,
                ..Steps::JAVA
            },
        );
        let directions: Vec<bool> = (0..4)
            .map(|i| kept.get_preferred_direction_is_horizontal(i))
            .collect();
        assert_eq!(directions, vec![true, true, true, false]);
    }

    /// A committed `.rules` fixture — the one the matrix and the `p4t1` differential use —
    /// through the whole pass, so the linear form is pinned against a real file and not only
    /// against bytes written in this module. The board carries the file's own layer names, which
    /// is what the board-structured parse resolves `layer_rule F.Cu`/`B.Cu` against.
    #[test]
    fn a_rules_file_source_drives_the_same_answer() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("data")
            .join("Plan4Matrix-primary.rules");
        let bytes = std::fs::read(&path).expect("committed fixture");
        let host = host();
        let dsn = bare_dsn(2);
        let inputs = SettingsInputs {
            json_file: None,
            dsn: Some(&dsn),
            scheduler_rules: Some(&bytes),
            ..SettingsInputs::default()
        };
        let board = named_board(&["F.Cu".to_string(), "B.Cu".to_string()]);
        let resolved = resolve_headless(&inputs, Some(&board), &host);
        assert_eq!(resolved.get_via_costs(), 40);
        assert_eq!(resolved.get_plane_via_costs(), 4);
        assert_eq!(resolved.get_start_ripup_costs(), 140);
        // `(active off)` on `B.Cu` survives the whole path: `layers` is an object array.
        assert!(!resolved.get_layer_active(1));
    }

    #[test]
    fn scheduler_rules_path_probes_are_injectable() {
        let job = Path::new("/jobs/job.rules");
        let cli = Path::new("/cli/cli.rules");
        let dsn = Path::new("/designs/design.dsn");
        let all = |_: &Path| true;
        let none = |_: &Path| false;

        assert_eq!(
            resolve_scheduler_rules_path_with(Some(job), Some(cli), Some(dsn), none),
            Some(job.to_path_buf()),
            "job.rules is never probed"
        );
        assert_eq!(
            resolve_scheduler_rules_path_with(None, Some(cli), Some(dsn), all),
            Some(cli.to_path_buf())
        );
        assert_eq!(
            resolve_scheduler_rules_path_with(None, None, Some(dsn), all),
            Some(PathBuf::from("/designs/design.rules"))
        );
        assert_eq!(
            resolve_scheduler_rules_path_with(None, None, Some(dsn), none),
            None
        );
    }
}
