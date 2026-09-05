# KiCad DRC port: design

Date: 2026-09-04. Status: approved in discussion, awaiting spec review.

## Goal

Replace the Java-derived design rule checker in `fr-drc` with a native Rust
implementation of KiCad's routing-type DRC checks, so that the router's
reports and its quality score agree with `kicad-cli pcb drc` on the boards
KiCad users route with Freerouting.

The router's own obstacle avoidance is not changed. The router routes with
Freerouting's rules and is judged by KiCad's. Changing routing outcomes
through the score is accepted.

## Decisions taken

| Question | Decision |
|---|---|
| Mechanism | Native Rust port. `kicad-cli` is the test oracle, never a runtime dependency. |
| Rule source | DSN-derivable rules always; an optional `.kicad_pro` project supplies the rest. |
| Scope of violations | The report lists every violation the ported checks find. The score counts only violations that involve a trace or a via. |
| Where the port lives | Inside `fr-drc`, behind the existing `DesignRulesChecker` façade. |
| Java DRC parity | Retired. The Java reference fixtures for DRC are removed. |

## Why not the alternatives

Shelling out to `kicad-cli` costs about five seconds per call on a small
board and needs the original `.kicad_pcb`, which the DSN pipeline discards.
The batch loop evaluates the score two to three times per pass, so an
external checker cannot sit in the loop. Porting KiCad's engine wholesale
is about a megabyte of C++ coupled to KiCad's board model, wxWidgets, and
its polygon kernel, with no library API. Linking it is not viable.

## Measured baseline

On the benchmark spike board, with the routed session imported into the
stripped KiCad board:

| | Current `fr-drc` | `kicad-cli` |
|---|---|---|
| Routing-type errors | 2 | 0 |
| Unconnected items | 1 | 1 |
| Wall time | 0.24 s | 5.2 s |

The two current errors are stacked GND sub-pads of one footprint's split
thermal pad. KiCad waives same-net pairs and same-logical-pad pairs. The
Java-derived predicate treats same-net pins as obstacles to each other.

## Architecture

### Components

1. **`DrcConstraints`** in `fr-drc`. Resolved minimums in board units. Every
   field is an `Option`; `None` means the check is skipped, which is how
   KiCad's providers behave when no rule exists for a constraint type.

   Fields: per-netclass clearance and track width, `min_clearance`,
   `min_track_width`, `hole_clearance`, `hole_to_hole`,
   `copper_edge_clearance`, `min_via_diameter`, `min_via_annular_width`,
   `min_through_hole_diameter`, `min_microvia_diameter`,
   `min_microvia_drill`, and a severity map keyed by violation kind.

2. **Two sources.**
   - `DrcConstraints::from_dsn(&BoardRules)` reads per-netclass clearance
     and width from the DSN class rules and via diameter and drill from the
     via padstack names.
   - `DrcConstraints::from_kicad_project(json, &CoordinateTransform)` reads
     `board.design_settings.rules`, `board.design_settings.rule_severities`,
     and `net_settings.classes`. Millimetres convert to board units through
     the transform and round with `java_round`, matching how the DSN reader
     rounds rule values.
   - `DrcConstraints::merge(dsn, project)`: the project wins per field; the
     DSN fills gaps.

3. **Storage.** `BoardRules` gains `drc_constraints: Option<DrcConstraints>`.
   This is the only change to `fr-board`. It exists so that every existing
   `DesignRulesChecker::new(board)` call site keeps working. When the field
   is `None` the checker derives DSN-only constraints on construction.

4. **Job input.** `RoutingJob` gains an optional project file. The CLI
   exposes it as `--kicad-project <path>` on `route` and `drc`. `fr-core`
   loads it next to the rules and session files and stores the merged
   constraints on the board. A missing or unparsable project logs a warning
   and the run continues DSN-only, the same contract the rules file has.

5. **Checker.** `DesignRulesChecker` keeps its connectivity methods
   unchanged. `get_all_clearance_violations` is replaced by
   `get_all_violations() -> Vec<DrcViolation>`.

   ```rust
   pub struct DrcViolation {
       pub kind: DrcViolationKind,
       pub severity: DrcSeverity,
       pub first_item: ItemId,
       pub second_item: Option<ItemId>,
       pub layer: Option<usize>,
       pub position: FloatPoint,
       pub expected: f64,
       pub actual: f64,
       pub estimated: bool,
   }
   ```

   `DrcViolationKind` has one variant per KiCad type string in the checks
   table below. `estimated` is set when a through-hole pad drill was
   estimated rather than read.

