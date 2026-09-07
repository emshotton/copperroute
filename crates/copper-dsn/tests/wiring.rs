mod common;

#[test]
fn a_multi_subnet_via_carries_every_net_number() {
    let (board, _) = common::read_directed("via-net-numbers");

    common::assert_rows_match(
        &common::directed_nets(&board),
        &common::directed_rows("via-net-numbers", "[net]"),
        "via-net-numbers nets",
    );

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
    let wire_row = common::directed_items(&board)
        .into_iter()
        .find(|r| r.starts_with("[item] 6 PolylineTrace"))
        .expect("the fixture puts the wire at id 6");
    assert!(wire_row.contains("nets=[1,2]"));

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
