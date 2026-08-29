//! Port of `io/specctra/RulesRoundTripTest.java`, plus the two evidence layers the Java test
//! does not have: a JVM golden of the whole rules-read state and a byte comparison of the writer
//! against the pinned 2.3.0 jar.
//!
//! # Regenerating the goldens
//!
//! ```text
//! export PATH=/opt/homebrew/opt/openjdk@25/bin:$PATH
//! javac -cp tools/freerouting-2.3.0.jar -d /tmp/rprobe crates/fr-dsn/tests/data/RProbe.java
//! CP=tools/freerouting-2.3.0.jar:/tmp/rprobe; F=../freerouting/fixtures; D=crates/fr-dsn/tests/data
//! java -Djava.awt.headless=true -cp $CP RProbe dump  $F/Issue593-BBD_Mars-64.dsn $F/Issue593-BBD_Mars-64.rules Issue593-BBD_Mars-64.dsn
//! java -Djava.awt.headless=true -cp $CP RProbe dump  $F/Issue029-hw48na.dsn $F/Issue029-hw48na_valid.rules   hw48na
//! java -Djava.awt.headless=true -cp $CP RProbe dump  $F/Issue029-hw48na.dsn $F/Issue029-hw48na_invalid.rules hw48na
//! java -Djava.awt.headless=true -cp $CP RProbe dump  $F/Issue029-hw48na.dsn $F/Issue107-freq_teiler_200kHz_kicad_bad.rules x
//! java -Djava.awt.headless=true -cp $CP RProbe write $F/Issue593-BBD_Mars-64.dsn - Issue593-BBD_Mars-64  > $D/Issue593-BBD_Mars-64-written.rules
//! java -Djava.awt.headless=true -cp $CP RProbe write $F/Issue029-hw48na.dsn      - Issue029-hw48na       > $D/Issue029-hw48na-written.rules
//! java -Djava.awt.headless=true -cp $CP RProbe writesettings $F/Issue029-hw48na.dsn - Issue029-hw48na    > $D/Issue029-hw48na-settings.rules
//! ```
//!
//! The four `dump` outputs get a one-line `#` header prepended by hand (see [`common::golden`]).
//!
//! # Java-wins corrections to the Task 14 brief
//!
//! * The brief puts `rules_round_trip` / `rules_round_trip_with_autoroute_settings` on
//!   `Issue593-BBD_Mars-64`; `RulesRoundTripTest.java:30,49` loads **`Issue029-hw48na.dsn`**, and
//!   `readExistingRulesFixture` (:103-107) pairs it with `Issue029-hw48na_valid.rules` and the
//!   design name `"hw48na"`. Java wins; Issue593 is carried by the JVM golden and the writer
//!   byte comparison instead.
//! * The brief expects `Issue107-freq_teiler_200kHz_kicad_bad.rules` to make `read` answer
//!   `false`. It does **not**: the 2.3.0 jar reads it to `true` (`Issue107_bad-rules.txt` line 1),
//!   because nothing inside a `(rules …)` scope can fail the read — every helper warns and
//!   returns. `RulesRoundTripTest.invalidRulesFileReturnsFalse` (:93-99) uses the literal
//!   `"not a rules file"` for exactly that reason, and so does the port. The "bad" and "invalid"
//!   fixtures are kept, as goldens of what Java actually builds from them.
//! * `loadingProducesWarningsForDegenerateWires` (:114-135) is already ported, verbatim and with
//!   the exact 2.3.0 message strings, as `dsn_reader::loading_produces_warnings_for_degenerate_wires`
//!   (`tests/dsn_reader.rs`). It exercises `DsnReader`, not `RulesReader`; it is not duplicated
//!   here.

mod common;

use std::path::Path;

use fr_board::{Board, PadstackId, ViaInfo, ViaInfoId, ViaRule};
use fr_dsn::parser::DsnRouterSettings;
use fr_dsn::parser::scope_parameter::DsnReadOptions;
use fr_dsn::{BoardReadResult, CoordinateTransform, rules_reader, rules_writer};

// ------------------------------------------------------------------------------------------
// fixtures
// ------------------------------------------------------------------------------------------

/// `DsnTestFixtures.loadBoard(name)` — a fresh board plus the transform its `structure` scope
/// built, which Java reads back off `board.communication.coordinateTransform` (plan ruling A).
fn load_board(name: &str) -> (Board, CoordinateTransform) {
    let bytes = fixture_bytes(name);
    let options = DsnReadOptions::default();
    let stem = name.strip_suffix(".dsn").unwrap_or(name);
    match fr_dsn::read_board(&bytes[..], None, Some(stem), &options) {
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
        BoardReadResult::ParseError { location, detail } => {
            panic!("{name}: parse error at {location}: {detail}")
        }
        BoardReadResult::IoError(e) => panic!("{name}: io error: {e}"),
    }
}

