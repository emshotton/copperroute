//! `OutlineJson.clearance` — the one of `KiCadBoardJson`'s four never-read DTO fields that is a
//! **fix** rather than a wire contract.
//!
//! `KiCadJsonReader.java:310` hard-codes `outlineClearanceNo = 1` with the comment "Default
//! clearance class", and `KiCadBoardJson.java:90` calls the field it ignores the "outline/edge
//! clearance class mapping". The writer has always written it back, so the field round-tripped
//! through a reader that never looked at it.
//!
//! fixed: T7 (#281) — **wired, not dropped**. Dropping it would make the DTO the reader's private
//! struct, which survey §9.1 forbids; the other three fields (`NetClassJson.netNames`,
//! `NetJson.id`, `LayerJson.index`) keep their `keep` verdict and go on round-tripping untouched.

use fr_board::{Board, Item};
use fr_dsn::error::BoardReadResult;
use fr_dsn::kicad::{KiCadBoardJson, read_board, write};

fn read(json: &str) -> Board {
    match read_board(json, None) {
        BoardReadResult::Success { board: Some(b), .. } => *b,
        other => panic!("expected a loaded board, got {other:?}"),
    }
}

/// The clearance class the board's outline item carries.
fn outline_clearance_class(board: &Board) -> usize {
    match board.get_outline().and_then(|id| board.get_item(id)) {
        Some(Item::BoardOutline(outline)) => outline.hdr.clearance_class(),
        other => panic!("expected a board outline item, got {other:?}"),
    }
}

/// A board whose `netClasses` declare a `power` class at `0.3 mm` and whose outline asks for
/// `0.3` — the mapping the field's own comment describes. The default resolution for `MM` is
/// 10000, so `0.3` is 3000 internal units, which is `power`'s diagonal in the clearance matrix.
///
/// Class 0 is the reserved `"null"` row and class 1 is `"default"`, so `power` is class 2.
const BOARD_WITH_A_MAPPED_OUTLINE_CLEARANCE: &str = r#"{
    "layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
    "netClasses":[{"name":"power","clearance":0.3}],
    "outline":{"corners":[{"x":0,"y":0},{"x":10,"y":0},{"x":10,"y":10},{"x":0,"y":10}],
               "clearance":0.3}
}"#;

/// The value reaches the board, and comes back out of the writer unchanged.
#[test]
fn the_outline_clearance_round_trips() {
    let board = read(BOARD_WITH_A_MAPPED_OUTLINE_CLEARANCE);
    assert_eq!(
        outline_clearance_class(&board),
        2,
        "the outline takes the `power` class, not Java's hard-coded 1"
    );

    let written: KiCadBoardJson =
        serde_json::from_str(&write(&board, "round-trip")).expect("the writer emits valid JSON");
    let outline = written
        .outline
        .expect("the writer always writes an outline");
    assert!(
        (outline.clearance - 0.3).abs() < 1e-9,
        "the clearance comes back as it went in, got {}",
        outline.clearance
    );

    // And it survives a second pass, which is what makes it a round trip rather than a
    // coincidence of the first read.
    let reread = read(&write(&board, "round-trip"));
    assert_eq!(outline_clearance_class(&reread), 2);
}

/// A clearance that names no class keeps Java's `1`. The reader does not invent a clearance
/// class: a KiCad board's classes are exactly the ones its `netClasses` declare, and a value this
/// reader cannot map is a value it does not understand.
#[test]
fn an_outline_clearance_matching_no_class_keeps_the_default() {
    let board = read(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "netClasses":[{"name":"power","clearance":0.3}],
            "outline":{"corners":[{"x":0,"y":0},{"x":10,"y":0},{"x":10,"y":10}],
                       "clearance":0.75}}"#,
    );
    assert_eq!(outline_clearance_class(&board), 1);
}

