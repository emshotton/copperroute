# copper-drc

The design-rule checker. It answers three questions about a board — *which
rules are violated, which items are unconnected, and how many connections
are still incomplete* — and writes the answer as a KiCad DRC report. Use
`copper_drc::prelude::*` to bring in every public type.

The crate sits on `copper-board` (the boards it checks) and on `copper-dsn` (for
`CoordinateTransform` and the shared JSON formatter), and depends on `serde`,
`serde_json` and `thiserror` besides — and on **nothing else**, in particular
not on `copper-settings`. `copper-drc → copper-dsn → copper-board → copper-geometry` is strict
and acyclic. No `tracing`, no GUI, no static mutable state, and **no
clock**: the report's `date` is an injected string.

## API surface

Everything starts from a checker borrowing the board **mutably**:

```rust,ignore
let mut drc = DesignRulesChecker::new(&mut board);
```

`&mut Board` is deliberate. Clearance queries advance the search tree's
entry counter and lower each item's memoised smallest clearance; hiding
either behind interior mutability would hide a side effect a caller can
observe.

| What you want | Call |
|---|---|
| every DRC violation | `get_all_violations() -> Vec<DrcViolation>` |
| the unconnected/dangling list | `get_all_unconnected_items() -> Vec<UnconnectedItems>` |
| the ratsnest | `calculate_all_incompletes()`, then `get_all_airlines() -> Vec<AirLine>` |
| the counters | `max_connections()`, `get_incomplete_count()`, `get_incomplete_count_for_net(n)`, `get_length_violation_count()`, `get_length_violation(n)`, `recalculate_length_violations()` |
| one net's state | `get_net_incompletes(n) -> Option<&NetIncompletes>`, `recalculate_net_incompletes(n)`, `..._with(n, &[ItemId])` |
| the KiCad DRC report | `generate_report(&DrcCoordinates, &DrcReportOptions) -> KiCadDrcReport` |
| that report as JSON | `report_to_json(&DrcCoordinates, &DrcReportOptions, DrcJsonFlavor) -> Result<String, DrcError>` |
| a hand-built report as JSON | `KiCadDrcReport::to_json(DrcJsonFlavor)` |
| the clearance block of the board statistics | `BoardStatisticsClearanceViolations::from_violations(&[DrcViolation], board_unit_to_um_factor)` |
| constraints from a KiCad project file | `apply_kicad_project(&project_json, &mut board, &transform)` |
| normalising two DRC documents for comparison | `parity::normalize_drc_json(&str)` (the `tests/parity` helper crate) |

`generate_report` and `report_to_json` are `&mut self` for the same reason
`new` takes `&mut Board`: they call `get_all_violations` internally.

### The checks

`get_all_violations` runs `checks::run_all(board, &constraints)`, one
module per check family:

| module | kinds |
|---|---|
| `checks/copper.rs` | `Clearance`, `ShortingItems`, `TracksCrossing` — copper-to-copper on each layer |
| `checks/holes.rs` | `HoleClearance`, `HoleToHole` |
| `checks/edge.rs` | `CopperEdgeClearance` |
| `checks/geometry.rs` | `TrackWidth`, `ViaDiameter`, `AnnularWidth`, `DrillOutOfRange`, `MicroviaDrillOutOfRange` |
| `checks/single.rs` | the per-item pass the geometry checks share |

A `DrcViolation` carries its `kind`, `severity`, the one or two items
involved, the layer, a position, and `expected`/`actual` values;
`kind.kicad_type()` is the string the report writes.

### Constraints

`constraints.rs` resolves the rule set a check runs under. `resolve(board)`
takes the board's own `DrcConstraints` when a KiCad project has been applied
and otherwise derives one from the design (`from_dsn`): per-class clearances
from the clearance matrix, the minimum track width, severities, and the
search radius the copper checks use. `apply_kicad_project` reads a KiCad
`.kicad_pro` document — its board design rules (minimum clearance, track
width, hole clearances, via and microvia sizes), its rule severities and
its net classes' clearances — merges them over the design-derived set, and
stores the result in `board.rules.drc_constraints`, so a board checked
against its project gets the project's numbers rather than the design
file's.

### The two injected parameter blocks

`copper-drc` has no clock and no version constant, so everything the report
needs from outside arrives as a parameter:

```rust,ignore
pub struct DrcReportOptions {
    pub source: String,               // the input file's base name
    pub coordinate_unit: String,      // "mm"
    pub date: String,                 // already formatted
    pub freerouting_version: String,  // without the "Freerouting " prefix, which the report adds
    pub quality_score: Option<f32>,   // None omits the key
}

pub struct DrcCoordinates {
    pub transform: CoordinateTransform,
    pub board_unit: Unit,
}
```

`quality_score` is `Option<f32>` because the score is computed one crate up
(`copper_router::score::BoardStatistics::normalized_score`) as a single-precision
value, and the report widens it to `f64` on the way out; typing the input
`f64` would let a caller hand in a number the scorer cannot produce.

## The two schema flavors

The KiCad DRC schema (`https://schemas.kicad.org/drc.v1.json`) spells its
keys in snake_case. The report can be written in that spelling
(`DrcJsonFlavor::KiCad`, which the `copperroute drc` command uses by default)
or in the camelCase variant earlier releases wrote (`DrcJsonFlavor::Legacy`,
selectable with `--schema legacy`). The two differ in exactly eight strings:

