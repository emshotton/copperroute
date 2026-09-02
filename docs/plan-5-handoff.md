# Plan 5 hand-off — the design-rule checker, the ratsnest and the KiCad DRC JSON report (`fr-drc`)

Branch `plan-5-drc`, 21 implementation commits on top of `main` (`329797c`,
Plan 4 merged 2026-08-29) plus the plan document itself (`dfa0c97`) — see
`git log` for the tip, which includes the final documentation commit this file
is part of.

**Read this before Plans 6-8.** Plans 6/7 call this crate's ratsnest on every
autoroute pass and inherit a Java bug (#82) that is now load-bearing in two
crates. Plan 8 owns the entire `-drc` command line and inherits two product
decisions plus one unported reader. Section 1 is the one thing to read even if
nothing else here is: **Java's DRC is not reproducible run to run, this port
is, and knowing exactly where the two part company is what makes every
reference, differential and test in this plan legible.**

---

## 1. Determinism: Java hashes, the port sorts

Java's `DesignRulesChecker` builds its connected sets as `HashSet<Item>`
(`DesignRulesChecker.java:118`) and `NetIncompletes.calculateNetItems`
iterates a `HashSet<Item>` (`NetIncompletes.java:295`, `:299`). `Item`
overrides neither `hashCode` nor `equals`, so **both iterate in JVM
identity-hash order**. Five runs of the clone's HEAD jar under
`-XX:+UnlockExperimentalVMOptions -XX:hashCode=0..4` give five different
`unconnectedItems[0].items` orders on the dev board — and on one fixture a
different *number* of violations.

There is no Java order here to port. The port therefore **fixes** one, as a
deliberate divergence (plan ruling 3, quirk #144):

| where | port | Java |
|---|---|---|
| `unconnectedItems[].items` | ascending item id | identity-hash order |
| `calculateNetItems`' seed order | ascending item id | identity-hash order |
| `itemsByNet` | `BTreeMap<i32, _>`, ascending net number | `HashMap` (coincides on every corpus fixture) |
| `findRepresentativeItem` | lowest-id `Pin`, else lowest-id `Trace`, else lowest-id item | *a* `Pin`, not a particular one |

**Within** one connected set the order *is* Java's and is reproduced exactly
(plan ruling 15): `Item.getConnectedSet` returns a `TreeSet<Item>`
(`Item.java:606`) and `Item.compareTo`'s subtraction is reversed
(`Item.java:95-103`, quirk #44), so a connected set iterates **descending**
item id. The port's `Board::connected_set` is an ascending `BTreeSet<ItemId>`,
so every DRC walk of one iterates `.rev()`. Getting this backwards silently
reverses the Delaunay insertion order inside every component — a wrong-output
bug with no crash, which is why it has its own test
(`net_items_are_ordered_descending_within_a_component`).

### What is bit-parity and what is not

- **Bit-parity, strict:** the clearance-violation list; the report's
  `violations` array on every hash-independent fixture; `maxConnections`;
  `getIncompleteCount()`; every per-net incomplete count; every connected-group
  count; every length violation; the JSON bytes.
- **Not a parity surface:** the *endpoints* of each airline, and the
  `unconnectedItems[].items` order. Both are downstream of the `HashSet`.
- **Hash-nondeterministic in Java, so the port's answer is simply different
  (controller ruling S):** the emitted `track_dangling` **count**. The
  dangling-trace dedup (`DesignRulesChecker.java:160`) drops whichever
  candidate happens to be a net entry's hash-ordered `firstItem`, so the jar
  itself answers 109, 110 or 111 of 111 candidates on
  `Issue575-drc_Natural_Tone_Preamp_7_unconnected_items` depending on the hash
  mode — **and 110 vs 111 across two runs of the same mode**. The port's
  ascending-id representatives drop exactly three, every time, giving **112**
  report violations where the jar gives **113-115**. That is a documented
  expected-diff class, not a number to tune.

### The three consequences worth carrying forward

1. **Only `-XX:hashCode=2` reproduces run to run.** Modes 0 and the default are
   PRNG-seeded, modes 1 and 4 are address-derived, mode 3 is a per-thread
   xorshift that is deterministic only inside a single-threaded run. Three
   independent sweeps of the same jar agreed on mode 2 (115 violations on
   Natural Tone) and each disagreed with the others on at least one of modes 0,
   1, 3 and 4. **All eight committed references are generated under mode 2**,
   and `references_are_from_the_head_jar` asserts every `drc.meta.txt` says so.
   No per-mode count should ever be quoted for the other four.
2. **The airline gate is `p5t2` mode 3, not a union of JVM runs (controller
   ruling V, superseding T).** Grading the port's airlines against the union of
   the jar's hash-mode runs does not converge: on
   `Issue022-AutoRouter_interrupted.dsn` (235 port airlines) 6 runs leave 11
   outside, 60 runs still leave 2. So mode 3 re-seeds a transcribed
   `NetIncompletes` **inside the JVM** with the port's ascending-id order and
   requires **exact** equality of both airline blocks — the canonical `AL` block
   (the unordered pair, sorted) and the `ALD` block (`ALD <net> <fromId>
   <toId>`, in Kruskal's acceptance order and with the edge's own direction):
   **112/112 rows match**, with mode 4 as the standing self-check that the
   transcription still reproduces the jar's own `getAllAirlines()`, `ALD`
   included. Ruling T's containment test is still
   applied strictly where a committed `*.airlines-union.txt` exists (ruling 4's
   three fixtures — all pass, 0 outside).
3. **The report is locale-free.** Every `%.4f` in a Java description goes
   through the default `FORMAT` locale, so a German JVM writes
   `expected: 0,0500 mm` into a machine-readable document (quirk #145). The
   port formats through `fr_dsn::format::double::java_format_fixed` and
   `scripts/gen-drc-reference.sh` pins `-Duser.language=en -Duser.country=US`.

---

## 2. Delivered

- **`fr-drc`**: 2.8k lines of source across `checker` (`DesignRulesChecker`,
  the three unconnected phases, `calculateAllIncompletes` and the eight
  counters), `net_incompletes` (the ratsnest: the item filter, the Delaunay
  triangulation, Kruskal, the five-way `Edge` comparator, the length
  violation), `airline`, `unconnected`, `statistics`
  (`BoardStatisticsClearanceViolations`), `report/{mod,build,json}` (the four
  KiCad DTOs, `generateReport` and the two-flavor Gson-compatible writer) and
  `error`. Dependencies are exactly the six the plan allowed: `fr-board`,
  `fr-dsn`, `fr-geometry`, `serde`, `serde_json`, `thiserror` (dev: `parity`).
  **No `fr-settings` (ruling 12), no `tracing`, no GUI, no clock, no static
  mutable state.**
- **Five additions to `fr-board`** (plan ruling 9 — `Item.clearanceViolations`
  returns `ClearanceViolation` and `fr-board` cannot depend upward on
  `fr-drc`): `items/clearance_violation.rs` and `board/clearance.rs`,
  consuming **both** of Plan 2's `// added in Plan 5:` markers, plus one
  accessor pair on `SearchTreeManager`. `grep -rn "added in Plan 5" crates/`
  now returns nothing.
- **One addition to `fr-dsn`** (ruling 7, Task 1, a pure move):
  `fr_dsn::format::json::{JavaNumberFormatter, to_gson_string_pretty}`, moved
  down out of `fr-settings`' private `json.rs` so both crates render Gson bytes
  from **one** copy. `fr-settings`' entry points and its whole `tests/json.rs`
  are untouched, which is the proof the move was behaviour-preserving.
- **JVM parity, two drivers** (ruling 14). `p5t1` (report level) over 112 rows:
  **0 unexpected diffs** — 94 MATCH, 18 XDIFF (all one class), 1 SKIP.
  `p5t2` (algorithm level) over 560 pairs (112 rows x 5 modes): **0 unexpected
  diffs**. Mode 0 (the clearance list) is exact on every row; mode 3 (the
  seed-pinned ratsnest) is exact on every row, endpoints included.
- **Eight committed CLI references** generated by the real `-drc` CLI of the
  HEAD jar (ruling 10), plus `parity::normalize_drc_json` and the
  `reference_parity` suite that applies it to both sides.
- **Six JVM probes** in `crates/fr-drc/tests/data/` — every expected value in
  the crate is a measurement, not an inference.
- **`docs/java-quirks.md`**: 12 new pinned quirks (**#144-#155**, the register
  now runs contiguously **1 to 155**), 2 new totalization rows, and 2 new
  obligation-register rows (both Plan 8: `DrcJsonFlavor`'s CLI default and
  `quality_score`'s computation). No register row was discharged by Plan 5;
  quirk **#82 stays deliberately unfixed**.
- **`scripts/audit-map/fr-drc.map`**; all four `fr-drc` audit invocations exit 0
  with no `MISSING` and no `UNMAPPED`, as does `fr-board`'s `board/state` after
  Task 12's marker re-point.
- **1 479 tests across the workspace** (5 ignored), of which **104** in
  `fr-drc` plus 1 ignored — `corpus`, which is `#[cfg_attr(debug_assertions,
  ignore)]` and runs in 51 s in release over 147 boards.
- **`crates/freerouting` is untouched** (ruling 13), verified: `git diff
  --name-only 329797c..HEAD -- crates/freerouting` is empty.

### The commits, by task

| task | commits |
|---|---|
| 1 — the formatter move | `1a8f0a9` |
| 2 — `Item.clearanceViolations` in `fr-board` | `0275ad7`, `e527626` |
| 3 — crate skeleton + the deduplicated clearance list | `6a7b5d0` |
| 4 — `getAllUnconnectedItems` | `eecf057`, `e97df94`, `db0dbd0` |
| 5 — `NetIncompletes` | `16e573c`, `7254579` |
| 6 — `calculateAllIncompletes`, counters, statistics block | `b3a9155`, `99fe611` |
| 7 — the four DTOs and `generateReport` | `b127bd3`, `b73494e`, `6482879` |
| 8 — the JSON writer, both flavors | `f73d5fb`, `e6a788d` |
| 9 — the CLI references and the normaliser | `45f82b2`, `6e5dd91` |
| 10 — `p5t1` and `p5t2` | `eb91e66` |
| 11 — the ported Java suites, audit to zero, README | `ada8063`, `ff0027b` |
| 12 — this document, the quirks register, the obligation ticks | see `git log` |

---

## 3. Public API surface (what Plans 6-8 call)

Everything starts from a checker borrowing the board **mutably**:

```rust,ignore
use fr_drc::prelude::*;

let mut drc = DesignRulesChecker::new(&mut board);      // ruling 8
```

`&mut Board` is deliberate. `Item.clearanceViolations` has two side effects in
Java — it lowers `this.smallestClearance` (`Item.java:451-453`, quirk #153) and
it advances the search tree's entry counter (`ShapeSearchTree.java:55`) — and
hiding either behind interior mutability would make quirk #153 invisible.
There is **no** `DesignRulesCheckerSettings` parameter (ruling 12, quirk #155).

```rust,ignore
impl<'a> DesignRulesChecker<'a> {
    pub fn new(board: &'a mut Board) -> Self;

    // the three answers
    pub fn get_all_clearance_violations(&mut self) -> Vec<ClearanceViolation>;
    pub fn get_all_unconnected_items(&mut self) -> Vec<UnconnectedItems>;
    pub fn calculate_all_incompletes(&mut self);

    // the eight counters (all after calculate_all_incompletes)
    pub fn max_connections(&self) -> i32;
    pub fn get_incomplete_count(&mut self) -> usize;
    pub fn get_incomplete_count_for_net(&mut self, net_number: i32) -> usize;
    pub fn get_length_violation_count(&mut self) -> usize;
    pub fn get_length_violation(&mut self, net_number: i32) -> f64;
    pub fn recalculate_length_violations(&mut self) -> bool;
    pub fn get_all_airlines(&mut self) -> Vec<AirLine>;
    pub fn get_net_incompletes(&mut self, net_number: i32) -> Option<&NetIncompletes>;

    // per-net recompute (what an interactive/router caller needs)
    pub fn recalculate_net_incompletes(&mut self, net_number: i32);
    pub fn recalculate_net_incompletes_with(&mut self, net_number: i32, item_list: &[ItemId]);

    // the report
    pub fn generate_report(&mut self, coords: &DrcCoordinates, options: &DrcReportOptions)
        -> KiCadDrcReport;
    pub fn report_to_json(&mut self, coords: &DrcCoordinates, options: &DrcReportOptions,
        flavor: DrcJsonFlavor) -> Result<String, DrcError>;
}

impl KiCadDrcReport {
    pub fn to_json(&self, flavor: DrcJsonFlavor) -> Result<String, DrcError>;
}
```

`generate_report` and `report_to_json` are `&mut self` for the same reason
`new` takes `&mut Board`: they call `get_all_clearance_violations` internally
(`DesignRulesChecker.java:216`).

### The two injected parameter blocks (ruling 5)

`fr-drc` has no clock, no `Constants` and no `BoardStatistics`, so everything
`Freerouting.initializeDrc` supplies arrives as a parameter:

```rust,ignore
pub struct DrcReportOptions {
    pub source: String,               // Freerouting.java:339 — the input file's base name
    pub coordinate_unit: String,      // Freerouting.java:335 — hard-coded "mm" (quirk #151)
    pub date: String,                 // already formatted; Java calls ZonedDateTime.now()
    pub freerouting_version: String,  // without the "Freerouting " prefix, which the port adds
    pub quality_score: Option<f32>,   // Freerouting.java:349 — a float; fr-drc widens it
}

pub struct DrcCoordinates {
    pub transform: CoordinateTransform,  // fr_dsn; board.communication.coordinateTransform
    pub board_unit: Unit,                // board.communication.unit
}
```

`quality_score` is `Option<f32>`, **not `f64`** (a Task 11 change from the
plan's `f64`): `BoardStatistics.getNormalizedScore` returns a `float` and
`Freerouting.java:349` widens it with an explicit `(double)` cast, which is
where every reference's `.078369140625`-style tail comes from. Typing it `f64`
would let a caller hand in a number no jar can produce. `None` is Java's
`null`, which Gson omits from the document entirely.

### Types

```rust,ignore
pub enum DrcJsonFlavor { FreeroutingHead /* Default */, KiCad }
pub enum UnconnectedKind { UnconnectedItems, TrackDangling, ViaDangling }
pub enum DrcError { Json(serde_json::Error) }        // one variant, from to_json

pub struct UnconnectedItems { first_item: ItemId, second_item: Option<ItemId>,
                              all_items: Vec<ItemId>, kind: UnconnectedKind }
pub struct AirLine { net_number: i32, from_item: ItemId, from_corner: FloatPoint,
                     to_item: ItemId, to_corner: FloatPoint }
pub struct NetIncompletes { /* count(), get_connected_group_count(), get_marker_radius(),
                               get_length_violation(), get_net_number(), calc_length_violation() */ }
pub struct BoardStatisticsClearanceViolations { /* from_violations(&[ClearanceViolation], f64) */ }

// the four KiCad DTOs, fields in Java declaration order (= Gson's emission order)
pub struct KiCadDrcReport { json_schema, coordinate_units, date, kicad_version,
                            freerouting_version, source, unconnected_items, violations,
                            schematic_parity, quality_score }
pub struct KiCadDrcViolation { description, items, severity, kind }
pub struct KiCadDrcViolationItem { description, pos, uuid }
pub struct KiCadDrcPosition { x, y }
```

### The `fr-board` additions (ruling 9)

```rust,ignore
pub struct ClearanceViolation {                 // crates/fr-board/src/items/clearance_violation.rs
    pub first_item: ItemId, pub second_item: ItemId,
    pub shape: TileShape, pub layer: usize,
    pub expected_clearance: f64, pub actual_clearance: f64,
}

impl Board {                                    // crates/fr-board/src/board/clearance.rs
    pub fn clearance_violations(&mut self, id: ItemId) -> Vec<ClearanceViolation>;
    pub fn clearance_violation_count(&mut self, id: ItemId) -> usize;
    pub fn calculate_clearance_between_two_shapes(
        raw_shape1: &TileShape, raw_shape2: &TileShape,
        minimum_clearance: f64, cl_comp1: i32, cl_comp2: i32) -> f64;   // associated, not a method
    pub fn aggregate_violations_sorted_by_severity(&mut self) -> Vec<ClearanceViolation>;
    pub fn smallest_clearance(&self) -> f64;
}

impl SearchTreeManager {                        // crates/fr-board/src/searchtree/manager.rs
    pub fn entry_counter_mut(&mut self) -> &mut u64;
    pub fn default_tree_and_counter_mut(&mut self) -> (&ShapeSearchTree, &mut u64);
}
```

`fr-drc` re-exports `ClearanceViolation`, so callers spell it
`fr_drc::ClearanceViolation` as Java's `drc` package does.
`ClearanceViolation.printInfo` is `// not ported:` (GUI/`TextManager`).

### The `fr-dsn` addition (ruling 7)

```rust,ignore
pub use fr_dsn::format::{JavaNumberFormatter, to_gson_string_pretty};
```

Two-space indent, `": "` after every key, no trailing newline, every `f64`
through `Double.toString`, HTML escaping off but `U+2028`/`U+2029` escaped
anyway, `null` omitted rather than written. `fr-settings` and `fr-drc` both
render through it.

### The parity helper

```rust,ignore
pub fn parity::normalize_drc_json(s: &str) -> Result<String, serde_json::Error>;
```

Three rules, on purpose: drop `date`, sort each `unconnectedItems` entry's
`items` by **numeric** uuid, and touch **nothing** in `violations` — neither
the array order nor a clearance entry's two-element `items`, because those are
deterministic and sorting them would hide a real ordering regression.

---

## 4. Rulings

### Plan rulings 1-15 — and what execution did with them

1. **Parity target is the clone's HEAD jar, and HEAD's camelCase drift from the
   schema it names is a Java bug.** *Held.* Both jars were re-run; the
   difference is exactly six keys and two `type` strings. Recorded as quirk
   **#154** (the plan pre-assigned it #143, but Plan 4's register had already
   reached #143 — see §5). *Cost if wrong:* ruling 2's flavor table makes an
   upstream revert a one-line default change.
2. **Two schema flavors, one key table, both tested.** *Held and strengthened.*
   `DrcJsonFlavor::{FreeroutingHead, KiCad}`, one `const KEYS: [FlavorKeys; 2]`,
   and `flavors_differ_only_in_the_key_tables_eight_strings` proves the
   relationship *mechanically*: applying seven quoted-token substitutions to a
   HEAD document yields the KiCad document byte for byte. Task 7 found the
   `type` **value** needs the flavor treatment too, not only the key.
   **Which flavor the CLI defaults to is still Plan 8's product decision** and
   is now an obligation-register row. *Cost if wrong:* nil — the second flavor
   is dead code Plan 8 either wires or deletes.
3. **Determinism where Java has none: ascending item id everywhere.** *Held,
   and the probe held over the reference set.* Seven of the eight reference
   stems produce **one** distinct normalised document across all five hash
   modes; the eighth (`drc-natural-tone-preamp`) produces **five**, which is
   ruling S's fixture and the reason all eight are generated under mode 2. The
   normaliser is what makes a reference a property of the board rather than of
   the JVM run. *Cost if wrong:* a reference that only matches one JVM —
   measured, not assumed.
4. **The airline *list* is not a parity surface; the airline *counts* are.**
   *Held, and superseded as a grading rule by ruling V.* The counts held
   exactly: `maxConnections`, `getIncompleteCount`, the per-net counts, the
   connected-group counts and the length violations are byte-identical across
   `-XX:hashCode=0..4` on every measured fixture, and `p5t2` grades them
   strictly. The endpoint *table* in the ruling (5 / 2 / 3 distinct digests) is
   illustrative, not exact — four of the six modes resample per run, so only
   modes 2 and 3 reproduce. `p5t2`'s airline-difference counts therefore are
   **not** graded against that table; they are graded by mode 3's exact
   equality plus mode 2's measured `AIRLINE_BUDGETS` ratchet. *Cost if wrong:*
   mitigated exactly as the ruling said, and then removed by mode 3.
5. **`date`, `source`, `freerouting_version` and `quality_score` are injected.**
   *Held*, with one type change: `quality_score` is `Option<f32>`, not
   `Option<f64>` (Task 11). The eight references pin the values Plan 8's
   `getNormalizedScore` must produce. *Cost if wrong:* Plan 8 threads one
   parameter; §10 names the call site.
6. **Every `%.4f` is `.`-decimal and the generator pins the locale.** *Held.*
   `percent_four_f_uses_a_dot` asserts no description of the three Issue575
   fixtures carries a digit-comma-digit, and checks its own guard against
   `"expected: 0,0500 mm"`. Quirk #145.
7. **`fr-drc` may depend on `fr-dsn`, and the Gson formatter moves down.**
   *Held.* `fr-drc -> fr-dsn -> fr-board -> fr-geometry`, strict and acyclic.
   `cargo test -p fr-settings --test json` was the guard and stayed green.
   *Cost if wrong:* nil — the move was mechanical.
8. **`&mut self` throughout; `DesignRulesChecker` borrows `&mut Board`.**
   *Held, and it earned its keep*: quirk #153 is visible precisely because the
   type says the compute mutates. *Cost if wrong:* a `Cell<u64>` counter — a
   contained change. No caller in this plan wanted `&Board`.
9. **`ClearanceViolation` is defined in `fr-board`.** *Held.* The two headless
   statics became `Board::aggregate_violations_sorted_by_severity` and
   `Board::smallest_clearance`, both with a ported Java test caller.
10. **References come from a sibling script driving the real `-drc` CLI.**
    *Held.* `scripts/gen-drc-reference.sh` + `tests/reference/drc-fixtures.txt`.
    Task 9 found one correction: there is **no `-ds` flag** anywhere in the
    parser, so a session file reaches `globalSettings.designSessionFilename`
    only through the **`-de` slot list** (`-de "<dsn>+<ses>"`). Ruling 10's
    "verified while writing this plan" claim about `-de` was right and the
    hand-off restates it because Plan 8 will need it.
11. **The two count families measure different things, and both are ported.**
    *Held, with one number changed by ruling S.* Every left-hand number was
    re-measured with `new BoardStatistics(board)` on the HEAD jar in Task 11:
    (2, 9), (76, 3), (0, 145). The report-side numbers held too, except Natural
    Tone's `violations`, which is **115 on the jar (`-XX:hashCode=2`, the
    committed reference) and 112 in the port** — ruling S, not a defect.
    `the_board_statistics_oracle` and
    `the_report_arrays_are_longer_than_the_statistics_counters` assert both
    halves of every row in the same binary.
12. **`DesignRulesCheckerSettings` is not a parameter.** *Held*, and the
    evidence strengthened: Task 12 re-counted at HEAD and found **12 of the 14
    constructions in `src/main`** pass `null` (the plan said 10 of 15); only
    `Freerouting.java:333` and `api/v1/JobOutputResource.java:645` pass a real
    object, and both pass one whose contents are ignored. Recorded as quirk
    **#155**. *Cost if wrong:* one dependency and one parameter, later.
13. **Scope: `-drc` plumbing, KiCad JSON session input and `BoardComparator`
    are all out.** *Held.* `crates/freerouting` has no Plan 5 commit. The three
    `io/kicad` session/board classes are `// added in Plan 8:` lines in the
    audit map. `BoardComparator`'s `// added in Plan 5:` marker at
    `crates/fr-board/src/board/mod.rs:57` was **re-pointed to Plan 8** by Task
    12 with the reason (the result-manifest/report layer, spec §10 — nothing in
    `drc/**` or on the `-drc` path references it); `board/state`'s audit still
    exits 0. *Cost if wrong:* one marker moves again.
14. **The differential is two drivers, not one.** *Held, and it paid on the
    first run*: `p5t2` mode 2 found a counter divergence on
    `Issue269-z10_module.dsn` that `p5t1` structurally cannot see (that
    fixture's report contains no airlines and its `p5t1` row MATCHes). The
    divergence turned out to be Java-side seed order crossing quirk #82 — mode
    3 proves it, and no `fr-drc` change was warranted.
15. **Within one connected set the order is Java's and is descending.** *Held.*
    Pinned by `net_items_are_ordered_descending_within_a_component`; ruling 3's
    free choice covers the seed order only.

### Controller rulings Q-W (made during execution)

- **Q — plan rulings 1-15 accepted as written**, including `&mut Board` (8),
  CLI-driven references (10) and no `DesignRulesCheckerSettings` parameter
  (12). Task 1 runs before Task 2 rather than in parallel (one implementer).
  *Outcome:* no ruling was overturned; three were corrected in detail (4, 11,
  12) and one was superseded as a grading rule (4 by V).
- **R — the quirks register is numbered contiguously.** Task 2 numbered its row
  `#153` against the plan's pre-assignment, leaving a 146-152 gap that later
  tasks filled out of order; later tasks were told to take **the next free
  number**, not the plan's. *Outcome:* the gap closed itself as Tasks 4-8
  landed. Task 12 verified the register is contiguous **1 to 155** with no
  duplicates, sorted Plan 5's eleven rows into ascending order in the file, and
  found exactly **one** stale citation anywhere in the tree
  (`crates/fr-drc/src/statistics.rs:29` still cited the plan's `#143` for
  ruling 1's drift, now `#154`). *Cost if wrong:* a hand-off that cites a row
  number nobody can find.
- **S — Java's `track_dangling` count is hash-nondeterministic, so the port's
  108 stands.** Measured in Task 4 with `UnconnectedProbe.java`: 109, 110 or
  111 of 111 candidates across the hash modes, and 110 vs 111 across two runs
  of mode 0. The port emits **108** deterministically, hence **112** report
  violations against the jar's 113-115. *Outcome:* an expected-diff class in
  `p5t1` and `p5t2` and a by-uuid parity assertion in `reference_parity`, not a
  hand-tuned number. *Cost if wrong:* a real over- or under-detection hiding
  behind "expected" — mitigated because both sweeps pin the differing uuids and
  require the rest of the document to be byte-identical.
- **T — the airline union gate.** Java's triangulation varies with insertion
  order (net 16's MST total weight moves across modes), so `p5t2` was to grade
  airlines on "every port airline is in the union of the JVM runs' airlines"
  plus exact per-net counts. *Outcome:* **superseded by V** as the corpus-wide
  gate, because the union does not converge (6 runs leave 11 airlines outside
  on `Issue022`; 60 runs still leave 2). T is still applied strictly where a
  committed union file exists.
- **U — the Natural Tone golden is committed by Task 9** as the
  `-XX:hashCode=2` real-CLI reference with a byte-level minus-three-entries
  parity check. *Outcome:* done;
  `natural_tone_preamp_is_the_reference_minus_three_dangling_tracks` pins the
  three deleted `track_dangling` entries **by uuid** (1909, 1696, 1242) and in
  place, not by count.
- **V — `p5t2` mode 3's reseeded in-JVM transcription supersedes T as the
  airline gate.** `P5T2.java` transcribes `NetIncompletes`' constructor,
  `calculateNetItems`, `joinConnectedSets`, `Edge` and `calcLengthViolation`
  and makes the seed order a parameter. Seeded the port's way it matches
  **112/112 rows exactly, endpoints included** — and, since the final fix wave,
  their **direction and Kruskal acceptance order** too, through the `ALD` block
  both sides now print in modes 3 and 4; seeded Java's way (mode 4) it
  reproduces the jar's own `getAllAirlines()` on all 112 rows
  (`TRANSCRIPTION equal 0`). *Cost if wrong:* the transcription could drift
  from the jar — which is exactly what mode 4 exists to catch, and the sweep
  runs it on every row.
- **W — the `-drc` CLI defaults to `DrcJsonFlavor::KiCad`.** Made at the final
  whole-branch review, closing the product decision ruling 2 deferred. The
  user's stated focus is KiCad, and the document's own `$schema`
  (`https://schemas.kicad.org/drc.v1.json`) promises KiCad's snake_case
  spelling; HEAD's camelCase — the drift quirk #154 records — stays reachable
  **behind a flag**. *Outcome:* Plan 8 wires it (§10); nothing in `fr-drc`
  re-baselines, because `DrcJsonFlavor::default()` stays `FreeroutingHead` —
  the *parity* default `head_flavor_is_the_jvms_gson_bytes` pins against the
  jar — and the CLI simply asks for the other row. *Cost if wrong:* one flag
  default flips; both flavors are already tested byte for byte.

---

## 5. Corrections to the plan discovered during execution

1. **Quirk numbering.** The plan pre-assigned #143-#154 to Plan 5's rows, but
   Plan 4's register had already reached **#142 plus a #143** (the
   thread-count-fields row). Plan 5's rows are therefore **#144-#155**, and
   ruling 1's `@SerializedName` drift — the plan's #143 — is **#154**. Every
   citation in the tree agrees; one stale one was found and fixed by Task 12.
2. **Java line citations drifted by 2-5 lines** in the plan's survey against
   the clone's HEAD, in four separate places. Each was re-read line by line and
   corrected in a dedicated commit: Task 4 (`e97df94`), Task 6 (`99fe611`),
   Task 7 (`b73494e`), Task 8 (`e6a788d`). The most dangerous was
   `BoardStatistics.java:361-365`, which is a **different** `else` arm
   (`includeClearanceViolations == false`) from the one
   `from_violations` models (`:357-361`).
3. **The `focusNets` block is `:594-615`, not `:598-615`**, and its
   commented-out `validateAndLogPolylineIntegrity()` is at `:611`, not `:613`
   (quirk #149).
4. **A review claim was itself wrong and Java settled it.** Task 2's reviewer
   read `BoardStatistics.java:268` and `:339` as two clearance passes per
   `BoardStatistics`; they are two `DesignRulesChecker` constructions, only one
   of which runs the clearance scan. Quirk #153 carries the corrected account.
5. **`Board::get_items()` is already descending by item id** (Plan 2 established
   it — quirk #44), so Task 4's phase walk needed no sort of its own.
6. **`Edge`'s `PartialEq` must be the comparator's, not derived** (Task 5 fix
   round). A derived `PartialEq` compares five fields structurally while `cmp`
   compares the transcribed `Edge.compareTo` keys, so `a.cmp(b) == Equal` did
   not imply `a == b` — for exactly the pair quirk #147 describes, which
   `BTreeSet` is entitled to rely on. Fixed, with `Eq` documented as *asserted,
   not proved* (reflexivity fails on a NaN edge, exactly as Java's `Comparable`
   contract does).
7. **The NaN half of quirk #147 was overstated twice.** A NaN edge reduces the
   set to one element only when inserted **first**; offered later it is simply
   the edge dropped. And because the relation is not transitive there, Rust's
   B-tree and Java's red-black tree may drop **different** edges. The parity
   claim is narrowed everywhere it appears: the port reproduces Java's
   **comparator**, and matches Java's **tree** only in the consistent
   (five-way-tie) case — which is every input a real board produces.
8. **There is no `-ds` flag.** A session file reaches `-drc` only through the
   `-de` slot list.
9. **`quality_score` is a `float`.** `Option<f64>` in the plan, `Option<f32>`
   in the crate (Task 11) — an API change Plan 8 will meet.
10. **`drcSettings`' construction census.** 12 of 14 in `src/main` pass `null`,
    not 10 of 15; the field is assigned at `:45`, not `:44` (Task 12).

---

## 6. Quirks pinned in Plan 5 (`docs/java-quirks.md` #144-#155)

The register is contiguous **1 to 155** with no duplicates and no gaps. One
line each:

| # | in one sentence |
|---|---|
| 144 | `getAllUnconnectedItems` and `findRepresentativeItem` iterate `HashSet<Item>`, so the DRC's unconnected list, its representatives and the ratsnest's seed order are not reproducible run to run. **Not reproduced — the port sorts, deliberately** (ruling 3). |
| 145 | Every `%.4f` in a DRC description follows the JVM's default `FORMAT` locale, so a German JVM writes `0,0500 mm` into a machine-readable document. **Not reproduced** (ruling 6). |
| 146 | The dangling-trace dedup tests `firstItem` alone (`:160`), so a trace that is a net entry's `secondItem` or merely in its `allItems` is emitted twice; the via phase has no dedup at all; the scan is O(n^2). **Reproduced**, and the reason the port's count is 108 not 111. |
| 147 | `Edge.compareTo` is a five-way `f64` tie-break through `Signum.asInt`; an exact tie returns 0 and the `TreeSet` silently drops the edge, and NaN maps to 0 as well. **Reproduced** (comparator exactly; tree parity claimed only in the consistent case). |
| 148 | `AirLine implements Comparable` but compares `net.name` alone, so a `TreeSet<AirLine>` would keep one airline per net. Latent — no Java caller sorts them. **Not reproduced as `Ord`**: the port exposes `compare_by_net_name` and implements neither `Ord` nor `PartialOrd`. |
| 149 | `calculateAllIncompletes` carries a hard-coded `int[] focusNets = {98, 99}` debug block around a commented-out `validateAndLogPolylineIntegrity()`, costing two log calls and two full net walks per call. **`// not ported:`**. |
| 150 | `getIncompleteCount`'s log line appends the `NetIncompletes[]` **array** where it means the per-net count, printing an identity hash. Log-only. **`// not ported:`** (every `FRLogger` call is dropped). |
| 151 | `coordinateUnit` is hard-coded `"mm"` by the CLI, so four of `convertCoordinate`'s five branches — including the board-unit fallback — are unreachable from `-drc`, on fixtures that all declare `(unit um)`. **Reproduced in full**: all five arms are ported, because Plan 8's API/MCP path can reach them. |
| 152 | `isHole` classifies **every** `Pin` as a hole, surface-mount pads included, and its own comment admits it; the `type` string *and* the description's first three words change with it. **Reproduced.** |
| 153 | `Item.smallestClearance` is a monotone minimum over the whole life of the item and is never reset, so a board whose clearances improve still reports the worst any earlier call saw. **Reproduced** — and visible only because the port's compute takes `&mut self` (ruling 8). |
| 154 | The DRC report's `@SerializedName`s drifted to camelCase away from the snake_case schema the report's own `$schema` names; 2.3.0 matched it and HEAD does not. **Reproduced as the default**, with `DrcJsonFlavor::KiCad` as the way out (rulings 1 and 2). |
| 155 | `DesignRulesChecker.drcSettings` is stored and never read; `includeWarnings`/`includeErrors` filter nothing and `enabled` is `transient`; 12 of the 14 `src/main` constructions pass `null`. **Not ported at all** (ruling 12) — which is what keeps `fr-drc` off `fr-settings`. |

Two **totalization** rows were added as well: `getItemDescription`'s unguarded
`nets.get(...).name` and `getDetailedTraceDescription`'s unguarded
`layerStructure.layers[layer]`, both of which NPE/AIOOBE in Java. The port
panics with a message naming the item rather than returning `""` or
`(0.0, 0.0)` — Java is handed the `Item` and would throw, and a plausible-looking
wrong value in a parity document is worse than a crash. `corpus.rs` is the
guard: **147 boards, 1 skipped, no panic**.

Two **obligation-register** rows were added, both Plan 8: `DrcJsonFlavor`'s CLI
default (ruling 2) and `quality_score`'s computation (ruling 5). **No register
row was discharged by Plan 5.**

---

## 7. Test and fixture inventory

### Suites (`crates/fr-drc/tests/`)

| suite | what it pins |
|---|---|
| `clearance_list.rs` | `getAllClearanceViolations`: the four fixture counts, the dedup's surviving `firstItem`, the ordered list against the JVM transcript |
| `unconnected.rs` | the three unconnected phases, their order, the dedup and the representative rule |
| `net_incompletes.rs` | net-item order, the triangulation, length violations, the two non-total comparators (#147, #148) |
| `incompletes.rs` | `calculateAllIncompletes`, the eight counters, the clearance statistics block |
| `report.rs` | `generateReport` and the four DTOs, as normalised text against `ReportProbe` |
| `report_json.rs` | both flavors' key order, Gson byte parity on three fixtures, the escape/number facts |
| `reference_parity.rs` | the eight committed CLI references |
| `java_ports.rs` | **the five ported Java DRC suites, one named test per `@Test`** |
| `corpus.rs` | the whole `-drc` path over every `.dsn` in the corpus (`#[cfg_attr(debug_assertions, ignore)]`) |

### The five ported Java suites (Task 11)

`DesignRulesCheckerTest` (2 `@Test`s), `DrcCoordinateTest` (1),
`RatsnestClearanceHeadlessTest` (4), `UnconnectedItemsReproductionTest` (1),
`fixtures/KiCadDrcViolationRoutingTest` (3, folded into
`the_board_statistics_oracle`). **Nothing is `// not ported:`.** Four
*assertions* did not survive and each is recorded at its site: seven
`assertNotNull`s that the type system makes (`Option`/`Vec`), nine
`System.out.println`s that assert nothing, and
`RatsnestClearanceHeadlessTest`'s architectural claim (that the ratsnest is
reachable without `interactive.RatsNest`), which is a tautology in a crate with
no GUI to import. Java's lower bounds are kept **and** the exact numbers pinned
beside them — a lower bound cannot fail on over-detection, which is the
regression these fixtures are most likely to grow.

The two router-side DRC suites — `autoroute/StrictDrcEnforcementTest` and
`fixtures/StrictDrcRoutingTest` — are **Plans 6/7's**, and they call this crate.

### The six JVM probes (`crates/fr-drc/tests/data/`)

`DrcListProbe`, `UnconnectedProbe`, `NetIncompletesProbe`, `IncompletesProbe`,
`ReportProbe`, `JsonProbe`. Every expected value in the crate came out of one
of them, run against the clone's HEAD jar; `tests/data/README.md` carries the
exact command line for each, including the `-XX:hashCode=0..4` sweeps that
establish which outputs are hash-independent. `fr-board` has a seventh,
`DrcProbe.java`, for the item-level compute.

A rebuilt jar with a new `Constants.FREEROUTING_VERSION` needs `JAR_VERSION` in
`crates/fr-drc/tests/common/mod.rs` — **one** constant, shared by every suite —
regenerated along with the transcripts.

### The eight committed references (`tests/reference/drc-*/`)

`scripts/gen-drc-reference.sh` drives the real `-drc` CLI over
`tests/reference/drc-fixtures.txt`, writing `drc.json` (the jar's **verbatim**
bytes), `drc.meta.txt` and `java.log`.

| stem | inputs | violations | unconnectedItems | qualityScore |
|---|---|---|---|---|
| `drc-dev-board` | `Issue575-drc_dev-board_4_hole_clearance_violations.dsn` | 10 | 4 | 902.078369140625 |
| `drc-bbd-mars-64` | `Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn` | 96 | 3 | 828.276123046875 |
| `drc-natural-tone-preamp` | `Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.dsn` | **115** (port: 112) | 44 | 334.8545227050781 |
| `drc-issue593-rules` | `Issue593-BBD_Mars-64.dsn` + `.rules` via `-dr` | 0 | 74 | 0.0 |
| `drc-issue593-ses` | `Issue593-BBD_Mars-64.dsn` + `.ses` via the `-de` slot | 13 | 45 | 556.5908203125 |
| `drc-issue753-cpu85` | `Issue753-CPU-85_r104.dsn` | 107 | 169 | 331.1068115234375 |
| `drc-issue110-relay` | `Issue110-RelayModule.dsn` | 26 | 44 | 0.0 |
| `drc-tutorial-board` | `examples/tutorial_board/tutorial_board.dsn` | 0 | 0 | 0.0 |

They cover all five `type` strings, both auxiliary input paths, one wholly
empty report, three `0.0` quality scores and five non-trivial ones.

**Regeneration** (needs a JDK 25 in `JAVA` and the clone's HEAD jar in
`FREEROUTING_JAR`; pins `-Duser.language=en -Duser.country=US`, ruling 6, and
`-XX:+UnlockExperimentalVMOptions -XX:hashCode=2`, ruling S):

```sh
scripts/gen-drc-reference.sh                       # all eight stems
scripts/gen-drc-reference.sh drc-dev-board         # one
scripts/gen-drc-reference.sh --meta-only           # rewrite drc.meta.txt, leave drc.json
scripts/gen-drc-reference.sh --verify-hash-modes   # 5 runs/stem, count distinct documents (~3 min)
```

**`../freerouting/fixtures/*-freerouting_drc.json` are NOT references.** They
are `Freerouting 2.1.2-SNAPSHOT`-era, carry no `quality_score`, and the
dev-board one claims 5 violations / 1 unconnected where both current jars
produce 10 / 4. The lower bounds in `UnconnectedItemsReproductionTest` were
read out of those stale files, which is why they sit so far below what the
current jar produces.

### What needs the sibling checkout

`../freerouting` (or `$FREEROUTING_JAVA_DIR`) supplies the `.dsn` corpus.
**Every fixture-reading test calls `parity::require_java_dir()` first and
returns with a printed SKIP** — `FREEROUTING_JAVA_DIR=/nonexistent cargo test
-p fr-drc` is green. The **jar** is needed only by the probes, by
`gen-drc-reference.sh` and by the two differentials — never by `cargo test`.

---

## 8. The `p5t1` and `p5t2` differentials

```sh
./scripts/differential/run.sh p5t1 <dsn> [rules|-] [ses|-]
./scripts/differential/run.sh p5t2 <dsn> [rules|-] [ses|-] <mode 0-4>
./scripts/differential/sweep-p5t1.sh          # 112 rows, 110 s
./scripts/differential/sweep-p5t2.sh          # 112 rows x 5 modes = 560 pairs, 425 s
```

| driver / mode | rows | unexpected diffs | verdict |
|---|---|---|---|
| `p5t1` — report level, both sides normalised | 112 | **0** | 94 MATCH, 18 XDIFF (class 1), 1 SKIP |
| `p5t2` mode 0 — the raw clearance list (`id1,id2,layer,expected,actual`) | 112 | **0** | 112 MATCH, exact on every row |
| `p5t2` mode 1 — the unconnected list | 112 | **0** | 94 MATCH, 18 XDIFF (class 1) |
| `p5t2` mode 2 — the ratsnest through the jar's real accessors | 112 | **0** | 50 MATCH, 62 XDIFF (classes 2 and 3) |
| `p5t2` mode 3 — the ratsnest, seed pinned (ruling V) | 112 | **0** | **112 MATCH** — every counter, every endpoint, and (`ALD`) every edge's direction and acceptance position |
| `p5t2` mode 4 — transcription self-check | 112 | **0** | 112 MATCH (`TRANSCRIPTION equal 0`) |

`p5t1` covers 482 253 compared lines over its 104 corpus rows. Runtimes on the
reference machine (JDK 25, M-series): a single `run.sh p5t1` is ~13 s of which
0.7 s is the JVM; the evidence sweep (17 fixtures x 5 hash modes) is ~90 s; the
45- and 60-run union samples behind class 1 and class 2 are ~11 min.

### The three mismatch classes

**Class 1 — which dangling trace the dedup drops** (ruling S, quirk #146).
17 fixtures, 18 rows (Natural Tone appears twice). The candidate set is
hash-independent; the *emitted* set is the candidates minus those that are some
net entry's `firstItem`, and `firstItem` comes out of a `HashSet`. Every one of
the 17 rows has **at least 2 distinct** jar answers over `-XX:hashCode=0..4`;
five of them have 5. Two rows (`Issue214-freerouting`,
`Issue690-kit-dev-coldfire-xilinx_5213`) failed containment against a 5-run
union and passed against a 45-run one (7 and 28 distinct sets; unions of 94 and
384 uuids; **0** port uuids outside either). The class is **checked, not
waived**: both sweeps pin the differing uuids per row and require the two
documents to be equal once exactly those entries are removed, so a regression
from 3 differing entries to 300 is a failure.

**Class 2 — which airlines the triangulation produces** (ruling 4). `p5t2` mode
2 only, 61 of the 62 XDIFF rows. Answered by mode 3 (ruling V) and graded in
mode 2 against `AIRLINE_BUDGETS`, a per-row count of differing lines measured
under `-XX:hashCode=2` where both sides are deterministic. **It is a ratchet:
at or below the budget passes, above it fails.** After any deliberate ratsnest
change, regenerate it with the two commands in `sweep-p5t2.sh`'s header and
read every number that grew.

**A count-only budget is safe here only because mode 3 is strict.** A number of
differing lines cannot tell a hash-order artifact from a wrong edge — on its
own it would let the ratsnest degrade line-for-line as long as the total held.
What stops that is mode 3, one row over: with the seed pinned, `AL` **and**
`ALD` must be equal, so any change to which edges are chosen, which way round
they run, or the order Kruskal accepted them in fails there first. The budget
then measures only the residue mode 3 has already excluded — the drift between
two legitimate seed orders — which is exactly what ruling 4 says it measures.

**Class 3 — one net where the two seed orders find a different *number* of
airlines.** `Issue269-z10_module.dsn`, `p5t2` mode 2 only, and **the thing
`p5t2` exists for** — `p5t1` MATCHes that row. Net 1 has 4 connected groups;
the jar finds 3 airlines and the port 2, so `INCOMPLETE` reads 117 against 116.
The jar answers the same in **all six** hash modes, so it was a genuine open
question rather than a known class. It is class 2's cause reaching the count: a
spanning "tree" that leaves two groups unjoined is a normal outcome of the
degenerate triangulation (quirk #82) on **both** sides — `count == groups - 1`
fails on 13 of `Issue022`'s nets identically on both sides. **Mode 3 is the
proof**: with the seed order pinned the port's way, Java answers `NET 1 2 4`
too and that fixture matches line for line. No `fr-drc` change was warranted.

**The class that must not appear, and does not.**
`Issue229-display-8-digit-hc595.dsn` carries a documented 2.3.0-vs-HEAD *reader*
divergence (Plan 3 controller ruling E). The `p5t*` drivers run against HEAD,
so it must not appear here, and it does not. If it ever starts diffing in
`p5t1` or `p5t2` mode 0/1, **the reader has regressed, not the DRC.**

`scripts/differential/README.md` carries the classes with their evidence and
the budget table. Both drivers need a JDK 25 (`JAVA25_HOME`) and the clone's
HEAD jar (`FREEROUTING_JAR`).

---

## 9. Parked residuals, by task

| task | residual |
|---|---|
| 2 | Clearance **compensation** is never exercised: `is_clearance_compensation_used()` is `false` on every headless path (`SearchTreeManager.java:35` — both setters are GUI), so the `true` arm of the compensation split (`Item.java:429-434`) is ported but unreached by any test or fixture. |
| 2 | The `null` arm of `Via`'s `clearanceViolations` override is unreachable (`first_item` is always the queried item). Transcribed and commented rather than simplified. |
| 2 | The entry-counter divergence (per-`SearchTreeManager`, quirk #61) is unchanged; result *order* within a query is unaffected, which the 112-row `p5t1`/`p5t2` sweep confirms empirically. |
| 3 | `get_all_clearance_violations` is not idempotent at the `smallest_clearance` level — its return value is stable, every call lowers the field (quirk #153), exactly as Java's does. |
| 4 | `UnconnectedItems::new_pair` has no caller — in Java either. Kept as the ported class's public API. |
| 5 | The `.airlines.txt` goldens are a snapshot of one JVM run and will drift when the jar is rebuilt, though nothing asserts against them. The `.airlines-union.txt` files are the durable form. |
| 5 | Two order tests live in `src/` rather than `tests/`, because moving them would cost a `#[doc(hidden)] pub fn` of non-Java API. |
| 7 | `KiCadDrcReport::{new, add_violation, add_unconnected_item}` have exactly one caller each (`generate_report`); ported because they are the Java class's public API. |
| 8 | `FlavorKeys::violation_type` dispatches on the **string** the DTO holds, not on a variant — deliberate (Java's field is a `String`), but a caller who hand-builds `kind: "hole_clearance"` gets it passed through unrenamed in both flavors. Unreachable from inside the crate. |
| 8 | `schematic_parity` is `Vec<serde_json::Value>` and always empty; nothing in the Java tree ever adds to it. If Plan 8 fills it, that element type needs its own `Serialize` in declaration order like the other four. |
| 9 | `java.log` carries absolute paths and timestamps, so a regeneration dirties eight files beyond `drc.json`'s `date`. Same convention as `scripts/gen-reference.sh`. |
| 9 | `reference_parity` does not compare **key order** — it re-serialises through typed structs. Key order is pinned byte-exactly elsewhere (`head_flavor_is_the_jvms_gson_bytes`, three fixtures) and is content-independent, so the gap is covered, but it is a gap in *that* suite. |
| 11 | `corpus.rs` costs 51 s in release and minutes in debug; it is `debug_assertions`-ignored, so `cargo test --workspace` skips it and it runs only when someone remembers. |
| 11 | `java_ports.rs` deliberately duplicates assertions the task suites already make, so a number that moves has to move in two files. Both sites cite the same Java line. |
| 11 | Nothing in the crate exercises the `KiCad` flavor **against a jar**. It is pinned by `javap -p` on the 2.3.0 jar plus the mechanical rewrite proof; a 2.3.0 CLI run would be a second reference set under a second jar, which ruling 10 exists to avoid. |
| 12 | `crates/fr-drc/src/checker.rs:22`'s `drcSettings` marker carried the plan's "10 of 15" census and cited `:44`; both corrected against HEAD (12 of 14, `:45`) along with the `#143` -> `#154` citation in `statistics.rs`. Those two doc-comment lines and the `BoardComparator` marker are the only source lines Task 12 touched. |

---

## 10. Obligations for later plans

### Plans 6/7 (`fr-router`)

**Status after Plan 6** (`docs/plan-6-handoff.md`): the router is built up to and
including one connection (`fr_router::route_connection`), and it calls this crate
on every acceptance run — `tests/reference_parity.rs` and `tests/fixtures.rs` both
build a `DesignRulesChecker` per connection for spec §9's metric block, and
`violations == 0` holds on all 369 corpus connections. Of the bullets below,
`get_all_airlines`' marker has been **re-pointed from Plan 6 to Plan 7** (its
consumer `AutorouteUnroutedReport` is `autoroute/pipeline`'s, which plan-6
ruling 2 puts there), the `-mt` warning was **honoured** (`fr-router` is
single-threaded and says so), and the rest stand unchanged for Plan 7.

- **Quirk #82 (Delaunay in-circle vacuous on axis-aligned input) must stay
  unfixed.** Carried unchanged from Plans 2 and 3, and now **load-bearing in a
  second crate**: `NetIncompletes` was ported on top of it, and that is *why*
  `p5t2` mode 3 reproduces the jar's airline endpoints exactly. Fixing it
  changes which airlines the ratsnest emits and therefore what the autorouter
  routes. Post-parity only, with both crates' expectations re-baselined in the
  same commit.
- **The router's DRC gate calls this crate.**
  `autoroute/StrictDrcEnforcementTest` and `fixtures/StrictDrcRoutingTest` are
  **Plan 7's** — Plan 6 confirmed the split: both drive
  `BatchAutorouter.enforceStrictDrc`, which is `autoroute/pipeline`'s, so
  `crates/fr-router/tests/java_ports.rs` names them as out of scope rather than
  porting them. They are the two in-scope Java DRC suites Plan 5 did not
  land. They exercise `DesignRulesChecker` through the router.
- **`get_all_airlines` is the ratsnest the batch loop consumes**
  (`autoroute/pipeline/AutorouteUnroutedReport.java:19-80`, a *consumer* of
  this crate: `new DesignRulesChecker(board, null)`, `calculateAllIncompletes`,
  `getAllAirlines` at `:20-22`; the marker in `crates/fr-drc/src/lib.rs` reads
  `added in Plan 7:` since Plan 6 Task 18 re-pointed it — `AutorouteUnroutedReport`
  is an `autoroute/pipeline` class, and plan-6 ruling 2 puts that whole package in
  Plan 7). Its **counts** are stable and its **endpoints
  are order-dependent** (ruling 4). *A router that branches on which airline it
  gets will not be metric-stable.* Consume the counts, or consume the endpoints
  knowing they are one legitimate spanning tree among several.
- **`smallest_clearance` accumulates across router passes** (quirk #153). Java
  never resets it, and a routing job builds `BoardStatistics` repeatedly
  between board mutations (`BatchOptimizer.java:250`, `:371`,
  `BoardHistory.java:56`, `:198` — once per scored candidate,
  `RoutingResultManifest.java:117`), so by the end of a run every item's field
  is the minimum over **every pass of the session**, not over the board as it
  now stands. `BoardStatisticsClearanceViolations::from_violations` is a pure
  fold over a list it is handed and adds **no** pass of its own; the pass
  belongs to whichever `get_all_clearance_violations` the caller ran. Plans 6/7
  must keep that direction — build the statistics **from** the list, never
  re-scan the board for a counter — and must expect the field to be
  monotone across passes if they read it.
- **`Board::clearance_violations` is O(items x tree) and is `&mut`.** It is the
  optimizer's inner-loop cost if used naively: Java runs the whole scan once per
  `generateReport` **and** once per `BoardStatistics` built with
  `includeClearanceViolations` (`BoardStatistics.java:341`, the default of the
  one- and two-argument constructors). If the optimizer wants a violation count
  per candidate, that is a full board scan per candidate — measure before
  wiring it into a scoring loop, and note that `&mut self` means it cannot be
  parallelised over items without restructuring `SearchTreeManager`'s counter.
- **`-mt` drives nothing headless** (quirk #143, from Plan 4) — restated here
  because `fr-drc`'s compute is the obvious first candidate for a thread pool
  and Java has no counterpart. **Honoured by Plan 6** (ruling 17): `fr-router`
  spawns nothing, and its README's house rules restate the warning for Plan 7,
  because a threaded maze would be non-deterministic and would dissolve the
  per-connection acceptance ladder.

### Plan 8 (`fr-core` + surfaces)

> ## Plan 8 close-out — written by Plan 8 Task 14, the last task of the last plan
>
> **There is no Plan 9.** `docs/plan-8-handoff.md` is the project completion report; this block is
> the status of *this* hand-off's Plan-8 items, written here so a reader of this file does not have
> to go looking.
>
> | item | status |
> |---|---|
> | the whole `-drc` command line (`Freerouting.initializeDrc`) | **DISCHARGED, Task 7** — thirteen steps, with report byte parity on seven of the eight committed stems |
> | `DrcJsonFlavor`'s CLI default (obligation-register row) | **DISCHARGED, controller ruling W**: the shipped CLI writes **KiCad's** snake_case — the spelling `$schema` promises and the one freerouting 2.3.0 itself wrote — passed *explicitly* at the call site so a future change to the enum's `Default` cannot move it. `--schema freerouting` is the way back to the jar's bytes. Quirk #154 |
> | `quality_score`'s computation (obligation-register row) | **DISCHARGED, Task 7**: computed from `fr_router::score::normalized_score`, not injected. **All eight committed references match exactly**, which is the acceptance this hand-off predicted — and `cli_e2e.rs::every_committed_reference_score_is_recomputed` re-checks them without a JDK |
> | `coordinateUnit` hard-coded `"mm"` — may Plan 8 expose a unit flag? | **DECIDED: NO.** A recorded decision, carried on quirk **#151**: exposing one would make the port more capable than the jar, on neither the CLI nor the MCP tool |
> | `io/kicad`'s board/session classes (`// added in Plan 8:` markers) | **DISCHARGED**, Tasks 8-10; the markers in `crates/fr-drc/src/lib.rs` are now `renamed:` rows pointing at `crates/fr-dsn/src/kicad/**` |
> | the eight `// added in Plan 8:` score markers in `crates/fr-router/src/score/mod.rs` | **CONSUMED.** Three became `renamed:` rows in Task 2 (`BoardStatistics`' byte constructor, `countOccurrences`, `toString` — all in `fr-core`); the other five are ruling **AS**'s dead pair and **Task 14 re-pointed them to `not ported:`** with the reachability grep |
> | `schematic_parity` — if Plan 8 fills it, the element type needs its own `Serialize` | **NOT FILLED.** Nothing in the Java tree ever adds to it either; it stays `Vec<serde_json::Value>` and always empty |
> | the `-drc` **product decisions** restated for Plan 8 | **All closed** — ruling AQ (the dead flags and `-mt`), ruling W (the schema), plan ruling 6 (the stdout mode, quirk #275) |


- **Port `Freerouting.initializeDrc` (`Freerouting.java:246-372`) and call this
  crate.** The whole surface Plan 8 needs is
  `DesignRulesChecker::new(&mut board)` then `generate_report` /
  `report_to_json`. Specifically:
  - the **`-de` slot list** (`GlobalSettings.java:564-648`) is the *only* way to
    supply a session file: `-de "<dsn>+<ses>"`. **There is no `-ds` flag** and a
    `-do` SES changes nothing in the report. The `.json`-vs-`.ses` branch is at
    `Freerouting.java:296-329`.
  - **The load order is DSN → `.rules` → `.ses`, and it is load-bearing.**
    `initializeDrc` reads them in exactly that sequence — the board
    (`Freerouting.java:262-274`), then `RulesReader.read` onto the
    already-built board (`:276-293`), then `SesReader.read` /
    `KiCadJsonReader.importSession` (`:295-328`). The middle step **changes the
    clearance matrix and the net rules**, and the session's wires and vias are
    inserted *after* it, so they are created — and then checked — against the
    clearances the `.rules` file installed rather than the ones the DSN
    declared. Swapping the last two steps yields a different violation list from
    the same three files, so Plan 8 must reproduce the order and not merely the
    set of inputs. `tests/reference/drc-issue593-rules` and
    `drc-issue593-ses` are the committed references that exercise the two
    optional slots.
  - **`-dr`** supplies the rules file (`GlobalSettings.java:670-674`).
  - **`-drc [file]`** writes to the file or to stdout (`:357-370`).
  - `coordinateUnit` is hard-coded `"mm"` at `:335` (quirk #151) — the port
    ports all five `convert_coordinate` arms because the API/MCP path can reach
    them, so Plan 8 may expose a unit flag, but doing so makes the port more
    capable than Java and is a recorded decision, not a detail.
  - `source` is the input file's **base name** (`:339`).
  - `date` is `ZonedDateTime.now().format(ISO_OFFSET_DATE_TIME)`
    (`KiCadDrcReport.java:70`) — **inject a clock**; `fr-drc` has none.
  - `freerouting_version` arrives **without** the `"Freerouting "` prefix, which
    the port adds.
- **Ship `DrcJsonFlavor::KiCad` as the `-drc` CLI default** (ruling **W**,
  which closes the product decision ruling 2 deferred; obligation-register row,
  `obligation:` marker at `crates/fr-drc/src/report/json.rs:57`). The user's
  focus is KiCad and `KiCad` is what the document's own `$schema` promises;
  HEAD's camelCase — bug-compatible with the jar, quirk #154 — stays reachable
  **behind a flag**. This re-baselines nothing in `fr-drc`:
  `DrcJsonFlavor::default()` stays `FreeroutingHead`, the *parity* default the
  crate's tests pin against the jar, and the CLI passes the other row
  explicitly.
- **Supply `quality_score` from `BoardStatistics.getNormalizedScore`**
  (ruling 5, obligation-register row). It is a **`f32`**, widened to the
  report's `double` by `fr-drc`. `getNormalizedScore` (`BoardStatistics.java:600-624`)
  needs trace lengths, via counts and bend counts. The eight committed
  references pin the values it must produce — turning the injection into a
  computation gives Plan 8 eight free acceptance cases.
  > **Status (2026-09-01): AMENDED by controller ruling AG — Plan 7 Task 17's tick.**
  > The *computation* is no longer Plan 8's: `BoardStatistics`, `calculateScore`,
  > `getMaximumScore` and `getNormalizedScore` all landed in **Plan 7 Task 1** as
  > `fr_router::score` (ruling AG put the score in `fr-router` rather than opening a new
  > crate), pinned against the HEAD jar by `p7t7` under four `ScoringSettings` presets,
  > MATCH x11 plus a five-way hash-mode sweep. What is left for Plan 8 is only the
  > **wiring**: read `fr_router::score::BoardStatistics::normalized_score` and widen the
  > `f32` to the report's `double`. Two corrections to the text above, both measured:
  > `getNormalizedScore` is at `BoardStatistics.java:619-635`, not `:600-624`, and it does
  > **not** return `NaN` on a board with no connections — `:626-633` opens with
  > `if (maximumScore <= 0f) { return 0f; }`, which the port reproduces and
  > `crates/fr-router/tests/score.rs`'s `an_empty_board_scores_zero_not_nan` pins. The
  > plan had pre-assigned a quirk row to that non-existent NaN; it was struck before it
  > was written. Ruling 4 leaves `BoardScoreBreakdown`, `ScoringWeightComparison`, the
  > `byte[]`/`FileFormat` constructor and the Gson JSON as Plan 8's — they are eight
  > `// added in Plan 8:` markers in `crates/fr-router/src/score/mod.rs` and two
  > `ROSTERED` lines on the `core/scoring crates/fr-router/src` audit.
- **The KiCad JSON session/board reader is unported.**
  `KiCadJsonReader.importSession` (called at `Freerouting.java:302-307`),
  `KiCadJsonReader.readBoard` (1 011 loc), `KiCadJsonWriter.write` (227 loc) and
  the `KiCadBoardJson` DTO tree (142 loc) are `// added in Plan 8:` lines in
  `crates/fr-drc/src/lib.rs`'s roster and in the audit map. Spec §2 drops KiCad
  session JSON *output*; the *input* path is Plan 8's.
- **`board/state/BoardComparator.java`** (758 loc) was re-pointed here by
  Task 12 (ruling 13). Its marker is `// added in Plan 8:` at
  `crates/fr-board/src/board/mod.rs:57`, with a matching line in
  `crates/fr-drc/src/lib.rs`. It belongs to the result-manifest/report layer
  (spec §10); nothing in `drc/**` or on the `-drc` path references it.
- **`-drc`'s `routerSettings.enabled = false` writes only the dead
  `LegacyBridge`** (quirk #131, from Plan 4). Making `-drc` actually disable the
  router makes the port more capable than Java — Plan 8's recorded decision,
  unchanged by Plan 5, which never touched `crates/freerouting`.
- **The REST twin is `api/v1/JobOutputResource.getDrcReport` (`:590-661`)** and
  makes the *same* call the CLI does (`:645`, `:648`, `:651`, `:654`). Nothing
  behavioural is missing from `fr-drc`; what is missing is the HTTP plumbing
  and the job store, which spec §2 drops. The MCP `check_drc` tool (spec §13) is
  the surface that would replace it.
- **`BoardMetadata::router_settings` still needs a `read_metadata` second
  pass** (Plan 3's obligation). Plan 5 never needed it — the DRC surface takes
  its coordinates and options as parameters — so the obligation lands on
  whoever wires `-drc`'s settings.

---

## 11. Known limitations

- **No quality score.** Injected until Plan 8; see §10.
- **Natural Tone Preamp's report is 112 violations where the jar says
  113-115** (ruling S, quirk #146). Deliberate, documented in four places
  (`crates/fr-drc/README.md`, `tests/reference/README.md`,
  `scripts/differential/README.md` and the quirks register), and **not a number
  to tune**.
- **The airline endpoint list is not a parity surface** (ruling 4). The counts
  are, and mode 3 makes the endpoints one *given the same seed order*.
- **`Issue269-z10_module.dsn`'s net 1 finds a different airline count** under
  the jar's seed order (3) than under the port's (2). Proven Java-side seed
  order crossing quirk #82 — mode 3 makes both sides answer 2. Class 3 above.
- **`AIRLINE_BUDGETS` is a measured ratchet, not a fact about the algorithm.**
  It reproduces because `-XX:hashCode=2` is deterministic. Regenerate it after
  any deliberate ratsnest change and read every number that grew.
- **`corpus.rs` is debug-ignored.** It is the guard on the report layer's three
  `panic`s and it runs only in release, by hand.
- **Clearance compensation is never exercised** (`is_clearance_compensation_used()`
  is `false` on every headless path).
- **`get_all_clearance_violations` is not idempotent** at the
  `smallest_clearance` level (quirk #153) — as Java's is not.
- **`schematic_parity` is always empty**, in Java too.

---

## 12. Evidence

| claim | how to re-check |
|---|---|
| the workspace is green | `cargo test --workspace` — 1 479 passed, 5 ignored |
| the corpus invariant holds | `cargo test -p fr-drc --release --test corpus` — 147 boards, 1 skip, 51 s |
| the references match | `cargo test -p fr-drc --test reference_parity` |
| the crate has no unaccounted-for Java method | the four `scripts/audit-port.sh` invocations in `crates/fr-drc/README.md` §Audit, plus `drc … ClearanceViolation.java` against `fr-board` |
| `fr-board`'s markers are consumed | `grep -rn "added in Plan 5" crates/` returns nothing |
| the report is Gson's bytes | `crates/fr-drc/tests/report_json.rs::head_flavor_is_the_jvms_gson_bytes` (three fixtures) |
| the flavors differ in exactly eight strings | `::flavors_differ_only_in_the_key_tables_eight_strings` |
| the report matches the jar corpus-wide | `./scripts/differential/sweep-p5t1.sh` — 112 rows, 0 unexpected |
| the algorithm matches corpus-wide | `./scripts/differential/sweep-p5t2.sh` — 560 pairs, 0 unexpected |
| the references are portable across JVMs | `scripts/gen-drc-reference.sh --verify-hash-modes` — 7 stems give 1 document, 1 gives 5 |
| the quirks register is contiguous | `grep -o '^| [0-9]\+ ' docs/java-quirks.md` — 155 rows, 1 to 155, no gaps, no duplicates |

---

## 13. Open items for the user

1. ~~**`DrcJsonFlavor`'s CLI default is a product decision and nobody has made
   it.**~~ **Decided by ruling W**: the `-drc` CLI defaults to `KiCad`, with
   HEAD's camelCase behind a flag. Plan 8 wires it (§10); `fr-drc`'s
   `Default` stays `FreeroutingHead` for parity.
2. **The port's DRC report is deterministic and the jar's is not.** On 17 of the
   112 corpus fixtures the port emits a different number of `track_dangling`
   entries than any given jar run — always a subset, always by the same rule.
   If "match the jar exactly" is ever a product requirement, it is unsatisfiable
   as stated, because the jar does not match itself.
3. **`corpus.rs` runs only in release, by hand.** If CI ever gains a release
   test job, that is where it belongs; until then the report layer's three
   `panic`s are guarded by a test nobody runs automatically.
4. **Quirk #82 remains unfixed on purpose**, and Plans 6/7 will make it harder
   to fix, not easier. The eventual re-baseline spans `fr-geometry`, `fr-drc`
   and the router.
