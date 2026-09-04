//! that disables a step is `#[cfg(test)]` and deliberately not public API.
mod matrix;

use fr_board::Board;
use fr_settings::prelude::*;

fn host() -> HostEnvironment {
    HostEnvironment::with_processors(4)
}

fn boxed<S: SettingsSource + 'static>(source: S) -> Box<dyn SettingsSource> {
    Box::new(source)
}


fn two_merge_form(
    case: &matrix::Case,
    board: Option<&Board>,
    host: &HostEnvironment,
    with_ses: bool,
) -> RouterSettings {
    let dsn = matrix::dsn_source(case.dsn);
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

    if let Some(board) = board {
        if merged1.get_layer_count() != board.get_layer_count() {
            merged1.set_layer_count(board.get_layer_count());
        }
        merged1.apply_board_specific_optimizations(board);
    }

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
    sources.push(boxed(ApiSettings::new(Some(merged1))));
    let mut merged2 = SettingsMerger::new(sources).merge(host);

    if let (Some(bytes), Some(board)) = (&scheduler_rules_bytes, board) {
        apply_rules_file_against_board(bytes, board, &mut merged2);
    }

    if let Some(board) = board {
        merged2.apply_board_specific_optimizations(board);
    }
    merged2
}

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
        json_file: None,
        dsn: dsn.as_ref().and_then(SettingsSource::get_settings),
        cli_rules: cli_rules.as_deref(),
        scheduler_rules: scheduler_rules.as_deref(),
        env: env.get_settings(),
        cli: cli.get_settings(),
    };
    resolve_headless(&inputs, board, host)
}

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

const SPLIT_RULES_RACE: [&str; 4] = [
    "dsn-none/rules-split/env-none/cli-none",
    "dsn-none/rules-split/env-none/cli-set",
    "dsn-none/rules-split/env-set/cli-none",
    "dsn-none/rules-split/env-set/cli-set",
];

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
    assert_eq!(costs(&two), Some(vec![6.5, 1.0]));
    assert_eq!(costs(&linear), Some(vec![2.5, 4.5]));

    let board = matrix::board(case.dsn);
    assert_eq!(
        two_merge_form(&case, Some(&board), &host, false),
        linear_form(&case, Some(&board), &host)
    );
}

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

fn matrix_rules(name: &str) -> Vec<u8> {
    std::fs::read(matrix::data_path(name)).expect("committed fixture")
}

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
        json_file: None,
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

#[test]
fn rules_per_layer_trace_costs_are_discarded_in_the_headless_path() {
    if !parity::require_java_dir() {
        return;
    }
    let host = host();
    let case = matrix::Case {
        id: "q9".to_string(),
        dsn: &matrix::DSN_CASES[1], 
        rules: &matrix::RULES_CASES[2], 
        env: &matrix::ENV_CASES[0],
        cli: &matrix::CLI_CASES[0],
    };
    let board = matrix::board(case.dsn);
    let resolved = linear_form(&case, Some(&board), &host);

    assert_eq!(resolved.get_preferred_direction_trace_costs(0), 1.0);
    assert!(!resolved.get_preferred_direction_is_horizontal(0));
    assert_eq!(resolved, two_merge_form(&case, Some(&board), &host, false));
}

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

#[test]
#[should_panic(expected = "RouterSettings.java:934")]
fn merge_with_no_default_source_panics_in_validate() {
    let argv = vec!["--router.enabled=true".to_string()];
    let sources: Vec<Box<dyn SettingsSource>> = vec![boxed(CliSettings::new(&argv))];
    let _ = SettingsMerger::new(sources).merge(&host());
}

#[test]
fn the_second_validate_is_not_idempotent_for_max_passes_zero() {
    let host = host();
    let dsn = dsn_settings(2, 10);
    let cli = cli_settings(&["--router.max_passes=0"]);
    let inputs = SettingsInputs {
        json_file: None,
        dsn: Some(&dsn),
        cli: cli.get_settings(),
        ..SettingsInputs::default()
    };

    let one_merge = SettingsMerger::new(vec![
        boxed(DefaultSettings::new(&host)),
        boxed(cli_settings(&["--router.max_passes=0"])),
    ])
    .merge(&host);
    assert_eq!(one_merge.max_passes, Some(i32::MAX));

    assert_eq!(
        resolve_headless(&inputs, None, &host).max_passes,
        Some(9999)
    );
}

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
            json_file: None,
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

    assert_eq!(
        resolve_scheduler_rules_path(Some(&job), Some(&cli), Some(&dsn)),
        Some(job.clone())
    );
    assert_eq!(
        resolve_scheduler_rules_path(None, Some(&cli), Some(&dsn)),
        Some(cli.clone())
    );
    assert_eq!(
        resolve_scheduler_rules_path(None, Some(&missing), Some(&dsn)),
        None
    );
    assert_eq!(
        resolve_scheduler_rules_path(None, None, Some(&dsn)),
        Some(adjacent)
    );
    assert_eq!(resolve_scheduler_rules_path(None, None, None), None);
}

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
