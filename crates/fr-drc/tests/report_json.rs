//! Plan 5 Task 8: `KiCadDrcReport::to_json` and `DesignRulesChecker::report_to_json` — the port
//! of `generateReportJson` (`drc/DesignRulesChecker.java:817-820`, i.e.
//! `GsonProvider.GSON.toJson(report)`), in plan-5 ruling 2's two key flavors.
//!
//! # Provenance
//!
//! `crates/fr-drc/tests/data/JsonProbe.java` runs `generateReport` on the clone's HEAD jar
//! (plan-5 ruling 1) and hands the result to the same `GsonProvider.GSON`, so the committed
//! `<stem>.head.json` files are **Gson's own bytes**. The single normalisation is ruling 3's, and
//! it happens on the Java side *before* Gson sees the object: each entry's `items` list is sorted
//! by numeric uuid, because it comes out of a `HashSet<Item>` on the JVM (quirk #144). `date` is
//! the real `ZonedDateTime.now()` of that run; the port has no clock (ruling 5), so
//! [`date_of`] reads it back out of the golden and injects it — which is also what makes
//! [`dates_are_iso_offset`] a round-trip of a *real* `ISO_OFFSET_DATE_TIME` string.
//!
//! `data/gson-escapes.txt` is the same probe's `--escapes` mode: the four one-line facts about
//! `GsonProvider.GSON` that no fixture exercises — `Double.toString` for `qualityScore`, HTML
//! escaping **off** (`GsonProvider.java:15`) and `U+2028`/`U+2029` escaped anyway.
//!
//! # The fixture that is not committed as a golden here
//!
//! Natural Tone Preamp's report is hash-dependent on the JVM (113-115 violations; only
//! `-XX:hashCode=2` and `=3` reproduce run to run) and the port emits 112 — Task 4's finding,
//! pinned at report level by `tests/report.rs`'s
//! `natural_tone_preamp_is_the_jvms_maximal_run_minus_three_dangling_tracks`. Its JSON was checked
//! against the JVM once while writing this task, at byte level: the `-XX:hashCode=2` run's
//! document, with exactly the three `track_dangling` entries whose item uuids are 1909, 1696 and
//! 1242 deleted from the `violations` array, is byte-identical to the port's 279 222 bytes. The
//! 280 KB golden is not committed, because the serialiser it would exercise is
//! content-independent and is already pinned by the three fixtures below — which between them
//! carry all five `type` strings, both `items` shapes, negative and positive coordinates, and the
//! empty-array case. `tests/data/README.md` records the command that reproduces the check.

use fr_board::prelude::*;
use fr_drc::DesignRulesChecker;
use fr_drc::report::{
    DrcCoordinates, DrcJsonFlavor, DrcReportOptions, KiCadDrcPosition, KiCadDrcReport,
};
use fr_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};

// ---------------------------------------------------------------------------------------------
// Fixtures and helpers
// ---------------------------------------------------------------------------------------------

const DEV_BOARD: &str = "Issue575-drc_dev-board_4_hole_clearance_violations.dsn";
const BBD_MARS_64: &str = "Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn";
const EMPTY_BOARD: &str = "empty_board.dsn";

/// The three fixtures whose Gson bytes are committed under `tests/data/<stem>.head.json`.
const GOLDEN_FIXTURES: [&str; 3] = [DEV_BOARD, BBD_MARS_64, EMPTY_BOARD];
/// `Constants.FREEROUTING_VERSION` of the jar the goldens came from — the same jar and the same
/// literal as `tests/report.rs`. A rebuilt jar with a new version needs both updated.
const JAR_VERSION: &str = "2.3.1-SNAPSHOT";

/// The dev board's `quality_score` as the 2.3.0 CLI printed it (`-de … -drc …`, verified while
/// writing this task): a `float` widened to `double` (Freerouting.java:349), which is where the
/// `.078369140625` tail comes from. `fr-drc` never computes it — it is injected (ruling 5).
const DEV_BOARD_SCORE: f64 = 902.078369140625;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

