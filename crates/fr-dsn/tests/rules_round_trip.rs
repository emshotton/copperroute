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
//! `RProbe`'s fifth mode, `divergence`, has no committed golden: it prints what each via *rule*
//! reaches, and is how the jar's answer quoted in
//! `re_declared_via_info_leaves_the_rule_on_the_detached_original_like_java` was obtained.
//!
//! # What is and is not jar-pinned
//!
//! `javap -p tools/freerouting-2.3.0.jar app.freerouting.io.specctra.RulesReader` lists a single
//! public method, `read(InputStream, String, BasicBoard)`. The four-argument `read`,
//! `readRouterSettings`, `discoverLayerStructure` and `RouterSettings.applyNewValuesFrom` are
//! **clone-HEAD additions**, absent from the pinned jar — so `read_router_settings_*`,
//! `discover_layer_structure_*` and `rules_round_trip_with_autoroute_settings`' target-settings
//! assertions are read from the Java source at HEAD and are **not** jar-verified. Everything
//! under "JVM goldens" and "Byte parity" below is.
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
        let vias: Vec<&str> = rule.iter().map(fr_board::ViaInfo::get_name).collect();
        out.push(format!("viarule {i} {} [{}]", rule.name, vias.join(" ")));
    }

    for i in 0..board.rules.net_classes.count() {
        let net_class = board.rules.net_classes.get(fr_board::NetClassId(i));
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
/// `ViaInfos` — Java's remove-then-add — while every `ViaRule` keeps the `ViaInfo` it already
/// held.
///
/// The list order and the names are Java's, and are what every writer emits.
///
/// **Since Plan 7 Task 0 so is what the rule reaches.** Java's `ViaRule` holds `ViaInfo` object
/// references (`ViaRule.java:21`), so after the replacement its rule still holds the **removed
/// original**; the port's rule holds an owned copy taken when the rule was built, which is the
/// same original. `re_declared_via_info_leaves_the_rule_on_the_detached_original_like_java`
/// isolates that with the jar's own answer next to it; see the "Via-info / via-rule re-pointing"
/// row in `docs/java-quirks.md`.
///
/// Named `apply_via_info_renumbers_via_rules` until Plan 7 Task 0. `fr-board`'s
/// `replace_via_info_leaves_every_rule_alone` pins the replacement itself; this pins that the
/// reader reaches it.
#[test]
fn apply_via_info_leaves_via_rules_on_their_own_copies() {
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

    let mut rule = ViaRule::new("leave_me_alone");
    rule.append_via(board.rules.via_infos.get(a).clone());
    rule.append_via(board.rules.via_infos.get(b).clone());
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

    // The rule was not touched: same vias, same order, and its "Via[0-1]_800:400_um" is still
    // the detached original with `attach=false`.
    let rule = &board.rules.via_rules[rule_index];
    let resolved: Vec<&str> = rule.iter().map(ViaInfo::get_name).collect();
    assert_eq!(resolved, ["Via[0-1]_800:400_um", "B"]);
    assert!(
        !rule.get_via(0).attach_smd_allowed(),
        "the rule keeps the original, not the replacement"
    );
}

/// The re-pointing divergence, isolated — **closed by Plan 7 Task 0: the port now answers what
/// Java answers.**
///
/// Named `re_declared_via_info_re_points_the_existing_via_rule_unlike_java` until Plan 7 Task 0,
/// when `ViaRule` gained ownership of its `ViaInfo`s (`ViaRule.java:21` is a `List<ViaInfo>` of
/// object *references*) and the assertion below inverted. The old name is kept in this comment so
/// the trail is one grep.
///
/// `Issue593-BBD_Mars-64.dsn` already carries one via info (`Via[0-1]_800:400_um`, `attach=false`)
/// and two `default` via rules that reach it. A one-line `.rules` file re-declaring that same via
/// *with* `attach` makes `RulesReader.applyViaInfo` remove the original and append a replacement.
///
/// Java (verified with `tools/freerouting-2.3.0.jar` via `RProbe divergence`):
///
/// ```text
/// viainfo 0 Via[0-1]_800:400_um padstack=Via[0-1]_800:400_um cl=1 attach=true
/// rulevia default Via[0-1]_800:400_um attach=false cl=1 inList=false id=1992550266
/// rulevia default Via[0-1]_800:400_um attach=false cl=1 inList=false id=1992550266
/// ```
///
/// — the list holds the replacement (`attach=true`) while both rules still hold the **detached
/// original** (`attach=false`, and `viaInfos.get(name) != thatObject`). The port's rules hold
/// owned copies made when the rule was built, so they keep the original's `attach=false` too:
/// `ViaInfos::remove` can no longer reach into a rule. Nothing a Plan 3 writer emits differs
/// either way (both entries share a name); `attach_smd_allowed`, `get_padstack` and
/// `get_clearance_class_index` are router inputs, and the router now agrees with the jar —
/// `crates/fr-router/tests/data/p7t0-ruling-h-match.txt`.
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

    // Java agrees about the list entry.
    assert!(
        board
            .rules
            .via_infos
            .get_by_name("Via[0-1]_800:400_um")
            .expect("the replacement")
            .attach_smd_allowed()
    );
    // And now it agrees about what the rules reach: the detached original, `attach=false`.
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

// ------------------------------------------------------------------------------------------
// Java bug #112: an unknown layer name made a layer rule apply to every layer
// ------------------------------------------------------------------------------------------

/// `RulesReader.applyLayerRules` + `applyRules`' layer-scoped arm (RulesReader.java:284-338) with
/// a layer the board **does not have**: `layerIndex` stays `-1`, the warning is not followed by a
/// `return`, and `-1` is exactly the "all layers" sentinel — so the rule overwrites the default
/// trace width on the whole board instead of being dropped (quirk #112).
///
/// JVM-verified with `RProbe dump` against `tools/freerouting-2.3.0.jar`: `Issue029-hw48na.dsn`
/// alone gives `defaulthw [1016,1016]`; with this rules file the jar gives `[2500,2500]`.
///
/// fixed: T4 (#112) — renamed from `…_applies_to_every_layer` and inverted: the width does not
/// move. See `a_rules_file_naming_an_absent_layer_leaves_the_default_width_untouched` for the
/// survey's own four-layer case.
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

/// The same scope with a layer the board *does* have, which is the branch quirk #112 is measured
/// against: only `B.Cu` moves.
///
/// JVM-verified the same way: `defaulthw [1016,2500]`.
#[test]
fn a_layer_rule_naming_a_real_layer_applies_only_there() {
    let (mut board, ct) = load_board("Issue029-hw48na.dsn");
    let rules = b"(rules PCB x\n  (layer B.Cu\n    (rule (width 500.0))\n  )\n)\n";
    assert!(rules_reader::read(&rules[..], "x", &mut board, &ct, None).expect("no scanner error"));
    assert_eq!(default_half_widths(&mut board), [1016, 2500]);
}

/// `RProbe`'s `defaulthw` line, as a `Vec` (`BoardRules.get_default_trace_half_width` per layer).
fn default_half_widths(board: &mut Board) -> Vec<i32> {
    (0..board.get_layer_count())
        .map(|layer| board.rules.get_default_trace_half_width(layer))
        .collect()
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

/// Absence is not the coalesced default: a scope that names no `(vias …)`, `(via_costs …)`,
/// `(plane_via_costs …)` or `(start_ripup_costs …)`, and no per-layer trace cost, leaves all four
/// scalars **`None`** and `areBoardSpecificTraceCostsApplied()` **`false`** — while the coalescing
/// getters still answer Java's `true`/`1`/`1.0` so the writers are unaffected.
///
/// This is the shape Plan 4's `From<&DsnRouterSettings> for RouterSettings` needs (controller
/// ruling L): storing the coalesced default instead would make a `.rules` file that mentions none
/// of these fields silently overwrite a higher-priority source's values. JVM-verified —
/// `crates/fr-settings/tests/data/SProbe.java` block `H`, run against the clone-HEAD jar:
/// `H.reduced.raw.viasAllowed = null`, `H.reduced.raw.scoring.viaCosts = null`,
/// `H.reduced.raw.areBoardSpecificTraceCostsApplied = false`.
#[test]
fn read_router_settings_leaves_unnamed_scalars_absent() {
    let rules = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fr-settings/tests/data/Issue029-hw48na_reduced.rules"
    ))
    .expect("the reduced fixture is committed next to SProbe.java");
    let settings = rules_reader::read_router_settings(&rules[..])
        .expect("no scanner error")
        .expect("the fixture has an (autoroute_settings ...) scope");

    // Absent, because the file names none of them.
    assert_eq!(settings.vias_allowed_raw(), None);
    assert_eq!(settings.via_costs_raw(), None);
    assert_eq!(settings.plane_via_costs_raw(), None);
    assert_eq!(settings.start_ripup_costs_raw(), None);
    assert!(!settings.are_board_specific_trace_costs_applied());

    // The coalescing getters — what the writers call — still answer Java's defaults, so no
    // emitted byte changes.
    assert!(settings.vias_allowed());
    assert_eq!(settings.via_costs(), 1);
    assert_eq!(settings.plane_via_costs(), 1);
    assert_eq!(settings.start_ripup_costs(), 1);
    assert_eq!(settings.get_preferred_direction_trace_costs(0), 1.0);
    assert_eq!(settings.get_against_preferred_direction_trace_costs(1), 1.0);

    // What the file *does* name still lands: `setLayerCount` seeded the layers and both
    // `(preferred_direction …)` lines were read (SProbe H.reduced.raw.layers[*]).
    assert_eq!(settings.get_layer_count(), 2);
    assert_eq!(
        settings.preferred_direction_is_horizontal_raw(0),
        Some(false)
    );
    assert_eq!(
        settings.preferred_direction_is_horizontal_raw(1),
        Some(true)
    );

    // The unmodified fixture names both trace costs, so the flag comes out true
    // (SProbe H.full.raw.areBoardSpecificTraceCostsApplied = true).
    let full = fixture_bytes("Issue029-hw48na_valid.rules");
    let full = rules_reader::read_router_settings(&full[..])
        .expect("no scanner error")
        .expect("has an (autoroute_settings ...) scope");
    assert!(full.are_board_specific_trace_costs_applied());
    assert_eq!(full.via_costs_raw(), Some(50));
}

