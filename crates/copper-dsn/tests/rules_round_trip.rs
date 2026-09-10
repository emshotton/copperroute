mod common;

use copper_board::{Board, PadstackId, ViaInfo, ViaInfoId, ViaRule};
use copper_dsn::parser::DsnRouterSettings;
use copper_dsn::parser::scope_parameter::DsnReadOptions;
use copper_dsn::{BoardReadResult, CoordinateTransform, rules_reader, rules_writer};

fn load_board(name: &str) -> (Board, CoordinateTransform) {
    let bytes = fixture_bytes(name);
    let options = DsnReadOptions::default();
    let stem = name.strip_suffix(".dsn").unwrap_or(name);
    match copper_dsn::read_board(&bytes[..], None, Some(stem), &options) {
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
            *board.unwrap_or_else(|| panic!("{name} produced no board")),
            coordinate_transform
                .unwrap_or_else(|| panic!("{name} produced no coordinate transform")),
        ),
        BoardReadResult::Partial { diagnostic, .. } => {
            panic!("{name}: truncated: {diagnostic}")
        }
        BoardReadResult::ParseError { location, detail } => {
            panic!("{name}: parse error at {location}: {detail}")
        }
        BoardReadResult::IoError(e) => panic!("{name}: io error: {e}"),
    }
}

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = testkit::fixture(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()))
}

fn write_rules(
    board: &Board,
    ct: &CoordinateTransform,
    settings: Option<&DsnRouterSettings>,
    design_name: &str,
) -> Vec<u8> {
    let mut out = Vec::new();
    rules_writer::write(board, ct, settings, &mut out, design_name).expect("writing to a Vec");
    out
}

#[test]
fn rules_round_trip() {
    let (mut board, ct) = load_board("Issue029-hw48na.dsn");

    let out = write_rules(&board, &ct, None, "Issue029-hw48na");
    let content = String::from_utf8(out.clone()).expect("the writer emits UTF-8");
    assert!(
        content.contains("(rules PCB"),
        "Rules file must start with (rules PCB ...)"
    );
    assert!(
        content.contains("(rule"),
        "Rules file must contain at least one (rule ...) scope"
    );

    let ok = rules_reader::read(&out[..], "Issue029-hw48na", &mut board, &ct, None)
        .expect("no scanner error");
    assert!(ok, "RulesReader.read must return true on valid input");
}

#[test]
fn rules_round_trip_with_autoroute_settings() {
    let (mut board, ct) = load_board("Issue029-hw48na.dsn");

    let settings = round_trip_settings(board.get_layer_count());
    let out = write_rules(&board, &ct, Some(&settings), "Issue029-hw48na");
    let content = String::from_utf8(out.clone()).expect("the writer emits UTF-8");
    assert!(
        content.contains("(autoroute_settings"),
        "Rules file must contain autoroute_settings scope"
    );
    assert!(
        content.contains("(layer_rule"),
        "Rules file must contain layer_rule scopes"
    );

    let mut target = DsnRouterSettings::new();
    let ok = rules_reader::read(
        &out[..],
        "Issue029-hw48na",
        &mut board,
        &ct,
        Some(&mut target),
    )
    .expect("no scanner error");
    assert!(ok, "RulesReader.read must return true on valid input");

    assert_eq!(target.via_costs(), 75);
    assert_eq!(target.plane_via_costs(), 8);
    assert_eq!(target.start_ripup_costs(), 120);
    assert!(target.get_preferred_direction_is_horizontal(0));
    assert!(!target.get_preferred_direction_is_horizontal(1));
    assert_eq!(target.get_preferred_direction_trace_costs(0), 1.2);
    assert_eq!(target.get_against_preferred_direction_trace_costs(0), 2.8);
    assert_eq!(target.get_preferred_direction_trace_costs(1), 1.0);
    assert_eq!(target.get_against_preferred_direction_trace_costs(1), 3.1);
}