/// The `<stem>.head.json` `JsonProbe.java` wrote for `fixture` (a `.dsn` name).
fn golden(fixture: &str) -> String {
    let stem = fixture.strip_suffix(".dsn").expect("a .dsn fixture name");
    let path = data_dir().join(format!("{stem}.head.json"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read golden {}: {e}", path.display()))
}

/// The `date` the JVM run of `<stem>` stamped, read back out of its golden so the port can be
/// handed the identical string (ruling 5 — the port has no clock).
fn date_of(golden: &str) -> String {
    for line in golden.lines() {
        if let Some(rest) = line.trim().strip_prefix("\"date\": \"") {
            return rest.trim_end_matches(',').trim_matches('"').to_string();
        }
    }
    panic!("no date in the golden");
}

fn fixture_board(name: &str) -> (Board, CoordinateTransform) {
    let path = parity::fixture(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    match fr_dsn::read_board(&bytes[..], None, Some(name), &DsnReadOptions::default()) {
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
            coordinate_transform.unwrap_or_else(|| panic!("{name} produced no transform")),
        ),
        other => panic!("{name} did not read: {other:?}"),
    }
}

fn options(source: &str, date: &str, quality_score: Option<f64>) -> DrcReportOptions {
    DrcReportOptions {
        source: source.to_string(),
        // `Freerouting.initializeDrc` hard-codes `"mm"` (Freerouting.java:335-336, quirk #151).
        coordinate_unit: "mm".to_string(),
        date: date.to_string(),
        freerouting_version: JAR_VERSION.to_string(),
        quality_score,
    }
}

/// `report_to_json` end to end — the port of `generateReportJson`
/// (DesignRulesChecker.java:817-820).
fn json_for(
    fixture: &str,
    date: &str,
    quality_score: Option<f64>,
    flavor: DrcJsonFlavor,
) -> String {
    let (mut board, transform) = fixture_board(fixture);
    let board_unit = board.communication.unit;
    let coords = DrcCoordinates {
        transform,
        board_unit,
    };
    DesignRulesChecker::new(&mut board)
        .report_to_json(&coords, &options(fixture, date, quality_score), flavor)
        .expect("the report serialises")
}

/// The same, with plan-5 ruling 3's normalisation applied to the report *before* it is
/// serialised, so the bytes compared against the JVM are still entirely the port's serialiser's.
///
/// The normalisation is one sort, and only on `unconnectedItems`: those entries' `items` are
/// `connectedSets.get(0)` then `connectedSets.get(1)` (DesignRulesChecker.java:143-145), two
/// `HashSet<Item>`s, so the JVM's order is identity-hash order and there is nothing to copy
/// (quirk #144). A `violations` entry is left alone — a clearance entry's two items are
/// `[firstItem, secondItem]` in `ClearanceViolation`'s deterministic order (`:319-324`), which the
/// port reproduces (`tests/data/*.list.txt`: `first=278 second=277`), and sorting them would hide
/// a real divergence. `JsonProbe.java` normalises the Java side identically.
fn normalised_json_for(fixture: &str, date: &str, flavor: DrcJsonFlavor) -> String {
    let (mut board, transform) = fixture_board(fixture);
    let board_unit = board.communication.unit;
    let coords = DrcCoordinates {
        transform,
        board_unit,
    };
    let mut report =
        DesignRulesChecker::new(&mut board).generate_report(&coords, &options(fixture, date, None));
    for entry in &mut report.unconnected_items {
        entry
            .items
            .sort_by_key(|item| item.uuid.parse::<i64>().expect("a numeric uuid"));
    }
    report.to_json(flavor).expect("the report serialises")
}

/// The top-level keys of a two-space pretty document, in the order they were written — the whole
/// point of the flavor tests, and something a `serde_json::Value` round trip would destroy.
fn top_level_keys(json: &str) -> Vec<String> {
    json.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("  \"")?;
            if line.starts_with("   ") {
                return None;
            }
            Some(rest.split_once("\": ")?.0.to_string())
        })
        .collect()
}

