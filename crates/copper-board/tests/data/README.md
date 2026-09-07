# `copper-board` test data

The transcripts here are expected-value tables for
`crates/copper-board/tests/clearance_violations.rs`: for every item of the dev board, each
clearance violation's `(firstItem, secondItem, layer, expectedClearance, actualClearance)`, the
aggregated list sorted by severity, the smallest clearance, and seven synthetic box pairs for the
bisection. Doubles are printed the way `copper_dsn::format::double::format_double` prints them,
so the comparison is exact.

`copper-board` cannot read a `.dsn` itself (the dependency runs the other way), so the
whole-board rows are checked one crate up.