fn round_trip_settings(layer_count: usize) -> DsnRouterSettings {
    let mut settings = DsnRouterSettings::new();
    settings.set_layer_count(layer_count);
    settings.set_via_costs(75);
    settings.set_plane_via_costs(8);
    settings.set_start_ripup_costs(120);
    settings.set_layer_active(0, true);
    settings.set_layer_active(1, true);
    settings.set_preferred_direction_is_horizontal(0, true);
    settings.set_preferred_direction_is_horizontal(1, false);
    settings.set_preferred_direction_trace_costs(0, 1.2);
    settings.set_against_preferred_direction_trace_costs(0, 2.8);
    settings.set_preferred_direction_trace_costs(1, 1.0);
    settings.set_against_preferred_direction_trace_costs(1, 3.1);
    settings
}

#[test]
fn invalid_rules_file_returns_false() {
    let (mut board, ct) = load_board("Issue029-hw48na.dsn");
    let garbage = b"not a rules file";
    let ok =
        rules_reader::read(&garbage[..], "x", &mut board, &ct, None).expect("no scanner error");
    assert!(!ok, "RulesReader.read must return false for garbage input");
}

#[test]
fn every_bad_header_token_returns_false() {
    let (mut board, ct) = load_board("Issue029-hw48na.dsn");
    for input in [
        &b""[..],
        &b"rules pcb x"[..],
        &b"(pcb x)"[..],
        &b"(rules structure x)"[..],
    ] {
        let ok = rules_reader::read(input, "x", &mut board, &ct, None).expect("no scanner error");
        assert!(
            !ok,
            "expected false for {:?}",
            String::from_utf8_lossy(input)
        );
    }
}

#[test]
fn read_existing_rules_fixture() {
    let (mut board, ct) = load_board("Issue029-hw48na.dsn");
    let rules = fixture_bytes("Issue029-hw48na_valid.rules");
    let ok = rules_reader::read(&rules[..], "hw48na", &mut board, &ct, None).expect("valid file");
    assert!(
        ok,
        "RulesReader.read must return true for a known-valid rules fixture"
    );
}

#[test]
fn rules_state_matches_java_issue593() {
    assert_rules_golden(
        "Issue593-BBD_Mars-64.dsn",
        "Issue593-BBD_Mars-64.rules",
        "Issue593-BBD_Mars-64.dsn",
        "Issue593-BBD_Mars-64-rules.txt",
        None,
    );
}

#[test]
fn rules_state_matches_java_except_fractional_via_hw48na_valid() {
    assert_rules_golden(
        "Issue029-hw48na.dsn",
        "Issue029-hw48na_valid.rules",
        "hw48na",
        "Issue029-hw48na_valid-rules.txt",
        Some(2),
    );
}

#[test]
fn rules_state_matches_java_except_fractional_via_hw48na_invalid() {
    assert_rules_golden(
        "Issue029-hw48na.dsn",
        "Issue029-hw48na_invalid.rules",
        "hw48na",
        "Issue029-hw48na_invalid-rules.txt",
        Some(2),
    );
}

#[test]
fn rules_state_matches_java_except_fractional_via_issue107_bad() {
    assert_rules_golden(
        "Issue029-hw48na.dsn",
        "Issue107-freq_teiler_200kHz_kicad_bad.rules",
        "x",
        "Issue107_bad-rules.txt",
        Some(1),
    );
}