/// The line of `json` that carries `key`, stripped — the shape `gson-escapes.txt` records.
fn line_with(json: &str, key: &str) -> String {
    json.lines()
        .find(|line| line.contains(key))
        .unwrap_or_else(|| panic!("no {key} in {json}"))
        .trim()
        .to_string()
}

/// A report with nothing in it but the three constructor strings, for the escape and number
/// checks — Java's `new KiCadDrcReport(unit, source, version)` (KiCadDrcReport.java:66-74).
fn bare_report(source: &str) -> KiCadDrcReport {
    KiCadDrcReport::new("mm", source, "Freerouting probe", "2026-08-29T00:00:00Z")
}

// ---------------------------------------------------------------------------------------------
// The two flavors' key order
// ---------------------------------------------------------------------------------------------

/// Measured on the HEAD jar (`tests/data/*.head.json`, and ruling 1's own run).
#[test]
fn head_flavor_key_order() {
    if !parity::require_java_dir() {
        return;
    }
    let date = date_of(&golden(DEV_BOARD));

    let with_score = json_for(
        DEV_BOARD,
        &date,
        Some(DEV_BOARD_SCORE),
        DrcJsonFlavor::default(),
    );
    assert_eq!(
        top_level_keys(&with_score),
        [
            "$schema",
            "coordinateUnits",
            "date",
            "kicadVersion",
            "freeroutingVersion",
            "source",
            "unconnectedItems",
            "violations",
            "schematicParity",
            "qualityScore",
        ]
    );

    // `quality_score: None` is Java's `null`, which Gson omits (`serializeNulls` is off,
    // GsonProvider.java:12-20) — the `generateReportJson` path, which never sets it.
    let without = json_for(DEV_BOARD, &date, None, DrcJsonFlavor::FreeroutingHead);
    assert_eq!(top_level_keys(&without), top_level_keys(&with_score)[..9]);
    assert!(!without.contains("qualityScore"));
}

/// Measured on `tools/freerouting-2.3.0.jar`: `javap -p app/freerouting/drc/DrcReport.class`
/// declares the ten fields in this order, and the jar's own `-de … -drc …` run on the dev board
/// writes them in it.
#[test]
fn kicad_flavor_key_order() {
    if !parity::require_java_dir() {
        return;
    }
    let date = date_of(&golden(DEV_BOARD));
    let json = json_for(
        DEV_BOARD,
        &date,
        Some(DEV_BOARD_SCORE),
        DrcJsonFlavor::KiCad,
    );
    assert_eq!(
        top_level_keys(&json),
        [
            "$schema",
            "coordinate_units",
            "date",
            "kicad_version",
            "freerouting_version",
            "source",
            "unconnected_items",
            "violations",
            "schematic_parity",
            "quality_score",
        ]
    );
}

/// A mechanical proof that the flavor switch touches nothing else: applying the key table's eight
/// substitutions to the HEAD document — as *quoted tokens*, so a `type` value is rewritten by the
/// same rule as a key — yields the KiCad document byte for byte, indentation and order included.
///
/// The plan's ruling 2 calls this "nine strings"; the table has **eight** entries and seven
/// distinct rewrites, because `unconnectedItems` → `unconnected_items` is both the key and one of
/// the two `type` values. The count is the plan's arithmetic, not a measurement.
#[test]
fn flavors_differ_only_in_the_key_tables_eight_strings() {
    if !parity::require_java_dir() {
        return;
    }
    const REWRITES: [(&str, &str); 8] = [
        ("coordinateUnits", "coordinate_units"),
        ("kicadVersion", "kicad_version"),
        ("freeroutingVersion", "freerouting_version"),
        ("unconnectedItems", "unconnected_items"),
        ("schematicParity", "schematic_parity"),
        ("qualityScore", "quality_score"),
        ("holeClearance", "hole_clearance"),
        // The eighth entry is the `type` half of `unconnectedItems`, rewritten by the same pair.
        ("unconnectedItems", "unconnected_items"),
    ];

    for fixture in GOLDEN_FIXTURES {
        let date = date_of(&golden(fixture));
        let head = json_for(
            fixture,
            &date,
            Some(DEV_BOARD_SCORE),
            DrcJsonFlavor::FreeroutingHead,
        );
        let kicad = json_for(fixture, &date, Some(DEV_BOARD_SCORE), DrcJsonFlavor::KiCad);

        let mut renamed = head.clone();
        for (from, to) in REWRITES {
            renamed = renamed.replace(&format!("\"{from}\""), &format!("\"{to}\""));
        }
        assert_eq!(renamed, kicad, "{fixture}");

        // And the rewrite is not vacuous: the two documents really do differ.
        assert_ne!(head, kicad, "{fixture}");
    }
}

