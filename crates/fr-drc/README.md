# fr-drc

A behavioral Rust port of freerouting's design-rule checker (clone HEAD):
`drc/{DesignRulesChecker,NetIncompletes,AirLine,UnconnectedItems}.java`, the
four `io/kicad/KiCadDrc*.java` report DTOs and
`core/scoring/BoardStatisticsClearanceViolations.java`. It answers the three
questions Java's `-drc` mode answers — *which clearances are violated, which
items are unconnected, and how many connections are still incomplete* — but
does so with KiCad's own routing-type checks: `get_all_violations()` returns
`DrcViolation`s carrying KiCad's check-family kinds (`kind.kicad_type()`),
not Java's `ClearanceViolation`. Use `fr_drc::prelude::*` to bring in every
public type.

The crate sits on `fr-board` (the boards it checks) and on `fr-dsn` (plan-5
ruling 7: `CoordinateTransform` lives there, and so does the Gson-compatible
JSON formatter). `fr-drc → fr-dsn → fr-board → fr-geometry` stays strict and
acyclic. It depends on those three plus `serde`, `serde_json` and `thiserror`,
and on **nothing else** — in particular **not** on `fr-settings` (ruling 12).
**No `tracing`:** every `FRLogger` call in the Java sources is dropped, and
`DesignRulesChecker` is about 40 % of them. No GUI, no static mutable state,
and **no clock** — `KiCadDrcReport`'s `ZonedDateTime.now()`
(`KiCadDrcReport.java:70`) is an injected `String` (ruling 5).

Deliberate Java bugs are reproduced rather than fixed; each carries a
`// Java bug:` or `// totalized:` marker at the site and a row in
`docs/java-quirks.md`. Rows **144-155** are this crate's.

`ClearanceViolation` remains defined in **`fr-board`**, in
`items/clearance_violation.rs` (ruling 9): `Item.clearanceViolations` returns
it and `fr-board` cannot depend upward on `fr-drc`. It is not this crate's
violation type — this crate's checker produces `DrcViolation` — and stays
`fr-board`'s own construct, for the router's own scoring; the five `Board`
methods behind it (`clearance_violations`, `clearance_violation_count`,
`calculate_clearance_between_two_shapes`,
`aggregate_violations_sorted_by_severity`, `smallest_clearance`) are
`fr-board`'s for the same reason.

## API surface

Everything starts from a checker borrowing the board **mutably**:

```rust,ignore
let mut drc = DesignRulesChecker::new(&mut board);   // ruling 8
```

