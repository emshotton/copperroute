//! Java's real headless settings precedence, proved twice: the literal two-merge shape it runs
//! (`Freerouting.java:125-146`, `management/HeadlessBoardManager.java:739-748`,
//! `management/jobs/RoutingJobScheduler.java:103-186`, `io/specctra/RulesReader.java:153-157`)
//! and the single linear pass [`fr_settings::resolve_headless`] the port uses instead
//! (plan ruling 1).
//!
//! # (a) The equivalence matrix
//!
//! [`matrix`] is the table; [`two_merge_form`] below is the literal transcription of the Java
//! call sequence, built from the real [`SettingsMerger`] over the real sources. Every case runs
//! both and asserts field-for-field equality. This is the Rust half of ruling 1's proof; Task 9's
//! `p4t1` drives the same table against the JVM.
//!
//! # (b) The precedence assertions
//!
//! One test per quirk, each named after it. Two of them — the ones that need the merge's steps
//! turned off one at a time — live in `src/resolve.rs`'s unit tests instead, because the switch
//! that disables a step is `#[cfg(test)]` and deliberately not public API.
//!
//! # Java-wins corrections to the task brief (details in the Task 8 report)
//!
//! 1. **The headless path runs `applyBoardSpecificOptimizations` *twice*, not once.**
//!    `HeadlessBoardManager.applyRouterSettingsForLoadedBoard` (`:739-748`) fires between the two
//!    merges — the brief and ruling 1 carry only its `setLayerCount` half (`:741-744`) and omit
//!    the `applyBoardSpecificOptimizations(board)` at `:745`. Both forms here run both halves;
//!    `src/resolve.rs`'s `the_first_board_pass_leaves_no_trace_in_the_final_result` pins why the
//!    omission was harmless.
//! 2. **`validate()` is not idempotent.** The brief says the second call is; it is not, for
//!    `max_passes = 0` — see `the_second_validate_is_not_idempotent_for_max_passes_zero`.
//! 3. **36 of the 64 cross-product cases are reachable from the CLI-started path**, 8 more only
//!    up to an equivalence (`dsn-none` models a KiCad input, whose merge-#1 `DsnFileSettings` is
//!    a no-op rather than absent), and the other 20 are deliberate extra coverage — so the matrix
//!    runs all 64 rather than the brief's 40. The argument, and the arithmetic, are in
//!    [`matrix`]'s module docs.
//! 4. **One `.rules` file, two parses.** Java reads the scheduler's `.rules` twice — once against
//!    the layer structure discovered from the file (`RulesReader.java:198`, priority 40) and once
//!    against the **board's** (`:112`, the post-merge re-apply). Both forms below therefore
//!    consume the same raw **bytes** and parse them twice, which is what
//!    [`resolve_headless`] does internally (quirk #142). Feeding one pre-parsed `RouterSettings`
//!    for both steps disagreed with the JVM on 13 of `p4t1`'s 84 rows (Task 8 fix round 2,
//!    controller ruling N).

mod matrix;

use fr_board::Board;
use fr_settings::prelude::*;

fn host() -> HostEnvironment {
    HostEnvironment::with_processors(4)
}

fn boxed<S: SettingsSource + 'static>(source: S) -> Box<dyn SettingsSource> {
    Box::new(source)
}

// ---------------------------------------------------------------------------------------------
// (a) the equivalence matrix
// ---------------------------------------------------------------------------------------------