// ---------------------------------------------------------------------------------------------
// Byte parity with `GsonProvider.GSON`
// ---------------------------------------------------------------------------------------------

/// `to_json(FreeroutingHead)` against the bytes `GsonProvider.GSON.toJson(report)` wrote on the
/// HEAD jar, for three fixtures — 2 hole-clearance + 8 dangling entries with a 73-item unconnected
/// net, 96 entries over all four remaining `type` strings, and the empty board's three empty
/// arrays.
#[test]
fn head_flavor_is_the_jvms_gson_bytes() {
    if !parity::require_java_dir() {
        return;
    }
    for fixture in GOLDEN_FIXTURES {
        let expected = golden(fixture);
        // `generateReportJson` never sets `qualityScore`, so the goldens omit it.
        let actual =
            normalised_json_for(fixture, &date_of(&expected), DrcJsonFlavor::FreeroutingHead);
        assert_eq!(actual, expected, "{fixture}");
    }
}

/// The four facts about `GsonProvider.GSON` no fixture exercises, against the JVM's own lines
/// (`JsonProbe --escapes`): `Double.toString` rather than `%f`, HTML escaping **off**
/// (`disableHtmlEscaping()`, GsonProvider.java:15) and `U+2028`/`U+2029` escaped regardless.
#[test]
fn quality_score_is_java_double_text() {
    let expected = std::fs::read_to_string(data_dir().join("gson-escapes.txt"))
        .expect("cannot read tests/data/gson-escapes.txt");

    let mut score = bare_report("probe");
    score.quality_score = Some(DEV_BOARD_SCORE);
    let score_902 = line_with(
        &score.to_json(DrcJsonFlavor::FreeroutingHead).unwrap(),
        "qualityScore",
    );

    score.quality_score = Some(1.0e7);
    let score_1e7 = line_with(
        &score.to_json(DrcJsonFlavor::FreeroutingHead).unwrap(),
        "qualityScore",
    );

    let html = bare_report("<'&=>\"")
        .to_json(DrcJsonFlavor::FreeroutingHead)
        .unwrap();
    let separators = bare_report("a\u{2028}b\u{2029}c")
        .to_json(DrcJsonFlavor::FreeroutingHead)
        .unwrap();

    let actual = format!(
        "qualityScore-902\t{score_902}\n\
         qualityScore-1e7\t{score_1e7}\n\
         source-html\t{}\n\
         source-separators\t{}\n",
        line_with(&html, "\"source\""),
        line_with(&separators, "\"source\""),
    );
    assert_eq!(actual, expected);

    // The two that the plan calls out by name (`1.0E7` is what Rust's `{}` would *not* print).
    assert!(score_1e7.ends_with("1.0E7"), "{score_1e7}");
    assert!(score_902.ends_with("902.078369140625"), "{score_902}");
}