fn assert_rules_golden(
    dsn: &str,
    rules: &str,
    design_name: &str,
    golden_name: &str,
    restored_via_rule: Option<usize>,
) {
    let (mut board, ct) = load_board(dsn);
    let rules_bytes = fixture_bytes(rules);
    let ok = rules_reader::read(&rules_bytes[..], design_name, &mut board, &ct, None)
        .expect("no scanner error");
    let actual = dump_rules(&mut board, ok);
    let mut expected = common::golden(golden_name);
    if let Some(index) = restored_via_rule {
        // The JVM recording drops this rule because use_via retains fractional names.
        // Keep the recording intact and assert only the deliberate import correction.
        let original = format!("viarule {index} 1A_EXTERNAL_1oz []");
        let line = expected
            .iter_mut()
            .find(|line| **line == original)
            .expect("JVM recording must contain the empty fractional-via rule");
        *line = format!("viarule {index} 1A_EXTERNAL_1oz [Via[0-1]_685:330_um-1A_EXTERNAL_1oz]");
    }
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(a, e, "{golden_name}: line {} differs", i + 1);
    }
    assert_eq!(
        actual.len(),
        expected.len(),
        "{golden_name}: line count differs"
    );
}

fn dump_rules(board: &mut Board, ok: bool) -> Vec<String> {
    let mut out = Vec::new();
    out.push(format!("read {ok}"));
    out.push(format!(
        "snapangle {}",
        match board.rules.trace_angle_restriction {
            copper_board::AngleRestriction::NinetyDegree => "NINETY_DEGREE",
            copper_board::AngleRestriction::FortyFiveDegree => "FORTYFIVE_DEGREE",
            copper_board::AngleRestriction::None => "NONE",
        }
    ));
    let layer_count = board.get_layer_count();
    let default_half_widths: Vec<String> = (0..layer_count)
        .map(|layer| board.rules.get_default_trace_half_width(layer).to_string())
        .collect();
    out.push(format!("defaulthw [{}]", default_half_widths.join(",")));

    let via_padstacks = board.library.get_via_padstacks();
    let names: Vec<&str> = via_padstacks
        .iter()
        .map(|id| {
            board
                .library
                .get_padstack(*id)
                .map_or("null", |p| p.name.as_str())
        })
        .collect();
    out.push(format!(
        "viapadstacks[{}] {}",
        via_padstacks.len(),
        names.join(" ")
    ));
    out.push(format!("padstacks {}", board.library.padstacks.count()));

    for (i, via) in board.rules.via_infos.iter().enumerate() {
        out.push(format!(
            "viainfo {i} {} padstack={} cl={} attach={}",
            via.get_name(),
            board
                .library
                .get_padstack(via.get_padstack())
                .map_or("null", |p| p.name.as_str()),
            via.get_clearance_class_index(),
            via.attach_smd_allowed(),
        ));
    }

    for (i, rule) in board.rules.via_rules.iter().enumerate() {
        let vias: Vec<&str> = rule.iter().map(copper_board::ViaInfo::get_name).collect();
        out.push(format!("viarule {i} {} [{}]", rule.name, vias.join(" ")));
    }

    for i in 0..board.rules.net_classes.count() {
        let net_class = board.rules.net_classes.get(copper_board::NetClassId(i));
        let via_rule = net_class
            .get_via_rule()
            .map_or("null", |rule| rule.name.as_str());
        let half_widths: Vec<String> = (0..net_class.layer_count())
            .map(|layer| net_class.get_trace_half_width(layer).to_string())
            .collect();
        out.push(format!(
            "netclass {i} {} traceCl={} viaRule={via_rule} hw=[{}] pullTight={} shoveFixed={} \
             minLen={:?} maxLen={:?}",
            net_class.get_name(),
            net_class.get_trace_clearance_class(),
            half_widths.join(","),
            net_class.get_pull_tight(),
            net_class.is_shove_fixed(),
            net_class.get_minimum_trace_length(),
            net_class.get_maximum_trace_length(),
        ));
    }

    let cm = &board.rules.clearance_matrix;
    let mut class_names = String::new();
    for i in 0..cm.get_class_count() {
        class_names.push_str(&format!("{i}:{} ", cm.get_name(i).unwrap_or("null")));
    }
    out.push(format!(
        "clclasses[{}] {}",
        cm.get_class_count(),
        class_names.trim_end()
    ));
    for layer in 0..layer_count {
        for i in 0..cm.get_class_count() {
            let mut row = String::new();
            for j in 0..cm.get_class_count() {
                row.push_str(&format!("{} ", cm.get_value(i, j, layer, false)));
            }
            out.push(format!("clrow {layer} {i} {}", row.trim_end()));
        }
    }
    out.push(format!(
        "pinedge {:?}",
        board.rules.get_pin_edge_to_turn_dist()
    ));
    out
}

