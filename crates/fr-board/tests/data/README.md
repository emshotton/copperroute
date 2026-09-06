# `fr-board` JVM probes (Plan 5)

JUnit-free Java drivers whose output is the source of the expected values in
`crates/fr-board/tests/clearance_violations.rs` and of quirks row 153 in `docs/java-quirks.md`.
They are committed so the numbers can be re-checked against a rebuilt jar, and so Plan 5's later
`p5t2` differential can reuse this format instead of re-deriving it.

| Driver | What it probes | Needs the jar |
|---|---|---|
| `DrcProbe.java` | `Item.clearanceViolations()` (Item.java:363-469) over a real DSN board: block **A** dumps, for every item of `board.getItems()` (descending id, quirk #63), each violation's `(firstItem, secondItem, layer, expectedClearance, actualClearance)` plus the `smallestClearance` the call left behind; block **B** is `ClearanceViolation.aggregateSortedBySeverity(board.getItems())` (:64-75) — a **second** pass over the same items, which is what makes quirk #153 visible; block **C** is `ClearanceViolation.smallestClearance(board.getItems())` (:86-94); block **D** invokes the private `Item.calculateClearanceBetweenTwoShapes` (:471-493) by reflection on seven synthetic `IntBox` pairs, which are the goldens `the_bisection_matches_the_jvm` asserts. Doubles are printed with `Double.toString`, so the port compares exact bits through `fr_dsn::format::double::format_double`. | yes |

## Recorded command

Run from a scratch directory; the jar is the clone's HEAD build (plan-5 ruling 1), which was
`freerouting-current-executable.jar`, 63 288 650 bytes, mtime 2026-08-27 20:03 when the Task 2
transcript was taken.

```sh
JAR=/Users/em/Development/freerouting/freerouting/build/libs/freerouting-current-executable.jar
F=/Users/em/Development/freerouting/freerouting/fixtures
ls -la "$JAR"   # record size + mtime alongside any transcript

/opt/homebrew/opt/openjdk@25/bin/javac -cp "$JAR" -d . DrcProbe.java
/opt/homebrew/opt/openjdk@25/bin/java -Djava.awt.headless=true -Duser.language=en \
    -Duser.country=US -cp "$JAR:." DrcProbe \
    "$F/Issue575-drc_dev-board_4_hole_clearance_violations.dsn"
```

Block D is board-independent, so any readable `.dsn` produces the same seven rows. The Task 2
transcripts — the dev board (4 violations, all `actual=0.0`, the bisection's early return) and
`Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn` (110 violations with non-zero
bisection results) — are in `.superpowers/sdd/2026-08-29-plan-5-drc/task-2-report.md`.

**Why there is no fixture-level Rust test here.** `fr-board` cannot depend on `fr-dsn` (the
dependency runs the other way), so nothing in this crate can read a `.dsn`. Task 2 verified the
whole-board parity out of tree, by building the same dump from the Rust side in the differential
harness and diffing it against this probe; the standing regression test for it is Plan 5's `p5t2`
driver.
