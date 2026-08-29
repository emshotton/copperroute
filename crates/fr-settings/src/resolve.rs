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
//! | 7 | `:172-181` → `RulesReader.java:153-157` | re-parse the `.rules` bytes and `applyNewValuesFrom` them onto the **merged** object |
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
//! s.apply(cli_rules)                                   // 40   ( -dr / -de …rules only )
//! s.apply(env)                                         // 55
//! s.apply(cli)                                         // 60
//! s.validate()                                         // merge #1's validate, == the priority-70 payload
//! s.set_layer_count(board) if it disagrees             // HeadlessBoardManager.java:741-744
//! s.apply_board_specific_optimizations(board)          // HeadlessBoardManager.java:745
//! s.board_specific_trace_costs_applied = None          // the private flag never survives merge #2
//! s.fill_absent_from(scheduler_rules)                  // merge #2's own 0..60 chain, all of it
//! s.validate()                                         // merge #2's validate
//! s.apply(scheduler_rules)                             // RulesReader.java:153-157
//! s.apply_board_specific_optimizations(board)          // RoutingJobScheduler.java:186
//! ```
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
//!    call (`RouterSettings.java:936-940`) and `9999` on the second, because `MAX_VALUE > 9999`.
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
//! board never reaches `RoutingJobScheduler.java:172` or `:186` at all — `:186` re-derives both
//! arrays from the board and the disagreement is gone. `tests/precedence.rs`'s
//! `a_split_rules_pair_restarts_the_cost_array_race` pins both halves of that.

use std::path::{Path, PathBuf};

use fr_board::Board;

use crate::{HostEnvironment, RouterSettings, SettingsSource, sources::DefaultSettings};

/// Everything the headless path can feed the merge. Every field is optional; `None` means the
/// corresponding Java source was never registered.
#[derive(Debug, Clone, Copy, Default)]
pub struct SettingsInputs<'a> {
    /// `DsnFileSettings`, priority 20 (`Freerouting.java:126-127`,
    /// `RoutingJobScheduler.java:111-115`). `None` is a non-DSN input, where the scheduler
    /// registers no DSN source at all.
    pub dsn: Option<&'a RouterSettings>,
    /// `RulesFileSettings` from `-dr` / `-de …rules`, priority 40 — **merge #1 only**
    /// (`Freerouting.java:129-136`).
    pub cli_rules: Option<&'a RouterSettings>,
    /// The `.rules` the scheduler resolved (`job.rules ?? -dr ?? adjacent <design>.rules`,
    /// `RoutingJobScheduler.java:118-152` — see [`resolve_scheduler_rules_path`]). It reaches the
    /// result twice: at priority 40 in merge #2, and again through the post-merge re-apply at
    /// `RulesReader.java:153-157`.
    pub scheduler_rules: Option<&'a RouterSettings>,
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
/// `board` is `None` for "there is no board" — the merge without the board tuning. Java reaches
/// that shape nowhere in the headless path, and its three sites disagree about what it would
/// mean: `HeadlessBoardManager.java:740` guards on `board != null` and skips its pass,
/// `RoutingJobScheduler.java:172` guards the post-merge `.rules` re-apply on
/// `rulesData != null && job.board != null`, and `:186` guards nothing at all — a null board
/// there is an `NPE` at `RouterSettings.java:267`. This port skips both board passes and **keeps**
/// the re-apply: `scheduler_rules` arrives already parsed, so there is nothing board-shaped left
/// in it, and dropping a whole rules tier is the more surprising of the two readings. See the
/// module docs for the one input family where the board-less answer then differs from Java's two
/// merges.
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
    // priority order. `JsonFileSettings(10)` is out of scope (spec §2) and is a no-op when the
    // file is absent, which is the only shape the headless path has.
    if let Some(dsn) = inputs.dsn {
        settings.apply_new_values_from(dsn); // 20
    }
    if let Some(cli_rules) = inputs.cli_rules {
        settings.apply_new_values_from(cli_rules); // 40
    }
    if let Some(env) = inputs.env {
        settings.apply_new_values_from(env); // 55
    }
    if let Some(cli) = inputs.cli {
        settings.apply_new_values_from(cli); // 60
    }
    // `SettingsMerger.java:189`. This is the object `RoutingJobScheduler.java:163-166` re-injects
    // at priority 70.
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
    if let Some(scheduler_rules) = inputs.scheduler_rules {
        settings.fill_absent_from(scheduler_rules);
    }
    if steps.second_validate {
        settings.validate(host); // `SettingsMerger.java:189`, again.
    }

    // --- the post-merge re-apply (`:172-181` → `RulesReader.java:153-157`) --------------------
    // `targetSettings.applyNewValuesFrom(parsedSettings)` on the already-merged object: this is
    // what puts `(autoroute_settings)` above the environment and the command line (quirk Q2).
    if let (true, Some(scheduler_rules)) = (steps.post_merge_rules_reapply, inputs.scheduler_rules)
    {
        settings.apply_new_values_from(scheduler_rules);
    }

    // --- `RoutingJobScheduler.java:186` -------------------------------------------------------
    // `applyBoardSpecificOptimizations`, not the `IfNeeded` variant: unconditional.
    if let Some(board) = board {
        settings.apply_board_specific_optimizations(board);
    }

    settings
}