/// `$schema` survives verbatim, and `schematicParity` is the empty array Java constructs and never
/// fills (KiCadDrcReport.java:52-53, `:73`).
#[test]
fn schema_is_verbatim_and_schematic_parity_is_empty() {
    let json = bare_report("probe")
        .to_json(DrcJsonFlavor::FreeroutingHead)
        .unwrap();
    assert!(
        json.contains("  \"$schema\": \"https://schemas.kicad.org/drc.v1.json\",\n"),
        "{json}"
    );
    // Last key when `quality_score` is `None`, so no trailing comma.
    assert!(json.contains("\n  \"schematicParity\": []\n}"), "{json}");
    // `disableHtmlEscaping()` — none of Gson's HTML escapes may appear anywhere.
    for escape in ["\\u003c", "\\u003e", "\\u0026", "\\u003d", "\\u0027"] {
        assert!(!json.contains(escape), "{escape} in {json}");
    }
}

/// Java's `ZonedDateTime.now().format(ISO_OFFSET_DATE_TIME)` (KiCadDrcReport.java:70) is an
/// injected string in the port (ruling 5), so the only thing to pin is that it round-trips
/// unmangled — offset, fractional seconds and all. The sample is a real HEAD-jar run's.
#[test]
fn dates_are_iso_offset() {
    const SAMPLED: &str = "2026-08-29T01:38:28.155317-07:00";
    let mut report = bare_report("probe");
    report.date = SAMPLED.to_string();
    let json = report.to_json(DrcJsonFlavor::FreeroutingHead).unwrap();
    assert!(
        json.contains(&format!("  \"date\": \"{SAMPLED}\",\n")),
        "{json}"
    );

    // And the goldens really carry that shape, so the sample is not invented.
    if parity::require_java_dir() {
        let date = date_of(&golden(DEV_BOARD));
        assert_eq!(date.len(), SAMPLED.len(), "{date}");
        assert!(
            date.contains('T') && (date.contains('+') || date[10..].contains('-')),
            "{date}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// The `KiCad` flavor against KiCad itself
// ---------------------------------------------------------------------------------------------

/// The `KiCad` flavor is named after a real schema, so it is checked against real KiCad output:
/// `../freerouting/fixtures/*-kicad_drc.json`, written by KiCad 9.0.1 (plan-5 ruling 1).
///
/// The comparison is **structural** — the values are a different tool's. KiCad's own document has
/// eight top-level keys; freerouting adds exactly two of its own, `freerouting_version` and
/// `quality_score`, neither of which is in `https://schemas.kicad.org/drc.v1.json`'s vocabulary.
/// Everything else — the key spellings, the per-violation and per-item key sets, and both flavored
/// `type` strings — has to match.
#[test]
fn kicad_flavor_matches_the_real_kicad_schema() {
    if !parity::require_java_dir() {
        return;
    }
    const KICAD_FIXTURE: &str =
        "Issue575-drc_Natural_Tone_Preamp_7_unconnected_items-kicad_drc.json";
    let kicad: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(parity::fixture(KICAD_FIXTURE)).unwrap())
            .expect("the KiCad fixture parses");

    let date = date_of(&golden(DEV_BOARD));
    let ours: serde_json::Value = serde_json::from_str(&json_for(
        DEV_BOARD,
        &date,
        Some(DEV_BOARD_SCORE),
        DrcJsonFlavor::KiCad,
    ))
    .expect("our document parses");

    let keys = |v: &serde_json::Value| -> Vec<String> {
        v.as_object()
            .expect("an object")
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    };

    let theirs = keys(&kicad);
    let mine = keys(&ours);
    let extra: Vec<&String> = mine.iter().filter(|k| !theirs.contains(k)).collect();
    assert_eq!(extra, ["freerouting_version", "quality_score"]);
    let missing: Vec<&String> = theirs.iter().filter(|k| !mine.contains(k)).collect();
    assert!(missing.is_empty(), "{missing:?}");

    // Per-violation and per-item key sets, exactly.
    let sample = |v: &serde_json::Value| -> (Vec<String>, Vec<String>) {
        for list in ["violations", "unconnected_items"] {
            if let Some(first) = v[list].as_array().and_then(|a| a.first()) {
                let item = first["items"].as_array().and_then(|a| a.first()).unwrap();
                return (keys(first), keys(item));
            }
        }
        panic!("no violation to sample");
    };
    assert_eq!(sample(&kicad), sample(&ours));

    // Both flavored `type` values are KiCad's own vocabulary, and so are the three that do not
    // move — collected over all three KiCad fixtures.
    let mut kicad_types = std::collections::BTreeSet::new();
    for name in [
        KICAD_FIXTURE,
        "Issue575-drc_dev-board_4_hole_clearance_violations-kicad_drc.json",
        "Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations-kicad_drc.json",
    ] {
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(parity::fixture(name)).unwrap()).unwrap();
        for list in ["violations", "unconnected_items"] {
            for entry in doc[list].as_array().into_iter().flatten() {
                kicad_types.insert(entry["type"].as_str().unwrap().to_string());
            }
        }
    }
    for ours in [
        "clearance",
        "hole_clearance",
        "unconnected_items",
        "track_dangling",
        "via_dangling",
    ] {
        assert!(kicad_types.contains(ours), "KiCad never writes {ours}");
    }
    // And HEAD's two spellings are *not* KiCad's — quirk #154, the reason this flavor exists.
    for head_only in ["holeClearance", "unconnectedItems"] {
        assert!(!kicad_types.contains(head_only), "{head_only}");
    }
}

// ---------------------------------------------------------------------------------------------
// The checker's entry point
// ---------------------------------------------------------------------------------------------

/// `report_to_json` is `generateReportJson`: `generateReport` then the serialiser, nothing else.
#[test]
fn report_to_json_is_generate_report_then_to_json() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, transform) = fixture_board(DEV_BOARD);
    let board_unit = board.communication.unit;
    let coords = DrcCoordinates {
        transform,
        board_unit,
    };
    let options = options(DEV_BOARD, "2026-08-29T00:00:00Z", None);

    let mut checker = DesignRulesChecker::new(&mut board);
    let direct = checker
        .generate_report(&coords, &options)
        .to_json(DrcJsonFlavor::FreeroutingHead)
        .unwrap();
    let through = checker
        .report_to_json(&coords, &options, DrcJsonFlavor::FreeroutingHead)
        .unwrap();
    assert_eq!(direct, through);
}

