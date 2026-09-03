//! Plan 9 Task 5: `Wiring.readViaScope`'s net-number loop — quirk **#105**, fixed.
//!
//! This file exists because the row's evidence changed shape. Plan 8 Task 13 closed the last of
//! Plan 3's four zero-coverage paths by asserting that the port reproduced Java's padding
//! *exactly*, row for row, against `tests/data/p8t13-directed-via-net-numbers.txt` — a transcript
//! of the HEAD jar. Task 5 fixes the loop, so the port now **diverges** from that transcript on
//! one row, deliberately. The test below is that divergence written down: it keeps every row the
//! jar and the port still agree on, quotes the one they no longer do, and keeps both jar verdicts
//! so the attribution cannot rot.
//!
//! The register row (`docs/java-quirks.md` #105) and `tests/plan_3_zero_coverage.rs` both point
//! here.

mod common;

/// **fixed: T5 (#105).** `Wiring.readViaScope` (Wiring.java:684-690) declares
/// `int currentIndex = 0` and writes `netNumbers[currentIndex] = currentNet.netNumber` inside its
/// loop over `foundNets` **without ever incrementing it** — unlike the line-for-line identical
/// loop in `readWireScope` (:439-445), which has the `++currentIndex`. A `(via … (net NAME))`
/// whose name carries several subnets therefore reads `netNumbers = [lastSubnetsNumber, 0, 0, …]`
/// where a wire on the same name reads all of them.
///
/// The `++currentIndex` is the whole fix.
///
/// # The fixture
///
/// `tests/data/p8t13-via-net-numbers.dsn` (Plan 8 Task 13). Its `(order U1-1 U2-1 U3-1)` makes
/// `Network.readNetScope`'s `createOrderedSubnets` split `NORDERED` into subnets 1 and 2, and its
/// `(wiring …)` scope puts a via **and** a wire on the bare name, so the two loops stand side by
/// side in one file. Nothing else in the 105-file corpus reaches the padding: several subnets
/// come only from `(order …)` or `(fromto …)` (Network.java:1374-1386, :1401-1406), and no corpus
/// `.dsn` writes either.
///
/// # What moved, and what did not
///
/// One row of the jar transcript, item 5:
///
/// ```text
/// jar:  [item] 5 Via           … nets=[2,0]    <- readViaScope:684-687, no ++currentIndex
/// port: [item] 5 Via           … nets=[1,2]
///       [item] 6 PolylineTrace … nets=[1,2]    <- readWireScope:441-445, unchanged in both
/// ```
///
/// The via now reads what the wire on the same net name has always read, which is the point: the
/// two loops are the same loop, and one of them was missing a token. Every other row of the
/// transcript — the nets, the outline, the three pins, the trace, the item count — is asserted
/// against the jar unchanged.
///
/// # The control asserts the single-subnet case did not move
///
/// `tests/data/p8t13-via-net-numbers-control.dsn` is the same file, byte for byte, except that
/// its via reads `(net NORDERED 1)`. A subnet number `> 0` sends `Wiring.getSubnets`
/// (Wiring.java:226-230) down its single-net branch, so `foundNets.size() == 1`, index 0 is the
/// only index, and the fix cannot change anything. Its whole item graph is still asserted against
/// the jar, row for row.
///
/// # The jar verdicts, kept
///
/// They are the reason the row was ever more than cosmetic. The padded `0` reaches
/// `DesignRulesChecker.calculateAllIncompletes` (:558), whose `rules.nets.get(0)` is
/// `Vector.get(-1)`, so every autoroute pass throws `ArrayIndexOutOfBoundsException: Index -1`
/// and `AutorouteBatchLoop.run` retries for ever: `[jar-cli] exit=<none: still running after
/// 60s>`. The control, one token different, exits 0 with a routed `.ses`. Controller ruling BI.
/// Both assertions are kept below so that "one changed token separates a hang from a routed
/// board" stays a checked claim.
#[test]
fn a_multi_subnet_via_carries_every_net_number() {
    let (board, _) = common::read_directed("via-net-numbers");

    // The nets are the reader's, not the via's, and they did not move.
    common::assert_rows_match(
        &common::directed_nets(&board),
        &common::directed_rows("via-net-numbers", "[net]"),
        "via-net-numbers nets",
    );

    // The item graph, with the one deliberate divergence substituted into the jar's rows. Doing
    // it this way rather than by hand-writing the expected rows keeps every *other* row pinned to
    // the jar: if the reader moves anywhere else, this fails.
    let jar_rows = common::directed_rows("via-net-numbers", "[item");
    let expected: Vec<String> = jar_rows
        .iter()
        .map(|row| row.replace("nets=[2,0]", "nets=[1,2]"))
        .collect();
    assert_ne!(
        jar_rows, expected,
        "the jar transcript must still carry the padded `nets=[2,0]` row — without it this test \
         asserts nothing about the fix"
    );
    common::assert_rows_match(
        &common::directed_items(&board),
        &expected,
        "via-net-numbers item graph",
    );

    // Said directly as well, so the claim is not buried in a substitution.
    let via_row = |rows: &[String]| -> String {
        rows.iter()
            .find(|r| r.starts_with("[item] 5 Via"))
            .expect("the fixture puts the via at id 5")
            .clone()
    };
    assert!(
        via_row(&common::directed_items(&board)).contains("nets=[1,2]"),
        "the bare `(net NORDERED)` via must carry both subnets — quirk #105 fixed"
    );
    assert!(
        via_row(&jar_rows).contains("nets=[2,0]"),
        "and the jar must still pad it, or the divergence has gone away on its own"
    );
    // The wire on the same net name is the loop that always had the `++currentIndex`; the via now
    // agrees with it.
    let wire_row = common::directed_items(&board)
        .into_iter()
        .find(|r| r.starts_with("[item] 6 PolylineTrace"))
        .expect("the fixture puts the wire at id 6");
    assert!(wire_row.contains("nets=[1,2]"));

    // ---- the control: one changed token, and nothing moves --------------------------------
    let (control, _) = common::read_directed("via-net-numbers-control");
    common::assert_rows_match(
        &common::directed_nets(&control),
        &common::directed_rows("via-net-numbers-control", "[net]"),
        "via-net-numbers-control nets",
    );
    common::assert_rows_match(
        &common::directed_items(&control),
        &common::directed_rows("via-net-numbers-control", "[item"),
        "via-net-numbers-control item graph — the single-subnet case is byte-identical to the \
         jar's, before and after the fix",
    );
    assert!(
        via_row(&common::directed_items(&control)).contains("nets=[1]"),
        "the `(net NORDERED 1)` via takes getSubnets' single-net branch and never padded"
    );

    // ---- the jar's own verdict on each -----------------------------------------------------
    let hang = common::test_data("p8t13-directed-via-net-numbers.txt");
    let ok = common::test_data("p8t13-directed-via-net-numbers-control.txt");
    assert!(
        hang.contains("[jar-cli] exit=<none: still running after"),
        "the padded fixture must still hang the HEAD jar"
    );
    assert!(
        hang.contains(
            "[jar-cli] throwable java.lang.ArrayIndexOutOfBoundsException: Index -1 out of bounds"
        ),
        "and it must still hang for the `nets.get(0)` reason, not some other one"
    );
    assert!(
        ok.contains("[jar-cli] exit=0"),
        "the control must still route and exit 0"
    );
    assert!(
        !ok.contains("[jar-cli] throwable "),
        "the control must reach no throwable at all"
    );
}
