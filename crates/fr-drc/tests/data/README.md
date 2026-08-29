# `fr-drc` JVM probes and transcripts (Plan 5)

JUnit-free Java drivers whose output is the source of the expected values in this crate's tests.
They are committed so the numbers can be re-checked against a rebuilt jar.

| Driver | What it probes | Needs the jar |
|---|---|---|
| `DrcListProbe.java` | `DesignRulesChecker.getAllClearanceViolations()` (DesignRulesChecker.java:52-81) — the **deduplicated** list, in walk order, one line per violation with every field. The complement of Task 2's `crates/fr-board/tests/data/DrcProbe.java`, which dumps the *per item* lists before deduplication. Doubles are printed with `Double.toString`, so the port compares exact bits through `fr_dsn::format::double::java_double_to_string`. | yes |
| `UnconnectedProbe.java` | `DesignRulesChecker.getAllUnconnectedItems()` (DesignRulesChecker.java:91-178) — the **hash-independent projection** of the list (see below). Writes the transcript to the file named by its second argument, not to stdout, because `FRLogger` prints a warning line to stdout on one of the fixtures. | yes |
| `NetIncompletesProbe.java` | `drc.NetIncompletes`, per net number, through `DesignRulesChecker.getNetIncompletes` (DesignRulesChecker.java:800-815), which lazily runs `calculateAllIncompletes`. Writes **two** files: `<stem>.netincompletes.txt`, the hash-independent projection (`count`, `getConnectedGroupCount`, `getLengthViolation`, `getMarkerRadius` per net, plus the two totals), and `<stem>.airlines.txt`, the endpoint list, which is hash-**dependent** and is committed for one run as documentation only (plan-5 ruling 4). | yes |
| `IncompletesProbe.java` | `DesignRulesChecker.calculateAllIncompletes` (DesignRulesChecker.java:542-623) and the eight accessors that hang off it: `maxConnections`, `getIncompleteCount()`, `getAllAirlines().length`, `getLengthViolationCount()`, `recalculateLengthViolations()` and the per-net `getIncompleteCount(int)`/`getLengthViolation(int)` — plus `BoardStatistics`' clearance block (`BoardStatistics.java:200-202`, `:338-367`) computed from `getAllClearanceViolations()` the way that block does, because `BoardStatistics` itself is Plan 8's (plan-5 ruling 5). Writes `<stem>.incompletes.txt`. All of it is hash-independent, unlike `NetIncompletesProbe`'s second output. | yes |
| `ReportProbe.java` | `DesignRulesChecker.generateReport` (DesignRulesChecker.java:210-290) and the four `io/kicad/KiCadDrc*.java` DTOs it fills, as **normalised text** rather than as Gson JSON — Task 7 ports the DTOs and the builder, not the serialiser (that is Task 8's, with plan-5 ruling 2's two key flavors). Writes `<stem>.report.txt`. | yes |
| `JsonProbe.java` | `DesignRulesChecker.generateReportJson` (DesignRulesChecker.java:817-820), i.e. `GsonProvider.GSON.toJson(report)` — the **bytes Gson writes** for the same report, which is Task 8's parity target. Writes `<stem>.head.json`. Its `--escapes` mode writes `gson-escapes.txt`, the four one-line facts about `GsonProvider.GSON` no fixture exercises. | yes |

| Transcript | Fixture | Rows |
|---|---|---|
| `Issue575-drc_dev-board_4_hole_clearance_violations.list.txt` | the dev board | 2 |
| `Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.list.txt` | BBD Mars-64 | 76 |
| `Issue575-drc_dev-board_4_hole_clearance_violations.unconnected.txt` | the dev board | 4 nets / 8 candidates / 0 vias |
| `Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.unconnected.txt` | BBD Mars-64 | 3 / 2 / 18 |
| `Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.unconnected.txt` | Natural Tone Preamp | 44 / 111 / 4 |
| `Issue575-drc_dev-board_4_hole_clearance_violations.netincompletes.txt` | the dev board | 47 nets, 9 airlines |
| `Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.netincompletes.txt` | BBD Mars-64 | 94 nets, 3 airlines |
| `Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.netincompletes.txt` | Natural Tone Preamp | 58 nets, 145 airlines |
| `Issue575-drc_*.airlines.txt` | all three | 9 / 3 / 145 endpoint lines, **informational** |
| `Issue575-drc_*.airlines-union.txt` | all three | 17 / 3 / 170 distinct `(net, item, item)` triples over six JVM runs |
| `Issue575-drc_dev-board_4_hole_clearance_violations.incompletes.txt` | the dev board | 96 / 9 / 9 / 2 |
| `Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.incompletes.txt` | BBD Mars-64 | 106 / 3 / 3 / 76 |
| `Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.incompletes.txt` | Natural Tone Preamp | 218 / 145 / 145 / 0 |
| `empty_board.incompletes.txt` | the empty board | 0 / 0 / 0 / 0 |
| `Issue575-drc_dev-board_4_hole_clearance_violations.report.txt` | the dev board | 10 violations / 4 unconnected |
| `Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.report.txt` | BBD Mars-64 | 96 / 3 |
| `Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.report.txt` | Natural Tone Preamp | 115 / 44, the **maximal** run — see below |
| `empty_board.report.txt` | the empty board | 0 / 0 |