#[test]
fn apply_via_info_leaves_via_rules_on_their_own_copies() {
    let (mut board, ct) = load_board("Issue593-BBD_Mars-64.dsn");

    let padstack = board
        .library
        .padstacks
        .get_by_name("Via[0-1]_800:400_um")
        .map(|p| PadstackId(p.no))
        .expect("the fixture's only via padstack");
    board
        .rules
        .via_infos
        .add(ViaInfo::new("B", padstack, 1, false));
    let a = board
        .rules
        .via_infos
        .get_no("Via[0-1]_800:400_um")
        .expect("A");
    let b = board.rules.via_infos.get_no("B").expect("B");
    assert_eq!((a, b), (ViaInfoId(0), ViaInfoId(1)));

    let mut rule = ViaRule::new("leave_me_alone");
    rule.append_via(board.rules.via_infos.get(a).clone());
    rule.append_via(board.rules.via_infos.get(b).clone());
    board.rules.via_rules.push(rule);
    let rule_index = board.rules.via_rules.len() - 1;

    let rules = b"(rules PCB x\n  (via \"Via[0-1]_800:400_um\" \"Via[0-1]_800:400_um\" default attach)\n)\n";
    let ok = rules_reader::read(&rules[..], "x", &mut board, &ct, None).expect("no scanner error");
    assert!(ok);

    let names: Vec<&str> = board
        .rules
        .via_infos
        .iter()
        .map(ViaInfo::get_name)
        .collect();
    assert_eq!(names, ["B", "Via[0-1]_800:400_um"]);
    assert!(
        board.rules.via_infos.get(ViaInfoId(1)).attach_smd_allowed(),
        "the tail entry is the replacement, not the original"
    );

    let rule = &board.rules.via_rules[rule_index];
    let resolved: Vec<&str> = rule.iter().map(ViaInfo::get_name).collect();
    assert_eq!(resolved, ["Via[0-1]_800:400_um", "B"]);
    assert!(
        !rule.get_via(0).attach_smd_allowed(),
        "the rule keeps the original, not the replacement"
    );
}

#[test]
fn re_declared_via_info_leaves_the_rule_on_the_detached_original_like_java() {
    let (mut board, ct) = load_board("Issue593-BBD_Mars-64.dsn");
    assert!(
        !board
            .rules
            .via_infos
            .get_by_name("Via[0-1]_800:400_um")
            .expect("the fixture's via info")
            .attach_smd_allowed(),
        "the .dsn's via info starts with attach=false"
    );

    let rules =
        b"(rules PCB x\n  (via \"Via[0-1]_800:400_um\" \"Via[0-1]_800:400_um\" default attach)\n)\n";
    assert!(rules_reader::read(&rules[..], "x", &mut board, &ct, None).expect("no scanner error"));

    assert!(
        board
            .rules
            .via_infos
            .get_by_name("Via[0-1]_800:400_um")
            .expect("the replacement")
            .attach_smd_allowed()
    );
    let reached: Vec<bool> = board
        .rules
        .via_rules
        .iter()
        .flat_map(|rule| rule.iter())
        .map(ViaInfo::attach_smd_allowed)
        .collect();
    assert!(
        !reached.is_empty(),
        "the fixture has via rules to reach through"
    );
    assert!(
        reached.iter().all(|attach| !*attach),
        "every rule keeps the detached original, exactly as the jar's `RProbe divergence` prints"
    );
}