| field | `Legacy` | `KiCad` |
|---|---|---|
| coordinate unit | `coordinateUnits` | `coordinate_units` |
| KiCad version | `kicadVersion` | `kicad_version` |
| router version | `copperrouteVersion` | `copperroute_version` |
| unconnected list | `unconnectedItems` | `unconnected_items` |
| schematic parity | `schematicParity` | `schematic_parity` |
| quality score | `qualityScore` | `quality_score` |
| violation `type` | `holeClearance` | `hole_clearance` |
| violation `type` | `unconnectedItems` | `unconnected_items` |

`crates/copper-drc/tests/report_json.rs::flavors_differ_only_in_the_key_tables_eight_strings`
proves it mechanically. Everything below the key is
`copper_dsn::format::json`'s: two-space indent, `": "` after every key, no
trailing newline, shortest round-trip floats, `null` omitted rather than
written.

## Determinism

Every walk that could depend on hash order is pinned to item id:

| where | order |
|---|---|
| `unconnectedItems[].items` | ascending item id |
| the seed order of each net's connected-set walk | ascending item id |
| `itemsByNet` | `BTreeMap<i32, …>`, ascending net number |
| the representative item of a connected set | lowest-id `Pin`, else lowest-id `Trace`, else lowest-id item |

**Within** one connected set the walk is descending item id, because
`Board::connected_set` is a `BTreeSet<ItemId>` and the triangulation's
corner-insertion order is the board's item order. Getting this backwards
silently reverses the Delaunay insertion order inside every component and
changes which equal-length airline the ratsnest picks — a wrong-output bug
with no crash.

Two consequences worth knowing before reading a diff:

- **Airline counts are stable; airline endpoints are a function of seed
  order.** `max_connections`, `get_incomplete_count()`, the per-net counts
  and the whole violation list depend only on the board. Which equal-length
  edge the triangulation picks depends on insertion order, so two checkers
  seeded differently can agree on every count and differ on an endpoint.
- **`generate_report` folds dangling-trace entries into `violations`** and
  deduplicates them against each net entry's representative item, so the
  report's `violations` array is longer than `get_all_violations()` and the
  `unconnected_items` array is one entry per net with two or more connected
  sets. The counters and the report measure different quantities of the
  same board; `tests/incompletes.rs` asserts both against one board.

Every `%.4f` in a description is written with a `.` unconditionally, through
`copper_dsn::format::double::java_format_fixed`, so the report does not depend on
the process locale.

## Tests

`cargo test -p copper-drc` runs the unit tests plus the integration suites:

| suite | what it pins |
|---|---|
| `checks.rs` | `get_all_violations`: one check per kind, the dedup and ordering, the report keys and severities |
| `constraints.rs` | constraint resolution from the design and from a KiCad project |
| `unconnected.rs` | `get_all_unconnected_items`: the three phases, their order, the dedup, the representative rule |
| `net_incompletes.rs` | `NetIncompletes`: net-item order, the triangulation, length violations |
| `incompletes.rs` | `calculate_all_incompletes`, the counters, `BoardStatisticsClearanceViolations` |
| `report.rs` | `generate_report` and the four report types, as normalised text |
| `report_json.rs` | `report_to_json`: both flavors' key order, the escape and number rules |
| `kicad_oracle.rs` | the checker against KiCad's own DRC output on the committed boards |
| `corpus.rs` | the whole DRC path over every `.dsn` in the fixture corpus (`#[cfg_attr(debug_assertions, ignore)]`) |

`corpus.rs` checks one invariant per board — `generate_report` does not
panic, the JSON parses, and
`violations.len() == hole_clearance + clearance + track_dangling + via_dangling`
with the clearance half checked against an independent
`get_all_violations().len()` — and is the guard on the report layer's three
`panic`s (`item_description`, `detailed_trace_description`,
`item_position`), which replaced silent fallbacks that would have put a
plausible-looking wrong value into a report.

```sh
cargo test -p copper-drc --release --test corpus     # or: --test corpus -- --ignored
```

The committed reference reports in `tests/reference/drc-*` (eight stems
covering every violation type, both auxiliary input paths and both zero and
non-trivial quality scores) are the goldens `crates/copperroute`'s
end-to-end tests compare the `drc` command against;
`scripts/gen-drc-reference.sh` regenerates them.

### What needs the fixture corpus

`../freerouting` (or `FREEROUTING_JAVA_DIR`) supplies the `.dsn` corpus; it
is not vendored. Every fixture-reading test in this crate calls
`parity::require_java_dir()` first and returns with a printed SKIP when it
is absent. The synthetic-board tests (the majority of `net_incompletes.rs`,
`incompletes.rs`, `checks.rs` and `report.rs`) need nothing.

## Known limitations

- **The quality score is computed one crate up.** It needs the board
  statistics' trace lengths, via counts and bend counts, so this crate takes
  it injected and `crates/copperroute/src/commands/drc.rs` computes it.
- **Clearance compensation is never exercised.**
  `is_clearance_compensation_used()` is `false` on every headless path, so
  the compensated arm of the clearance split is unreached by any test or
  fixture.
- **`schematic_parity` is `Vec<serde_json::Value>` and always empty.** If a
  later change fills it, the element type needs its own `Serialize` in
  declaration order, like the other report types.

## Conventions this crate shares with the workspace

**`#![forbid(unsafe_code)]`** sits in the crate root, as it does in every
workspace crate.