/// Java's two merges, transcribed call for call.
///
/// `with_ses` adds a [`SesFileSettings`] at priority 30 to both mergers — the tier spec §11 lists
/// and Java never registers (`ses_tier_is_a_no_op`).
fn two_merge_form(
    case: &matrix::Case,
    board: Option<&Board>,
    host: &HostEnvironment,
    with_ses: bool,
) -> RouterSettings {
    let dsn = matrix::dsn_source(case.dsn);
    // One set of bytes per rules slot, exactly as Java holds them: `routingJob.rules.getData()`
    // for merge #1 (`Freerouting.java:131-135`) and `rulesData` for merge #2 and the post-merge
    // re-apply (`RoutingJobScheduler.java:118-181`). Both forms parse *these* bytes.
    let cli_rules_bytes = matrix::rules_bytes(case.rules.cli_rules);
    let scheduler_rules_bytes = matrix::rules_bytes(case.rules.scheduler_rules);
    let cli_rules = cli_rules_bytes
        .as_ref()
        .map(|bytes| RulesFileSettings::new(&bytes[..], "cli.rules"));
    let scheduler_rules = scheduler_rules_bytes
        .as_ref()
        .map(|bytes| RulesFileSettings::new(&bytes[..], "scheduler.rules"));
    let env = matrix::env_source(case.env);
    let cli = matrix::cli_source(case.cli);

    // --- merge #1: `Freerouting.java:125-146` -------------------------------------------------
    // The prototype merger (`Freerouting.java:1408-1413`) is `DefaultSettings`,
    // `JsonFileSettings` (ported in Plan 8 Task 5, not yet fed into `resolve_headless` — see the
    // `// obligation:` in `resolve.rs`), `CliSettings` and `EnvironmentVariablesSource`;
    // `:126-127` adds the DSN and `:129-136` the `-dr` rules. Ruling K: `SettingsMerger.clone` is
    // not ported, so the second merger is rebuilt from the same source list.
    let mut sources = vec![boxed(DefaultSettings::new(host))];
    if let Some(dsn) = dsn.clone() {
        sources.push(boxed(dsn));
    }
    if let Some(rules) = cli_rules {
        sources.push(boxed(rules));
    }
    if with_ses {
        sources.push(boxed(SesFileSettings::new("matrix.ses")));
    }
    sources.push(boxed(cli.clone()));
    sources.push(boxed(env.clone()));
    let mut merged1 = SettingsMerger::new(sources).merge(host);

    // --- `HeadlessBoardManager.applyRouterSettingsForLoadedBoard` (`:739-748`) ----------------
    // Runs on merge #1's result, before merge #2, because `RoutingJobScheduler.java:93-96`
    // constructs the `HeadlessBoardManager` around the job whose `routerSettings` merge #1 just
    // set (`Freerouting.java:146`).
    if let Some(board) = board {
        if merged1.get_layer_count() != board.get_layer_count() {
            merged1.set_layer_count(board.get_layer_count());
        }
        merged1.apply_board_specific_optimizations(board);
    }

    // --- merge #2: `RoutingJobScheduler.java:103-170` -----------------------------------------
    let mut sources = vec![boxed(DefaultSettings::new(host))];
    if let Some(dsn) = dsn {
        sources.push(boxed(dsn));
    }
    if let Some(rules) = scheduler_rules.clone() {
        sources.push(boxed(rules));
    }
    if with_ses {
        sources.push(boxed(SesFileSettings::new("matrix.ses")));
    }
    sources.push(boxed(cli));
    sources.push(boxed(env));
    // `:163-166` — the whole result of merge #1, at priority 70.
    sources.push(boxed(ApiSettings::new(Some(merged1))));
    let mut merged2 = SettingsMerger::new(sources).merge(host);

    // --- the post-merge rules re-apply: `:172-181` -> `RulesReader.java:153-157` --------------
    // `RulesReader.read(new ByteArrayInputStream(rulesData), designName, job.board,
    // job.routerSettings)`: the **second** parse of the same bytes, against the board's layer
    // structure (`RulesReader.java:112`) rather than the file's own names — see quirk #142. The
    // guard is Java's `rulesData != null && job.board != null` (`:172`), both halves.
    if let (Some(bytes), Some(board)) = (&scheduler_rules_bytes, board) {
        apply_rules_file_against_board(bytes, board, &mut merged2);
    }

    // --- `:186` ------------------------------------------------------------------------------
    if let Some(board) = board {
        merged2.apply_board_specific_optimizations(board);
    }
    merged2
}

