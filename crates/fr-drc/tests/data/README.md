# `fr-drc` JVM probes and transcripts (Plan 5)

JUnit-free Java drivers whose output is the source of the expected values in this crate's tests.
They are committed so the numbers can be re-checked against a rebuilt jar.

| Driver | What it probes | Needs the jar |
|---|---|---|
| `DrcListProbe.java` | `DesignRulesChecker.getAllClearanceViolations()` (DesignRulesChecker.java:52-81) — the **deduplicated** list, in walk order, one line per violation with every field. The complement of Task 2's `crates/fr-board/tests/data/DrcProbe.java`, which dumps the *per item* lists before deduplication. Doubles are printed with `Double.toString`, so the port compares exact bits through `fr_dsn::format::double::java_double_to_string`. | yes |
| `UnconnectedProbe.java` | `DesignRulesChecker.getAllUnconnectedItems()` (DesignRulesChecker.java:91-178) — the **hash-independent projection** of the list (see below). Writes the transcript to the file named by its second argument, not to stdout, because `FRLogger` prints a warning line to stdout on one of the fixtures. | yes |

| Transcript | Fixture | Rows |
|---|---|---|
| `Issue575-drc_dev-board_4_hole_clearance_violations.list.txt` | the dev board | 2 |
| `Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.list.txt` | BBD Mars-64 | 76 |
| `Issue575-drc_dev-board_4_hole_clearance_violations.unconnected.txt` | the dev board | 4 nets / 8 candidates / 0 vias |
| `Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.unconnected.txt` | BBD Mars-64 | 3 / 2 / 18 |
| `Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.unconnected.txt` | Natural Tone Preamp | 44 / 111 / 4 |

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

## What `*.unconnected.txt` can and cannot hold

`getAllUnconnectedItems` is **not reproducible run to run on the JVM**: `connectedSets` is a
`HashSet<Item>` (DesignRulesChecker.java:123) over a class with no `hashCode` override, and
`itemsByNet` is a `HashMap` (`:95`). Plan-5 ruling 3 measured that and fixed the port's order
(ascending item id, ascending net number, quirks row #144). So the probe prints only what both
sides can be held to:

- the net entries **sorted by net number**, each with its `items` list **sorted ascending** and
  each representative reduced to its **kind class** (`Pin` / `Trace` / `other`) — a set holding a
  Pin always yields a Pin, one holding no Pin but a Trace always yields a Trace (`:188-198`), so
  the class survives any hash order even though the item does not;
- `track_dangling_candidates`: the trace phase **before** its dedup (`:152-158` without `:160`),
  i.e. every trace with a contact-free end, in `board.getItems()` order;
- `via_dangling`: as emitted — that phase has no dedup at all (`:168-175`).

The **emitted** `track_dangling` count is deliberately *not* in the transcript, because it is
hash-dependent: the dedup at `:160` drops whichever dangling trace a net entry's `firstItem`
happens to be, and `findRepresentativeItem` picks that item out of a `HashSet`. Measured on this
jar, the Natural Tone Preamp fixture emits

| run | emitted `track_dangling` |
|---|---|
| `-XX:hashCode=0` | 110, then 111 on a second run (mode 0 is a PRNG) |
| `-XX:hashCode=1`, `=2` | 111 |
| `-XX:hashCode=3`, `=4` | 109 |
| default | 110 |

out of 111 candidates, because 4 of its 44 net entries have a `Trace` representative and between
0 and 2 of those happened to be dangling. The port's ascending-id rule makes 3 of the 4 dangling,
so it emits **108** — a different point in the same space, pinned by
`natural_tone_preamp_phase_counts` as a regression guard rather than as Java parity. The dev
board (8 of 8) and BBD Mars-64 (2 of 2) are hash-stable, and `the_three_fixtures_match_the_jvm`
reconstructs the candidate set from the port's output — emitted entries plus the net-entry
representatives that are dangling traces — so the comparison stays exact on all three.

## Recorded command for `UnconnectedProbe`

Same jar as above (`freerouting-current-executable.jar`, 63 288 650 bytes, mtime 2026-08-27
20:03), same JDK 25.

```sh
JAR=/Users/em/Development/freerouting/freerouting/build/libs/freerouting-current-executable.jar
F=/Users/em/Development/freerouting/freerouting/fixtures
J=/opt/homebrew/opt/openjdk@25/bin

$J/javac -cp "$JAR" -d . UnconnectedProbe.java
for b in Issue575-drc_dev-board_4_hole_clearance_violations \
         Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations \
         Issue575-drc_Natural_Tone_Preamp_7_unconnected_items; do
  $J/java -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
      -cp "$JAR:." UnconnectedProbe "$F/$b.dsn" "$b.unconnected.txt"
done
```

The hash sweep that establishes the projection is stable adds
`-XX:+UnlockExperimentalVMOptions -XX:hashCode=$h` for `h` in `0..4`; all three transcripts come
out byte-identical across the five modes and the default, while the `info: emitted
track_dangling=` line the probe writes to **stderr** moves as tabulated above.