/// `DsnTestFixtures.openResource(name)` / `openFixtureStream(name)` — both resolve to the Java
/// repo's `fixtures/` directory here.
fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../freerouting/fixtures/"
    ))
    .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()))
}

/// A byte-for-byte golden under `tests/data/`, with no `#` header to strip.
fn raw_golden(name: &str) -> Vec<u8> {
    let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/")).join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read golden {}: {e}", path.display()))
}

/// `RulesRoundTripTest.rulesRoundTrip`'s write half, as a `String`.
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

// ------------------------------------------------------------------------------------------
// RulesRoundTripTest.java
// ------------------------------------------------------------------------------------------

/// `RulesRoundTripTest.rulesRoundTrip` (RulesRoundTripTest.java:28-45).
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

/// `RulesRoundTripTest.rulesRoundTripWithAutorouteSettings` (RulesRoundTripTest.java:47-90).
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

/// The settings `RulesRoundTripTest.rulesRoundTripWithAutorouteSettings` builds
/// (RulesRoundTripTest.java:51-63), shared with `RProbe.writesettings`.
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

/// `RulesRoundTripTest.invalidRulesFileReturnsFalse` (RulesRoundTripTest.java:92-99).
#[test]
fn invalid_rules_file_returns_false() {
    let (mut board, ct) = load_board("Issue029-hw48na.dsn");
    let garbage = b"not a rules file";
    let ok =
        rules_reader::read(&garbage[..], "x", &mut board, &ct, None).expect("no scanner error");
    assert!(!ok, "RulesReader.read must return false for garbage input");
}

/// The three other ways to fail the `(rules pcb <name>` header, each of which Java answers
/// `false` to from a different one of its three checks (RulesReader.java:81-97).
#[test]
fn every_bad_header_token_returns_false() {
    let (mut board, ct) = load_board("Issue029-hw48na.dsn");
    for input in [
        &b""[..],                    // no tokens at all
        &b"rules pcb x"[..],         // no opening bracket
        &b"(pcb x)"[..],             // not `rules`
        &b"(rules structure x)"[..], // not `pcb`
    ] {
        let ok = rules_reader::read(input, "x", &mut board, &ct, None).expect("no scanner error");
        assert!(
            !ok,
            "expected false for {:?}",
            String::from_utf8_lossy(input)
        );
    }
}

/// `RulesRoundTripTest.readExistingRulesFixture` (RulesRoundTripTest.java:101-108).
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

// ------------------------------------------------------------------------------------------
// JVM goldens: the rules state a `.dsn` + `.rules` pair leaves on the board
// ------------------------------------------------------------------------------------------

/// The Issue593 golden the Task 14 brief asks for: a `.dsn` read plus its own `.rules`, every
/// piece of rules state compared against the pinned 2.3.0 jar.
#[test]
fn rules_state_matches_java_issue593() {
    assert_rules_golden(
        "Issue593-BBD_Mars-64.dsn",
        "Issue593-BBD_Mars-64.rules",
        "Issue593-BBD_Mars-64.dsn",
        "Issue593-BBD_Mars-64-rules.txt",
    );
}

/// The fixture `RulesRoundTripTest.readExistingRulesFixture` uses — 14 clearance classes, quoted
/// class names holding spaces and a comma, and a `(clear … (type "a"-"b"))` for most pairs of
/// them, so it pins `Structure.setClearanceRule`'s quoted-pair branch hard.
#[test]
fn rules_state_matches_java_hw48na_valid() {
    assert_rules_golden(
        "Issue029-hw48na.dsn",
        "Issue029-hw48na_valid.rules",
        "hw48na",
        "Issue029-hw48na_valid-rules.txt",
    );
}

/// The same file with its quoting mangled: Java still answers `true` and still builds a
/// clearance matrix, just a 23-class one full of half-parsed names (`"1A`, `EXTERNAL`, an empty
/// one). Reproducing *that* is what proves the quoted-pair branch is ported and not merely
/// approximated.
#[test]
fn rules_state_matches_java_hw48na_invalid() {
    assert_rules_golden(
        "Issue029-hw48na.dsn",
        "Issue029-hw48na_invalid.rules",
        "hw48na",
        "Issue029-hw48na_invalid-rules.txt",
    );
}