/// The same case through [`resolve_headless`].
fn linear_form(
    case: &matrix::Case,
    board: Option<&Board>,
    host: &HostEnvironment,
) -> RouterSettings {
    let dsn = matrix::dsn_source(case.dsn);
    let cli_rules = matrix::rules_bytes(case.rules.cli_rules);
    let scheduler_rules = matrix::rules_bytes(case.rules.scheduler_rules);
    let env = matrix::env_source(case.env);
    let cli = matrix::cli_source(case.cli);

    let inputs = SettingsInputs {
        dsn: dsn.as_ref().and_then(SettingsSource::get_settings),
        cli_rules: cli_rules.as_deref(),
        scheduler_rules: scheduler_rules.as_deref(),
        env: env.get_settings(),
        cli: cli.get_settings(),
    };
    resolve_headless(&inputs, board, host)
}

/// Ruling 1's Rust-side proof: the linear pass and Java's two merges answer the same thing for
/// every one of the 64 cases.
#[test]
fn the_two_forms_agree_over_the_whole_matrix() {
    if !parity::require_java_dir() {
        return;
    }
    let host = host();
    let cases = matrix::cases();
    assert_eq!(cases.len(), 64, "the matrix is the full cross product");

    for case in &cases {
        let board = matrix::board(case.dsn);
        let two = two_merge_form(case, Some(&board), &host, false);
        let linear = linear_form(case, Some(&board), &host);
        assert_eq!(two, linear, "case {}", case.id);
    }
}

/// The merge on its own, with no board to wash the answer: with both board passes skipped the two
/// forms still agree — **except** for the one family where merge #1 and merge #2 see *different*
/// `.rules` files and there is no DSN source, which `a_split_rules_pair_restarts_the_cost_array_race`
/// covers.
///
/// This is the sharper half of the equivalence: `applyBoardSpecificOptimizations`
/// (`RoutingJobScheduler.java:186`) re-derives `layers` and both cost arrays from the board at the
/// end, so the board-full comparison above cannot see a difference in what the *merge* produced.
///
/// With no board, neither form runs the post-merge `.rules` re-apply either — Java guards it on
/// `job.board != null` (`RoutingJobScheduler.java:173`) and its parse needs the board's layer
/// structure (quirk #142) — so what this compares is the two merges and nothing else.
#[test]
fn the_merge_alone_agrees_without_a_board() {
    if !parity::require_java_dir() {
        return;
    }
    let host = host();
    for case in &matrix::cases() {
        if SPLIT_RULES_RACE.contains(&case.id.as_str()) {
            continue;
        }
        let two = two_merge_form(case, None, &host, false);
        let linear = linear_form(case, None, &host);
        assert_eq!(two, linear, "case {}", case.id);
    }
}

/// The four cases the board-less comparison excludes, and the only inputs anywhere in the matrix
/// where the linear form and Java's two merges can disagree.
const SPLIT_RULES_RACE: [&str; 4] = [
    "dsn-none/rules-split/env-none/cli-none",
    "dsn-none/rules-split/env-none/cli-set",
    "dsn-none/rules-split/env-set/cli-none",
    "dsn-none/rules-split/env-set/cli-set",
];