#[test]
fn a_layer_rule_naming_an_unknown_layer_is_dropped_not_widened() {
    let (mut board, ct) = load_board("Issue029-hw48na.dsn");
    assert_eq!(default_half_widths(&mut board), [1016, 1016]);

    let rules = b"(rules PCB x\n  (layer BOGUS\n    (rule (width 500.0))\n  )\n)\n";
    assert!(rules_reader::read(&rules[..], "x", &mut board, &ct, None).expect("no scanner error"));
    assert_eq!(
        default_half_widths(&mut board),
        [1016, 1016],
        "the jar answers [2500,2500] here"
    );
}

#[test]
fn a_layer_rule_naming_a_real_layer_applies_only_there() {
    let (mut board, ct) = load_board("Issue029-hw48na.dsn");
    let rules = b"(rules PCB x\n  (layer B.Cu\n    (rule (width 500.0))\n  )\n)\n";
    assert!(rules_reader::read(&rules[..], "x", &mut board, &ct, None).expect("no scanner error"));
    assert_eq!(default_half_widths(&mut board), [1016, 2500]);
}

fn default_half_widths(board: &mut Board) -> Vec<i32> {
    (0..board.get_layer_count())
        .map(|layer| board.rules.get_default_trace_half_width(layer))
        .collect()
}

#[test]
fn read_router_settings_extracts_the_autoroute_scope() {
    let rules = fixture_bytes("Issue593-BBD_Mars-64.rules");
    let settings = rules_reader::read_router_settings(&rules[..])
        .expect("no scanner error")
        .expect("the fixture has an (autoroute_settings ...) scope");

    assert_eq!(settings.via_costs(), 50);
    assert_eq!(settings.plane_via_costs(), 5);
    assert_eq!(settings.start_ripup_costs(), 100);
    assert!(settings.run_router());
    assert!(!settings.run_optimizer());
    assert!(settings.vias_allowed());
    assert_eq!(settings.get_layer_count(), 2);
    assert!(!settings.get_preferred_direction_is_horizontal(0));
    assert!(settings.get_preferred_direction_is_horizontal(1));
    assert_eq!(settings.get_against_preferred_direction_trace_costs(0), 4.7);
    assert_eq!(settings.get_against_preferred_direction_trace_costs(1), 3.0);
}

#[test]
fn read_router_settings_leaves_unnamed_scalars_absent() {
    let rules = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../copper-settings/tests/data/Issue029-hw48na_reduced.rules"
    ))
    .expect("the reduced fixture is committed next to SProbe.java");
    let settings = rules_reader::read_router_settings(&rules[..])
        .expect("no scanner error")
        .expect("the fixture has an (autoroute_settings ...) scope");

    assert_eq!(settings.vias_allowed_raw(), None);
    assert_eq!(settings.via_costs_raw(), None);
    assert_eq!(settings.plane_via_costs_raw(), None);
    assert_eq!(settings.start_ripup_costs_raw(), None);
    assert!(!settings.are_board_specific_trace_costs_applied());

    assert!(settings.vias_allowed());
    assert_eq!(settings.via_costs(), 1);
    assert_eq!(settings.plane_via_costs(), 1);
    assert_eq!(settings.start_ripup_costs(), 1);
    assert_eq!(settings.get_preferred_direction_trace_costs(0), 1.0);
    assert_eq!(settings.get_against_preferred_direction_trace_costs(1), 1.0);

    assert_eq!(settings.get_layer_count(), 2);
    assert_eq!(
        settings.preferred_direction_is_horizontal_raw(0),
        Some(false)
    );
    assert_eq!(
        settings.preferred_direction_is_horizontal_raw(1),
        Some(true)
    );

    let full = fixture_bytes("Issue029-hw48na_valid.rules");
    let full = rules_reader::read_router_settings(&full[..])
        .expect("no scanner error")
        .expect("has an (autoroute_settings ...) scope");
    assert!(full.are_board_specific_trace_costs_applied());
    assert_eq!(full.via_costs_raw(), Some(50));
}