(`maxConnections` / `incompleteCount` / `getAllAirlines().length` / `clearanceViolations.totalCount`.)

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

## What `*.netincompletes.txt` holds, and what `*.airlines.txt` deliberately does not

`NetIncompletes` (`drc/NetIncompletes.java:57-226`) is split by plan-5 ruling 4 into a
hash-independent half and a hash-dependent one.

**The parity half** — per net, `count()` (the number of airlines), `getConnectedGroupCount()`,
`getLengthViolation()` and `getMarkerRadius()`, plus `getAllAirlines().length` and
`getIncompleteCount()` — is a graph invariant of the net plus its net class's length limits.
Verified by sweeping the HEAD jar over `-XX:+UnlockExperimentalVMOptions -XX:hashCode=0..4`:

| fixture | distinct `.netincompletes.txt` over 5 modes | distinct `.airlines.txt` over 5 modes |
|---|---|---|
| dev board | **1** | 5 |
| BBD Mars-64 | **1** | 2 |
| Natural Tone Preamp | **1** | 5 |

Those `.airlines.txt` counts are **file digests over five modes**, so they are sensitive to line
order and to which end of an airline the probe printed as `from=`. The second table below counts
something coarser — the per-net set of *unordered* `{from, to}` pairs, over **six** runs (the five
modes plus the default) — which is why BBD Mars-64 reads `2` here and `1` there: its two distinct
transcripts carry the same airlines, printed differently. Read either number as illustrative
rather than as the cardinality of what the jar can produce: `-XX:hashCode=0` and the default
(mode 5) are PRNG-seeded and modes 1 and 4 are derived from object addresses, so only modes 2 and
3 are reproducible at all — Task 4 measured mode 0 giving two different answers on consecutive
runs of this same jar.

`the_three_fixtures_match_the_jvm` (`tests/net_incompletes.rs`) renders the port's per-net results
in this exact format and compares them line by line.

**The hash-dependent half** is the endpoints. `NetIncompletes.calculateNetItems` seeds its outer
loop from a `HashSet<Item>` (`:295`, `:299`), so the Delaunay corner insertion order — and with it
which edges the triangulation even *has* — is identity-hash ordered. `<stem>.airlines.txt` is one
run (the default hash mode) kept as documentation. Measured against it, the port's list differs on

| fixture | port airlines | differing from the committed run | also chosen by ≥1 of the 6 JVM runs | distinct JVM lists over the 6 runs |
|---|---|---|---|---|
| dev board | 9 | 1 | **9 of 9** | 6 |
| BBD Mars-64 | 3 | 0 | **3 of 3** | 1 |
| Natural Tone Preamp | 145 | 6 | **145 of 145** | 6 |

so every airline the port picks is one some JVM run picks: the port's answer sits inside the space
Java itself spans, it is not beside it. The differences are **not** confined to swapping one
edge for an equally long one — on Natural Tone Preamp nets 16 and 23 the port's spanning tree has
a different total weight than the committed run's — but neither are the JVM's own: net 16's total
squared length ranges over 73 717 948 593 … 87 707 629 368 across the six runs, because a
different insertion order changes the edge *set*, not only Kruskal's choice among ties. Quirk #82
(the in-circle degeneracy on axis-aligned input) is unfixed on both sides, which is what keeps the
two algorithms the same algorithm.

`the_airline_endpoints_are_a_hash_dependent_choice` asserts the per-net **counts** and prints the
rest; run it with `--nocapture`.

The one endpoint claim that *is* asserted lives in `<stem>.airlines-union.txt`: the union, over
the six runs, of the unordered `(net, item, item)` triples the jar produced — 17, 3 and 170
triples against 9, 3 and 145 airlines per run. `every_port_airline_is_one_some_jvm_run_picks`
requires every airline the port emits to be in that union, which is run-independent in a way the
list itself is not. The union is not *closed* — a seventh JVM run may add a triple — so a failure
there is a reason to re-derive the file before it is a bug report.