/// The boundary of ruling 1's linearisation, pinned so nobody has to rediscover it.
///
/// `copyFields` rule 5 (`ReflectionUtil.java:269-290`) is first-writer-wins for primitive arrays.
/// Merge #2 starts from a *fresh* `DefaultSettings.clone()`, so it re-runs that race — and when
/// merge #1's rules (`-dr`) and merge #2's rules (`job.rules`) are different files **and** no DSN
/// source seeded the arrays at priority 20, the two merges pick different winners:
/// merge #1's `-dr` file, merge #2's `job.rules`. The linear form carries merge #1's answer
/// forward and cannot un-write it.
///
/// It is unreachable in Java: this only shows up with **no board**, and a job with no board never
/// reaches `RoutingJobScheduler.java:173` or `:186` at all — with a board, `:186` re-initialises
/// both arrays from the board and the difference disappears, which is what
/// `the_two_forms_agree_over_the_whole_matrix` shows for these same four cases.
#[test]
fn a_split_rules_pair_restarts_the_cost_array_race() {
    if !parity::require_java_dir() {
        return;
    }
    let host = host();
    let case = matrix::cases()
        .into_iter()
        .find(|case| case.id == SPLIT_RULES_RACE[0])
        .expect("the matrix contains it");

    let two = two_merge_form(&case, None, &host, false);
    let linear = linear_form(&case, None, &host);
    let costs = |settings: &RouterSettings| {
        settings
            .scoring
            .as_ref()
            .and_then(|scoring| scoring.preferred_direction_trace_cost.clone())
    };
    // merge #2's own chain wrote the scheduler's file (`Plan4Matrix-adjacent.rules`) ...
    assert_eq!(costs(&two), Some(vec![6.5, 1.0]));
    // ... while the linear form still carries merge #1's `-dr` file
    // (`Plan4Matrix-primary.rules`).
    assert_eq!(costs(&linear), Some(vec![2.5, 4.5]));

    // With the board Java always has, the difference is gone.
    let board = matrix::board(case.dsn);
    assert_eq!(
        two_merge_form(&case, Some(&board), &host, false),
        linear_form(&case, Some(&board), &host)
    );
}

/// Q3 (`docs/java-quirks.md` #130): `SesFileSettings.getSettings` returns
/// `new RouterSettings()` unconditionally (`SesFileSettings.java:27-36`), so registering it
/// at priority 30 in either merge changes
/// nothing — the spec's SES tier has no behaviour to port.
#[test]
fn ses_tier_is_a_no_op() {
    if !parity::require_java_dir() {
        return;
    }
    let host = host();
    for case in &matrix::cases() {
        let board = matrix::board(case.dsn);
        let without = two_merge_form(case, Some(&board), &host, false);
        let with = two_merge_form(case, Some(&board), &host, true);
        assert_eq!(without, with, "case {}", case.id);
    }
}

// ---------------------------------------------------------------------------------------------
// (b) the precedence assertions, one per quirk
// ---------------------------------------------------------------------------------------------

/// A DSN source's settings: what `DsnFileSettings` produces for a file whose
/// `(autoroute_settings)` block named `layer_rule`s and a `(via_costs …)`.
fn dsn_settings(layer_count: usize, via_costs: i32) -> RouterSettings {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(layer_count);
    settings.set_via_costs(via_costs);
    settings
}

fn env_settings(vars: &[(&str, &str)]) -> EnvironmentVariablesSource {
    let map = vars
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect();
    EnvironmentVariablesSource::new(&map)
}

fn cli_settings(argv: &[&str]) -> CliSettings {
    let argv: Vec<String> = argv.iter().map(|arg| (*arg).to_string()).collect();
    CliSettings::new(&argv)
}

/// The bytes of a committed `.rules` fixture — what `resolve_headless` takes, because it parses
/// the file twice itself (quirk #142).
fn matrix_rules(name: &str) -> Vec<u8> {
    std::fs::read(matrix::data_path(name)).expect("committed fixture")
}

