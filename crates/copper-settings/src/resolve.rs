//! |---|---|---|
use std::path::{Path, PathBuf};

use copper_board::Board;

use crate::sources::rules_file::apply_rules_file_against_board;
use crate::{HostEnvironment, RouterSettings, SettingsSource, sources::DefaultSettings};

#[derive(Debug, Clone, Copy, Default)]
pub struct SettingsInputs<'a> {
    pub json_file: Option<&'a RouterSettings>,
    pub dsn: Option<&'a RouterSettings>,
    pub cli_rules: Option<&'a [u8]>,
    pub scheduler_rules: Option<&'a [u8]>,
    pub env: Option<&'a RouterSettings>,
    pub cli: Option<&'a RouterSettings>,
}

#[derive(Debug, Clone, Copy)]
struct Steps {
    first_board_layer_count: bool,
    first_board_optimization: bool,
    post_merge_rules_reapply: bool,
    second_validate: bool,
}

impl Steps {
    const JAVA: Self = Self {
        first_board_layer_count: true,
        first_board_optimization: true,
        post_merge_rules_reapply: true,
        second_validate: true,
    };
}

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
    let defaults = DefaultSettings::new(host);
    let mut settings = defaults
        .get_settings()
        .expect("DefaultSettings always has settings")
        .duplicate();

    if let Some(json_file) = inputs.json_file {
        settings.apply_new_values_from(json_file);
    }
    if let Some(dsn) = inputs.dsn {
        settings.apply_new_values_from(dsn);
    }
    if let Some(cli_rules) = parse_rules_file(inputs.cli_rules) {
        settings.apply_new_values_from(&cli_rules);
    }
    if let Some(env) = inputs.env {
        settings.apply_new_values_from(env);
    }
    if let Some(cli) = inputs.cli {
        settings.apply_new_values_from(cli);
    }
    settings.validate(host);

    if let Some(board) = board {
        let board_layer_count = board.get_layer_count();
        if steps.first_board_layer_count && settings.get_layer_count() != board_layer_count {
            settings.set_layer_count(board_layer_count);
        }
        if steps.first_board_optimization {
            settings.apply_board_specific_optimizations(board);
        }
    }

    settings.board_specific_trace_costs_applied = None;

    if let Some(json_file) = inputs.json_file {
        settings.fill_absent_from(json_file);
    }
    if let Some(scheduler_rules) = parse_rules_file(inputs.scheduler_rules) {
        settings.fill_absent_from(&scheduler_rules);
    }
    if steps.second_validate {
        settings.validate(host);
    }

    if let (true, Some(bytes), Some(board)) = (
        steps.post_merge_rules_reapply,
        inputs.scheduler_rules,
        board,
    ) {
        apply_rules_file_against_board(bytes, board, &mut settings);
    }

    if let Some(board) = board {
        settings.apply_board_specific_optimizations(board);
    }

    settings
}

fn parse_rules_file(bytes: Option<&[u8]>) -> Option<RouterSettings> {
    match copper_dsn::rules_reader::read_router_settings(bytes?) {
        Ok(Some(parsed)) => Some(RouterSettings::from(parsed)),
        Ok(None) | Err(_) => None,
    }
}

#[must_use]
pub fn resolve_scheduler_rules_path(
    job_rules: Option<&Path>,
    cli_rules: Option<&Path>,
    dsn_path: Option<&Path>,
) -> Option<PathBuf> {
    resolve_scheduler_rules_path_with(job_rules, cli_rules, dsn_path, |path| path.exists())
}

#[must_use]
pub fn resolve_scheduler_rules_path_with(
    job_rules: Option<&Path>,
    cli_rules: Option<&Path>,
    dsn_path: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    if let Some(job_rules) = job_rules {
        return Some(job_rules.to_path_buf());
    }
    if let Some(cli_rules) = cli_rules {
        return exists(cli_rules).then(|| cli_rules.to_path_buf());
    }
    let dsn_path = dsn_path?;
    let file_name = dsn_path.file_name()?.to_string_lossy().into_owned();
    let base_name = match file_name.rfind('.') {
        Some(dot) if dot > 0 => &file_name[..dot],
        _ => &file_name[..],
    };
    let adjacent = dsn_path.with_file_name(format!("{base_name}.rules"));
    exists(&adjacent).then_some(adjacent)
}