/// The one thing that can fail: Gson throws `IllegalArgumentException` on a non-finite float,
/// because `GsonProvider` never calls `serializeSpecialFloatingPointValues()`
/// (GsonProvider.java:12-20). `DrcError::Json` exists for exactly this. Nothing `generate_report`
/// produces is non-finite; a hand-built report can be.
#[test]
fn a_non_finite_coordinate_is_refused_where_gson_throws() {
    let mut report = bare_report("probe");
    report.quality_score = Some(f64::NAN);
    assert!(report.to_json(DrcJsonFlavor::FreeroutingHead).is_err());

    let mut report = bare_report("probe");
    report.add_violation(fr_drc::report::KiCadDrcViolation::new(
        "clearance",
        "d",
        "error",
        vec![fr_drc::report::KiCadDrcViolationItem::new(
            "i",
            KiCadDrcPosition::new(0.0, f64::INFINITY),
            "1",
        )],
    ));
    assert!(report.to_json(DrcJsonFlavor::KiCad).is_err());
}

/// The DTOs are plain data, so the serialiser has to work on a hand-built report too — the shape
/// Plan 8's MCP tool will hand it.
#[test]
fn a_hand_built_position_renders_through_the_java_formatter() {
    let mut report = bare_report("probe");
    report.add_violation(fr_drc::report::KiCadDrcViolation::new(
        "clearance",
        "d",
        "error",
        vec![fr_drc::report::KiCadDrcViolationItem::new(
            "i",
            // `Double.toString` diverges from Rust's `{}` outside [1e-3, 1e7).
            KiCadDrcPosition::new(1.0e7, -72.18960000000001),
            "1",
        )],
    ));
    let json = report.to_json(DrcJsonFlavor::FreeroutingHead).unwrap();
    assert!(json.contains("\"x\": 1.0E7"), "{json}");
    assert!(json.contains("\"y\": -72.18960000000001"), "{json}");
}