/// Rule 2 of `ReflectionUtil.copyFields` (:235) in `apply_new_values_from`: a source whose field
/// is absent must **not** overwrite the target's value with a coalesced default.
#[test]
fn apply_new_values_from_skips_absent_scalars() {
    let mut target = DsnRouterSettings::new();
    target.set_layer_count(2);
    target.set_via_costs(50);
    target.set_plane_via_costs(5);
    target.set_start_ripup_costs(100);
    target.set_vias_allowed(false);

    // A source that names none of the four.
    let mut source = DsnRouterSettings::new();
    source.set_layer_count(2);
    target.apply_new_values_from(&source);

    assert_eq!(target.via_costs(), 50);
    assert_eq!(target.plane_via_costs(), 5);
    assert_eq!(target.start_ripup_costs(), 100);
    assert!(!target.vias_allowed());

    // …and one that names two of them overwrites exactly those two.
    source.set_via_costs(7);
    source.set_vias_allowed(true);
    target.apply_new_values_from(&source);
    assert_eq!(target.via_costs(), 7);
    assert!(target.vias_allowed());
    assert_eq!(target.plane_via_costs(), 5);
    assert_eq!(target.start_ripup_costs(), 100);
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

/// #112's binding test, the survey's own: a stale `.rules` file naming a layer the board does not
/// have must leave the board's default trace width alone — on the four-layer board the survey
/// names, not only on the two-layer one above.
///
/// **JAR-VERIFIED** — the jar's behaviour *is* the bug being recorded. `Issue508-DAC2020_bm10.dsn`
/// is a four-layer board whose copper layers are named `Top`, `Route2`, `Route15` and `Bottom`;
/// it has no `B.Cu`, which is exactly how a `.rules` file written for a different stack-up goes
/// stale. Three runs of the pinned 2.3.0 jar over it
/// (`RProbe dump <dsn> <rules> Issue508-DAC2020_bm10`, the `defaulthw` line):
///
/// ```text
/// no .rules at all                   defaulthw [1000,1000,1000,1000]
/// (layer B.Cu   (rule (width 500)))  defaulthw [2500,2500,2500,2500]  <- every layer overwritten
/// (layer Bottom (rule (width 500)))  defaulthw [1000,1000,1000,2500]  <- the control: one layer
/// ```
///
/// `RulesReader.applyRules` warns "layer not found" and does **not** return
/// (RulesReader.java:286-290), so `layerIndex` stays `-1` — the sentinel the branches below read
/// as "all layers" — and a rule meant for one absent layer silently rewrites the whole board.
///
/// fixed: T4 (#112): the sentinel is unrepresentable (`fr_dsn::RuleLayerScope`), the lookup moved
/// to the one caller that has a name to resolve, and a name the board does not carry drops that
/// scope's rules. The control row is what proves the guard did not simply stop applying layer
/// rules altogether.
#[test]
fn a_rules_file_naming_an_absent_layer_leaves_the_default_width_untouched() {
    const DSN: &str = "Issue508-DAC2020_bm10.dsn";
    const DESIGN: &str = "Issue508-DAC2020_bm10";

    // The jar's own baseline, with no `.rules` applied.
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

    // A `.rules` naming a layer the board does not have.
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

    // The control: the same rule, on a layer the board does have.
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