#[test]
fn apply_new_values_from_skips_absent_scalars() {
    let mut target = DsnRouterSettings::new();
    target.set_layer_count(2);
    target.set_via_costs(50);
    target.set_plane_via_costs(5);
    target.set_start_ripup_costs(100);
    target.set_vias_allowed(false);

    let mut source = DsnRouterSettings::new();
    source.set_layer_count(2);
    target.apply_new_values_from(&source);

    assert_eq!(target.via_costs(), 50);
    assert_eq!(target.plane_via_costs(), 5);
    assert_eq!(target.start_ripup_costs(), 100);
    assert!(!target.vias_allowed());

    source.set_via_costs(7);
    source.set_vias_allowed(true);
    target.apply_new_values_from(&source);
    assert_eq!(target.via_costs(), 7);
    assert!(target.vias_allowed());
    assert_eq!(target.plane_via_costs(), 5);
    assert_eq!(target.start_ripup_costs(), 100);
}

#[test]
fn read_router_settings_returns_none_without_a_scope() {
    for input in [
        &b""[..],
        &b"not a rules file"[..],
        &b"(rules PCB x (rule (width 1.0)))"[..],
    ] {
        assert_eq!(
            rules_reader::read_router_settings(input).expect("no scanner error"),
            None,
            "expected None for {:?}",
            String::from_utf8_lossy(input)
        );
    }
}

#[test]
fn discover_layer_structure_is_insertion_ordered_and_deduplicated() {
    let text = "(rules PCB x (autoroute_settings (layer_rule B.Cu) (layer_rule F.Cu) \
                (layer_rule B.Cu)) (layer In1.Cu (rule (width 1.0))))";
    let structure = rules_reader::discover_layer_structure(text).expect("no scanner error");
    let names: Vec<&str> = structure.layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["B.Cu", "F.Cu", "In1.Cu"]);
    assert_eq!(structure.get_no("F.Cu"), Some(1));
    assert!(structure.layers.iter().all(|l| l.is_signal));

    let fallback =
        rules_reader::discover_layer_structure("(rules PCB x)").expect("no scanner error");
    let names: Vec<&str> = fallback.layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["F.Cu", "B.Cu"]);
}

#[test]
fn a_rules_file_naming_an_absent_layer_leaves_the_default_width_untouched() {
    const DSN: &str = "Issue508-DAC2020_bm10.dsn";
    const DESIGN: &str = "Issue508-DAC2020_bm10";

    let (mut board, _) = load_board(DSN);
    let names: Vec<&str> = board
        .layer_structure()
        .layers
        .iter()
        .map(|l| l.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["Top", "Route2", "Route15", "Bottom"],
        "the board this test needs is the four-layer one with no B.Cu"
    );
    assert_eq!(
        default_half_widths(&mut board),
        [1000, 1000, 1000, 1000],
        "the jar's `defaulthw` for this board with no rules file"
    );

    let (mut board, ct) = load_board(DSN);
    let stale =
        b"(rules PCB Issue508-DAC2020_bm10\n  (layer B.Cu\n    (rule (width 500))\n  )\n)\n";
    assert!(
        rules_reader::read(&stale[..], DESIGN, &mut board, &ct, None).expect("no scan error"),
        "the read still succeeds — nothing inside a (rules …) scope can fail it"
    );
    assert_eq!(
        default_half_widths(&mut board),
        [1000, 1000, 1000, 1000],
        "the jar answers [2500,2500,2500,2500]: an absent layer name meant `all layers`"
    );

    let (mut board, ct) = load_board(DSN);
    let good =
        b"(rules PCB Issue508-DAC2020_bm10\n  (layer Bottom\n    (rule (width 500))\n  )\n)\n";
    assert!(rules_reader::read(&good[..], DESIGN, &mut board, &ct, None).expect("no scan error"));
    assert_eq!(
        default_half_widths(&mut board),
        [1000, 1000, 1000, 2500],
        "a resolvable layer name still lands, on that one layer — the jar's own answer"
    );
}