/// Q2 (`docs/java-quirks.md` #142): `RulesReader.read` re-applies the `.rules` file's
/// `(autoroute_settings)` block to the **already merged** settings
/// (`RoutingJobScheduler.java:173-184` →
/// `RulesReader.java:153-157`), so for every field that block carries the `.rules` file outranks
/// the environment (55) and the command line (60) — the reverse of the priority ladder.
///
/// The DSN gives `via_costs = 10`, the environment 20, the CLI 30 and the `.rules` file 40; the
/// answer is 40. `max_passes`, which `(autoroute_settings)` cannot express, keeps the CLI's
/// value — which is what proves the effect is scoped to the block's own fields rather than being
/// a general "rules win" rule.
#[test]
fn rules_outrank_env_and_cli_for_autoroute_fields() {
    let host = host();
    let dsn = dsn_settings(2, 10);
    let env = env_settings(&[
        ("FREEROUTING__ROUTER__SCORING__VIA_COSTS", "20"),
        ("FREEROUTING__ROUTER__MAX_PASSES", "111"),
    ]);
    let cli = cli_settings(&["--router.scoring.via_costs=30", "--router.max_passes=222"]);
    let rules = matrix_rules(matrix::PRIMARY_RULES);

    let inputs = SettingsInputs {
        dsn: Some(&dsn),
        scheduler_rules: Some(&rules),
        env: env.get_settings(),
        cli: cli.get_settings(),
        ..SettingsInputs::default()
    };
    let board = matrix::board(&matrix::DSN_CASES[1]);
    let resolved = resolve_headless(&inputs, Some(&board), &host);

    assert_eq!(resolved.get_via_costs(), 40, "the .rules file wins");
    assert_eq!(
        resolved.max_passes,
        Some(222),
        "a field the block cannot carry keeps the CLI's value"
    );
}

/// Q9 (`docs/java-quirks.md` #127) + Q18 (#128), and the single most surprising behaviour in
/// this plan: **a `.rules` file's explicit per-layer trace costs never reach the router in
/// the headless path.** The whole chain, because
/// anyone reading the assertion will assume a bug:
///
/// 1. The DSN has no `(autoroute_settings)` block, so `DsnFileSettings.java:46-48` calls
///    `setLayerCount(2)`, which seeds `scoring.preferredDirectionTraceCost` with `[1.0, 1.0]`
///    (`RouterSettings.java:466-472`) at priority **20**.
/// 2. `copyFields` rule 5 (`ReflectionUtil.java:269-290`) copies a primitive array only into a
///    target that is null or empty — first writer wins — so the `.rules` file's `[2.5, 4.5]` at
///    priority 40 is dropped, in both merges.
/// 3. The post-merge `RulesReader.read` re-apply (`RulesReader.java:153-157`) is the same
///    `copyFields`, so it is dropped there too.
/// 4. `applyBoardSpecificOptimizations` (`RoutingJobScheduler.java:186`) then re-initialises both
///    arrays from `defaultPreferredDirectionTraceCost`, because `boardSpecificTraceCostsApplied`
///    is `private` and never survived a merge (quirk #127).
///
/// The answer is `1.0`. The per-layer *direction* from the same file does land (`layers` is an
/// object array, which merges element-wise) — which is what makes the loss so easy to miss.
#[test]
fn rules_per_layer_trace_costs_are_discarded_in_the_headless_path() {
    if !parity::require_java_dir() {
        return;
    }
    let host = host();
    let case = matrix::Case {
        id: "q9".to_string(),
        dsn: &matrix::DSN_CASES[1], // 2-layer, no (autoroute_settings) block
        rules: &matrix::RULES_CASES[2], // scheduler-only rules
        env: &matrix::ENV_CASES[0],
        cli: &matrix::CLI_CASES[0],
    };
    let board = matrix::board(case.dsn);
    let resolved = linear_form(&case, Some(&board), &host);

    // The `.rules` file asks for 6.5 on layer 0 (`Plan4Matrix-adjacent.rules`).
    assert_eq!(resolved.get_preferred_direction_trace_costs(0), 1.0);
    // ... while the direction it asks for on the same layer *does* arrive.
    assert!(!resolved.get_preferred_direction_is_horizontal(0));
    // And the two forms agree about it.
    assert_eq!(resolved, two_merge_form(&case, Some(&board), &host, false));
}