6. **Report.** The report builder maps `DrcViolationKind` to KiCad's type
   strings directly. The two-flavour key mapping in `report/json.rs` keeps
   only the key renames the Freerouting-head flavour needs.

7. **Untouched.** `Board::clearance_violations`, the search-tree obstacle
   predicate, and the router's two internal uses of the former as an "is
   this item clean" guard stay as they are.

### Data flow

```
DSN ──► BoardRules ──► DrcConstraints::from_dsn ──┐
                                                  ├─ merge ──► BoardRules.drc_constraints
.kicad_pro ──► DrcConstraints::from_kicad_project ─┘
                                                            │
Board ──► DesignRulesChecker::get_all_violations ◄──────────┘
                │
                ├──► report::generate_report ──► KiCad DRC JSON
                ├──► RoutingResult.drc_violations
                └──► BoardStatistics (routing-involved subset only)
```

## Rule resolution

1. **Pair clearance.** For copper items A and B, the value is the larger of
   the two netclass clearances, floored by `min_clearance`. An item with no
   net contributes nothing, so the other side's netclass decides. This
   mirrors KiCad's implicit netclass rules. Freerouting's class-pair
   matrix is not consulted, and the `smd_smd` quarter clearance that
   KiCad's DSN exporter writes is ignored on purpose: it is a routing
   workaround, not a design rule.

2. **Netclass lookup.** A net's class comes from `BoardRules::nets`. The
   class clearance comes from the project's `net_settings.classes` entry
   with the same name when a project is loaded, else from the DSN class
   rule. KiCad's exporter names the default class `kicad_default` and the
   project names it `Default`; the resolver treats them as one class.

3. **Same-net waiver.** Clearance, shorting, and hole-clearance checks are
   skipped when both items carry the same defined net.

4. **Same logical pad.** Two pins are the same logical pad when they belong
   to the same component and their pin names are equal after stripping a
   trailing `@N`. Such pairs skip hole checks.

5. **Net ties.** Not implemented. The DSN carries no footprint net-tie
   metadata. Boards with net-tie footprints will diverge from KiCad here.

6. **Single-item constraints.** Track width uses the netclass width floored
   by `min_track_width`. Via diameter, annular width, and drill size use
   the project minimums, with the microvia variants for microvias.
   Hole-to-hole and hole clearance use project values only.

7. **Severity.** With a project loaded, `rule_severities` applies per kind:
   `ignore` skips the check, `warning` and `error` set the reported
   severity. Without a project every kind is `error`.

## Checks

| Kind | Subject | Rule |
|---|---|---|
| `clearance` | two copper items, different nets, same layer | gap below pair clearance and above zero |
| `shorting_items` | two copper items, different defined nets | shapes overlap, gap is zero |
| `tracks_crossing` | two trace segments, same layer, different nets | segments intersect; reported instead of a clearance or short for that pair |
| `hole_clearance` | copper item versus a via or through-hole pin drill, different or no net | gap below hole clearance, tested even when that value is zero |
| `hole_to_hole` | two drilled holes, any nets | gap below hole-to-hole minimum, each pair once |
| `copper_edge_clearance` | copper item versus board outline | gap below edge minimum |
| `track_width` | one trace | width below the resolved minimum |
| `via_diameter` | one via | diameter below minimum |
| `annular_width` | one via or through-hole pin | annulus below minimum |
| `drill_out_of_range`, `microvia_drill_out_of_range` | one via or through-hole pin | drill below minimum |
| `unconnected_items`, `track_dangling`, `via_dangling` | existing connectivity checks | unchanged |

Implementation notes:

- Candidate pairs come from the board's search-tree query with clearance.
  Gaps come from the existing tile-shape enlarge-and-intersect bisection.
  Segment intersections come from trace polyline corners. No new geometry
  kernel.