`&mut Board` is deliberate, not an accident of the port. `Item.clearanceViolations`
has two side effects in Java: it lowers `this.smallestClearance`
(`Item.java:451-453`, quirk #153, and only ever downward — it is never reset)
and it advances the search tree's entry counter through every query
(`ShapeSearchTree.java:55`). Hiding either behind interior mutability would make
quirk #153 invisible. There is no `DesignRulesCheckerSettings` parameter
(ruling 12): Java stores that field and never reads it, and 10 of its 15
constructions pass `null`.

| What you want | Call | Java |
|---|---|---|
| the deduplicated clearance list | `get_all_clearance_violations() -> Vec<ClearanceViolation>` | `DesignRulesChecker.java:52-81` |
| the unconnected/dangling list | `get_all_unconnected_items() -> Vec<UnconnectedItems>` | `:91-178` |
| the ratsnest | `calculate_all_incompletes()`, then `get_all_airlines() -> Vec<AirLine>` | `:542-623`, `:780-798` |
| the counters | `max_connections()`, `get_incomplete_count()`, `get_incomplete_count_for_net(n)`, `get_length_violation_count()`, `get_length_violation(n)`, `recalculate_length_violations()` | `:33`, `:663-778` |
| one net's state | `get_net_incompletes(n) -> Option<&NetIncompletes>`, `recalculate_net_incompletes(n)`, `..._with(n, &[ItemId])` | `:800-815`, `:630-643`, `:647-660` |
| the KiCad DRC report | `generate_report(&DrcCoordinates, &DrcReportOptions) -> KiCadDrcReport` | `:210-290` |
| that report as JSON | `report_to_json(&DrcCoordinates, &DrcReportOptions, DrcJsonFlavor) -> Result<String, DrcError>` | `:817-820` |
| a hand-built report as JSON | `KiCadDrcReport::to_json(DrcJsonFlavor)` | `GsonProvider.GSON.toJson` |
| the clearance half of `BoardStatistics` | `BoardStatisticsClearanceViolations::from_violations(&[ClearanceViolation], board_unit_to_um)` | `BoardStatistics.java:338-367` |
| normalising two DRC documents for comparison | `parity::normalize_drc_json(&str)` (the `tests/parity` helper crate) | — |

`generate_report` and `report_to_json` are `&mut self` for the same reason
`new` takes `&mut Board`: they call `get_all_clearance_violations` internally
(`DesignRulesChecker.java:216`).

### The two injected parameter blocks

`fr-drc` has no clock, no `Constants` and no `BoardStatistics`, so everything
`Freerouting.initializeDrc` supplies arrives as a parameter (ruling 5):

```rust,ignore
pub struct DrcReportOptions {
    pub source: String,               // Freerouting.java:339 — the input file's base name
    pub coordinate_unit: String,      // Freerouting.java:335 — hard-coded "mm" (quirk #151)
    pub date: String,                 // already formatted; Java calls ZonedDateTime.now()
    pub freerouting_version: String,  // without the "Freerouting " prefix, which the port adds
    pub quality_score: Option<f32>,   // Freerouting.java:349
}

pub struct DrcCoordinates {
    pub transform: CoordinateTransform,  // board.communication.coordinateTransform
    pub board_unit: Unit,                // board.communication.unit
}
```

Two notes on those types:

- **`quality_score` is `Option<f32>`, and the widening to the report's `Double`
  is this crate's.** `BoardStatistics.getNormalizedScore` returns a `float`
  (`BoardStatistics.java:624`) and `Freerouting.java:349` widens it with an
  explicit `(double)` cast — which is where every reference's
  `.078369140625`-style tail comes from. Typing the injected value `f64` would
  let a caller hand in a number no jar can produce, and `Double.toString` would
  faithfully render it. `None` is Java's `null`, which Gson omits from the
  document entirely — the only shape `generateReportJson` itself can produce,
  because the CLI assigns the score after the call.
- **`DrcCoordinates` is a parameter because Plan 3 ruling A left
  `CoordinateTransform` in `fr-dsn`.** Java reaches both halves through
  `board.communication` (`DesignRulesChecker.java:507`, `:510`); the port's
  board owns only the unit.

## The two schema flavors

Ruling 1: the clone's HEAD emits **camelCase** keys, and the schema its own
`$schema` names (`https://schemas.kicad.org/drc.v1.json`, which KiCad 9.0.1 and
freerouting 2.3.0 both write) is **snake_case**. That drift is a Java bug
(quirk #154) and is reproduced, because HEAD is what this crate ports and
HEAD's own `DesignRulesCheckerTest.java:88-96` asserts camelCase. Ruling 2
carries both spellings instead of picking one:

| field | `DrcJsonFlavor::FreeroutingHead` (default) | `DrcJsonFlavor::KiCad` |
|---|---|---|
| — | `$schema`, `date`, `source`, `violations` | identical |
| coordinate unit | `coordinateUnits` | `coordinate_units` |
| KiCad version | `kicadVersion` | `kicad_version` |
| freerouting version | `freeroutingVersion` | `freerouting_version` |
| unconnected list | `unconnectedItems` | `unconnected_items` |
| schematic parity | `schematicParity` | `schematic_parity` |
| quality score | `qualityScore` | `quality_score` |
| violation `type` | `holeClearance` | `hole_clearance` |
| violation `type` | `unconnectedItems` | `unconnected_items` |
| violation `type` | `clearance`, `track_dangling`, `via_dangling` | identical |
| per-violation / per-item keys | `description`, `items`, `severity`, `type`, `pos`, `uuid`, `x`, `y` | identical |

Seven distinct string rewrites over eight fields (`unconnectedItems` is both a
key and a `type` value). `crates/fr-drc/tests/report_json.rs::flavors_differ_only_in_the_key_tables_eight_strings`
proves it mechanically: applying those seven substitutions to a HEAD document
as *quoted tokens* yields the KiCad document byte for byte.

~~**Which flavor the `-drc` CLI defaults to is Plan 8's to wire** (ruling 2,
recorded as an `obligation:` marker on the enum)~~ — **Plan 8 Task 7 wired it,
and the `obligation:` is closed.** `crates/freerouting/src/commands/drc.rs`
passes `DrcJsonFlavor::KiCad` **by name**, never `Default::default()`, and
`--schema freerouting` (native form only) is the way back to HEAD's bytes —
which is what `scripts/differential/rust/src/bin/p8t3.rs` runs the port with
for its byte comparison against the jar. `Default` here is the *parity* choice,
not a recommendation, and shipping `KiCad` re-baselined nothing in this crate:
`head_flavor_is_the_jvms_gson_bytes` keeps pinning HEAD's spelling against the
jar, and `tests/reference/drc-*` are still the jar's verbatim camelCase
documents.

Everything below the key is `fr_dsn::format::json`'s
(`to_gson_string_pretty`/`JavaNumberFormatter`, which Plan 5 ruling 7 moved down
from `fr-settings` so both crates share one copy): two-space indent, `": "`
after every key, no trailing newline, every `f64` through `Double.toString`,
HTML escaping **off** but `U+2028`/`U+2029` escaped anyway, and a `null`
omitted rather than written.

## Determinism: ascending item id everywhere Java hashes

Java's DRC is **not** reproducible run to run. `getAllUnconnectedItems` builds
its connected sets as `HashSet<Item>` (`DesignRulesChecker.java:118`) and
`NetIncompletes.calculateNetItems` iterates a `HashSet<Item>` (`:295`, `:299`);
`Item` overrides neither `hashCode` nor `equals`, so both are identity-hash
ordered. Five runs of the HEAD jar under
`-XX:+UnlockExperimentalVMOptions -XX:hashCode=0..4` give five different
`unconnectedItems[0].items` orders on the dev board.

The port fixes an order, deliberately (ruling 3, quirk #144):

| where | port | Java |
|---|---|---|
| `unconnectedItems[].items` | ascending item id | identity-hash order |
| `calculateNetItems`' seed order | ascending item id | identity-hash order |
| `itemsByNet` | `BTreeMap<i32, …>`, ascending net number | `HashMap` (coincides for every corpus fixture) |
| `findRepresentativeItem` | lowest-id `Pin`, else lowest-id `Trace`, else lowest-id item | *a* `Pin`, not a particular one |

**Within** one connected set the order is Java's and is reproduced exactly
(ruling 15): `Item.getConnectedSet` returns a `TreeSet<Item>`
(`Item.java:606`) and `Item.compareTo`'s subtraction is reversed
(`Item.java:95-103`, quirk #44), so a connected set iterates **descending**
item id. The port's `Board::connected_set` is a `BTreeSet<ItemId>`
(ascending), so every DRC walk of one iterates `.rev()`. Getting this backwards
would silently reverse the Delaunay insertion order inside every component —
a wrong-output bug with no crash.

Three consequences worth knowing before reading a diff:

- **The airline *endpoints* are not a parity surface; the airline *counts*
  are** (ruling 4, correcting the port survey). `getAllAirlines` is
  deterministic in its *flattening* — net-number ascending, `LinkedList` order
  within a net — but the airlines themselves come out of a Delaunay
  triangulation whose corner-insertion order is `calculateNetItems`' `HashSet`
  order. `maxConnections`, `getIncompleteCount()`, the per-net counts and the
  whole clearance list are hash-**independent** and are strict parity; which
  equal-length edge the triangulation picks is not. Java's triangulation
  varies with insertion order too (ruling T): net 16's MST total weight moves
  across hash modes.
- **Ruling V: `p5t2` mode 3 is the airline gate.** Rather than grade endpoints
  against a union of JVM runs (which does not converge), the differential
  re-seeds the JVM's `NetIncompletes` in-process with the port's order and
  requires **exact** equality — 112/112 rows — of both airline blocks: the
  canonical `AL` block (the unordered pair, sorted, which says *what the airline
  set is*) and the `ALD` block (`ALD <net> <fromId> <toId>`, in Kruskal's
  acceptance order with the edge's own direction, which says *how it was
  built*). Mode 4 is the self-check that the transcription still reproduces the
  jar's own `getAllAirlines()`, `ALD` block included.
- **Ruling S: Natural Tone Preamp's report differs from any single Java run,
  on purpose.** `generateReport` folds `getAllUnconnectedItems`'
  `track_dangling` entries into `violations`, and that phase's dedup
  (`DesignRulesChecker.java:160`) drops whichever dangling trace happens to be
  a net entry's hash-ordered `firstItem` — 0 to 3 of them depending on the hash
  mode. The port's ascending-id representatives drop exactly **three**, every
  time, so it emits **112** where the jar emits 113-115. That is a documented
  expected-diff class in `p5t1`/`p5t2` and in `tests/reference/README.md`, not
  a number to hand-tune. Quirk #146.

Finally, ruling 6: every `%.4f` is written with a `.` unconditionally.
Java's is locale-dependent (a German JVM writes `expected: 0,0500 mm`, quirk
#145); the port formats through `fr_dsn::format::double::java_format_fixed`,
which is locale-free, and `scripts/gen-drc-reference.sh` pins
`-Duser.language=en -Duser.country=US`.

## The two count families measure different things (ruling 11)

`KiCadDrcViolationRoutingTest` asserts `BoardStatistics.connections.incompleteCount`
and `BoardStatistics.clearanceViolations.totalCount`. The **report**'s arrays
are longer, and both are right. All six left-hand numbers were re-measured on
the clone's HEAD jar with `new BoardStatistics(board)` itself while writing
Task 11:

| fixture | `clearanceViolations.totalCount` | report `violations` | breakdown | `incompleteCount` | report `unconnectedItems` |
|---|---|---|---|---|---|
| `…dev-board_4_hole_clearance_violations` | **2** | 10 | 2 `holeClearance` + 8 `track_dangling` | **9** | 4 |
| `…BBD_Mars-64_6_track_1_hole…` | **76** | 96 | 64 `holeClearance` + 12 `clearance` + 18 `via_dangling` + 2 `track_dangling` | **3** | 3 |
| `…Natural_Tone_Preamp_7_unconnected_items` | **0** | 115 JVM (`-XX:hashCode=2`, the committed reference) / **112** port | 111 JVM / 108 port `track_dangling` + 4 `via_dangling` | **145** | 44 |

The reconciliation is exact and mechanical:

- `report.violations` = the deduplicated clearance list **plus** the
  `track_dangling`/`via_dangling` entries `generateReport` routes out of
  `getAllUnconnectedItems` (`DesignRulesChecker.java:231-233` then `:271-276`).
  `clearanceViolations.totalCount` is only the first summand.
- `report.unconnectedItems` = one entry per net with ≥ 2 connected sets;
  `incompleteCount` = the number of **airlines**, Σ (groups − 1) per net. The
  dev board's single 7-group net contributes 1 entry and 6 airlines, which is
  why 9 and 4 are both correct for the same board.
  `KiCadDrcViolationRoutingTest.java:41-48` explains it in its own words.

So the survey's "dev board gives 10 / 4" and the Java test's "9, 2" are both
measurements of the same board, of different quantities. Nobody should
reconcile them by changing a number:
`tests/java_ports.rs::the_board_statistics_oracle` and
`::the_report_arrays_are_longer_than_the_statistics_counters` assert both
halves of every row, against the same board, in the same test binary.

## Ported, and not

| Java | Here |
|---|---|
| `drc/DesignRulesChecker` | `checker.rs` + `report/build.rs` + `report/json.rs` |
| `drc/UnconnectedItems` | `unconnected.rs` (`UnconnectedItems`, `UnconnectedKind`) |
| `drc/NetIncompletes` (+ its nested `NetItem`, `Edge`) | `net_incompletes.rs` |
| `drc/AirLine` | `airline.rs` |
| `drc/ClearanceViolation` | **`fr-board`**: `items/clearance_violation.rs` + `board/clearance.rs` (ruling 9), re-exported here |
| `io/kicad/KiCadDrc{Report,Violation,ViolationItem,Position}` | `report/mod.rs`, one struct each, fields in Java declaration order (which is Gson's emission order) |
| `core/scoring/BoardStatisticsClearanceViolations` | `statistics.rs` (`from_violations`) |
| `GsonProvider.GSON.toJson` | `report/json.rs`, over `fr_dsn::format::json::to_gson_string_pretty` |
| `DesignRulesChecker`'s `drcSettings` field | **dropped whole** (ruling 12) — stored, never read; `src/checker.rs:22` |
| the `focusNets = {98, 99}` debug block (`:594-615`) | **not ported** (quirk #149) — `src/checker.rs:348` |
| `ClearanceViolation.printInfo` | **not ported** — GUI/`TextManager`; marker in `fr-board` |
| `io/kicad/{KiCadJsonReader, KiCadJsonWriter, KiCadBoardJson}` | **Plan 8** — the KiCad board/session JSON codec; spec §2 drops session-JSON *output*, and the input path is `-drc`'s plumbing (ruling 13) |
| `Freerouting.initializeDrc` (`Freerouting.java:246-372`) | **Plan 8** — the `-de`/`-dr` slots, the `.json`-vs-`.ses` branch, the quality-score merge, the write-or-stdout choice. This crate's public surface is exactly what it will call |
| `gui/workspace/progress/RatsNest.java` (457 loc) | **not ported** — a GUI façade. **Not a second ratsnest algorithm**: its constructor is `new DesignRulesChecker(board, null)` + `calculateAllIncompletes()` (`RatsNest.java:105-107`) and every method delegates; what it adds is a per-net visibility array and drag-time recalculation |
| `gui/workspace/progress/ClearanceViolations.java`, `RatsNestItemInfo`, `RatsNestItemType`, `gui/rendering/NetIncompletesGraphics.java`, `gui/windows/board/{AirLineInfo,WindowIncompletes}.java`, `gui/windows/routing/WindowClearanceViolations.java` | **not ported** — GUI |
| `api/v1/JobOutputResource.getDrcReport` (`:590-661`) | **not ported** — the REST twin of `-drc`; spec §2 drops the API. It calls the same `generateReportJson` (`:654`), so nothing behavioural is missing |
| `autoroute/pipeline/AutorouteUnroutedReport` | **Plan 6** — a *consumer* of this crate (`getAllAirlines` at `:22`), and the router's |
| `board/state/BoardComparator.java` (758 loc) | **Plan 8** — the result-manifest/report layer (spec §10); ruling 13 established nothing in `drc/**` needs it |
| every `FRLogger` call | dropped, per the plan's global constraints |

The complete roster, with a reason per class and per method, is the comment
block at the end of `crates/fr-drc/src/lib.rs`. It is not decoration:
`scripts/audit-port.sh` reads those markers, so a deferral is a gate rather
than a silence.

## Tests

`cargo test -p fr-drc` runs the unit tests plus nine integration suites:

| suite | what it pins |
|---|---|
| `clearance_list.rs` | `getAllClearanceViolations`: the four fixture counts, the dedup's surviving `firstItem`, the ordered list against the JVM |
| `unconnected.rs` | `getAllUnconnectedItems`: the three phases, their order, the dedup, the representative rule |
| `net_incompletes.rs` | `NetIncompletes`: net-item order, the triangulation, length violations, the two non-total comparators (#147, #148) |
| `incompletes.rs` | `calculateAllIncompletes`, the eight counters, `BoardStatisticsClearanceViolations` |
| `report.rs` | `generateReport` and the four DTOs, as normalised text against `ReportProbe`'s transcripts |
| `report_json.rs` | `generateReportJson`: both flavors' key order, Gson byte parity on three fixtures, the escape/number facts |
| `reference_parity.rs` | the eight committed CLI references (below) |
| **`java_ports.rs`** | **the five Java DRC suites, by name** — see below |
| **`corpus.rs`** | the whole `-drc` path over every `.dsn` in the corpus (`#[cfg_attr(debug_assertions, ignore)]`) |

### `java_ports.rs` — the ported Java suites

Every Java `@Test` in this crate's scope has a named counterpart there, so the
audit trail from a Java test to its Rust port is one `grep`:

| Java | ported as |
|---|---|
| `DesignRulesCheckerTest.testDrcReportStructure` (`:28-62`) | `report_structure_on_issue555_bbd_mars_64` |
| `DesignRulesCheckerTest.testDrcReportJsonFormat` (`:64-96`) | `report_json_has_every_head_key` |
| `DrcCoordinateTest.testDrcCoordinatesAreInCorrectRange` (`:26-72`) | `coordinates_are_in_a_plausible_mm_range` |
| `RatsnestClearanceHeadlessTest` (`:50-74`) | `incompletes_are_computable_via_the_checker_alone` |
| `RatsnestClearanceHeadlessTest` (`:78-88`) | `clearance_violations_are_computable_via_the_checker_alone` |
| `RatsnestClearanceHeadlessTest` (`:90-114`) | `aggregation_is_headless_and_severity_sorted` |
| `RatsnestClearanceHeadlessTest` (`:116-127`) | `empty_board_has_no_incompletes_and_no_violations` |
| `UnconnectedItemsReproductionTest` (`:49-209`) | `issue575_drc_reproduction_single_pass` |
| `KiCadDrcViolationRoutingTest` (three `@Test`s, `:51-65`) | `the_board_statistics_oracle` |

Java's lower bounds are kept *and* the exact numbers pinned beside them: a
lower bound cannot fail on over-detection, which is the regression these
fixtures are most likely to grow. `RatsnestClearanceHeadlessTest`'s
architectural point — that the ratsnest is reachable without
`interactive.RatsNest` — is a tautology here, since there is no GUI to import;
what survives is its arithmetic.

### `corpus.rs`

One cheap invariant per board, over every `.dsn` under
`../freerouting/fixtures`: `generate_report` does not panic,
`report_to_json(FreeroutingHead)` parses, and

```text
violations.len() == holeClearance + clearance + track_dangling + via_dangling
```

with the clearance half checked against an independently computed
`get_all_clearance_violations().len()`, and `unconnectedItems` checked to hold
*only* `unconnectedItems` entries. `generateReport` fills `violations` from two
sources (`:231-233` and `:271-276`), so a mis-routed entry is invisible to a
count of either list alone; that sum is what catches it.

It is also the guard on the report layer's three `panic`s. Task 11 replaced
silent fallbacks in `item_description`, `detailed_trace_description` and
`item_position` with panics — Java is handed the `Item` and would throw an
NPE, and returning `""` or `(0.0, 0.0)` would put a plausible-looking wrong
value into a parity document. "Unreachable" is only as good as the corpus that
tests it: **147 boards checked, 1 skipped** (`Issue006-LPC18XX_43XX_SCH.dsn`,
which neither reader accepts), 51 s in release.

```sh
cargo test -p fr-drc --release --test corpus     # or: --test corpus -- --ignored
```

### The JVM probes, and how to regenerate an expectation

Every expected value in this crate came out of a JUnit-free Java driver in
`tests/data/`, run against the clone's HEAD jar:

| probe | what it measures |
|---|---|
| `DrcListProbe.java` | `getAllClearanceViolations()` in list order |
| `UnconnectedProbe.java` | `getAllUnconnectedItems()`, hash-independent projection |
| `NetIncompletesProbe.java` | `NetIncompletes` per net, plus the (informational) airline endpoint list |
| `IncompletesProbe.java` | `calculateAllIncompletes` and the eight accessors, plus `BoardStatistics`' clearance block |
| `ReportProbe.java` | `generateReport` as normalised text |
| `JsonProbe.java` | `generateReportJson` — the bytes Gson writes; `--escapes` writes `gson-escapes.txt` |

`tests/data/README.md` says what each one probes and carries the exact command
line, including the `-XX:+UnlockExperimentalVMOptions -XX:hashCode=0..4` sweeps
that establish which outputs are hash-independent. A rebuilt jar with a new
`Constants.FREEROUTING_VERSION` needs the version constant regenerated along
with the transcripts. Since Plan 8 Task 14 (controller sweep item N6) that is
**one place for the whole workspace**: `crates/fr-drc/tests/common/mod.rs`'s
`JAR_VERSION` is an alias for [`fr_core::PARITY_VERSION`], which ruling AT/5
makes the single written-down form of the pinned jar's version. `fr-core` is a
dev-dependency of this crate and nothing more — the shipping `fr-drc` still
depends on nothing above `fr-dsn`.

### The committed CLI references

`scripts/gen-drc-reference.sh` drives the **real** `-drc` CLI (ruling 10 — no
bespoke driver is needed, and driving the CLI is what gets `qualityScore` into
the reference at all) over `tests/reference/drc-fixtures.txt`, writing
`tests/reference/<stem>/{drc.json, drc.meta.txt, java.log}`. `drc.json` is the
jar's **verbatim** bytes; normalisation is `parity::normalize_drc_json`'s job
and the parity test applies it to *both* sides.

Eight stems: `drc-dev-board`, `drc-bbd-mars-64`, `drc-natural-tone-preamp`,
`drc-issue593-rules` (the `-dr` path), `drc-issue593-ses` (a session in the
`-de` slot — there is no `-ds` flag), `drc-issue753-cpu85`, `drc-issue110-relay`
and `drc-tutorial-board` (a wholly empty report). They cover all five violation
types, both auxiliary input paths, three `0.0` quality scores and five
non-trivial ones.

```sh
scripts/gen-drc-reference.sh                       # all stems
scripts/gen-drc-reference.sh drc-dev-board         # one
scripts/gen-drc-reference.sh --meta-only           # rewrite drc.meta.txt, don't move drc.json
scripts/gen-drc-reference.sh --verify-hash-modes   # 5 runs/stem, count distinct documents
```

Needs a JDK 25 (`JAVA`) and the clone's HEAD jar (`FREEROUTING_JAR`), and pins
`-Duser.language=en -Duser.country=US` (ruling 6). Seven of the eight stems
produce **one** distinct normalised document across all five hash modes;
`drc-natural-tone-preamp` produces five, which is why its reference is the
`-XX:hashCode=2` run and its parity test asserts "the reference minus exactly
three `track_dangling` entries, in place" (ruling U).

**`../freerouting/fixtures/*-freerouting_drc.json` are NOT references.** They
are `Freerouting 2.1.2-SNAPSHOT`-era, carry no `quality_score`, and the
dev-board one claims 5 violations / 1 unconnected where both current jars
produce 10 / 4. `tests/reference/README.md` records that; the lower bounds in
`UnconnectedItemsReproductionTest` were read out of those stale files, which is
why they are so far below what the current jar produces.

### What needs the sibling checkout

`../freerouting` (or `$FREEROUTING_JAVA_DIR`) supplies the `.dsn` corpus; it is
not vendored. **Every fixture-reading test in this crate calls
`parity::require_java_dir()` first and returns with a printed SKIP** —
`FREEROUTING_JAVA_DIR=/nonexistent cargo test -p fr-drc` is green. The
synthetic-board tests (the majority of `net_incompletes.rs`,
`incompletes.rs`, `clearance_list.rs` and `report.rs`) need nothing.

The **jar** is needed only by the probes, by `gen-drc-reference.sh` and by the
two differentials — never by `cargo test`.

## The `p5t1` and `p5t2` differentials

Ruling 14 asks for two drivers, so that a report match cannot mask a
compensating pair of errors, and it earned its keep on the first run: `p5t2`
mode 2 found a counter divergence on `Issue269-z10_module.dsn` that `p5t1`
cannot see.

- **`p5t1`** — report level. Java's `DesignRulesChecker.generateReport` →
  `GsonProvider.GSON.toJson` versus the port's
  `report_to_json(FreeroutingHead)`, both normalised.
- **`p5t2`** — algorithm level, five modes: the raw violation list
  (`id1,id2,layer,expected,actual`), the unconnected list, the airline list and
  the three counters, plus mode 3's seed-pinned ratsnest and mode 4's
  self-check on that transcription.

```sh
./scripts/differential/run.sh p5t1                         # the dev board
./scripts/differential/run.sh p5t1 <dsn> [rules|-] [ses|-]
./scripts/differential/run.sh p5t2 <dsn> [rules|-] [ses|-] <mode 0-4>
./scripts/differential/sweep-p5t1.sh                       # all 112 rows
./scripts/differential/sweep-p5t2.sh                       # 112 rows x 5 modes
```

| run | diffs | notes |
|---|---|---|
| `p5t1` sweep, **112 rows** | **0 unexpected** | 94 MATCH + 18 XDIFF (all ruling S) + 1 SKIP; 110 s |
| `p5t2` sweep, **560 pairs** (112 × 5) | **0 unexpected** | 80 XDIFF + 1 SKIP; 425 s |
| `p5t2` mode 0 (the clearance list) | 0 | exact on every row |
| `p5t2` mode 1 (the unconnected list) | 0 unexpected | 18 XDIFF, ruling S again, one layer lower |
| `p5t2` mode 2 (the ratsnest, jar-seeded) | 0 unexpected | counters strict everywhere but one listed row; the airline block graded against a recorded budget (ruling 4) |
| `p5t2` mode 3 (the ratsnest, port-seeded) | **0** | exact on every row — every counter *and every airline endpoint* (ruling V) |
| `p5t2` mode 4 (transcription self-check) | 0 | mode 3's in-JVM transcription reproduces the jar's own `getAllAirlines()` |

The XDIFF rows are **checked, not waived**: both sweeps pin the differing item
uuids per row and require the rest of the document to be byte-identical, so a
regression from 3 differing entries to 300 is a failure rather than a shrug.
`scripts/differential/README.md` has the mismatch classes with their evidence,
the `AIRLINE_BUDGETS` ratchet and how to regenerate it. Both need a JDK 25
(`JAVA25_HOME`) and the clone's HEAD jar (`FREEROUTING_JAR`).

## Known limitations

- ~~**No quality score.**~~ **Closed by Plan 8 Task 7.** `getNormalizedScore`
  needs `BoardStatistics`' trace lengths, via counts and bend counts, which
  spec §4 put in `fr-core`; this crate still *takes* the score as an injected
  `Option<f32>` (ruling 5 — no `fr-router` dependency, no clock), and
  `crates/freerouting/src/commands/drc.rs` is what now **computes** it, from a
  settings merge of its own (quirk #272). The eight committed references
  pinned the values that implementation had to produce, and it produces all
  eight exactly — see `p8t3 e2e`'s table in
  `crates/freerouting/README.md`.
- **The airline endpoint list is not a parity surface** (ruling 4). The counts
  are, and mode 3 makes the endpoints one *given the same seed order* — with
  their direction and acceptance order, via the `ALD` block; two different seed
  orders into the same triangulation legitimately pick different equal-length
  edges, which is why mode 2 grades `AL` against a budget instead.
- **One board's incomplete *count* differs from any jar run** — `p5t2` mode 2's
  single `COUNTER_XDIFFS` row. On `Issue269-z10_module.dsn` net 1 has 4
  connected groups; the jar finds 3 airlines for it and the port 2, so
  `INCOMPLETE` reads 117 against 116 and `BoardStatistics.connections.incompleteCount`
  is off by one on that one board. The jar answers `NET 1 3 4` under
  `-XX:hashCode=0,1,2,3,4` and the default, so it is not the jar disagreeing
  with itself: it is quirk #82's degenerate triangulation meeting ruling 3's
  seed order, and a spanning "tree" that leaves two groups unjoined is a normal
  outcome on **both** sides (`count == groups - 1` fails on 13 of
  `Issue022-AutoRouter_interrupted.dsn`'s nets identically in both). Mode 3 is
  the proof: seeded the port's way, Java answers `NET 1 2 4` too and that
  fixture matches line for line.
- **Natural Tone Preamp's report is 112 where the jar says 113-115** (ruling S,
  quirk #146). Deliberate and documented in three places; not a number to tune.
- **Clearance compensation is never exercised.**
  `is_clearance_compensation_used()` is `false` on every headless path
  (`SearchTreeManager.java:35` — both setters are GUI), so the `true` arm of
  the compensation split (`Item.java:429-434`) is ported but unreached by any
  test or fixture.
- **`get_all_clearance_violations` is not idempotent at the
  `smallest_clearance` level.** Its return value is stable, but every call
  lowers each item's `smallest_clearance` (quirk #153) — as Java's does.
- **`schematic_parity` is `Vec<serde_json::Value>` and always empty.** Nothing
  in the Java tree ever adds to it. If Plan 8 fills it, that element type needs
  its own `Serialize` in declaration order, like the other four DTOs.

## Audit

`scripts/audit-port.sh` verifies the crate has no unaccounted-for public Java
method. Four invocations, all with the per-class map, all exit 0 with no
`MISSING` and no `UNMAPPED`:

```sh
./scripts/audit-port.sh drc          crates/fr-drc/src   '*.java' scripts/audit-map/fr-drc.map
./scripts/audit-port.sh io/kicad     crates/fr-drc/src   '*.java' scripts/audit-map/fr-drc.map
./scripts/audit-port.sh core/scoring crates/fr-drc/src \
    'BoardStatisticsClearanceViolations.java' scripts/audit-map/fr-drc.map
./scripts/audit-port.sh drc          crates/fr-board/src 'ClearanceViolation.java' \
    scripts/audit-map/fr-drc.map
```

The fourth is `fr-board`'s because ruling 9 puts `ClearanceViolation` there;
one map serves both crates, with the `ClearanceViolation` rows listed twice —
once crate-relative and once as `../../` — so each invocation resolves them.
`BoardStatistics.java` is deliberately absent from the `core/scoring` glob
(ruling 5 leaves it to Plan 8) and would report `UNMAPPED` if it were added.
The `io/kicad` deferrals are closed by name, per method, in `src/lib.rs`'s
roster — never by weakening the script or the map.

## Conventions this crate shares with the workspace

**`#![forbid(unsafe_code)]`** sits in the crate root (Plan 6 Task 18, at the
user's request). It holds for every workspace crate — `fr-geometry`, `fr-board`,
`fr-dsn`, `fr-settings`, `fr-drc`, `fr-router`, `tests/parity` and the
`freerouting` binary's `main.rs`. The only `unsafe` left in the repository is the
`static mut` PRNG in `scripts/differential/rust/src/bin/p2t13.rs`, a differential
driver rather than a crate; `scripts/differential/README.md` names it.

Deliberate divergences stay greppable: `// not ported:`, `// renamed:`,
`// added in Plan N:`, `// totalized:` and `// Java bug:`, each matching a row in
`docs/java-quirks.md` where the divergence is behavioural.