**Re-running the recorded commands below reproduces `*.netincompletes.txt` exactly and will not
reproduce `*.airlines.txt` or `*.airlines-union.txt`.** The first is hash-independent; the other
two are snapshots of particular runs, kept as documentation and as the union's inputs. That is why
nothing compares `*.airlines.txt` byte for byte.

## Recorded command for `NetIncompletesProbe`

Same jar as above (`freerouting-current-executable.jar`, 63 288 650 bytes, mtime 2026-08-27
20:03), same JDK 25.

```sh
JAR=/Users/em/Development/freerouting/freerouting/build/libs/freerouting-current-executable.jar
F=/Users/em/Development/freerouting/freerouting/fixtures
J=/opt/homebrew/opt/openjdk@25/bin

$J/javac -cp "$JAR" -d . NetIncompletesProbe.java
for b in Issue575-drc_dev-board_4_hole_clearance_violations \
         Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations \
         Issue575-drc_Natural_Tone_Preamp_7_unconnected_items; do
  $J/java -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
      -cp "$JAR:." NetIncompletesProbe "$F/$b.dsn" "$b"
done
```

The sweep adds `-XX:+UnlockExperimentalVMOptions -XX:hashCode=$h` for `h` in `0..4`.

The union file is built from the sweep's six `*.airlines.txt` (the five modes plus the default),
in the same scratch directory:

```sh
python3 - <<'EOF'
stems = ["Issue575-drc_dev-board_4_hole_clearance_violations",
         "Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations",
         "Issue575-drc_Natural_Tone_Preamp_7_unconnected_items"]
for stem in stems:
    union = set()
    for path in [f"h{m}-{stem}.airlines.txt" for m in range(5)] + [f"{stem}.airlines.txt"]:
        for line in open(path):
            if not line.strip():
                continue
            f = dict(p.split("=", 1) for p in line.split())
            a, b = sorted((int(f["from"]), int(f["to"])))
            union.add((int(f["net"]), a, b))
    with open(f"{stem}.airlines-union.txt", "w") as out:
        for n, a, b in sorted(union):
            out.write(f"net={n} a={a} b={b}\n")
EOF
```

## What `*.incompletes.txt` holds

Everything Task 6 ports is a **count** or a **length**, so — unlike `NetIncompletesProbe`'s
second output — all of it is hash-independent and all of it is compared exactly by
`the_four_fixtures_match_the_jvm` (`tests/incompletes.rs`). Verified by sweeping the HEAD jar over
`-XX:+UnlockExperimentalVMOptions -XX:hashCode=0..4`: **one** distinct transcript per fixture
across all five modes plus the default, on all four fixtures.

The four `maxConnections` / `incompleteCount` values reproduce plan-5 ruling 4's table and
`KiCadDrcViolationRoutingTest`'s three assertions (9/2, 3/76, 145/0) independently; the per-net
`incompleteCount=` lines are the same quantity `RatsnestClearanceHeadlessTest.java:69-74` sums.

`boardUnitToUmFactor` is `BoardStatistics.java:200-202`'s local, printed so the Rust side
multiplies by the same number rather than by one it re-derived: all four fixtures are 0.1 um per
board unit. `clearanceViolations` is the block at `BoardStatistics.java:338-367`, which the port
ships as `BoardStatisticsClearanceViolations::from_violations` — `BoardStatistics` itself stays in
Plan 8 (plan-5 ruling 5), so the probe transcribes the block rather than constructing the object.

**The fixture list is the Java test's, and it takes no `.rules` file.**
`KiCadDrcViolationRoutingTest.assertDrcOnLoadedBoard` calls `getRoutingJob(filename, null)` and
reads the plain `.dsn` (`:18-28`); the only `BBD_Mars-64` fixture that has a `.rules` beside it is
`Issue593-BBD_Mars-64`, a **different** board that no DRC test loads.

## Recorded command for `IncompletesProbe`

Same jar as above (`freerouting-current-executable.jar`, 63 288 650 bytes, mtime 2026-08-27
20:03), same JDK 25.

```sh
JAR=/Users/em/Development/freerouting/freerouting/build/libs/freerouting-current-executable.jar
F=/Users/em/Development/freerouting/freerouting/fixtures
J=/opt/homebrew/opt/openjdk@25/bin

$J/javac -cp "$JAR" -d . IncompletesProbe.java
for b in Issue575-drc_dev-board_4_hole_clearance_violations \
         Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations \
         Issue575-drc_Natural_Tone_Preamp_7_unconnected_items \
         empty_board; do
  $J/java -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
      -cp "$JAR:." IncompletesProbe "$F/$b.dsn" "$b"
done
```