/// Q18 (`docs/java-quirks.md` #128) in isolation: the DSN's layer seeding blocks the later
/// cost arrays with no board call anywhere in sight, so `1.0` is not an artefact of
/// `applyBoardSpecificOptimizations` — it is
/// what the merge itself produces.
#[test]
fn dsn_layer_seeding_blocks_every_later_cost_array() {
    if !parity::require_java_dir() {
        return;
    }
    let host = host();
    let case = matrix::Case {
        id: "q18".to_string(),
        dsn: &matrix::DSN_CASES[1],
        rules: &matrix::RULES_CASES[2],
        env: &matrix::ENV_CASES[0],
        cli: &matrix::CLI_CASES[0],
    };
    let resolved = linear_form(&case, None, &host);

    assert_eq!(resolved.get_preferred_direction_trace_costs(0), 1.0);
    assert_eq!(resolved.get_against_preferred_direction_trace_costs(0), 1.0);
}

/// Q5: `SettingsMerger.merge` calls `validate()` unconditionally (`SettingsMerger.java:189`), and
/// `validate` dereferences the unboxed `maxPasses` (`RouterSettings.java:934`). A merger whose
/// lowest-priority source is not `DefaultSettings` therefore throws `NullPointerException` — the
/// panic this port reproduces (quirk #125).
#[test]
#[should_panic(expected = "RouterSettings.java:934")]
fn merge_with_no_default_source_panics_in_validate() {
    let argv = vec!["--router.enabled=true".to_string()];
    let sources: Vec<Box<dyn SettingsSource>> = vec![boxed(CliSettings::new(&argv))];
    let _ = SettingsMerger::new(sources).merge(&host());
}

/// The brief expects `validate()`'s second call — merge #2's (`SettingsMerger.java:189`) — to be
/// idempotent, on the grounds that `.rules` carries none of the three fields it touches. It is
/// not, and the reason has nothing to do with `.rules`: `validate` maps `maxPasses == 0` to
/// `Integer.MAX_VALUE` (`RouterSettings.java:937-940`) and then, on the next call, maps
/// `Integer.MAX_VALUE` — being `> 9999` — to `9999`.
///
/// So `--router.max_passes=0`, the spelling a user reaches for to mean "no pass limit", yields
/// `Integer.MAX_VALUE` from a single merge and **9999** from the headless path's two. Java wins;
/// `docs/java-quirks.md` #140.
#[test]
fn the_second_validate_is_not_idempotent_for_max_passes_zero() {
    let host = host();
    let dsn = dsn_settings(2, 10);
    let cli = cli_settings(&["--router.max_passes=0"]);
    let inputs = SettingsInputs {
        dsn: Some(&dsn),
        cli: cli.get_settings(),
        ..SettingsInputs::default()
    };

    // One merge — what `SettingsMerger.merge` alone answers.
    let one_merge = SettingsMerger::new(vec![
        boxed(DefaultSettings::new(&host)),
        boxed(cli_settings(&["--router.max_passes=0"])),
    ])
    .merge(&host);
    assert_eq!(one_merge.max_passes, Some(i32::MAX));

    // Two merges — what the headless path answers.
    assert_eq!(
        resolve_headless(&inputs, None, &host).max_passes,
        Some(9999)
    );
}

/// The other three fields `validate` touches *are* stable under a second call, which is what
/// makes `max_passes = 0` the only exception rather than one of many.
#[test]
fn the_second_validate_changes_nothing_else() {
    let host = host();
    let dsn = dsn_settings(2, 10);
    for argv in [
        vec!["--router.max_passes=12345"],
        vec!["--router.max_threads=99"],
        vec!["--router.max_threads=-3"],
        vec!["--router.trace_pull_tight_accuracy=0"],
    ] {
        let cli = cli_settings(&argv);
        let inputs = SettingsInputs {
            dsn: Some(&dsn),
            cli: cli.get_settings(),
            ..SettingsInputs::default()
        };
        let mut once = SettingsMerger::new(vec![
            boxed(DefaultSettings::new(&host)),
            boxed(dsn_source_stub(&dsn)),
            boxed(cli_settings(&argv)),
        ])
        .merge(&host);
        let after_first = (
            once.max_passes,
            once.max_threads,
            once.trace_pull_tight_accuracy,
        );
        once.validate(&host);
        assert_eq!(
            after_first,
            (
                once.max_passes,
                once.max_threads,
                once.trace_pull_tight_accuracy
            ),
            "argv {argv:?}"
        );

        let resolved = resolve_headless(&inputs, None, &host);
        assert_eq!(resolved.max_passes, after_first.0, "argv {argv:?}");
        assert_eq!(resolved.max_threads, after_first.1, "argv {argv:?}");
        assert_eq!(
            resolved.trace_pull_tight_accuracy, after_first.2,
            "argv {argv:?}"
        );
    }
}