#[must_use]
pub fn resolve_kicad_project_path(
    explicit: Option<&Path>,
    pcb_path: Option<&Path>,
) -> Option<PathBuf> {
    resolve_kicad_project_path_with(explicit, pcb_path, |path| path.exists())
}

#[must_use]
pub fn resolve_kicad_project_path_with(
    explicit: Option<&Path>,
    pcb_path: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    if let Some(explicit) = explicit {
        return Some(explicit.to_path_buf());
    }
    let pcb_path = pcb_path?;
    let file_name = pcb_path.file_name()?.to_string_lossy().into_owned();
    let base_name = match file_name.rfind('.') {
        Some(dot) if dot > 0 => &file_name[..dot],
        _ => &file_name[..],
    };
    let adjacent = pcb_path.with_file_name(format!("{base_name}.kicad_pro"));
    exists(&adjacent).then_some(adjacent)
}

#[cfg(test)]
mod tests {
    use copper_board::prelude::*;
    use copper_geometry::{IntBox, PolylineShapeRef, TileShape};

    use super::*;
    use crate::sources::{CliSettings, EnvironmentVariablesSource};

    fn host() -> HostEnvironment {
        HostEnvironment::with_processors(4)
    }

    fn board(layer_count: usize) -> Board {
        named_board(
            &(0..layer_count)
                .map(|i| format!("L{i}"))
                .collect::<Vec<_>>(),
        )
    }

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

    fn bare_dsn(layer_count: usize) -> RouterSettings {
        let mut settings = RouterSettings::new();
        settings.set_layer_count(layer_count);
        settings
    }

    fn rules_bytes() -> Vec<u8> {
        b"(rules PCB unit\n  (autoroute_settings\n    (vias on)\n    (via_costs 99)\n    (layer_rule L0\n      (active on)\n      (preferred_direction horizontal)\n    )\n  )\n)\n"
            .to_vec()
    }

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
        assert!(without_reapply.get_preferred_direction_is_horizontal(0));
        assert!(!without_reapply.get_preferred_direction_is_horizontal(1));
    }

    #[test]
    fn a_rules_file_is_parsed_twice_against_two_layer_structures() {
        let bytes = b"(rules PCB unit\n  (autoroute_settings\n    (layer_rule L0\n      (active on)\n      (preferred_direction horizontal)\n    )\n    (layer_rule L3\n      (active off)\n      (preferred_direction horizontal)\n    )\n  )\n)\n";

        let discovered = parse_rules_file(Some(bytes)).expect("the scope parses");
        assert_eq!(discovered.get_layer_count(), 2);
        assert!(!discovered.get_layer_active(1), "`L3` landed at index 1");

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

    fn env_and_cli() -> (EnvironmentVariablesSource, CliSettings) {
        let cli = CliSettings::new(&[
            "--router.max_passes=88".to_string(),
            "--router.optimizer.enabled=false".to_string(),
        ]);
        let env = EnvironmentVariablesSource::new(
            &[
                (
                    "COPPERROUTE__ROUTER__MAX_PASSES".to_string(),
                    "77".to_string(),
                ),
                (
                    "COPPERROUTE__ROUTER__LAYERS__PREFERRED_DIRECTION_HORIZONTAL".to_string(),
                    "true,true".to_string(),
                ),
            ]
            .into_iter()
            .collect(),
        );
        (env, cli)
    }

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

        let resolved = resolve_headless(&inputs, Some(&board), &host);
        let directions: Vec<bool> = (0..4)
            .map(|i| resolved.get_preferred_direction_is_horizontal(i))
            .collect();
        assert_eq!(directions, vec![true, false, true, false]);

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

    #[test]
    fn kicad_project_path_probes_are_injectable() {
        let explicit = Path::new("/cli/other.kicad_pro");
        let pcb = Path::new("/boards/board.kicad_pcb");
        let all = |_: &Path| true;
        let none = |_: &Path| false;

        assert_eq!(
            resolve_kicad_project_path_with(Some(explicit), Some(pcb), none),
            Some(explicit.to_path_buf()),
            "an explicit path is never probed for existence"
        );
        assert_eq!(
            resolve_kicad_project_path_with(None, Some(pcb), all),
            Some(PathBuf::from("/boards/board.kicad_pro"))
        );
        assert_eq!(resolve_kicad_project_path_with(None, Some(pcb), none), None);
        assert_eq!(resolve_kicad_project_path_with(None, None, all), None);
    }
}