/// How the scheduler picks the `.rules` file that feeds merge #2 and the post-merge re-apply
/// (`RoutingJobScheduler.java:118-152`): the job's own rules, else `-dr`, else an adjacent
/// `<design>.rules` beside the DSN.
///
/// The `else if` chain matters: a `-dr` that names a **missing** file does not fall through to
/// the adjacent probe (`:122-131` — the branch is taken on `initialRulesFile != null`, and the
/// inner `rf.exists()` only decides whether the bytes are read), so a typo'd `-dr` silently
/// disables the adjacent file a user would otherwise have got.
///
/// `job_rules` is not probed for existence: Java tests `job.rules.getData() != null` (`:118`),
/// i.e. bytes that are already in hand. The other two are `File.exists()` calls
/// (`RoutingJobScheduler.java:124`, `:150`), reproduced here as `Path::exists`; the injectable
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
    // :118-121 — the job arrived with its own rules bytes.
    if let Some(job_rules) = job_rules {
        return Some(job_rules.to_path_buf());
    }
    // :122-131 — `globalSettings.initialRulesFile`, read only when it exists.
    if let Some(cli_rules) = cli_rules {
        return exists(cli_rules).then(|| cli_rules.to_path_buf());
    }
    // :131-151 — `new File(job.input.getDirectoryPath(), baseName + ".rules")`, gated on
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
    use crate::sources::{CliSettings, EnvironmentVariablesSource, RulesFileSettings};

    fn host() -> HostEnvironment {
        HostEnvironment::with_processors(4)
    }

    /// `BProbe.java`'s synthetic board: an `IntBox` outline that is also the bounding box, and a
    /// stack of signal layers.
    fn board(layer_count: usize) -> Board {
        let layers = LayerStructure::new(
            (0..layer_count)
                .map(|i| Layer::new(format!("L{i}"), true))
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

    /// A `.rules` file's `(autoroute_settings)` block, built the way
    /// `AutorouteSettings.readScope` builds one.
    fn rules(layer_count: usize) -> RouterSettings {
        let mut settings = RouterSettings::new();
        settings.set_layer_count(layer_count);
        settings.set_via_costs(99);
        settings.set_preferred_direction_is_horizontal(0, true);
        settings
    }

    /// Ruling 1's Q1 channel, and the correction to survey Q1: with no rules in merge #1, the
    /// scheduler's `.rules` reaches `layers[0].preferredDirectionHorizontal` — a field merge #1
    /// left `null` — through `fill_absent_from`, while its `viaCosts` reaches the answer only
    /// through the post-merge re-apply, because `DefaultSettings` already wrote 50 there.
    ///
    /// Turning the re-apply off is what tells the two channels apart: the direction still lands,
    /// `viaCosts` falls back to `DefaultSettings`' 50.
    ///
    /// The board is `None` here on purpose: with a board, the first board pass
    /// (`HeadlessBoardManager.java:745`) fills every `preferredDirectionHorizontal` before
    /// merge #2 runs, so the `.rules` direction arrives through the re-apply instead — which
    /// `the_first_board_pass_closes_the_direction_channel` pins separately.
    #[test]
    fn adjacent_rules_reach_only_the_fields_merge_one_left_null() {
        let host = host();
        let dsn = bare_dsn(2);
        let rules = rules(2);
        let inputs = SettingsInputs {
            dsn: Some(&dsn),
            scheduler_rules: Some(&rules),
            ..SettingsInputs::default()
        };

        let resolved = resolve_headless(&inputs, None, &host);
        assert!(resolved.get_preferred_direction_is_horizontal(0));
        assert_eq!(resolved.get_via_costs(), 99);

        let without_reapply = resolve_headless_steps(
            &inputs,
            None,
            &host,
            Steps {
                post_merge_rules_reapply: false,
                ..Steps::JAVA
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

    /// The other half of the same story: once there *is* a board, the first board pass fills
    /// `preferredDirectionHorizontal` on every layer (`RouterSettings.java:361-363`), so the
    /// priority-70 payload is no longer null there and the `.rules` direction can only arrive
    /// through the post-merge re-apply. The end result is the same either way — which is why
    /// nobody noticed `:745` was missing from the plan's step list.
    #[test]
    fn the_first_board_pass_closes_the_direction_channel() {
        let host = host();
        let board = board(2);
        let dsn = bare_dsn(2);
        let rules = rules(2);
        let inputs = SettingsInputs {
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
        let rules_2 = rules(2);
        let rules_4 = rules(4);

        for layer_count in [2usize, 4] {
            let board = board(layer_count);
            let dsn = bare_dsn(layer_count);
            let scheduler_rules = if layer_count == 2 { &rules_2 } else { &rules_4 };
            for (dsn_input, cli_rules) in [
                (None, None),
                (Some(&dsn), None),
                (Some(&dsn), Some(scheduler_rules)),
            ] {
                let inputs = SettingsInputs {
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

    /// `RulesFileSettings` is what feeds `scheduler_rules` in the real path; this pins that the
    /// linear form is happy with the object that source produces, not only with a hand-built one.
    #[test]
    fn a_rules_file_source_drives_the_same_answer() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("data")
            .join("Plan4Matrix-primary.rules");
        let source = RulesFileSettings::from_path(&path);
        let host = host();
        let dsn = bare_dsn(2);
        let inputs = SettingsInputs {
            dsn: Some(&dsn),
            scheduler_rules: source.get_settings(),
            ..SettingsInputs::default()
        };
        let resolved = resolve_headless(&inputs, Some(&board(2)), &host);
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