The sweep adds `-XX:+UnlockExperimentalVMOptions -XX:hashCode=$h` for `h` in `0..4`.


## What `*.report.txt` holds, and the one fixture that is not a byte golden

`ReportProbe.java` prints the report with plan-5 ruling 3's normalisation applied:

- **`date` is dropped entirely.** Java fills it from `ZonedDateTime.now()` (KiCadDrcReport.java:70)
  and the port takes it as an injected string (ruling 5), so there is nothing to compare.
- **Each entry's `items` array is sorted by numeric uuid**, because on the Java side it comes out
  of a `HashSet<Item>` (quirk #144). The one place that sort would hide something real — a
  clearance violation's `[firstItem, secondItem]` pair, which is *not* hash-ordered — is asserted
  unsorted instead, by `first_violation_is_verbatim`.
- **Entry order is kept.** `violations` is hash-independent (ruling 3's probe) and the
  `unconnectedItems` entries come out in ascending net number on both sides.

`qualityScore` is `null` in every transcript: the CLI fills it from `BoardStatistics`
(Freerouting.java:343-352), which is Plan 8's (ruling 5). `freeroutingVersion` is
`Freerouting 2.3.1-SNAPSHOT`, i.e. `Constants.FREEROUTING_VERSION` of the jar below; the Rust
side injects the same literal, so a rebuilt jar with a new version needs `JAR_VERSION` in
`tests/report.rs` updated alongside the transcripts.

`three_fixtures_match_the_jvm_byte_for_byte` (`tests/report.rs`) compares the dev board, BBD
Mars-64 and the empty board byte for byte. **Natural Tone Preamp cannot be one of them**: its
`violations` count is hash-dependent, for the reason recorded under `*.unconnected.txt` above —
`generateReport` folds the `track_dangling` entries into `violations` (`:271-276`), and quirk
#146's dedup drops whichever dangling trace a net entry's hash-ordered `firstItem` happens to be.
Measured on this jar over three separate sweeps (Task 7's, a reviewer's, and Task 8's), the count
lands in **113-115**, and only modes 2 and 3 reproduce run to run:

| run | hash source | `violations` |
|---|---|---|
| `-XX:hashCode=2` | a constant, so **deterministic** | `115` (the maximal run) |
| `-XX:hashCode=3` | a per-thread xorshift, deterministic in a single-threaded run | `114` |
| `-XX:hashCode=0`, `=1`, `=4` | a PRNG (0) or the object address (1, 4) — **not run-reproducible** | `113`, `114` or `115`, varying between sweeps |
| default (`=5`, thread-local xorshift) | seeded per run | `114` or `115` |

`unconnectedItems` is `44` in every run. **Only `-XX:hashCode=2` reproduces the committed
transcript**; regenerate that one fixture with it, and read any other mode's number as a sample
rather than as a bound. (The dev board, BBD Mars-64 and the empty board give **one** digest each
across every mode and every sweep.)

The committed `Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.report.txt` is therefore the
**maximal** run — 115 violations, every one of the 111 dangling-trace candidates plus the four
dangling vias — and
`natural_tone_preamp_is_the_jvms_maximal_run_minus_three_dangling_tracks` requires the port's
report to be that file with exactly three `track_dangling` entries removed **in place** (uuids
1909, 1696 and 1242, each the `firstItem` of a net entry whose group holds no `Pin`). Both sides
walk the board's items descending, so nothing else moves; the whole 44-entry `unconnectedItems`
block is byte-identical, and it is byte-identical across all six JVM runs too.

That is the same divergence Task 4 recorded — the port emits 108 of 111 `track_dangling` where
the JVM emits 109-111 — arriving at the report layer. Plan-5 ruling 11's table row for this
fixture (`114` report violations) is a single-run measurement of a hash-dependent quantity;
**112 is the port's number and must not be hand-tuned to match it.**

## What `*.head.json` and `gson-escapes.txt` hold

`<stem>.head.json` is `GsonProvider.GSON.toJson(report)` verbatim, for the report
`ReportProbe.java`'s transcript describes — the HEAD jar's key spelling (plan-5 ruling 1: camelCase
plus `holeClearance`/`unconnectedItems`), Gson's two-space pretty printing, `Number.toString()`
numbers, and **no** `qualityScore` key, because `generateReportJson` never sets it (Gson's default
`serializeNulls = false`). `tests/report_json.rs::head_flavor_is_the_jvms_gson_bytes` compares the
port's `to_json(FreeroutingHead)` against these files byte for byte.

The one normalisation is plan-5 ruling 3's, and the probe applies it to the DTO **before** Gson
sees it, so both sides of the comparison are still whole Gson/serde documents: each
`unconnectedItems` entry's `items` list is sorted by numeric uuid. Those lists are
`connectedSets.get(0)` followed by `connectedSets.get(1)` (DesignRulesChecker.java:143-145), two
`HashSet<Item>`s, so the JVM's order within each group is identity-hash order (quirk #144) and
there is nothing to copy. **A `violations` entry is left alone**: a clearance entry's two items are
`[firstItem, secondItem]` in `ClearanceViolation`'s own deterministic order (`:319-324`) — the dev
board's first violation really is `first=278 second=277`, as `*.list.txt` records — and sorting
them would hide a real ordering divergence. The Rust test normalises identically.

`date` is the real `ZonedDateTime.now()` of the run (KiCadDrcReport.java:70); the port has no clock
(ruling 5), so the test reads the string back out of the golden and injects it, which also makes
the `ISO_OFFSET_DATE_TIME` shape a round-trip of a real value rather than an invented one.

Three fixtures are committed. **Natural Tone Preamp is not**, for the reason the section above
gives: the port emits 112 violations to the JVM's 113-115. It was checked once, at byte level,
while writing Task 8 — the `-XX:hashCode=2` run's JSON, with exactly the three `track_dangling`
entries whose item uuids are 1909, 1696 and 1242 deleted from the `violations` array, is
byte-identical to the port's document (279 222 bytes). The 280 KB golden is not carried, because
the serialiser it would exercise is content-independent and the three committed fixtures already
cover all five `type` strings, both `items` shapes, positive and negative coordinates and the
empty-array case — while `tests/report.rs` guards that fixture's *content* permanently.

`gson-escapes.txt` is four `<name>\t<the line Gson wrote>` rows from `JsonProbe --escapes`, each a
`GsonProvider.GSON` behaviour no fixture reaches: `qualityScore` `902.078369140625` and `1.0E7`
(`Double.toString`, where Rust's `{}` would print `10000000`), a `source` of `<'&=>"` written raw
(`disableHtmlEscaping()`, GsonProvider.java:15) and a `source` of `a\u2028b\u2029c` escaped anyway.

## Recorded command for `JsonProbe`

Same jar and JDK as `ReportProbe`.

```sh
JAR=/Users/em/Development/freerouting/freerouting/build/libs/freerouting-current-executable.jar
F=/Users/em/Development/freerouting/freerouting/fixtures
J=/opt/homebrew/opt/openjdk@25/bin

$J/javac -cp "$JAR" -d . JsonProbe.java
for b in Issue575-drc_dev-board_4_hole_clearance_violations \
         Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations \
         empty_board; do
  $J/java -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
      -cp "$JAR:." JsonProbe "$F/$b.dsn" "$b.dsn" mm "$b"
done
$J/java -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
    -cp "$JAR:." JsonProbe --escapes gson-escapes.txt
```

Regenerating the goldens changes their `date`, which is fine — the test reads it back out. A
rebuilt jar with a new `Constants.FREEROUTING_VERSION` also needs `JAR_VERSION` in
`tests/report_json.rs` updated, exactly as `tests/report.rs` does.

## Recorded command for `ReportProbe`

Same jar as above (`freerouting-current-executable.jar`, 63 288 650 bytes, mtime 2026-08-27
20:03), same JDK 25.

```sh
JAR=/Users/em/Development/freerouting/freerouting/build/libs/freerouting-current-executable.jar
F=/Users/em/Development/freerouting/freerouting/fixtures
J=/opt/homebrew/opt/openjdk@25/bin

$J/javac -cp "$JAR" -d . ReportProbe.java
for b in Issue575-drc_dev-board_4_hole_clearance_violations \
         Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations \
         Issue575-drc_Natural_Tone_Preamp_7_unconnected_items \
         empty_board; do
  $J/java -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
      -cp "$JAR:." ReportProbe "$F/$b.dsn" "$b.dsn" mm "$b"
done
```

The sweep adds `-XX:+UnlockExperimentalVMOptions -XX:hashCode=$h` for `h` in `0..4`. The other
three fixtures' transcripts were taken from the default run, which agrees with every mode.
**Natural Tone Preamp must be regenerated with `-XX:hashCode=2`** — the only mode that both
reproduces run to run and produces the committed maximal transcript (see the table above):

```sh
$J/java -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
    -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "$JAR:." ReportProbe \
    "$F/Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.dsn" \
    "Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.dsn" mm \
    Issue575-drc_Natural_Tone_Preamp_7_unconnected_items
```