/// An absent or zero `clearance` — every board the jar ever wrote before this field meant
/// anything — keeps Java's `1` too, by the `> 0.0` guard rather than by the fallback.
///
/// **None of these three is why the corpus does not move.** Measured over the seven KiCad
/// fixtures: five write `0.5` on a board whose classes are 2000 (and 2540 or 3000), so 5000
/// matches nothing and they take the fallback; the two `*_output_session` fixtures write a value
/// that **does** match — `0.2` -> 2000 and `300.0` at `UM`/`10` -> 3000 — and what it matches is
/// class 1, which is the answer Java hard-coded. Same answer, opposite reason; see
/// `outline_clearance_class`'s value-tie note for the second one, where class 2 has the same
/// value and the ascending search picks 1.
#[test]
fn an_absent_outline_clearance_keeps_the_default() {
    let board = read(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "netClasses":[{"name":"power","clearance":0.3}],
            "outline":{"corners":[{"x":0,"y":0},{"x":10,"y":0},{"x":10,"y":10}]}}"#,
    );
    assert_eq!(outline_clearance_class(&board), 1);
}

/// The two shapes the **corpus** actually takes, both reaching class 1 and neither by the
/// fallback — the cases §2 of the fix's reasoning turns on.
#[test]
fn an_outline_clearance_that_matches_the_default_takes_the_default() {
    // `Issue368-CorneyIslandWireless_output_session.json`'s shape: `0.2` is 2000 at the default
    // `MM`/`10000`, which is class 1's own value. It matches, and what it matches is the default.
    let board = read(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "outline":{"corners":[{"x":0,"y":0},{"x":10,"y":0},{"x":10,"y":10}],
                       "clearance":0.2}}"#,
    );
    assert_eq!(outline_clearance_class(&board), 1);

    // `Issue733-kicad_complex_hierarchy_output_session.json`'s shape: a **value tie**. Class 2's
    // clearance equals class 1's, so 0.3 matches both, and the ascending search answers 1 — the
    // documented lossiness, pinned so it cannot drift into answering 2.
    let board = read(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "netClasses":[{"name":"power","clearance":0.2}],
            "outline":{"corners":[{"x":0,"y":0},{"x":10,"y":0},{"x":10,"y":10}],
                       "clearance":0.2}}"#,
    );
    assert_eq!(
        outline_clearance_class(&board),
        1,
        "class 1 and class 2 are both 2000; the ascending search answers the default"
    );
}

/// The other three §9.1 fields are a **wire contract** and must keep round-tripping: the reader
/// still ignores them and the writer still writes them.
#[test]
fn the_other_three_dto_fields_still_round_trip() {
    let source = r#"{
        "layers":[{"index":7,"name":"F.Cu"},{"index":9,"name":"B.Cu"}],
        "netClasses":[{"name":"power","clearance":0.3,"netNames":["GND","VCC"]}],
        "nets":[{"id":41,"name":"GND","className":"power"}]
    }"#;
    let parsed: KiCadBoardJson = serde_json::from_str(source).expect("parses");
    let layers = parsed.layers.as_ref().expect("layers");
    assert_eq!((layers[0].index, layers[1].index), (7, 9));
    assert_eq!(
        parsed.netClasses.as_ref().expect("netClasses")[0]
            .netNames
            .as_deref(),
        Some(["GND".to_string(), "VCC".to_string()].as_slice())
    );
    assert_eq!(parsed.nets.as_ref().expect("nets")[0].id, 41);

    // The reader ignores all three — the board's layer order is the list position, the net's
    // class comes from `className`, and its number comes from `Nets.add`.
    let board = read(source);
    assert_eq!(board.rules.nets.iter().next().expect("a net").net_number, 1);
    let round_tripped: KiCadBoardJson =
        serde_json::from_str(&write(&board, "wire-contract")).expect("valid JSON");
    assert!(
        round_tripped.layers.is_some() && round_tripped.nets.is_some(),
        "the writer still emits the sections these fields live in"
    );
}
