# `fr-drc` JVM probes and transcripts (Plan 5)

JUnit-free Java drivers whose output is the source of the expected values in this crate's tests.
They are committed so the numbers can be re-checked against a rebuilt jar.

| Driver | What it probes | Needs the jar |
|---|---|---|
| `DrcListProbe.java` | `DesignRulesChecker.getAllClearanceViolations()` (DesignRulesChecker.java:52-81) — the **deduplicated** list, in walk order, one line per violation with every field. The complement of Task 2's `crates/fr-board/tests/data/DrcProbe.java`, which dumps the *per item* lists before deduplication. Doubles are printed with `Double.toString`, so the port compares exact bits through `fr_dsn::format::double::java_double_to_string`. | yes |

| Transcript | Fixture | Rows |
|---|---|---|
| `Issue575-drc_dev-board_4_hole_clearance_violations.list.txt` | the dev board | 2 |
| `Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.list.txt` | BBD Mars-64 | 76 |

`the_ordered_list_matches_the_jvm` (`tests/clearance_list.rs`) renders the port's list in this
exact format and compares it byte for byte against both files. Neither fixture takes a `.rules`
file: `RatsnestClearanceHeadlessTest` and `KiCadDrcViolationRoutingTest` both load the plain
`.dsn`, and so does the probe.

The other two fixtures the suite covers — `Issue575-drc_Natural_Tone_Preamp_7_unconnected_items`
and `empty_board` — produce `count 0`, so they are asserted by count alone rather than by a
one-line transcript. (The Natural Tone Preamp run also writes one `FRLogger` warning line to
**stdout** ahead of the count; the port has no logger, which is why that fixture is not a
byte-comparison golden.)

## Recorded command

Run from a scratch directory; the jar is the clone's HEAD build (plan-5 ruling 1), which was
`freerouting-current-executable.jar`, 63 288 650 bytes, mtime 2026-08-27 20:03 when these
transcripts were taken — the same jar Task 2's transcripts came from.

```sh
JAR=/Users/em/Development/freerouting/freerouting/build/libs/freerouting-current-executable.jar
F=/Users/em/Development/freerouting/freerouting/fixtures
ls -la "$JAR"   # record size + mtime alongside any transcript

/opt/homebrew/opt/openjdk@25/bin/javac -cp "$JAR" -d . DrcListProbe.java
for b in Issue575-drc_dev-board_4_hole_clearance_violations \
         Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations; do
  /opt/homebrew/opt/openjdk@25/bin/java -Djava.awt.headless=true -Duser.language=en \
      -Duser.country=US -cp "$JAR:." DrcListProbe "$F/$b.dsn" > "$b.list.txt"
done
```