/// A priority-20 stand-in for `DsnFileSettings` over settings built in the test — the file-backed
/// source has no constructor that takes a `RouterSettings`.
fn dsn_source_stub(settings: &RouterSettings) -> impl SettingsSource + use<> {
    struct Stub(RouterSettings);
    impl SettingsSource for Stub {
        fn get_settings(&self) -> Option<&RouterSettings> {
            Some(&self.0)
        }
        fn get_source_name(&self) -> String {
            "DSN file: stub".to_string()
        }
        fn get_priority(&self) -> i32 {
            priority::DSN_FILE
        }
        fn kind(&self) -> SourceKind {
            SourceKind::DsnFile
        }
    }
    Stub(settings.clone())
}

// ---------------------------------------------------------------------------------------------
// resolve_scheduler_rules_path
// ---------------------------------------------------------------------------------------------

/// `RoutingJobScheduler.java:118-152`: the `else if` chain that picks merge #2's rules file.
#[test]
fn scheduler_rules_path_follows_javas_else_if_chain() {
    let dir = std::env::temp_dir().join("fr-settings-plan4-task8");
    let _ = std::fs::create_dir_all(&dir);
    let job = dir.join("job.rules");
    let cli = dir.join("cli.rules");
    let dsn = dir.join("design.dsn");
    let adjacent = dir.join("design.rules");
    for path in [&job, &cli, &dsn, &adjacent] {
        std::fs::write(path, b"(rules PCB x)").expect("scratch write");
    }
    let missing = dir.join("nope.rules");
    let _ = std::fs::remove_file(&missing);

    // job.rules wins (`:118-121`).
    assert_eq!(
        resolve_scheduler_rules_path(Some(&job), Some(&cli), Some(&dsn)),
        Some(job.clone())
    );
    // then `-dr`, but only when it exists (`:122-131`).
    assert_eq!(
        resolve_scheduler_rules_path(None, Some(&cli), Some(&dsn)),
        Some(cli.clone())
    );
    // a `-dr` that does not exist does **not** fall through to the adjacent probe: the `else if`
    // was already taken (`:122`), and the inner `rf.exists()` only guards the read.
    assert_eq!(
        resolve_scheduler_rules_path(None, Some(&missing), Some(&dsn)),
        None
    );
    // then the adjacent `<design>.rules` (`:131-151`).
    assert_eq!(
        resolve_scheduler_rules_path(None, None, Some(&dsn)),
        Some(adjacent)
    );
    // no DSN input — the probe is `isDsn`-gated.
    assert_eq!(resolve_scheduler_rules_path(None, None, None), None);
}

/// `:133-136`: the extension is stripped with `lastIndexOf('.') > 0`, so a dotfile keeps its
/// whole name and `design.rules` is looked for beside the input, not beside the process.
#[test]
fn adjacent_rules_discovery_strips_the_extension_javas_way() {
    let dir = std::env::temp_dir().join("fr-settings-plan4-task8-dotfile");
    let _ = std::fs::create_dir_all(&dir);
    let dotfile = dir.join(".dsn");
    std::fs::write(&dotfile, b"x").expect("scratch write");
    let dotfile_rules = dir.join(".dsn.rules");
    std::fs::write(&dotfile_rules, b"x").expect("scratch write");

    assert_eq!(
        resolve_scheduler_rules_path(None, None, Some(&dotfile)),
        Some(dotfile_rules)
    );
}