- Via drills are exact; KiCad encodes them in the padstack name.
  Through-hole pad drills are not in the DSN. The existing `drill_radius`
  estimate is used and the violation is marked `estimated`. A pin counts
  as through-hole when its padstack spans more than one copper layer.
- Each pair is reported once, in canonical item order, matching
  `kicad-cli --all-track-errors`.
- Violations are ordered by first item in board order, then kind, for a
  deterministic report.

### Out of scope

Zone-based checks (isolated copper, starved thermals, zone intersections,
connection width, slivers), courtyard, silk, text, library and schematic
parity, net ties, custom rule expressions, creepage, differential pairs,
and length matching. Zones are excluded because the DSN carries zone
outlines as wiring polygons without fills.

## Scoring and consumers

1. `BoardStatisticsClearanceViolations::from_violations` receives only the
   routing-involved subset: violations whose first or second item is a
   trace or a via, excluding the connectivity kinds, which the incomplete
   count already covers. Its fields keep their names. The "violation" size
   is `expected - actual` for every kind.
2. `RoutingResult.drc_violations` becomes `Vec<DrcViolation>`. The count
   method is unchanged.
3. The `drc` command, the `route` command's final report, and the MCP
   `check_drc` tool call `generate_report` as before and gain the
   `--kicad-project` input.
4. `fr_settings::DesignRulesCheckerSettings` is not touched.

## Error handling

- Project file missing: warning, continue DSN-only.
- Project JSON unparsable or missing `board.design_settings`: warning
  naming the path, continue DSN-only.
- A netclass named in the project but absent from the DSN: ignored.
- A net whose class is not in either source: uses the default class.
- An item whose shape cannot be resolved for a layer: skipped, as today.

## Testing

1. **Unit tests per check** in `fr-drc`, on small hand-written DSN fixtures
   that isolate one rule each: a pair under clearance, a short, a crossing,
   a via too near a pad hole, two vias too close, a trace too thin, a via
   too small, a trace too near the outline, stacked same-net sub-pads that
   must not report, and a project file that flips a severity to `ignore`.

2. **Project parsing tests** on the spike board's `stripped.kicad_pro`,
   asserting the converted board-unit values.

3. **Oracle fixtures.** A new script `scripts/gen-kicad-drc-reference.sh`
   runs, per stem in `tests/reference/kicad-drc-fixtures.txt`, the
   benchmark's vendor pipeline: SES import into the stripped board, zone
   refill, project copy, then `kicad-cli pcb drc --all-track-errors
   --format json`. It writes `kicad-drc.json` and a meta file under
   `tests/reference/<stem>/`. A new `crates/fr-drc/tests/kicad_oracle.rs`
   loads each stem through the port with the same DSN, session, and project
   and asserts per-kind counts equal the oracle's for the kinds this port
   implements, skipping when the reference is absent.

   Initial stems: the benchmark spike board, and the Java fixture boards
   that ship a KiCad board, a project, a DSN, and a routed session:
   `Issue191-processor.Z80`, `Issue269-NoViasOnPowerPlanes`,
   `Issue283-UnconnectedTracesUnderPads`, `Issue368-CorneyIslandWireless`,
   and `Issue742-tastexx-pcb`. Fixtures with a DSN but no session are added
   later by routing them with the port first.

4. **Retired tests.** `reference_parity.rs`, `java_ports.rs`,
   `clearance_list.rs`, and the clearance assertions in `report.rs` and
   `report_json.rs` that encode Java semantics are removed. The DRC entries
   in `tests/reference/drc-fixtures.txt` and the `tests/reference/drc-*`
   directories go with them. The frozen baseline under
   `tests/reference-frozen/` is not touched. Connectivity tests stay.

5. **Whole workspace.** Router and core tests that assert violation counts
   are re-baselined against the new checker and reviewed one by one, not
   bulk-updated.

6. **Benchmark.** No code change. The self-report versus referee
   disagreement metric in the comparison suite becomes the convergence
   measure for this work.

## Known divergences from `kicad-cli`

Recorded so the oracle test can filter them rather than hide them:

- Through-hole pad drills are estimated.
- Net-tie footprints are not exempted.
- Custom rule expressions in a project are not evaluated.
- Zone-dependent kinds are not produced.