/// A `.rules` file written for a different design, applied to `Issue029-hw48na`: the padstacks,
/// via infos and net classes it names are mostly foreign to the board. Java reads it to `true`
/// (see the module docs) and the port must build the same partly-foreign result.
#[test]
fn rules_state_matches_java_issue107_bad() {
    assert_rules_golden(
        "Issue029-hw48na.dsn",
        "Issue107-freq_teiler_200kHz_kicad_bad.rules",
        "x",
        "Issue107_bad-rules.txt",
    );
}

fn assert_rules_golden(dsn: &str, rules: &str, design_name: &str, golden_name: &str) {
    let (mut board, ct) = load_board(dsn);
    let rules_bytes = fixture_bytes(rules);
    let ok = rules_reader::read(&rules_bytes[..], design_name, &mut board, &ct, None)
        .expect("no scanner error");
    let actual = dump_rules(&mut board, ok);
    let expected = common::golden(golden_name);
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(a, e, "{golden_name}: line {} differs", i + 1);
    }
    assert_eq!(
        actual.len(),
        expected.len(),
        "{golden_name}: line count differs"
    );
}

/// `RProbe.main`'s `dump` mode, in Rust: same lines, same order, same spelling.
fn dump_rules(board: &mut Board, ok: bool) -> Vec<String> {
    let mut out = Vec::new();
    out.push(format!("read {ok}"));
    out.push(format!(
        "snapangle {}",
        // `AngleRestriction`'s implicit `Enum.toString` (AngleRestriction.java).
        match board.rules.trace_angle_restriction {
            fr_board::AngleRestriction::NinetyDegree => "NINETY_DEGREE",
            fr_board::AngleRestriction::FortyFiveDegree => "FORTYFIVE_DEGREE",
            fr_board::AngleRestriction::None => "NONE",
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
        let vias: Vec<&str> = rule
            .iter()
            .map(|id| board.rules.via_infos.get(*id).get_name())
            .collect();
        out.push(format!("viarule {i} {} [{}]", rule.name, vias.join(" ")));
    }

    for i in 0..board.rules.net_classes.count() {
        let net_class = board.rules.net_classes.get(fr_board::NetClassId(i));
        let via_rule = net_class
            .get_via_rule()
            .map_or("null", |id| board.rules.via_rules[id.0].name.as_str());
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

// ------------------------------------------------------------------------------------------
// Byte parity of the writer against the pinned 2.3.0 jar
// ------------------------------------------------------------------------------------------

/// `RulesWriter.write(board, out, designName)` on `Issue593-BBD_Mars-64`, byte for byte. The
/// design name is what makes the `// Java bug:` in `write_rules` observable: `DsnWriter` quotes
/// `Issue593-BBD_Mars-64` (it holds two reserved `-`), `RulesWriter` does not.
#[test]
fn rules_writer_matches_java_issue593() {
    let (board, ct) = load_board("Issue593-BBD_Mars-64.dsn");
    let actual = write_rules(&board, &ct, None, "Issue593-BBD_Mars-64");
    assert_bytes_match(&actual, "Issue593-BBD_Mars-64-written.rules");
}

/// The same, on the fixture `RulesRoundTripTest` itself uses — 29 padstacks, 6 via padstacks
/// (two of them duplicated in the via-padstack list) and 14 clearance classes.
#[test]
fn rules_writer_matches_java_hw48na() {
    let (board, ct) = load_board("Issue029-hw48na.dsn");
    let actual = write_rules(&board, &ct, None, "Issue029-hw48na");
    assert_bytes_match(&actual, "Issue029-hw48na-written.rules");
}

/// The `(autoroute_settings …)` branch, which the 2.3.0 `RulesWriter.write` cannot reach (it has
/// no `RouterSettings` overload — that is a clone-HEAD addition, RulesWriter.java:54-68). The
/// golden comes from `RProbe.writesettings`, which replays `writeRules` line for line against the
/// same public 2.3.0 scope writers with `autorouteSettings` non-null.
#[test]
fn rules_writer_with_settings_matches_java() {
    let (board, ct) = load_board("Issue029-hw48na.dsn");
    let settings = round_trip_settings(board.get_layer_count());
    let actual = write_rules(&board, &ct, Some(&settings), "Issue029-hw48na");
    assert_bytes_match(&actual, "Issue029-hw48na-settings.rules");
}

fn assert_bytes_match(actual: &[u8], golden_name: &str) {
    let expected = raw_golden(golden_name);
    if actual == expected.as_slice() {
        return;
    }
    let actual_text = String::from_utf8_lossy(actual);
    let expected_text = String::from_utf8_lossy(&expected);
    for (i, (a, e)) in actual_text.lines().zip(expected_text.lines()).enumerate() {
        assert_eq!(a, e, "{golden_name}: line {} differs", i + 1);
    }
    assert_eq!(
        actual_text.lines().count(),
        expected_text.lines().count(),
        "{golden_name}: line count differs"
    );
    assert_eq!(actual, expected.as_slice(), "{golden_name}: bytes differ");
}

// ------------------------------------------------------------------------------------------
// The Plan 2 `ViaInfoId` renumbering obligation, end to end
// ------------------------------------------------------------------------------------------

/// `RulesReader.applyViaInfo` (RulesReader.java:340-350) through the real reader: a `(via …)`
/// scope whose name the board already carries has to move that via to the **tail** of
/// `ViaInfos` — Java's remove-then-add — while every `ViaRule` keeps naming the same vias.
///
/// `fr-board`'s `replace_via_info_renumbers_every_rule` pins the renumbering itself; this pins
/// that the reader reaches it.
#[test]
fn apply_via_info_renumbers_via_rules() {
    let (mut board, ct) = load_board("Issue593-BBD_Mars-64.dsn");

    // The board's own via info, plus a second one so the removal has something after it to shift.
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

    let mut rule = ViaRule::new("renumber_me");
    rule.append_via(a);
    rule.append_via(b);
    board.rules.via_rules.push(rule);
    let rule_index = board.rules.via_rules.len() - 1;

    // Re-apply "A" with `attach`, which is the one field this `(via …)` scope changes.
    let rules = b"(rules PCB x\n  (via \"Via[0-1]_800:400_um\" \"Via[0-1]_800:400_um\" default attach)\n)\n";
    let ok = rules_reader::read(&rules[..], "x", &mut board, &ct, None).expect("no scanner error");
    assert!(ok);

    // Java's `Vector`/`LinkedList` order after remove-then-add: B, then the replacement A'.
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

    // The rule's indices were rewritten; the vias it names are unchanged and still in order.
    let rule = &board.rules.via_rules[rule_index];
    assert_eq!(
        rule.iter().copied().collect::<Vec<_>>(),
        [ViaInfoId(1), ViaInfoId(0)]
    );
    let resolved: Vec<&str> = rule
        .iter()
        .map(|id| board.rules.via_infos.get(*id).get_name())
        .collect();
    assert_eq!(resolved, ["Via[0-1]_800:400_um", "B"]);
}

// ------------------------------------------------------------------------------------------
// read_router_settings / discover_layer_structure
// ------------------------------------------------------------------------------------------

/// `RulesReader.readRouterSettings` (RulesReader.java:180-236) on a real fixture: no board
/// needed, and the `layer_rule` names resolve through [`rules_reader::discover_layer_structure`].
#[test]
fn read_router_settings_extracts_the_autoroute_scope() {
    let rules = fixture_bytes("Issue593-BBD_Mars-64.rules");
    let settings = rules_reader::read_router_settings(&rules[..])
        .expect("no scanner error")
        .expect("the fixture has an (autoroute_settings ...) scope");

    // The values in `../freerouting/fixtures/Issue593-BBD_Mars-64.rules`.
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

/// The three ways `readRouterSettings` answers `null` (RulesReader.java:194-196, :203-212, :235).
#[test]
fn read_router_settings_returns_none_without_a_scope() {
    for input in [
        &b""[..],                                 // empty stream (:194-196)
        &b"not a rules file"[..],                 // bad header (:203-205)
        &b"(rules PCB x (rule (width 1.0)))"[..], // no autoroute_settings scope (:235)
    ] {
        assert_eq!(
            rules_reader::read_router_settings(input).expect("no scanner error"),
            None,
            "expected None for {:?}",
            String::from_utf8_lossy(input)
        );
    }
}

/// `RulesReader.discoverLayerStructure` (RulesReader.java:238-274): insertion order, deduplicated,
/// and the `F.Cu`/`B.Cu` fallback when the buffer names no layer at all.
#[test]
fn discover_layer_structure_is_insertion_ordered_and_deduplicated() {
    let text = "(rules PCB x (autoroute_settings (layer_rule B.Cu) (layer_rule F.Cu) \
                (layer_rule B.Cu)) (layer In1.Cu (rule (width 1.0))))";
    let structure = rules_reader::discover_layer_structure(text).expect("no scanner error");
    let names: Vec<&str> = structure.layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["B.Cu", "F.Cu", "In1.Cu"]);
    assert_eq!(structure.get_no("F.Cu"), Some(1));
    assert!(structure.layers.iter().all(|l| l.is_signal));

    // RulesReader.java:263-266.
    let fallback =
        rules_reader::discover_layer_structure("(rules PCB x)").expect("no scanner error");
    let names: Vec<&str> = fallback.layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["F.Cu", "B.Cu"]);
}
