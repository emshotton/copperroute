# KiCad DRC Port Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Java-derived clearance checker inside `fr-drc` with a native port of KiCad's routing-type DRC checks, fed by the DSN plus an optional `.kicad_pro` project, validated against `kicad-cli pcb drc`.

**Architecture:** A plain `DrcConstraints` data type lives in `fr-board` so it can hang off `BoardRules`; `fr-drc` owns the builders that fill it from the DSN and from a KiCad project, a resolver that answers per-pair and per-item minimums with KiCad's netclass-max rule, and one module per check family. `DesignRulesChecker` keeps its façade; `get_all_clearance_violations` becomes `get_all_violations` returning `DrcViolation`s with KiCad kinds. Consumers in fr-router, fr-core, the CLI, and the MCP tool change only where they read the list; the router's own `Board::clearance_violations` is untouched.

**Tech Stack:** Rust 2024 workspace, `serde_json` for the project file, existing `fr-geometry` tile shapes, `kicad-cli` 10.x plus the benchmark's vendored KiCad Python scripts for the oracle.

**Spec:** `docs/superpowers/specs/2026-09-04-kicad-drc-port-design.md`

## Global Constraints

- `#![forbid(unsafe_code)]` stays on `fr-drc`.
- Comments only for unexpected behaviour (CLAUDE.md). No comments that restate code, describe old or future state, or cite plans or tickets. This applies to every code block below: copy the code, not any explanatory prose around it.
- `fr-board` must not depend on `fr-drc`. The data type crosses the boundary downward, the builders stay in `fr-drc`.
- Violation kinds serialise to KiCad's exact type strings: `clearance`, `shorting_items`, `tracks_crossing`, `hole_clearance`, `hole_to_hole`, `copper_edge_clearance`, `track_width`, `via_diameter`, `annular_width`, `drill_out_of_range`, `microvia_drill_out_of_range`, `unconnected_items`, `track_dangling`, `via_dangling`.
- Project millimetres convert to board units as `java_round(transform.dsn_to_board(Unit::scale(mm, Unit::Mm, board.communication.unit)))`.
- Every test that reads the Java checkout calls `parity::require_java_dir()` first and returns early when it is absent.
- Commit after every task with the trailer:
  ```
  Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01F97HiBCfG9KbuhEzkPmDp8
  ```
- Work happens on branch `kicad-drc-port` in the worktree at `.claude/worktrees/kicad-drc-exploration`. Never `cd` out of it.

## Spec amendments recorded by this plan

Two details discovered while planning differ from the spec and are adopted here:

1. `DrcConstraints` and `DrcSeverity` are defined in `fr-board` (`rules/drc_constraints.rs`), not `fr-drc`, because `BoardRules` stores the value and `fr-board` cannot import `fr-drc`. The builders `from_dsn`, `from_kicad_project`, `merge` semantics, and the resolver are in `fr-drc` as the spec says.
2. `tests/reference/drc-fixtures.txt` and the `tests/reference/drc-*` directories are **kept**: `scripts/quality-ab.sh` reads that list to define the 29-stem quality gate and refuses to run if the count changes. Only the parity *tests* are retired. Task 15 updates the spec.
3. The project file is loaded by the CLI and MCP layers through a helper next to `load_session_file`, mirroring how sessions and rules already reach the board. `RoutingJob` is not changed.

## File structure

| File | Responsibility |
|---|---|
| `crates/fr-board/src/rules/drc_constraints.rs` (new) | `DrcConstraints`, `DrcSeverity`, `merge` |
| `crates/fr-board/src/rules/board_rules.rs` | `drc_constraints: Option<DrcConstraints>` field |
| `crates/fr-board/src/rules/mod.rs`, `crates/fr-board/src/lib.rs` | exports |
| `crates/fr-drc/src/violation.rs` (new) | `DrcViolationKind`, `DrcViolation` |
| `crates/fr-drc/src/constraints.rs` (new) | `from_dsn`, `from_kicad_project`, `apply_kicad_project`, `resolve`, pair and item resolution |
| `crates/fr-drc/src/checks/mod.rs` (new) | `run_all`, ordering, dedupe, severity filter |
| `crates/fr-drc/src/checks/geometry.rs` (new) | gap test, hole shapes, through-hole and microvia predicates, candidate query |
| `crates/fr-drc/src/checks/copper.rs` (new) | `clearance`, `shorting_items`, `tracks_crossing`, `hole_clearance` |
| `crates/fr-drc/src/checks/holes.rs` (new) | `hole_to_hole` |
| `crates/fr-drc/src/checks/single.rs` (new) | `track_width`, `via_diameter`, `annular_width`, drill kinds |
| `crates/fr-drc/src/checks/edge.rs` (new) | `copper_edge_clearance` |
| `crates/fr-drc/src/checker.rs` | `get_all_violations` |
| `crates/fr-drc/src/report/build.rs`, `report/json.rs` | KiCad kinds and severities in the report |
| `crates/fr-drc/src/statistics.rs` | `from_violations(&[DrcViolation])` |
| `crates/fr-drc/src/error.rs`, `lib.rs` | `DrcError::Project`, exports |
| `crates/fr-drc/tests/common/synthetic.rs` (new) | programmatic test boards |
| `crates/fr-drc/tests/constraints.rs`, `checks.rs`, `kicad_oracle.rs` (new) | tests |
| `crates/fr-router/src/score/statistics.rs` | routing-involved filter |
| `crates/fr-core/src/ctx.rs`, `pipeline.rs`, `tests/pipeline.rs` | new violation type |
| `crates/freerouting/src/cli.rs`, `commands/drc.rs`, `commands/route.rs`, `mcp/tools/check_drc.rs`, `mcp/tools/schema.rs` | `--kicad-project` |
| `scripts/gen-kicad-drc-reference.sh`, `tests/reference/kicad-drc-fixtures.txt` (new) | oracle |

---

### Task 1: `DrcConstraints` data type in fr-board

**Files:**
- Create: `crates/fr-board/src/rules/drc_constraints.rs`
- Modify: `crates/fr-board/src/rules/mod.rs`, `crates/fr-board/src/rules/board_rules.rs:10-45`, `crates/fr-board/src/lib.rs:37`
- Test: `crates/fr-board/src/rules/drc_constraints.rs` (unit tests in-file)

**Interfaces:**
- Produces: `fr_board::DrcConstraints`, `fr_board::DrcSeverity`, `DrcConstraints::merge(dsn, project) -> DrcConstraints`, `BoardRules::drc_constraints: Option<DrcConstraints>`.

- [ ] **Step 1: Write the failing test**

Create `crates/fr-board/src/rules/drc_constraints.rs` with only the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_lets_the_project_win_per_field_and_the_dsn_fill_gaps() {
        let mut dsn = DrcConstraints::default();
        dsn.min_track_width = Some(2000);
        dsn.netclass_clearance.insert("Default".to_string(), 2000);
        dsn.netclass_clearance.insert("Power".to_string(), 3000);

        let mut project = DrcConstraints::default();
        project.min_track_width = Some(1500);
        project.hole_to_hole = Some(2500);
        project.netclass_clearance.insert("Default".to_string(), 1800);
        project
            .severities
            .insert("track_width".to_string(), DrcSeverity::Warning);

        let merged = DrcConstraints::merge(dsn, project);
        assert_eq!(merged.min_track_width, Some(1500));
        assert_eq!(merged.hole_to_hole, Some(2500));
        assert_eq!(merged.netclass_clearance["Default"], 1800);
        assert_eq!(merged.netclass_clearance["Power"], 3000);
        assert_eq!(merged.severities["track_width"], DrcSeverity::Warning);
    }

    #[test]
    fn a_fresh_board_rules_has_no_constraints() {
        use crate::rules::ClearanceMatrix;
        use crate::structure::{Layer, LayerStructure};
        let ls = LayerStructure::new(vec![Layer::new("F", true)]);
        let rules = crate::rules::BoardRules::new(ls.clone(), ClearanceMatrix::get_default_instance(&ls, 200));
        assert!(rules.drc_constraints.is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p fr-board drc_constraints`
Expected: compile error, `DrcConstraints` not found.

- [ ] **Step 3: Write the type**

Above the test module in the same file:

```rust
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrcSeverity {
    Error,
    Warning,
    Ignore,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DrcConstraints {
    pub netclass_clearance: BTreeMap<String, i32>,
    pub netclass_track_width: BTreeMap<String, i32>,
    pub min_clearance: Option<i32>,
    pub min_track_width: Option<i32>,
    pub hole_clearance: Option<i32>,
    pub hole_to_hole: Option<i32>,
    pub copper_edge_clearance: Option<i32>,
    pub min_via_diameter: Option<i32>,
    pub min_via_annular_width: Option<i32>,
    pub min_through_hole_diameter: Option<i32>,
    pub min_microvia_diameter: Option<i32>,
    pub min_microvia_drill: Option<i32>,
    pub severities: BTreeMap<String, DrcSeverity>,
}

impl DrcConstraints {
    #[must_use]
    pub fn merge(dsn: DrcConstraints, project: DrcConstraints) -> DrcConstraints {
        let mut netclass_clearance = dsn.netclass_clearance;
        netclass_clearance.extend(project.netclass_clearance);
        let mut netclass_track_width = dsn.netclass_track_width;
        netclass_track_width.extend(project.netclass_track_width);
        let mut severities = dsn.severities;
        severities.extend(project.severities);
        DrcConstraints {
            netclass_clearance,
            netclass_track_width,
            min_clearance: project.min_clearance.or(dsn.min_clearance),
            min_track_width: project.min_track_width.or(dsn.min_track_width),
            hole_clearance: project.hole_clearance.or(dsn.hole_clearance),
            hole_to_hole: project.hole_to_hole.or(dsn.hole_to_hole),
            copper_edge_clearance: project.copper_edge_clearance.or(dsn.copper_edge_clearance),
            min_via_diameter: project.min_via_diameter.or(dsn.min_via_diameter),
            min_via_annular_width: project.min_via_annular_width.or(dsn.min_via_annular_width),
            min_through_hole_diameter: project
                .min_through_hole_diameter
                .or(dsn.min_through_hole_diameter),
            min_microvia_diameter: project.min_microvia_diameter.or(dsn.min_microvia_diameter),
            min_microvia_drill: project.min_microvia_drill.or(dsn.min_microvia_drill),
            severities,
        }
    }
}
```

In `crates/fr-board/src/rules/mod.rs` add `pub mod drc_constraints;` and `pub use drc_constraints::{DrcConstraints, DrcSeverity};` next to the other rule exports.

In `crates/fr-board/src/rules/board_rules.rs` add to the struct, after `pub net_classes: NetClasses,`:

```rust
    pub drc_constraints: Option<DrcConstraints>,
```

and in `BoardRules::new` add `drc_constraints: None,` after `net_classes: NetClasses::new(),`. Add `DrcConstraints` to the `use super::{...}` list.

In `crates/fr-board/src/lib.rs`, extend the `pub use rules::{...}` list at line 37 with `DrcConstraints, DrcSeverity`, and add the same two names to the `pub use crate::{...}` list inside `pub mod prelude` a few lines below, keeping both lists alphabetical.

- [ ] **Step 4: Run tests**

Run: `cargo test -p fr-board`
Expected: PASS, including the two new tests. If any `BoardRules { .. }` struct literal elsewhere in fr-board fails to compile, add `drc_constraints: None` to it.

- [ ] **Step 5: Commit**

```bash
git add crates/fr-board
git commit -m "feat(board): add DrcConstraints storage on BoardRules"
```

---

### Task 2: `DrcViolation` and `DrcViolationKind`

**Files:**
- Create: `crates/fr-drc/src/violation.rs`
- Modify: `crates/fr-drc/src/lib.rs`

**Interfaces:**
- Produces: `fr_drc::{DrcViolation, DrcViolationKind}`; `DrcViolationKind::kicad_type(self) -> &'static str`; `DrcViolationKind::from_kicad_type(&str) -> Option<Self>`; `DrcViolationKind::ALL`; `DrcViolation::shortfall(&self) -> f64`; `DrcViolation::involves_routing(&self, &Board) -> bool`.

- [ ] **Step 1: Write the failing test**

Create `crates/fr-drc/src/violation.rs` with the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_round_trips_through_its_kicad_type_string() {
        for kind in DrcViolationKind::ALL {
            assert_eq!(DrcViolationKind::from_kicad_type(kind.kicad_type()), Some(kind));
        }
        assert_eq!(DrcViolationKind::Clearance.kicad_type(), "clearance");
        assert_eq!(DrcViolationKind::HoleToHole.kicad_type(), "hole_to_hole");
        assert_eq!(
            DrcViolationKind::MicroviaDrillOutOfRange.kicad_type(),
            "microvia_drill_out_of_range"
        );
        assert_eq!(DrcViolationKind::from_kicad_type("silk_overlap"), None);
    }

    #[test]
    fn the_shortfall_is_never_negative() {
        let violation = DrcViolation {
            kind: DrcViolationKind::TrackWidth,
            severity: DrcSeverity::Error,
            first_item: ItemId(1),
            second_item: None,
            layer: Some(0),
            position: FloatPoint::new(0.0, 0.0),
            expected: 100.0,
            actual: 150.0,
            estimated: false,
        };
        assert_eq!(violation.shortfall(), 0.0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p fr-drc --lib violation`
Expected: compile error, module not found.

- [ ] **Step 3: Write the types**

Above the tests in `violation.rs`:

```rust
use fr_board::{Board, DrcSeverity, Item, ItemId, ItemKind};
use fr_geometry::FloatPoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DrcViolationKind {
    Clearance,
    ShortingItems,
    TracksCrossing,
    HoleClearance,
    HoleToHole,
    CopperEdgeClearance,
    TrackWidth,
    ViaDiameter,
    AnnularWidth,
    DrillOutOfRange,
    MicroviaDrillOutOfRange,
}

impl DrcViolationKind {
    pub const ALL: [DrcViolationKind; 11] = [
        DrcViolationKind::Clearance,
        DrcViolationKind::ShortingItems,
        DrcViolationKind::TracksCrossing,
        DrcViolationKind::HoleClearance,
        DrcViolationKind::HoleToHole,
        DrcViolationKind::CopperEdgeClearance,
        DrcViolationKind::TrackWidth,
        DrcViolationKind::ViaDiameter,
        DrcViolationKind::AnnularWidth,
        DrcViolationKind::DrillOutOfRange,
        DrcViolationKind::MicroviaDrillOutOfRange,
    ];

    #[must_use]
    pub fn kicad_type(self) -> &'static str {
        match self {
            DrcViolationKind::Clearance => "clearance",
            DrcViolationKind::ShortingItems => "shorting_items",
            DrcViolationKind::TracksCrossing => "tracks_crossing",
            DrcViolationKind::HoleClearance => "hole_clearance",
            DrcViolationKind::HoleToHole => "hole_to_hole",
            DrcViolationKind::CopperEdgeClearance => "copper_edge_clearance",
            DrcViolationKind::TrackWidth => "track_width",
            DrcViolationKind::ViaDiameter => "via_diameter",
            DrcViolationKind::AnnularWidth => "annular_width",
            DrcViolationKind::DrillOutOfRange => "drill_out_of_range",
            DrcViolationKind::MicroviaDrillOutOfRange => "microvia_drill_out_of_range",
        }
    }

    #[must_use]
    pub fn from_kicad_type(name: &str) -> Option<DrcViolationKind> {
        DrcViolationKind::ALL
            .into_iter()
            .find(|kind| kind.kicad_type() == name)
    }

    #[must_use]
    pub fn is_pair(self) -> bool {
        matches!(
            self,
            DrcViolationKind::Clearance
                | DrcViolationKind::ShortingItems
                | DrcViolationKind::TracksCrossing
                | DrcViolationKind::HoleClearance
                | DrcViolationKind::HoleToHole
                | DrcViolationKind::CopperEdgeClearance
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
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

impl DrcViolation {
    #[must_use]
    pub fn shortfall(&self) -> f64 {
        (self.expected - self.actual).max(0.0)
    }

    #[must_use]
    pub fn involves_routing(&self, board: &Board) -> bool {
        let is_routing = |id: ItemId| {
            matches!(
                board.get_item(id).map(Item::kind),
                Some(ItemKind::Trace | ItemKind::Via)
            )
        };
        is_routing(self.first_item) || self.second_item.is_some_and(is_routing)
    }
}
```

In `crates/fr-drc/src/lib.rs` add `pub mod violation;` and `pub use violation::{DrcViolation, DrcViolationKind};` (also inside `prelude`). Add `pub use fr_board::DrcSeverity;` next to the existing `pub use fr_board::ClearanceViolation;`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p fr-drc --lib violation`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/fr-drc/src/violation.rs crates/fr-drc/src/lib.rs
git commit -m "feat(drc): add DrcViolation with KiCad violation kinds"
```

---

### Task 3: Constraints from the DSN and the pair resolver

**Files:**
- Create: `crates/fr-drc/src/constraints.rs`, `crates/fr-drc/tests/constraints.rs`
- Modify: `crates/fr-drc/src/lib.rs`

**Interfaces:**
- Produces: `fr_drc::constraints::{from_dsn, resolve, canonical_class_name, netclass_name, pair_clearance, track_width_min, severity, search_radius}` with signatures:
  - `pub fn from_dsn(board: &Board) -> DrcConstraints`
  - `pub fn resolve(board: &Board) -> DrcConstraints`
  - `pub fn canonical_class_name(name: &str) -> &str`
  - `pub fn netclass_name(board: &Board, net_number: i32) -> Option<String>`
  - `pub fn pair_clearance(board: &Board, c: &DrcConstraints, a: &Item, b: &Item) -> Option<i32>`
  - `pub fn track_width_min(board: &Board, c: &DrcConstraints, net_number: i32) -> Option<i32>`
  - `pub fn severity(c: &DrcConstraints, kind: DrcViolationKind) -> DrcSeverity`
  - `pub fn search_radius(c: &DrcConstraints) -> i32`

- [ ] **Step 1: Write the failing tests**

Create `crates/fr-drc/tests/constraints.rs`:

```rust
use fr_board::prelude::*;
use fr_drc::constraints::{canonical_class_name, from_dsn, pair_clearance, search_radius, severity, track_width_min};
use fr_drc::DrcViolationKind;
use fr_dsn::{BoardReadResult, DsnReadOptions};

fn spike_board() -> Board {
    let path = parity::workspace_root().join("benchmark/tests/data/spike/spike.dsn");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    match fr_dsn::read_board(&bytes[..], None, Some("spike"), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.expect("the spike DSN produces a board")
        }
        other => panic!("the spike DSN did not read: {other:?}"),
    }
}

#[test]
fn the_default_class_has_two_names() {
    assert_eq!(canonical_class_name("kicad_default"), "Default");
    assert_eq!(canonical_class_name("Default"), "Default");
    assert_eq!(canonical_class_name("Power"), "Power");
}

#[test]
fn from_dsn_reads_the_kicad_default_class_clearance_and_width() {
    let board = spike_board();
    let constraints = from_dsn(&board);
    assert_eq!(constraints.netclass_clearance.get("Default"), Some(&2000));
    assert_eq!(constraints.netclass_track_width.get("Default"), Some(&2000));
    assert_eq!(constraints.min_clearance, None);
    assert_eq!(constraints.hole_to_hole, None);
}

#[test]
fn pair_clearance_is_the_larger_netclass_value_floored_by_the_board_minimum() {
    let board = spike_board();
    let mut constraints = from_dsn(&board);
    constraints.netclass_clearance.insert("Power".to_string(), 3000);
    let items: Vec<&Item> = board.get_items().collect();
    let a = items
        .iter()
        .find(|item| item.net_count() > 0)
        .expect("a netted item");
    assert_eq!(pair_clearance(&board, &constraints, a, a), Some(2000));
    constraints.min_clearance = Some(2500);
    assert_eq!(pair_clearance(&board, &constraints, a, a), Some(2500));
}

#[test]
fn track_width_minimum_and_severity_default() {
    let board = spike_board();
    let mut constraints = from_dsn(&board);
    let net = board
        .rules
        .nets
        .iter()
        .next()
        .expect("the spike has nets")
        .net_number;
    assert_eq!(track_width_min(&board, &constraints, net), Some(2000));
    constraints.min_track_width = Some(2200);
    assert_eq!(track_width_min(&board, &constraints, net), Some(2200));
    assert_eq!(severity(&constraints, DrcViolationKind::Clearance), DrcSeverity::Error);
    constraints
        .severities
        .insert("clearance".to_string(), DrcSeverity::Ignore);
    assert_eq!(severity(&constraints, DrcViolationKind::Clearance), DrcSeverity::Ignore);
    assert!(search_radius(&constraints) >= 2000);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p fr-drc --test constraints`
Expected: compile error, `fr_drc::constraints` not found.

- [ ] **Step 3: Write the module**

Create `crates/fr-drc/src/constraints.rs`:

```rust
use fr_board::{Board, DrcConstraints, DrcSeverity, Item};

use crate::DrcViolationKind;

#[must_use]
pub fn canonical_class_name(name: &str) -> &str {
    if name == "kicad_default" { "Default" } else { name }
}

#[must_use]
pub fn from_dsn(board: &Board) -> DrcConstraints {
    let mut constraints = DrcConstraints::default();
    let matrix = &board.rules.clearance_matrix;
    for class in board.rules.net_classes.iter() {
        let name = canonical_class_name(class.get_name()).to_string();
        let clearance_class = class.get_trace_clearance_class();
        let clearance = matrix.get_value(clearance_class, clearance_class, 0, false);
        if clearance > 0 {
            constraints.netclass_clearance.insert(name.clone(), clearance);
        }
        let width = 2 * class.get_trace_half_width(0);
        if width > 0 {
            constraints.netclass_track_width.insert(name, width);
        }
    }
    constraints
}

#[must_use]
pub fn resolve(board: &Board) -> DrcConstraints {
    board
        .rules
        .drc_constraints
        .clone()
        .unwrap_or_else(|| from_dsn(board))
}

#[must_use]
pub fn netclass_name(board: &Board, net_number: i32) -> Option<String> {
    let net = board.rules.nets.get(net_number)?;
    let class = board.rules.net_classes.get(net.net_class);
    Some(canonical_class_name(class.get_name()).to_string())
}

fn netclass_clearance(board: &Board, constraints: &DrcConstraints, item: &Item) -> Option<i32> {
    if item.net_count() == 0 {
        return None;
    }
    let name = netclass_name(board, item.get_net_number(0))?;
    constraints.netclass_clearance.get(&name).copied()
}

#[must_use]
pub fn pair_clearance(
    board: &Board,
    constraints: &DrcConstraints,
    a: &Item,
    b: &Item,
) -> Option<i32> {
    [
        netclass_clearance(board, constraints, a),
        netclass_clearance(board, constraints, b),
        constraints.min_clearance,
    ]
    .into_iter()
    .flatten()
    .max()
}

#[must_use]
pub fn track_width_min(board: &Board, constraints: &DrcConstraints, net_number: i32) -> Option<i32> {
    let class_width = netclass_name(board, net_number)
        .and_then(|name| constraints.netclass_track_width.get(&name).copied());
    [class_width, constraints.min_track_width]
        .into_iter()
        .flatten()
        .max()
}

#[must_use]
pub fn severity(constraints: &DrcConstraints, kind: DrcViolationKind) -> DrcSeverity {
    constraints
        .severities
        .get(kind.kicad_type())
        .copied()
        .unwrap_or(DrcSeverity::Error)
}

#[must_use]
pub fn search_radius(constraints: &DrcConstraints) -> i32 {
    constraints
        .netclass_clearance
        .values()
        .copied()
        .chain(constraints.min_clearance)
        .chain(constraints.hole_clearance)
        .chain(constraints.hole_to_hole)
        .chain(constraints.copper_edge_clearance)
        .max()
        .unwrap_or(0)
}
```

In `lib.rs` add `pub mod constraints;`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p fr-drc --test constraints`
Expected: PASS. If `from_dsn_reads_the_kicad_default_class_clearance_and_width` reports a different number than 2000, print `board.communication.resolution` and the matrix diagonal; the spike DSN has `(resolution um 10)` and `(clearance 200)`, so 200 um at scale 10 is 2000 board units. Fix the test only if the DSN reader's scale differs from that.

- [ ] **Step 5: Commit**

```bash
git add crates/fr-drc/src/constraints.rs crates/fr-drc/src/lib.rs crates/fr-drc/tests/constraints.rs
git commit -m "feat(drc): derive KiCad constraints from the DSN and resolve pair clearances"
```

---

### Task 4: Constraints from a `.kicad_pro` project

**Files:**
- Modify: `crates/fr-drc/src/constraints.rs`, `crates/fr-drc/src/error.rs`, `crates/fr-drc/src/lib.rs`
- Test: `crates/fr-drc/tests/constraints.rs`

**Interfaces:**
- Produces: `pub fn from_kicad_project(json: &str, board: &Board, transform: &CoordinateTransform) -> Result<DrcConstraints, DrcError>`; `pub fn apply_kicad_project(json: &str, board: &mut Board, transform: &CoordinateTransform) -> Result<(), DrcError>`; `DrcError::Project(String)`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/fr-drc/tests/constraints.rs`:

```rust
use fr_drc::constraints::{apply_kicad_project, from_kicad_project};
use fr_drc::DrcError;

fn spike_board_with_transform() -> (Board, fr_dsn::CoordinateTransform) {
    let path = parity::workspace_root().join("benchmark/tests/data/spike/spike.dsn");
    let bytes = std::fs::read(&path).expect("the spike DSN is in the repo");
    match fr_dsn::read_board(&bytes[..], None, Some("spike"), &DsnReadOptions::default()) {
        BoardReadResult::Success {
            board,
            coordinate_transform,
            ..
        }
        | BoardReadResult::OutlineMissing {
            board,
            coordinate_transform,
            ..
        } => (
            *board.expect("a board"),
            coordinate_transform.expect("a transform"),
        ),
        other => panic!("the spike DSN did not read: {other:?}"),
    }
}

fn spike_project() -> String {
    std::fs::read_to_string(
        parity::workspace_root().join("benchmark/tests/data/spike/stripped.kicad_pro"),
    )
    .expect("the spike project is in the repo")
}

#[test]
fn the_spike_project_rules_convert_to_board_units() {
    let (board, transform) = spike_board_with_transform();
    let constraints = from_kicad_project(&spike_project(), &board, &transform).expect("parses");
    assert_eq!(constraints.min_clearance, None);
    assert_eq!(constraints.min_track_width, None);
    assert_eq!(constraints.hole_clearance, Some(2500));
    assert_eq!(constraints.hole_to_hole, Some(2500));
    assert_eq!(constraints.copper_edge_clearance, Some(5000));
    assert_eq!(constraints.min_via_diameter, Some(5000));
    assert_eq!(constraints.min_via_annular_width, Some(1000));
    assert_eq!(constraints.min_through_hole_diameter, Some(3000));
    assert_eq!(constraints.min_microvia_diameter, Some(2000));
    assert_eq!(constraints.min_microvia_drill, Some(1000));
    assert_eq!(constraints.netclass_clearance.get("Default"), Some(&2000));
    assert_eq!(constraints.netclass_clearance.get("Power"), Some(&2000));
    assert_eq!(constraints.netclass_track_width.get("Power"), Some(&4000));
    assert_eq!(constraints.severities.get("clearance"), Some(&DrcSeverity::Error));
}

#[test]
fn a_zero_hole_clearance_is_kept_and_other_zeros_are_dropped() {
    let (board, transform) = spike_board_with_transform();
    let json = r#"{"board":{"design_settings":{"rules":{"min_hole_clearance":0.0,"min_track_width":0.0,"min_hole_to_hole":0.0}}}}"#;
    let constraints = from_kicad_project(json, &board, &transform).expect("parses");
    assert_eq!(constraints.hole_clearance, Some(0));
    assert_eq!(constraints.min_track_width, None);
    assert_eq!(constraints.hole_to_hole, None);
}

#[test]
fn a_project_without_design_settings_is_an_error() {
    let (board, transform) = spike_board_with_transform();
    let error = from_kicad_project(r#"{"meta":{}}"#, &board, &transform).unwrap_err();
    assert!(matches!(error, DrcError::Project(_)), "{error}");
    let error = from_kicad_project("not json", &board, &transform).unwrap_err();
    assert!(matches!(error, DrcError::Json(_)), "{error}");
}

#[test]
fn apply_stores_the_merged_constraints_on_the_board() {
    let (mut board, transform) = spike_board_with_transform();
    assert!(board.rules.drc_constraints.is_none());
    apply_kicad_project(&spike_project(), &mut board, &transform).expect("applies");
    let stored = board.rules.drc_constraints.as_ref().expect("stored");
    assert_eq!(stored.hole_to_hole, Some(2500));
    assert_eq!(stored.netclass_track_width.get("Default"), Some(&2000));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p fr-drc --test constraints`
Expected: compile error, `from_kicad_project` not found.

- [ ] **Step 3: Write the parser**

Add to `crates/fr-drc/src/error.rs`:

```rust
    #[error("KiCad project: {0}")]
    Project(String),
```

Append to `crates/fr-drc/src/constraints.rs`:

```rust
use fr_board::Unit;
use fr_dsn::CoordinateTransform;
use fr_geometry::java_round;
use serde_json::Value;

use crate::DrcError;

fn to_board_units(mm: f64, board: &Board, transform: &CoordinateTransform) -> i32 {
    let dsn_value = Unit::scale(mm, Unit::Mm, board.communication.unit);
    java_round(transform.dsn_to_board(dsn_value)) as i32
}

fn rule(rules: &Value, key: &str, board: &Board, transform: &CoordinateTransform) -> Option<i32> {
    let mm = rules.get(key)?.as_f64()?;
    Some(to_board_units(mm, board, transform))
}

fn positive(value: Option<i32>) -> Option<i32> {
    value.filter(|v| *v > 0)
}

fn parse_severity(text: &str) -> Option<DrcSeverity> {
    match text {
        "error" => Some(DrcSeverity::Error),
        "warning" => Some(DrcSeverity::Warning),
        "ignore" | "exclusion" => Some(DrcSeverity::Ignore),
        _ => None,
    }
}

pub fn from_kicad_project(
    json: &str,
    board: &Board,
    transform: &CoordinateTransform,
) -> Result<DrcConstraints, DrcError> {
    let document: Value = serde_json::from_str(json)?;
    let settings = document
        .pointer("/board/design_settings")
        .ok_or_else(|| DrcError::Project("no board.design_settings object".to_string()))?;
    let rules = settings.get("rules").cloned().unwrap_or(Value::Null);
    let mut constraints = DrcConstraints {
        min_clearance: positive(rule(&rules, "min_clearance", board, transform)),
        min_track_width: positive(rule(&rules, "min_track_width", board, transform)),
        hole_clearance: rule(&rules, "min_hole_clearance", board, transform),
        hole_to_hole: positive(rule(&rules, "min_hole_to_hole", board, transform)),
        copper_edge_clearance: positive(rule(&rules, "min_copper_edge_clearance", board, transform)),
        min_via_diameter: positive(rule(&rules, "min_via_diameter", board, transform)),
        min_via_annular_width: positive(rule(&rules, "min_via_annular_width", board, transform)),
        min_through_hole_diameter: positive(rule(&rules, "min_through_hole_diameter", board, transform)),
        min_microvia_diameter: positive(rule(&rules, "min_microvia_diameter", board, transform)),
        min_microvia_drill: positive(rule(&rules, "min_microvia_drill", board, transform)),
        ..DrcConstraints::default()
    };
    if let Some(severities) = settings.get("rule_severities").and_then(Value::as_object) {
        for (name, value) in severities {
            if let Some(severity) = value.as_str().and_then(parse_severity) {
                constraints.severities.insert(name.clone(), severity);
            }
        }
    }
    if let Some(classes) = document
        .pointer("/net_settings/classes")
        .and_then(Value::as_array)
    {
        for class in classes {
            let Some(name) = class.get("name").and_then(Value::as_str) else {
                continue;
            };
            let name = canonical_class_name(name).to_string();
            if let Some(clearance) = positive(rule(class, "clearance", board, transform)) {
                constraints.netclass_clearance.insert(name.clone(), clearance);
            }
            if let Some(width) = positive(rule(class, "track_width", board, transform)) {
                constraints.netclass_track_width.insert(name, width);
            }
        }
    }
    Ok(constraints)
}

pub fn apply_kicad_project(
    json: &str,
    board: &mut Board,
    transform: &CoordinateTransform,
) -> Result<(), DrcError> {
    let project = from_kicad_project(json, board, transform)?;
    let dsn = from_dsn(board);
    board.rules.drc_constraints = Some(DrcConstraints::merge(dsn, project));
    Ok(())
}
```

Move the `use` lines to the top of the file with the others. `fr_board::Unit` is re-exported from `fr_board::structure`; if the path fails, use `fr_board::structure::Unit`. In `lib.rs` add `pub use constraints::apply_kicad_project;`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p fr-drc --test constraints`
Expected: PASS. The spike project has `min_hole_clearance: 0.25` mm; at 0.1 um board units that is 2500.

- [ ] **Step 5: Commit**

```bash
git add crates/fr-drc/src/constraints.rs crates/fr-drc/src/error.rs crates/fr-drc/src/lib.rs crates/fr-drc/tests/constraints.rs
git commit -m "feat(drc): read KiCad project design rules into DrcConstraints"
```

---

### Task 5: Geometry helpers and the synthetic test board

**Files:**
- Create: `crates/fr-drc/src/checks/mod.rs`, `crates/fr-drc/src/checks/geometry.rs`, `crates/fr-drc/tests/common/synthetic.rs`, `crates/fr-drc/tests/checks.rs`
- Modify: `crates/fr-drc/src/lib.rs`, `crates/fr-drc/tests/common/mod.rs`

**Interfaces:**
- Produces in `fr_drc::checks::geometry`:
  - `pub struct Hole { pub shape: TileShape, pub radius: f64, pub estimated: bool, pub center: FloatPoint }`
  - `pub fn gap_below(a: &TileShape, b: &TileShape, clearance: i32) -> Option<(f64, FloatPoint)>`
  - `pub fn hole_of(board: &Board, id: ItemId) -> Option<Hole>`
  - `pub fn is_through_hole_pin(board: &Board, id: ItemId) -> bool`
  - `pub fn is_microvia(board: &Board, id: ItemId) -> bool`
  - `pub fn is_copper(item: &Item) -> bool`
  - `pub fn item_shapes(board: &mut Board, id: ItemId) -> Vec<(usize, TileShape)>`
  - `pub fn candidates(board: &Board, shape: &TileShape, layer: Option<usize>, radius: i32) -> Vec<ItemId>`
  - `pub fn item_position(board: &Board, id: ItemId) -> FloatPoint`
- Produces in tests: `common::synthetic::{SyntheticBoard, two_layers}` with the builder API shown below.

- [ ] **Step 1: Write the synthetic board helper**

Create `crates/fr-drc/tests/common/synthetic.rs`:

```rust
#![allow(dead_code)]

use fr_board::prelude::*;
use fr_geometry::{Circle, IntBox, IntPoint, IntVector, Point, Polyline, PolylineShapeRef, Shape, TileShape};

pub const BOUNDING_BOX: IntBox = IntBox {
    ll: IntPoint {
        x: -100_000,
        y: -100_000,
    },
    ur: IntPoint {
        x: 100_000,
        y: 100_000,
    },
};

pub fn two_layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("F.Cu", true), Layer::new("B.Cu", true)])
}

pub struct PadSpec {
    pub name: &'static str,
    pub half: i32,
    pub offset: IntVector,
    pub through_hole: bool,
}

pub struct SyntheticBoard {
    pub board: Board,
    pub via_padstack: PadstackId,
    pub microvia_padstack: PadstackId,
}

impl SyntheticBoard {
    pub fn new(pads: &[PadSpec], net_count: usize, clearance: i32) -> SyntheticBoard {
        let ls = two_layers();
        let mut padstacks = Padstacks::new(ls.clone());
        let mut pins = Vec::new();
        for pad in pads {
            let shape = Shape::Tile(TileShape::Box(IntBox::from_coords(
                -pad.half, -pad.half, pad.half, pad.half,
            )));
            let shapes = if pad.through_hole {
                vec![Some(shape.clone()), Some(shape)]
            } else {
                vec![Some(shape), None]
            };
            let padstack = padstacks.add(pad.name, shapes, pad.through_hole, false);
            pins.push(PackagePin::new(pad.name, padstack, pad.offset.into(), 0.0));
        }
        let via_shape = Shape::Circle(Circle::new(IntPoint::new(0, 0), 3000));
        let via_padstack = padstacks.add(
            "Via[0-1]_600:300_um",
            vec![Some(via_shape.clone()), Some(via_shape)],
            true,
            false,
        );
        let micro_shape = Shape::Circle(Circle::new(IntPoint::new(0, 0), 1500));
        let microvia_padstack = padstacks.add(
            "Via[0-1]_300:100_um",
            vec![Some(micro_shape.clone()), Some(micro_shape)],
            true,
            false,
        );
        let mut packages = Packages::new();
        let package = packages.add(
            "pkg", pins, None, None, None, Vec::new(), Vec::new(), Vec::new(), true,
        );
        let mut components = Components::new();
        components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, package);

        let matrix = ClearanceMatrix::get_default_instance(&ls, clearance);
        let mut rules = BoardRules::new(ls, matrix);
        rules.create_default_net_class();
        let default_class = rules.get_default_net_class();
        let mut board = Board::new(
            Vec::new(),
            1,
            BOUNDING_BOX,
            rules,
            BoardLibrary::new(padstacks, packages),
            components,
            Communication::default(),
        );
        for i in 0..net_count {
            board
                .rules
                .nets
                .add(format!("N{}", i + 1), 1, false, default_class);
        }
        board.insert_outline(
            vec![PolylineShapeRef::Tile(TileShape::Box(IntBox::from_coords(
                -50_000, -50_000, 50_000, 50_000,
            )))],
            1,
        );
        SyntheticBoard {
            board,
            via_padstack,
            microvia_padstack,
        }
    }

    pub fn pin(&mut self, pin_index: i32, net: i32) -> ItemId {
        self.board
            .insert_pin(1, pin_index, vec![net], 1, FixedState::Unfixed)
    }

    pub fn trace(&mut self, points: &[(i32, i32)], layer: usize, half_width: i32, net: i32) -> ItemId {
        let points: Vec<Point> = points.iter().map(|(x, y)| Point::new(*x, *y)).collect();
        self.board
            .insert_trace_without_cleaning(
                Polyline::from_points(&points),
                layer,
                half_width,
                vec![net],
                1,
                FixedState::Unfixed,
            )
            .expect("a synthetic trace has at least two distinct corners")
    }

    pub fn via(&mut self, x: i32, y: i32, net: i32) -> ItemId {
        self.board
            .insert_via(self.via_padstack, Point::new(x, y), vec![net], 1, FixedState::Unfixed, true)
            .expect("a synthetic via inserts")
    }

    pub fn microvia(&mut self, x: i32, y: i32, net: i32) -> ItemId {
        self.board
            .insert_via(self.microvia_padstack, Point::new(x, y), vec![net], 1, FixedState::Unfixed, true)
            .expect("a synthetic microvia inserts")
    }
}
```

The synthetic board uses 0.1 um units like a KiCad export, so a 3000 radius via is 0.6 mm wide with a 0.3 mm drill encoded in its name, and the default clearance passed to `new` is in the same units.

In `crates/fr-drc/tests/common/mod.rs` add `pub mod synthetic;`.

- [ ] **Step 2: Write the failing geometry tests**

Create `crates/fr-drc/tests/checks.rs`:

```rust
mod common;

use common::synthetic::{PadSpec, SyntheticBoard};
use fr_board::prelude::*;
use fr_drc::checks::geometry::{gap_below, hole_of, is_microvia, is_through_hole_pin, item_shapes};
use fr_geometry::{IntBox, IntVector, TileShape};

fn boxes(gap: i32) -> (TileShape, TileShape) {
    (
        TileShape::Box(IntBox::from_coords(0, 0, 1000, 1000)),
        TileShape::Box(IntBox::from_coords(1000 + gap, 0, 2000 + gap, 1000)),
    )
}

#[test]
fn gap_below_reports_the_actual_gap_only_when_it_is_under_the_clearance() {
    let (a, b) = boxes(300);
    assert!(gap_below(&a, &b, 300).is_none());
    let (actual, _) = gap_below(&a, &b, 500).expect("300 is under 500");
    assert!((actual - 300.0).abs() < 2.0, "actual {actual}");
    let (a, b) = boxes(-100);
    let (actual, _) = gap_below(&a, &b, 500).expect("overlap is under any clearance");
    assert_eq!(actual, 0.0);
    assert!(gap_below(&a, &b, 0).is_some(), "overlap is reported even at zero clearance");
    let (a, b) = boxes(10);
    assert!(gap_below(&a, &b, 0).is_none(), "a gap is fine at zero clearance");
}

#[test]
fn via_holes_are_exact_and_pad_holes_are_estimated() {
    let mut synthetic = SyntheticBoard::new(
        &[
            PadSpec { name: "1", half: 800, offset: IntVector::new(0, 0), through_hole: true },
            PadSpec { name: "2", half: 800, offset: IntVector::new(5000, 0), through_hole: false },
        ],
        1,
        2000,
    );
    let via = synthetic.via(20_000, 0, 1);
    let micro = synthetic.microvia(30_000, 0, 1);
    let th = synthetic.pin(0, 1);
    let smd = synthetic.pin(1, 1);
    let board = &synthetic.board;

    let hole = hole_of(board, via).expect("a via has a hole");
    assert!(!hole.estimated);
    assert!((hole.radius - 1500.0).abs() < 1.0, "radius {}", hole.radius);
    assert!(is_through_hole_pin(board, th));
    assert!(!is_through_hole_pin(board, smd));
    let pad_hole = hole_of(board, th).expect("a through-hole pin has a hole");
    assert!(pad_hole.estimated);
    assert!(hole_of(board, smd).is_none());
    assert!(!is_microvia(board, via));
    assert!(is_microvia(board, micro));
}

#[test]
fn item_shapes_lists_one_shape_per_layer_for_a_via_and_one_for_a_trace() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    let via = synthetic.via(0, 0, 1);
    let trace = synthetic.trace(&[(0, 0), (10_000, 0)], 1, 500, 1);
    let via_shapes = item_shapes(&mut synthetic.board, via);
    assert_eq!(via_shapes.iter().map(|(layer, _)| *layer).collect::<Vec<_>>(), vec![0, 1]);
    let trace_shapes = item_shapes(&mut synthetic.board, trace);
    assert_eq!(trace_shapes.len(), 1);
    assert_eq!(trace_shapes[0].0, 1);
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p fr-drc --test checks`
Expected: compile error, `fr_drc::checks` not found.

- [ ] **Step 4: Write the geometry module**

Create `crates/fr-drc/src/checks/mod.rs`:

```rust
pub mod geometry;
```

Create `crates/fr-drc/src/checks/geometry.rs`:

```rust
use fr_board::{Board, Item, ItemId, ItemKind, TreeObject};
use fr_geometry::{Circle, FloatPoint, TileShape, java_round};

pub struct Hole {
    pub shape: TileShape,
    pub radius: f64,
    pub estimated: bool,
    pub center: FloatPoint,
}

#[must_use]
pub fn gap_below(a: &TileShape, b: &TileShape, clearance: i32) -> Option<(f64, FloatPoint)> {
    let half = f64::from(clearance) / 2.0;
    let (ea, eb) = if clearance > 0 {
        (a.enlarge(half), b.enlarge(half))
    } else {
        (a.clone(), b.clone())
    };
    let overlap = ea.intersection(&eb);
    if overlap.dimension() != 2 {
        return None;
    }
    let position = overlap.centre_of_gravity();
    if a.intersection(b).dimension() == 2 {
        return Some((0.0, position));
    }
    let actual = Board::calculate_clearance_between_two_shapes(a, b, f64::from(clearance), 0, 0);
    Some((actual, position))
}

#[must_use]
pub fn is_copper(item: &Item) -> bool {
    matches!(
        item.kind(),
        ItemKind::Trace | ItemKind::Via | ItemKind::Pin | ItemKind::ConductionArea
    )
}

#[must_use]
pub fn is_through_hole_pin(board: &Board, id: ItemId) -> bool {
    let ctx = board.ctx();
    match board.get_item(id) {
        Some(Item::Pin(pin)) => pin.first_layer(&ctx) != pin.last_layer(&ctx),
        _ => false,
    }
}

#[must_use]
pub fn is_microvia(board: &Board, id: ItemId) -> bool {
    let ctx = board.ctx();
    let Some(Item::Via(via)) = board.get_item(id) else {
        return false;
    };
    let Some(padstack) = via.get_padstack(&ctx) else {
        return false;
    };
    let last = padstack.board_layer_count() as i32 - 1;
    let (from, to) = (padstack.from_layer(), padstack.to_layer());
    to - from == 1 && (from == 0 || to == last) && last > 1
}

fn hole_from(center: FloatPoint, radius: f64, estimated: bool) -> Hole {
    let circle = Circle::new(center.round(), java_round(radius) as i32);
    Hole {
        shape: TileShape::Octagon(circle.bounding_octagon()),
        radius,
        estimated,
        center,
    }
}

#[must_use]
pub fn hole_of(board: &Board, id: ItemId) -> Option<Hole> {
    let ctx = board.ctx();
    match board.get_item(id)? {
        Item::Via(via) => {
            let padstack = via.get_padstack(&ctx)?;
            let estimated = !padstack.name.contains(':');
            Some(hole_from(via.get_center().to_float(), padstack.drill_radius(), estimated))
        }
        Item::Pin(pin) => {
            if pin.first_layer(&ctx) == pin.last_layer(&ctx) {
                return None;
            }
            let padstack = pin.get_padstack(&ctx)?;
            let estimated = !padstack.name.contains(':');
            Some(hole_from(pin.get_center(&ctx).to_float(), padstack.drill_radius(), estimated))
        }
        _ => None,
    }
}

#[must_use]
pub fn item_shapes(board: &mut Board, id: ItemId) -> Vec<(usize, TileShape)> {
    let layers: Vec<usize> = {
        let ctx = board.ctx();
        let Some(item) = board.get_item(id) else {
            return Vec::new();
        };
        (0..item.tile_shape_count(&ctx))
            .map(|i| item.shape_layer(i, &ctx))
            .collect()
    };
    layers
        .into_iter()
        .enumerate()
        .filter_map(|(i, layer)| board.item_tile_shape(id, i).map(|shape| (layer, shape)))
        .collect()
}

#[must_use]
pub fn candidates(board: &Board, shape: &TileShape, layer: Option<usize>, radius: i32) -> Vec<ItemId> {
    let query = if radius > 0 { shape.enlarge(f64::from(radius)) } else { shape.clone() };
    board
        .overlapping_objects(&query, layer)
        .into_iter()
        .filter_map(|object| match object {
            TreeObject::Item(id) => Some(id),
            TreeObject::Room(_) => None,
        })
        .collect()
}

#[must_use]
pub fn item_position(board: &Board, id: ItemId) -> FloatPoint {
    let ctx = board.ctx();
    match board.get_item(id) {
        Some(item) => TileShape::Box(item.bounding_box(&ctx)).centre_of_gravity(),
        None => FloatPoint::new(0.0, 0.0),
    }
}
```

In `lib.rs` add `pub mod checks;`. If `TreeObject` is not exported at the crate root of fr-board, import it from `fr_board::prelude::TreeObject` or `fr_board::ids::TreeObject`; check `crates/fr-board/src/lib.rs:23`.

- [ ] **Step 5: Run tests**

Run: `cargo test -p fr-drc --test checks`
Expected: PASS. If `is_microvia` fails for the synthetic microvia, note that both synthetic padstacks span layers 0 and 1 of a two-layer board, so `last > 1` is false and neither is a microvia on two layers; change the synthetic board to use `is_microvia` only in a four-layer variant, or relax the test to `assert!(!is_microvia(board, micro))` on two layers and add a four-layer case in Task 8. Do not change the predicate: a via spanning both outer layers of a two-layer board is a through via in KiCad too.

- [ ] **Step 6: Commit**

```bash
git add crates/fr-drc/src/checks crates/fr-drc/src/lib.rs crates/fr-drc/tests
git commit -m "feat(drc): geometry helpers for the KiCad checks and a synthetic test board"
```

---

### Task 6: Copper pair checks

**Files:**
- Create: `crates/fr-drc/src/checks/copper.rs`
- Modify: `crates/fr-drc/src/checks/mod.rs`
- Test: `crates/fr-drc/tests/checks.rs`

**Interfaces:**
- Produces: `pub fn run(board: &mut Board, constraints: &DrcConstraints, out: &mut Vec<DrcViolation>)` in `fr_drc::checks::copper`.
- Consumes: Task 3 resolver functions, Task 5 geometry.

- [ ] **Step 1: Write the failing tests**

Append to `crates/fr-drc/tests/checks.rs`:

```rust
use fr_board::DrcConstraints;
use fr_drc::checks::copper;
use fr_drc::{DrcViolation, DrcViolationKind};

fn kinds(violations: &[DrcViolation]) -> Vec<DrcViolationKind> {
    let mut kinds: Vec<DrcViolationKind> = violations.iter().map(|v| v.kind).collect();
    kinds.sort();
    kinds
}

fn constraints_with(clearance: i32) -> DrcConstraints {
    let mut constraints = DrcConstraints::default();
    constraints.netclass_clearance.insert("default".to_string(), clearance);
    constraints
}

#[test]
fn two_traces_on_different_nets_closer_than_the_clearance_are_a_clearance_violation() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 500, 1);
    synthetic.trace(&[(0, 1500), (10_000, 1500)], 0, 500, 2);
    let mut out = Vec::new();
    copper::run(&mut synthetic.board, &constraints_with(2000), &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::Clearance]);
    assert_eq!(out[0].expected, 2000.0);
    assert!((out[0].actual - 500.0).abs() < 2.0, "actual {}", out[0].actual);
    assert_eq!(out[0].layer, Some(0));
}

#[test]
fn the_same_pair_is_reported_once() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 500, 1);
    synthetic.trace(&[(0, 1500), (10_000, 1500)], 0, 500, 2);
    let mut out = Vec::new();
    copper::run(&mut synthetic.board, &constraints_with(2000), &mut out);
    assert_eq!(out.len(), 1);
}

#[test]
fn same_net_items_are_never_clearance_violations() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 500, 1);
    synthetic.trace(&[(0, 1500), (10_000, 1500)], 0, 500, 1);
    let mut out = Vec::new();
    copper::run(&mut synthetic.board, &constraints_with(2000), &mut out);
    assert!(out.is_empty(), "{out:?}");
}

#[test]
fn overlapping_items_on_different_nets_are_shorting_items() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 500, 1);
    synthetic.trace(&[(0, 200), (10_000, 200)], 0, 500, 2);
    let mut out = Vec::new();
    copper::run(&mut synthetic.board, &constraints_with(2000), &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::ShortingItems]);
    assert_eq!(out[0].actual, 0.0);
}

#[test]
fn crossing_traces_are_tracks_crossing_and_nothing_else() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.trace(&[(-5000, 0), (5000, 0)], 0, 500, 1);
    synthetic.trace(&[(0, -5000), (0, 5000)], 0, 500, 2);
    let mut out = Vec::new();
    copper::run(&mut synthetic.board, &constraints_with(2000), &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::TracksCrossing]);
    assert!(out[0].position.distance(&fr_geometry::FloatPoint::new(0.0, 0.0)) < 1.0);
}

#[test]
fn a_trace_near_a_via_hole_on_another_net_is_a_hole_clearance_violation() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.via(0, 0, 1);
    synthetic.trace(&[(-10_000, 4000), (10_000, 4000)], 0, 300, 2);
    let mut constraints = constraints_with(100);
    constraints.hole_clearance = Some(2500);
    let mut out = Vec::new();
    copper::run(&mut synthetic.board, &constraints, &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::HoleClearance]);
    assert_eq!(out[0].expected, 2500.0);
    assert!(!out[0].estimated);
}

#[test]
fn stacked_same_net_sub_pads_report_nothing() {
    let mut synthetic = SyntheticBoard::new(
        &[
            PadSpec { name: "41@1", half: 900, offset: IntVector::new(0, 0), through_hole: true },
            PadSpec { name: "41@2", half: 900, offset: IntVector::new(0, 1400), through_hole: true },
        ],
        1,
        2000,
    );
    synthetic.pin(0, 1);
    synthetic.pin(1, 1);
    let mut constraints = constraints_with(2000);
    constraints.hole_clearance = Some(500);
    let mut out = Vec::new();
    copper::run(&mut synthetic.board, &constraints, &mut out);
    assert!(out.is_empty(), "{out:?}");
}

#[test]
fn different_net_pads_of_one_footprint_still_get_clearance_checked() {
    let mut synthetic = SyntheticBoard::new(
        &[
            PadSpec { name: "1", half: 900, offset: IntVector::new(0, 0), through_hole: false },
            PadSpec { name: "2", half: 900, offset: IntVector::new(2500, 0), through_hole: false },
        ],
        2,
        2000,
    );
    synthetic.pin(0, 1);
    synthetic.pin(1, 2);
    let mut out = Vec::new();
    copper::run(&mut synthetic.board, &constraints_with(2000), &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::Clearance]);
}
```

The synthetic board's only net class is named `default`, so `constraints_with` keys the clearance under that name. Real DSN boards use `Default` via `canonical_class_name`; both flow through `netclass_name`, which returns the class's own name.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p fr-drc --test checks`
Expected: compile error, `checks::copper` not found.

- [ ] **Step 3: Write the copper module**

Create `crates/fr-drc/src/checks/copper.rs`:

```rust
use std::collections::BTreeSet;

use fr_board::{Board, DrcConstraints, DrcSeverity, Item, ItemId};
use fr_geometry::{FloatLine, FloatPoint, TileShape};

use crate::checks::geometry::{candidates, gap_below, hole_of, is_copper, item_shapes};
use crate::constraints::{pair_clearance, search_radius, severity};
use crate::{DrcViolation, DrcViolationKind};

type PairKey = (u32, u32, Option<usize>, DrcViolationKind);

fn ordered(a: ItemId, b: ItemId) -> (ItemId, ItemId) {
    if a.0 <= b.0 { (a, b) } else { (b, a) }
}

fn same_defined_net(a: &Item, b: &Item) -> bool {
    a.net_count() > 0 && a.shares_net(b)
}

fn logical_pad_name(name: &str) -> &str {
    name.split('@').next().unwrap_or(name)
}

fn same_logical_pad(board: &Board, a: &Item, b: &Item) -> bool {
    let ctx = board.ctx();
    match (a, b) {
        (Item::Pin(pa), Item::Pin(pb)) => {
            a.component_id() == b.component_id()
                && match (pa.name(&ctx), pb.name(&ctx)) {
                    (Some(na), Some(nb)) => logical_pad_name(na) == logical_pad_name(nb),
                    _ => false,
                }
        }
        _ => false,
    }
}

fn segments(item: &Item) -> Vec<FloatLine> {
    let Item::Trace(trace) = item else {
        return Vec::new();
    };
    let corners = trace.polyline().corner_approx_arr();
    corners
        .windows(2)
        .map(|pair| FloatLine::new(pair[0], pair[1]))
        .collect()
}

fn crossing_point(a: &Item, b: &Item) -> Option<FloatPoint> {
    for sa in segments(a) {
        for sb in segments(b) {
            let Some(point) = sa.intersection(&sb) else {
                continue;
            };
            if sa.segment_distance(&point) < 0.5 && sb.segment_distance(&point) < 0.5 {
                return Some(point);
            }
        }
    }
    None
}

struct Emit<'a> {
    constraints: &'a DrcConstraints,
    seen: BTreeSet<PairKey>,
    out: &'a mut Vec<DrcViolation>,
}

impl Emit<'_> {
    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        kind: DrcViolationKind,
        a: ItemId,
        b: ItemId,
        layer: Option<usize>,
        position: FloatPoint,
        expected: f64,
        actual: f64,
        estimated: bool,
    ) {
        let (first, second) = ordered(a, b);
        if !self.seen.insert((first.0, second.0, layer, kind)) {
            return;
        }
        let severity = severity(self.constraints, kind);
        if severity == DrcSeverity::Ignore {
            return;
        }
        self.out.push(DrcViolation {
            kind,
            severity,
            first_item: first,
            second_item: Some(second),
            layer,
            position,
            expected,
            actual,
            estimated,
        });
    }
}

pub fn run(board: &mut Board, constraints: &DrcConstraints, out: &mut Vec<DrcViolation>) {
    let radius = search_radius(constraints);
    let mut emit = Emit {
        constraints,
        seen: BTreeSet::new(),
        out,
    };
    let ids: Vec<ItemId> = board
        .items_in_board_order()
        .into_iter()
        .filter(|id| board.get_item(*id).is_some_and(is_copper))
        .collect();

    for &id in &ids {
        let shapes = item_shapes(board, id);
        for (layer, shape) in &shapes {
            let others = candidates(board, shape, Some(*layer), radius);
            for other in others {
                if other.0 <= id.0 {
                    continue;
                }
                if !board.get_item(other).is_some_and(is_copper) {
                    continue;
                }
                check_pair(board, constraints, &mut emit, id, other, *layer, shape);
            }
        }
    }
}

fn check_pair(
    board: &mut Board,
    constraints: &DrcConstraints,
    emit: &mut Emit<'_>,
    id: ItemId,
    other: ItemId,
    layer: usize,
    shape: &TileShape,
) {
    let (same_net, same_pad, crossing, clearance) = {
        let a = &board.items[&id];
        let b = &board.items[&other];
        let same_net = same_defined_net(a, b);
        let same_pad = same_logical_pad(board, a, b);
        let crossing = if same_net { None } else { crossing_point(a, b) };
        (same_net, same_pad, crossing, pair_clearance(board, constraints, a, b))
    };

    if let Some(point) = crossing {
        emit.push(DrcViolationKind::TracksCrossing, id, other, Some(layer), point, 0.0, 0.0, false);
        return;
    }

    let other_shapes: Vec<TileShape> = item_shapes(board, other)
        .into_iter()
        .filter(|(l, _)| *l == layer)
        .map(|(_, s)| s)
        .collect();

    if !same_net && let Some(clearance) = clearance && clearance > 0 {
        for other_shape in &other_shapes {
            if let Some((actual, position)) = gap_below(shape, other_shape, clearance) {
                let both_netted = board.items[&id].net_count() > 0 && board.items[&other].net_count() > 0;
                let kind = if actual == 0.0 && both_netted {
                    DrcViolationKind::ShortingItems
                } else {
                    DrcViolationKind::Clearance
                };
                emit.push(kind, id, other, Some(layer), position, f64::from(clearance), actual, false);
                break;
            }
        }
    }

    if same_net || same_pad {
        return;
    }
    let Some(hole_clearance) = constraints.hole_clearance else {
        return;
    };
    for (copper_id, hole_id, copper_shapes) in [
        (id, other, std::slice::from_ref(shape)),
        (other, id, other_shapes.as_slice()),
    ] {
        let Some(hole) = hole_of(board, hole_id) else {
            continue;
        };
        for copper_shape in copper_shapes {
            if let Some((actual, position)) = gap_below(copper_shape, &hole.shape, hole_clearance) {
                emit.push(
                    DrcViolationKind::HoleClearance,
                    copper_id,
                    hole_id,
                    Some(layer),
                    position,
                    f64::from(hole_clearance),
                    actual,
                    hole.estimated,
                );
                break;
            }
        }
    }
}
```

Add `pub mod copper;` to `checks/mod.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p fr-drc --test checks`
Expected: PASS. Two likely failure causes and their fixes: if `crossing_traces_are_tracks_crossing_and_nothing_else` also reports a `Clearance`, the early `return` after the crossing push is missing; if `same_net_items_are_never_clearance_violations` fails, `same_defined_net` must use `shares_net`, not `nets_equal`.

- [ ] **Step 5: Commit**

```bash
git add crates/fr-drc/src/checks crates/fr-drc/tests/checks.rs
git commit -m "feat(drc): KiCad copper clearance, shorting, crossing and hole clearance checks"
```

---

### Task 7: Hole-to-hole check

**Files:**
- Create: `crates/fr-drc/src/checks/holes.rs`
- Modify: `crates/fr-drc/src/checks/mod.rs`
- Test: `crates/fr-drc/tests/checks.rs`

**Interfaces:**
- Produces: `pub fn run(board: &mut Board, constraints: &DrcConstraints, out: &mut Vec<DrcViolation>)` in `fr_drc::checks::holes`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/fr-drc/tests/checks.rs`:

```rust
use fr_drc::checks::holes;

#[test]
fn two_via_holes_closer_than_the_minimum_are_hole_to_hole_regardless_of_net() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.via(0, 0, 1);
    synthetic.via(4500, 0, 1);
    let mut constraints = DrcConstraints::default();
    constraints.hole_to_hole = Some(2500);
    let mut out = Vec::new();
    holes::run(&mut synthetic.board, &constraints, &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::HoleToHole]);
    assert_eq!(out[0].layer, None);
    assert!((out[0].actual - 1500.0).abs() < 60.0, "actual {}", out[0].actual);
}

#[test]
fn hole_to_hole_is_skipped_without_a_rule_and_for_far_holes() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.via(0, 0, 1);
    synthetic.via(4500, 0, 1);
    let mut out = Vec::new();
    holes::run(&mut synthetic.board, &DrcConstraints::default(), &mut out);
    assert!(out.is_empty());
    let mut constraints = DrcConstraints::default();
    constraints.hole_to_hole = Some(1000);
    holes::run(&mut synthetic.board, &constraints, &mut out);
    assert!(out.is_empty());
}
```

The octagonal hole approximation widens each 1500 radius hole by a few percent, hence the 60 unit tolerance.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p fr-drc --test checks holes`
Expected: compile error, `checks::holes` not found.

- [ ] **Step 3: Write the module**

Create `crates/fr-drc/src/checks/holes.rs`:

```rust
use std::collections::BTreeSet;

use fr_board::{Board, DrcConstraints, DrcSeverity, ItemId};

use crate::checks::geometry::{candidates, gap_below, hole_of};
use crate::constraints::severity;
use crate::{DrcViolation, DrcViolationKind};

pub fn run(board: &mut Board, constraints: &DrcConstraints, out: &mut Vec<DrcViolation>) {
    let Some(minimum) = constraints.hole_to_hole else {
        return;
    };
    let severity = severity(constraints, DrcViolationKind::HoleToHole);
    if severity == DrcSeverity::Ignore {
        return;
    }
    let mut seen: BTreeSet<(u32, u32)> = BTreeSet::new();
    for id in board.items_in_board_order() {
        let Some(hole) = hole_of(board, id) else {
            continue;
        };
        for other in candidates(board, &hole.shape, None, minimum) {
            if other.0 <= id.0 {
                continue;
            }
            let Some(other_hole) = hole_of(board, other) else {
                continue;
            };
            if !seen.insert((id.0, other.0)) {
                continue;
            }
            if let Some((actual, position)) = gap_below(&hole.shape, &other_hole.shape, minimum) {
                out.push(DrcViolation {
                    kind: DrcViolationKind::HoleToHole,
                    severity,
                    first_item: id,
                    second_item: Some(other),
                    layer: None,
                    position,
                    expected: f64::from(minimum),
                    actual,
                    estimated: hole.estimated || other_hole.estimated,
                });
            }
        }
    }
}

#[allow(dead_code)]
fn ordered(a: ItemId, b: ItemId) -> (ItemId, ItemId) {
    if a.0 <= b.0 { (a, b) } else { (b, a) }
}
```

Remove the trailing `ordered` helper if clippy flags it as dead; it is not needed because the `other.0 <= id.0` guard already orders pairs. Add `pub mod holes;` to `checks/mod.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p fr-drc --test checks`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/fr-drc/src/checks crates/fr-drc/tests/checks.rs
git commit -m "feat(drc): KiCad hole-to-hole check"
```

---

### Task 8: Single-item checks

**Files:**
- Create: `crates/fr-drc/src/checks/single.rs`
- Modify: `crates/fr-drc/src/checks/mod.rs`
- Test: `crates/fr-drc/tests/checks.rs`

**Interfaces:**
- Produces: `pub fn run(board: &mut Board, constraints: &DrcConstraints, out: &mut Vec<DrcViolation>)` in `fr_drc::checks::single`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/fr-drc/tests/checks.rs`:

```rust
use fr_drc::checks::single;

#[test]
fn a_thin_trace_is_a_track_width_violation() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 500, 1);
    let mut constraints = DrcConstraints::default();
    constraints.netclass_track_width.insert("default".to_string(), 2000);
    let mut out = Vec::new();
    single::run(&mut synthetic.board, &constraints, &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::TrackWidth]);
    assert_eq!(out[0].expected, 2000.0);
    assert_eq!(out[0].actual, 1000.0);
    assert_eq!(out[0].second_item, None);
}

#[test]
fn the_board_minimum_floors_the_netclass_width() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 1000, 1);
    let mut constraints = DrcConstraints::default();
    constraints.netclass_track_width.insert("default".to_string(), 1000);
    constraints.min_track_width = Some(2500);
    let mut out = Vec::new();
    single::run(&mut synthetic.board, &constraints, &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::TrackWidth]);
    assert_eq!(out[0].expected, 2500.0);
}

#[test]
fn via_diameter_annular_width_and_drill_are_checked_against_project_minimums() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.via(0, 0, 1);
    let mut constraints = DrcConstraints::default();
    constraints.min_via_diameter = Some(7000);
    constraints.min_via_annular_width = Some(2000);
    constraints.min_through_hole_diameter = Some(4000);
    let mut out = Vec::new();
    single::run(&mut synthetic.board, &constraints, &mut out);
    assert_eq!(
        kinds(&out),
        vec![
            DrcViolationKind::ViaDiameter,
            DrcViolationKind::AnnularWidth,
            DrcViolationKind::DrillOutOfRange,
        ]
    );
    let diameter = out.iter().find(|v| v.kind == DrcViolationKind::ViaDiameter).unwrap();
    assert_eq!(diameter.actual, 6000.0);
    let annular = out.iter().find(|v| v.kind == DrcViolationKind::AnnularWidth).unwrap();
    assert!((annular.actual - 1500.0).abs() < 1.0);
    let drill = out.iter().find(|v| v.kind == DrcViolationKind::DrillOutOfRange).unwrap();
    assert!((drill.actual - 3000.0).abs() < 1.0);
    assert!(!drill.estimated);
}

#[test]
fn a_compliant_via_reports_nothing() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.via(0, 0, 1);
    let mut constraints = DrcConstraints::default();
    constraints.min_via_diameter = Some(5000);
    constraints.min_via_annular_width = Some(1000);
    constraints.min_through_hole_diameter = Some(3000);
    let mut out = Vec::new();
    single::run(&mut synthetic.board, &constraints, &mut out);
    assert!(out.is_empty(), "{out:?}");
}

#[test]
fn a_through_hole_pad_annular_violation_is_marked_estimated() {
    let mut synthetic = SyntheticBoard::new(
        &[PadSpec { name: "1", half: 800, offset: IntVector::new(0, 0), through_hole: true }],
        1,
        2000,
    );
    synthetic.pin(0, 1);
    let mut constraints = DrcConstraints::default();
    constraints.min_via_annular_width = Some(1000);
    let mut out = Vec::new();
    single::run(&mut synthetic.board, &constraints, &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::AnnularWidth]);
    assert!(out[0].estimated);
}
```

The synthetic via is a 3000 radius circle named with a 600:300 um ratio, so its drill radius is 1500, its annulus 1500, and its drill diameter 3000. The through-hole pad has no colon in its name, so its drill is 0.45 of the smallest radius, 360, and its annulus 440.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p fr-drc --test checks single`
Expected: compile error, `checks::single` not found.

- [ ] **Step 3: Write the module**

Create `crates/fr-drc/src/checks/single.rs`:

```rust
use fr_board::{Board, DrcConstraints, DrcSeverity, Item, ItemId};

use crate::checks::geometry::{hole_of, is_microvia, item_position};
use crate::constraints::{severity, track_width_min};
use crate::{DrcViolation, DrcViolationKind};

fn push(
    board: &Board,
    constraints: &DrcConstraints,
    out: &mut Vec<DrcViolation>,
    kind: DrcViolationKind,
    id: ItemId,
    layer: Option<usize>,
    expected: i32,
    actual: f64,
    estimated: bool,
) {
    if actual >= f64::from(expected) {
        return;
    }
    let severity = severity(constraints, kind);
    if severity == DrcSeverity::Ignore {
        return;
    }
    out.push(DrcViolation {
        kind,
        severity,
        first_item: id,
        second_item: None,
        layer,
        position: item_position(board, id),
        expected: f64::from(expected),
        actual,
        estimated,
    });
}

pub fn run(board: &mut Board, constraints: &DrcConstraints, out: &mut Vec<DrcViolation>) {
    for id in board.items_in_board_order() {
        let Some(item) = board.get_item(id) else {
            continue;
        };
        match item {
            Item::Trace(trace) => {
                if trace.hdr.net_nos.is_empty() {
                    continue;
                }
                let Some(minimum) = track_width_min(board, constraints, trace.hdr.net_nos[0]) else {
                    continue;
                };
                let actual = f64::from(2 * trace.get_half_width());
                push(board, constraints, out, DrcViolationKind::TrackWidth, id, Some(trace.get_layer()), minimum, actual, false);
            }
            Item::Via(via) => {
                let ctx = board.ctx();
                let micro = is_microvia(board, id);
                let diameter = 2.0 * via.smallest_radius(&ctx);
                let diameter_min = if micro { constraints.min_microvia_diameter } else { constraints.min_via_diameter };
                if let Some(minimum) = diameter_min {
                    push(board, constraints, out, DrcViolationKind::ViaDiameter, id, None, minimum, diameter, false);
                }
                if let Some(hole) = hole_of(board, id) {
                    if let Some(minimum) = constraints.min_via_annular_width {
                        let annulus = via.smallest_radius(&ctx) - hole.radius;
                        push(board, constraints, out, DrcViolationKind::AnnularWidth, id, None, minimum, annulus, hole.estimated);
                    }
                    let (kind, drill_min) = if micro {
                        (DrcViolationKind::MicroviaDrillOutOfRange, constraints.min_microvia_drill)
                    } else {
                        (DrcViolationKind::DrillOutOfRange, constraints.min_through_hole_diameter)
                    };
                    if let Some(minimum) = drill_min {
                        push(board, constraints, out, kind, id, None, minimum, 2.0 * hole.radius, hole.estimated);
                    }
                }
            }
            Item::Pin(pin) => {
                let Some(hole) = hole_of(board, id) else {
                    continue;
                };
                let ctx = board.ctx();
                if let Some(minimum) = constraints.min_via_annular_width {
                    let annulus = pin.smallest_radius(&ctx) - hole.radius;
                    push(board, constraints, out, DrcViolationKind::AnnularWidth, id, None, minimum, annulus, hole.estimated);
                }
                if let Some(minimum) = constraints.min_through_hole_diameter {
                    push(board, constraints, out, DrcViolationKind::DrillOutOfRange, id, None, minimum, 2.0 * hole.radius, hole.estimated);
                }
            }
            _ => {}
        }
    }
}
```

If `trace.hdr` is not public, use `item.net_count()` and `item.get_net_number(0)` on the `Item` before matching. Add `pub mod single;` to `checks/mod.rs`. Add `#[allow(clippy::too_many_arguments)]` above `push` if clippy complains.

- [ ] **Step 4: Run tests**

Run: `cargo test -p fr-drc --test checks`
Expected: PASS. If the annular test for the pad fails because `smallest_radius` on a `Pin` needs the tile shape cached, replace `pin.smallest_radius(&ctx)` with half the smaller side of `item.bounding_box(&ctx)`.

- [ ] **Step 5: Commit**

```bash
git add crates/fr-drc/src/checks crates/fr-drc/tests/checks.rs
git commit -m "feat(drc): KiCad track width, via diameter, annular width and drill checks"
```

---

### Task 9: Copper-to-edge clearance

**Files:**
- Create: `crates/fr-drc/src/checks/edge.rs`
- Modify: `crates/fr-drc/src/checks/mod.rs`
- Test: `crates/fr-drc/tests/checks.rs`

**Interfaces:**
- Produces: `pub fn run(board: &mut Board, constraints: &DrcConstraints, out: &mut Vec<DrcViolation>)` in `fr_drc::checks::edge`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/fr-drc/tests/checks.rs`:

```rust
use fr_drc::checks::edge;

#[test]
fn a_trace_near_the_board_edge_is_a_copper_edge_clearance_violation() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.trace(&[(-40_000, 48_000), (40_000, 48_000)], 0, 500, 1);
    let mut constraints = DrcConstraints::default();
    constraints.copper_edge_clearance = Some(5000);
    let mut out = Vec::new();
    edge::run(&mut synthetic.board, &constraints, &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::CopperEdgeClearance]);
    assert_eq!(out[0].expected, 5000.0);
    assert!((out[0].actual - 1500.0).abs() < 2.0, "actual {}", out[0].actual);
    assert_eq!(out[0].second_item, synthetic.board.get_outline());
}

#[test]
fn edge_clearance_is_silent_without_a_rule_or_away_from_the_edge() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.trace(&[(-10_000, 0), (10_000, 0)], 0, 500, 1);
    let mut out = Vec::new();
    edge::run(&mut synthetic.board, &DrcConstraints::default(), &mut out);
    assert!(out.is_empty());
    let mut constraints = DrcConstraints::default();
    constraints.copper_edge_clearance = Some(5000);
    edge::run(&mut synthetic.board, &constraints, &mut out);
    assert!(out.is_empty(), "{out:?}");
}
```

The synthetic outline is the box from -50000 to 50000, so a trace centred at y = 48000 with half width 500 leaves a 1500 gap to the edge.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p fr-drc --test checks edge`
Expected: compile error, `checks::edge` not found.

- [ ] **Step 3: Write the module**

Create `crates/fr-drc/src/checks/edge.rs`:

```rust
use fr_board::{Board, DrcConstraints, DrcSeverity, Item, ItemId};
use fr_geometry::TileShape;

use crate::checks::geometry::{gap_below, is_copper, item_shapes};
use crate::constraints::severity;
use crate::{DrcViolation, DrcViolationKind};

fn keepout_pieces(board: &Board, outline: ItemId) -> Vec<TileShape> {
    let ctx = board.ctx();
    match board.get_item(outline) {
        Some(Item::BoardOutline(item)) => {
            item.get_keepout_area(&ctx);
            item.keepout_convex_pieces(&ctx)
                .map(<[TileShape]>::to_vec)
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

pub fn run(board: &mut Board, constraints: &DrcConstraints, out: &mut Vec<DrcViolation>) {
    let Some(minimum) = constraints.copper_edge_clearance else {
        return;
    };
    let severity = severity(constraints, DrcViolationKind::CopperEdgeClearance);
    if severity == DrcSeverity::Ignore {
        return;
    }
    let Some(outline) = board.get_outline() else {
        return;
    };
    let pieces = keepout_pieces(board, outline);
    if pieces.is_empty() {
        return;
    }
    let ids: Vec<ItemId> = board
        .items_in_board_order()
        .into_iter()
        .filter(|id| board.get_item(*id).is_some_and(is_copper))
        .collect();
    for id in ids {
        for (layer, shape) in item_shapes(board, id) {
            let mut worst: Option<(f64, fr_geometry::FloatPoint)> = None;
            for piece in &pieces {
                if let Some((actual, position)) = gap_below(&shape, piece, minimum)
                    && worst.is_none_or(|(best, _)| actual < best)
                {
                    worst = Some((actual, position));
                }
            }
            if let Some((actual, position)) = worst {
                out.push(DrcViolation {
                    kind: DrcViolationKind::CopperEdgeClearance,
                    severity,
                    first_item: id,
                    second_item: Some(outline),
                    layer: Some(layer),
                    position,
                    expected: f64::from(minimum),
                    actual,
                    estimated: false,
                });
            }
        }
    }
}
```

Add `pub mod edge;` to `checks/mod.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p fr-drc --test checks`
Expected: PASS. If `keepout_convex_pieces` returns `None` even after `get_keepout_area`, read `crates/fr-board/src/structure/board_outline.rs:274-300` and call whichever accessor fills the pieces; the outline's keepout is the area outside the outline curves clipped to the board bounding box, and it is exactly what the copper must stay `minimum` away from.

- [ ] **Step 5: Commit**

```bash
git add crates/fr-drc/src/checks crates/fr-drc/tests/checks.rs
git commit -m "feat(drc): KiCad copper-to-edge clearance check"
```

---

### Task 10: The checker façade and retiring the Java-semantics tests

**Files:**
- Modify: `crates/fr-drc/src/checks/mod.rs`, `crates/fr-drc/src/checker.rs:25-45`, `crates/fr-drc/src/lib.rs`
- Delete: `crates/fr-drc/tests/reference_parity.rs`, `crates/fr-drc/tests/java_ports.rs`, `crates/fr-drc/tests/clearance_list.rs`
- Modify: `crates/fr-drc/tests/incompletes.rs:69`, `crates/fr-drc/tests/corpus.rs:87`, `crates/fr-drc/tests/report.rs`, `crates/fr-drc/tests/report_json.rs`
- Test: `crates/fr-drc/tests/checks.rs`

**Interfaces:**
- Produces: `DesignRulesChecker::get_all_violations(&mut self) -> Vec<DrcViolation>`; `fr_drc::checks::run_all(board: &mut Board, constraints: &DrcConstraints) -> Vec<DrcViolation>`.
- Removes: `DesignRulesChecker::get_all_clearance_violations`, `pub use fr_board::ClearanceViolation` from `fr_drc`.

- [ ] **Step 1: Write the failing test**

Append to `crates/fr-drc/tests/checks.rs`:

```rust
use fr_drc::DesignRulesChecker;

#[test]
fn get_all_violations_runs_every_family_in_a_deterministic_order() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 300, 1);
    synthetic.trace(&[(0, 1500), (10_000, 1500)], 0, 500, 2);
    synthetic.via(30_000, 30_000, 1);
    synthetic.via(30_000, 34_500, 2);
    let mut constraints = DrcConstraints::default();
    constraints.netclass_clearance.insert("default".to_string(), 2000);
    constraints.netclass_track_width.insert("default".to_string(), 1000);
    constraints.hole_to_hole = Some(2500);
    synthetic.board.rules.drc_constraints = Some(constraints);

    let first = DesignRulesChecker::new(&mut synthetic.board).get_all_violations();
    let second = DesignRulesChecker::new(&mut synthetic.board).get_all_violations();
    assert_eq!(first, second);
    assert_eq!(
        kinds(&first),
        vec![
            DrcViolationKind::Clearance,
            DrcViolationKind::HoleToHole,
            DrcViolationKind::TrackWidth,
        ]
    );
    let ids: Vec<u32> = first.iter().map(|v| v.first_item.0).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "ordered by first item");
}

#[test]
fn an_ignored_severity_drops_the_kind() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 500, 1);
    synthetic.trace(&[(0, 1500), (10_000, 1500)], 0, 500, 2);
    let mut constraints = DrcConstraints::default();
    constraints.netclass_clearance.insert("default".to_string(), 2000);
    constraints.severities.insert("clearance".to_string(), DrcSeverity::Ignore);
    synthetic.board.rules.drc_constraints = Some(constraints);
    assert!(DesignRulesChecker::new(&mut synthetic.board).get_all_violations().is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p fr-drc --test checks get_all_violations`
Expected: compile error, no method `get_all_violations`.

- [ ] **Step 3: Write `run_all` and the façade**

Replace `crates/fr-drc/src/checks/mod.rs` with:

```rust
pub mod copper;
pub mod edge;
pub mod geometry;
pub mod holes;
pub mod single;

use fr_board::{Board, DrcConstraints};

use crate::DrcViolation;

#[must_use]
pub fn run_all(board: &mut Board, constraints: &DrcConstraints) -> Vec<DrcViolation> {
    let mut out = Vec::new();
    copper::run(board, constraints, &mut out);
    holes::run(board, constraints, &mut out);
    single::run(board, constraints, &mut out);
    edge::run(board, constraints, &mut out);
    out.sort_by(|a, b| {
        (a.first_item.0, a.kind, a.second_item.map(|id| id.0), a.layer)
            .cmp(&(b.first_item.0, b.kind, b.second_item.map(|id| id.0), b.layer))
    });
    out
}
```

In `crates/fr-drc/src/checker.rs` replace `get_all_clearance_violations` (lines 25-45) with:

```rust
    pub fn get_all_violations(&mut self) -> Vec<DrcViolation> {
        let constraints = crate::constraints::resolve(self.board);
        crate::checks::run_all(self.board, &constraints)
    }
```

Update the imports at the top of `checker.rs`: remove `ClearanceViolation` from the `fr_board` import and add `use crate::DrcViolation;`. Remove the now-unused `BTreeSet` import only if nothing else in the file uses it (`get_all_unconnected_items` does, so keep it).

In `lib.rs` remove `pub use fr_board::ClearanceViolation;` from both the root and the prelude.

- [ ] **Step 4: Retire the Java-semantics tests**

```bash
git rm crates/fr-drc/tests/reference_parity.rs crates/fr-drc/tests/java_ports.rs crates/fr-drc/tests/clearance_list.rs
```

In `crates/fr-drc/tests/incompletes.rs:69` replace `assert!(drc.get_all_clearance_violations().is_empty());` with `assert!(drc.get_all_violations().is_empty());`.

In `crates/fr-drc/tests/corpus.rs:87` replace `.get_all_clearance_violations()` with `.get_all_violations()`.

In `crates/fr-drc/tests/report.rs` delete these test functions and any helper only they use (`golden`, `render`, `render_entry`): `dev_board_report_shape`, `first_violation_is_verbatim`, `three_fixtures_match_the_jvm_byte_for_byte`, `natural_tone_preamp_is_the_jvms_maximal_run_minus_three_dangling_tracks`, `smd_pins_are_classified_as_holes`. Keep the coordinate, unit, formatting, unconnected-entry, and `item_description_maps_every_item_variant` tests.

In `crates/fr-drc/tests/report_json.rs` delete `head_flavor_is_the_jvms_gson_bytes` and `kicad_flavor_matches_the_real_kicad_schema` and the `golden`/`data_dir` helpers if nothing else uses them. Keep the key-order, schema-string, date, and quality-score tests.

- [ ] **Step 5: Run the crate's tests**

Run: `cargo test -p fr-drc`
Expected: everything compiles; `checks`, `constraints`, `incompletes`, `unconnected`, `net_incompletes`, `corpus` pass. `report.rs` and `report_json.rs` will fail to compile until Task 11 changes the report builder; that is expected, and Task 11 finishes them. If they compile now and fail on assertions about `"holeClearance"`, leave them for Task 11.

- [ ] **Step 6: Commit**

```bash
git add -A crates/fr-drc
git commit -m "feat(drc): get_all_violations façade; retire the Java clearance parity tests"
```

---

### Task 11: Report builder, JSON flavour, and statistics on the new type

**Files:**
- Modify: `crates/fr-drc/src/report/build.rs:44-92,90-140,196-214`, `crates/fr-drc/src/report/json.rs:60-72`, `crates/fr-drc/src/statistics.rs`
- Test: `crates/fr-drc/tests/report.rs`, `crates/fr-drc/tests/report_json.rs`, `crates/fr-drc/tests/checks.rs`

**Interfaces:**
- Produces: `BoardStatisticsClearanceViolations::from_violations(violations: &[DrcViolation], board_unit_to_um_factor: f64)`; report `type` strings are KiCad's for every entry, mapped to camelCase only in the Freerouting-head flavour.

- [ ] **Step 1: Write the failing tests**

Append to `crates/fr-drc/tests/checks.rs`:

```rust
use fr_drc::report::{DrcCoordinates, DrcJsonFlavor, DrcReportOptions};
use fr_drc::BoardStatisticsClearanceViolations;

fn report_options() -> DrcReportOptions {
    DrcReportOptions {
        source: "synthetic.dsn".to_string(),
        coordinate_unit: "mm".to_string(),
        date: "2026-09-04T00:00Z".to_string(),
        freerouting_version: "test".to_string(),
        quality_score: None,
    }
}

#[test]
fn the_report_carries_kicad_types_and_severities() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 300, 1);
    synthetic.trace(&[(0, 1500), (10_000, 1500)], 0, 500, 2);
    let mut constraints = DrcConstraints::default();
    constraints.netclass_clearance.insert("default".to_string(), 2000);
    constraints.netclass_track_width.insert("default".to_string(), 1000);
    constraints.severities.insert("track_width".to_string(), DrcSeverity::Warning);
    synthetic.board.rules.drc_constraints = Some(constraints);
    let transform = fr_dsn::CoordinateTransform::new(10.0, 0.0, 0.0).expect("a scale");
    let coords = DrcCoordinates {
        board_unit: fr_board::Unit::Um,
        transform,
    };
    let mut checker = DesignRulesChecker::new(&mut synthetic.board);
    let report = checker.generate_report(&coords, &report_options());
    let types: Vec<(&str, &str)> = report
        .violations
        .iter()
        .map(|v| (v.kind.as_str(), v.severity))
        .collect();
    assert_eq!(types, vec![("clearance", "error"), ("track_width", "warning")]);
    assert!(report.violations[0].description.starts_with("Clearance violation between Trace [N1] and Trace [N2]"));
    assert_eq!(report.violations[0].items.len(), 2);
    assert_eq!(report.violations[1].items.len(), 1);

    let kicad = report.to_json(DrcJsonFlavor::KiCad).expect("serialises");
    assert!(kicad.contains("\"type\": \"track_width\""));
    let head = report.to_json(DrcJsonFlavor::FreeroutingHead).expect("serialises");
    assert!(head.contains("\"type\": \"track_width\""));
}

#[test]
fn hole_clearance_keeps_its_camel_case_name_in_the_head_flavour_only() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.via(0, 0, 1);
    synthetic.trace(&[(-10_000, 4000), (10_000, 4000)], 0, 300, 2);
    let mut constraints = DrcConstraints::default();
    constraints.netclass_clearance.insert("default".to_string(), 100);
    constraints.hole_clearance = Some(2500);
    synthetic.board.rules.drc_constraints = Some(constraints);
    let transform = fr_dsn::CoordinateTransform::new(10.0, 0.0, 0.0).expect("a scale");
    let coords = DrcCoordinates { board_unit: fr_board::Unit::Um, transform };
    let report = DesignRulesChecker::new(&mut synthetic.board).generate_report(&coords, &report_options());
    assert_eq!(report.violations[0].kind, "hole_clearance");
    let head = report.to_json(DrcJsonFlavor::FreeroutingHead).expect("serialises");
    assert!(head.contains("\"type\": \"holeClearance\""));
    let kicad = report.to_json(DrcJsonFlavor::KiCad).expect("serialises");
    assert!(kicad.contains("\"type\": \"hole_clearance\""));
}

#[test]
fn statistics_sum_the_shortfall_in_micrometres() {
    let violations = vec![
        DrcViolation {
            kind: DrcViolationKind::Clearance,
            severity: DrcSeverity::Error,
            first_item: ItemId(1),
            second_item: Some(ItemId(2)),
            layer: Some(0),
            position: fr_geometry::FloatPoint::new(0.0, 0.0),
            expected: 2000.0,
            actual: 500.0,
            estimated: false,
        },
        DrcViolation {
            kind: DrcViolationKind::TrackWidth,
            severity: DrcSeverity::Error,
            first_item: ItemId(3),
            second_item: None,
            layer: Some(0),
            position: fr_geometry::FloatPoint::new(0.0, 0.0),
            expected: 1000.0,
            actual: 600.0,
            estimated: false,
        },
    ];
    let stats = BoardStatisticsClearanceViolations::from_violations(&violations, 0.1);
    assert_eq!(stats.total_count, Some(2));
    assert_eq!(stats.min_violation_um, Some(40.0));
    assert_eq!(stats.max_violation_um, Some(150.0));
    assert_eq!(stats.avg_violation_um, Some(95.0));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p fr-drc --test checks report`
Expected: compile error, `generate_report` still calls `get_all_clearance_violations` / `from_violations` takes `ClearanceViolation`.

- [ ] **Step 3: Rewrite the report conversion**

In `crates/fr-drc/src/report/build.rs`:

Replace the imports `use fr_board::{Board, ClearanceViolation, Item, ItemId, ItemKind};` with `use fr_board::{Board, DrcSeverity, ItemId, ItemKind};` and add `use crate::{DrcViolation, DrcViolationKind};`.

In `generate_report`, replace the block from `let violations = self.get_all_clearance_violations();` through its `for` loop with:

```rust
        let violations = self.get_all_violations();

        for violation in &violations {
            report.add_violation(convert_violation(
                self.board,
                violation,
                coords,
                &options.coordinate_unit,
            ));
        }
```

Replace `convert_clearance_violation` and `is_hole` with:

```rust
fn severity_text(severity: DrcSeverity) -> &'static str {
    match severity {
        DrcSeverity::Warning => "warning",
        DrcSeverity::Error | DrcSeverity::Ignore => "error",
    }
}

fn convert_violation(
    board: &Board,
    violation: &DrcViolation,
    coords: &DrcCoordinates,
    coordinate_unit: &str,
) -> KiCadDrcViolation {
    let first_desc = item_description(board, violation.first_item);
    let mut items = vec![KiCadDrcViolationItem::new(
        &first_desc,
        item_position(board, violation.first_item, coords, coordinate_unit),
        violation.first_item.0.to_string(),
    )];
    let second_desc = violation.second_item.map(|id| {
        items.push(KiCadDrcViolationItem::new(
            item_description(board, id),
            item_position(board, id, coords, coordinate_unit),
            id.0.to_string(),
        ));
        item_description(board, id)
    });

    let expected = format_length(violation.expected, coords, coordinate_unit);
    let actual = format_length(violation.actual, coords, coordinate_unit);
    let values = format!("(expected: {expected} {coordinate_unit}, actual: {actual} {coordinate_unit})");
    let pair = |lead: &str| match &second_desc {
        Some(second) => format!("{lead} between {first_desc} and {second} {values}"),
        None => format!("{lead}: {first_desc} {values}"),
    };
    let mut description = match violation.kind {
        DrcViolationKind::Clearance => pair("Clearance violation"),
        DrcViolationKind::ShortingItems => match &second_desc {
            Some(second) => format!("Items shorting two nets: {first_desc} and {second}"),
            None => format!("Items shorting two nets: {first_desc}"),
        },
        DrcViolationKind::TracksCrossing => match &second_desc {
            Some(second) => format!("Tracks crossing: {first_desc} and {second}"),
            None => format!("Tracks crossing: {first_desc}"),
        },
        DrcViolationKind::HoleClearance => pair("Hole clearance violation"),
        DrcViolationKind::HoleToHole => pair("Drilled holes too close together"),
        DrcViolationKind::CopperEdgeClearance => pair("Copper to edge clearance violation"),
        DrcViolationKind::TrackWidth => pair("Track width violation"),
        DrcViolationKind::ViaDiameter => pair("Via diameter violation"),
        DrcViolationKind::AnnularWidth => pair("Annular width violation"),
        DrcViolationKind::DrillOutOfRange => pair("Drill out of range"),
        DrcViolationKind::MicroviaDrillOutOfRange => pair("Micro via drill out of range"),
    };
    if violation.estimated {
        description.push_str(" (drill size estimated)");
    }

    KiCadDrcViolation::new(
        violation.kind.kicad_type(),
        description,
        severity_text(violation.severity),
        items,
    )
}
```

Change `head_kind_string` so every stored string is KiCad's:

```rust
fn head_kind_string(kind: UnconnectedKind) -> &'static str {
    match kind {
        UnconnectedKind::UnconnectedItems => "unconnected_items",
        UnconnectedKind::TrackDangling => "track_dangling",
        UnconnectedKind::ViaDangling => "via_dangling",
    }
}
```

Rename it to `kicad_kind_string` at its definition and both call sites. Delete `is_hole` and the `Item` import if now unused.

In `crates/fr-drc/src/report/json.rs` replace `violation_type` with:

```rust
    fn violation_type<'a>(&'static self, stored: &'a str) -> &'a str {
        if stored == KICAD.hole_clearance_type {
            self.hole_clearance_type
        } else if stored == KICAD.unconnected_items_type {
            self.unconnected_items_type
        } else {
            stored
        }
    }
```

In `crates/fr-drc/src/statistics.rs` replace `use fr_board::ClearanceViolation;` with `use crate::DrcViolation;`, change the parameter to `violations: &[DrcViolation]`, and replace the shortfall computation with:

```rust
            for violation in violations {
                let shortfall_um = violation.shortfall() * board_unit_to_um_factor;
                minimum = java_min(minimum, shortfall_um);
                maximum = java_max(maximum, shortfall_um);
                sum += shortfall_um;
            }
```

- [ ] **Step 4: Fix the surviving report tests**

Run: `cargo test -p fr-drc`

`report.rs` and `report_json.rs` may still reference `"unconnectedItems"` as the stored kind for unconnected entries; change those expectations to `"unconnected_items"` where they inspect `report.unconnected_items[..].kind`, and leave JSON-text assertions on the head flavour alone (the flavour mapping still emits `unconnectedItems` there). Every remaining failure in these two files must be either a stored-kind rename or a removed Java golden; anything else is a bug in Task 11's code, not the test.

Expected: `cargo test -p fr-drc` PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/fr-drc
git commit -m "feat(drc): report and statistics on DrcViolation with KiCad type strings"
```

---

### Task 12: Router scoring and core consumers

**Files:**
- Modify: `crates/fr-router/src/score/statistics.rs:291-300`, `crates/fr-router/src/score/mod.rs:14`, `crates/fr-core/src/ctx.rs:40`, `crates/fr-core/src/pipeline.rs:19`, `crates/fr-core/tests/pipeline.rs:96-112`

**Interfaces:**
- Consumes: `DesignRulesChecker::get_all_violations`, `DrcViolation::involves_routing`.
- Produces: `RoutingResult.drc_violations: Vec<fr_drc::DrcViolation>`; `BoardStatistics.clearance_violations` counts routing-involved violations only.

- [ ] **Step 1: Update the fr-core pipeline test first**

In `crates/fr-core/tests/pipeline.rs`, replace the `assert_eq!(result.stats.clearance_violations.total_count, Some(result.violation_count() as i32), ...)` assertion (lines 109-113) with:

```rust
    let routing_involved = result
        .drc_violations
        .iter()
        .filter(|violation| violation.involves_routing(&run.board))
        .count();
    assert_eq!(
        result.stats.clearance_violations.total_count,
        Some(routing_involved as i32),
        "BoardStatistics counts the routing-involved subset of the DRC pass"
    );
```

Read the test's `route` helper to find the name under which the routed `Board` is returned (it is next to `result` in the same struct); if the board is not returned, add it to the helper's return struct. Add `use fr_drc::DrcViolation;` only if the compiler asks for it.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fr-core --test pipeline`
Expected: compile error, `involves_routing` not found on `ClearanceViolation`.

- [ ] **Step 3: Switch the consumers**

`crates/fr-router/src/score/statistics.rs:291-300`:

```rust
        stats.clearance_violations = if include_clearance_violations {
            let violations = {
                let mut drc = DesignRulesChecker::new(board);
                drc.get_all_violations()
            };
            let routing_involved: Vec<DrcViolation> = violations
                .into_iter()
                .filter(|violation| violation.involves_routing(board))
                .collect();
            BoardStatisticsClearanceViolations::from_violations(
                &routing_involved,
                board_unit_to_um_factor,
            )
        } else {
```

Add `DrcViolation` to the `use fr_drc::{...}` line at the top of that file.

`crates/fr-router/src/score/mod.rs:14`: `pub use fr_drc::{BoardStatisticsClearanceViolations, DrcViolation};`

`crates/fr-core/src/ctx.rs:40`: `pub drc_violations: Vec<fr_drc::DrcViolation>,`

`crates/fr-core/src/pipeline.rs:19`: `let drc_violations = fr_drc::DesignRulesChecker::new(board).get_all_violations();`

- [ ] **Step 4: Build and test the workspace**

Run: `cargo build --workspace && cargo test -p fr-router -p fr-core`
Expected: compiles. Any fr-router or fr-core test that asserts a specific clearance-violation count against a Java fixture now reflects KiCad semantics; for each such failure, read the test, confirm the new count is explained by a rule in the spec's Checks table (for example, stacked same-net pads no longer count), and update the expected number in that test with the reason in the commit message. Do not bulk-replace numbers.

- [ ] **Step 5: Commit**

```bash
git add crates/fr-router crates/fr-core
git commit -m "feat(router,core): score the routing-involved subset of KiCad DRC violations"
```

---

### Task 13: `--kicad-project` on the CLI and the MCP tool

**Files:**
- Modify: `crates/freerouting/src/cli.rs:30-76`, `crates/freerouting/src/commands/drc.rs:41-45,100-110`, `crates/freerouting/src/commands/route.rs:381`, `crates/freerouting/src/mcp/tools/check_drc.rs:16-27`, `crates/freerouting/src/mcp/tools/schema.rs:53-70`

**Interfaces:**
- Produces: `commands::drc::load_kicad_project_file(project: Option<&Path>, board: &mut Board, transform: &CoordinateTransform)`; CLI flag `--kicad-project <PATH>` on `route` and `drc`; MCP argument `kicad_project_path`.

- [ ] **Step 1: Write the failing test**

Append to the `tests` module at the bottom of `crates/freerouting/src/commands/drc.rs`:

```rust
    #[test]
    fn a_missing_project_file_leaves_the_board_without_constraints() {
        let path = parity_free_spike_dsn();
        let bytes = std::fs::read(&path).expect("the spike DSN is in the repo");
        let (mut board, transform) = match fr_dsn::read_board(
            &bytes[..],
            None,
            Some("spike"),
            &fr_dsn::DsnReadOptions::default(),
        ) {
            fr_dsn::BoardReadResult::Success { board, coordinate_transform, .. }
            | fr_dsn::BoardReadResult::OutlineMissing { board, coordinate_transform, .. } => {
                (*board.expect("a board"), coordinate_transform.expect("a transform"))
            }
            other => panic!("{other:?}"),
        };
        load_kicad_project_file(Some(Path::new("/nonexistent/x.kicad_pro")), &mut board, &transform);
        assert!(board.rules.drc_constraints.is_none());
        let project = path.with_file_name("stripped.kicad_pro");
        load_kicad_project_file(Some(&project), &mut board, &transform);
        assert_eq!(
            board.rules.drc_constraints.as_ref().and_then(|c| c.hole_to_hole),
            Some(2500)
        );
    }

    fn parity_free_spike_dsn() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/tests/data/spike/spike.dsn")
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p freerouting load_kicad_project`
Expected: compile error, `load_kicad_project_file` not found.

- [ ] **Step 3: Add the helper and the flag**

In `crates/freerouting/src/commands/drc.rs`, after `load_session_file`, add:

```rust
pub fn load_kicad_project_file(
    project: Option<&Path>,
    board: &mut fr_board::Board,
    transform: &fr_dsn::CoordinateTransform,
) {
    let Some(project) = project else {
        return;
    };
    if !project.exists() {
        tracing::warn!("KiCad project file not found: {}", project.display());
        return;
    }
    tracing::info!("Loading KiCad project design rules: {}", project.display());
    let text = match std::fs::read_to_string(project) {
        Ok(text) => text,
        Err(error) => {
            tracing::error!("Failed to read KiCad project file: {error}");
            return;
        }
    };
    match fr_drc::apply_kicad_project(&text, board, transform) {
        Ok(()) => tracing::info!("KiCad project design rules loaded"),
        Err(error) => tracing::error!("Failed to apply KiCad project design rules: {error}"),
    }
}
```

In `run` (same file), after `load_session_file(args.ses.as_deref(), &mut board, &transform);` add:

```rust
    load_kicad_project_file(args.kicad_project.as_deref(), &mut board, &transform);
```

In `crates/freerouting/src/cli.rs` add to both `RouteArgs` and `DrcArgs`, after the `kicad_json` field:

```rust
    #[arg(long)]
    pub kicad_project: Option<PathBuf>,
```

In `crates/freerouting/src/commands/route.rs`, directly after the line `job.router_settings = settings.clone();` (line 381), add:

```rust
    super::drc::load_kicad_project_file(args.kicad_project.as_deref(), &mut board, &transform);
```

In `crates/freerouting/src/mcp/tools/check_drc.rs` add after the `rules` line:

```rust
    let project = super::optional_string(&args, "kicad_project_path")?.map(PathBuf::from);
```

and after `load_session_file(...)`:

```rust
    load_kicad_project_file(project.as_deref(), &mut board, &transform);
```

extending the `use crate::commands::drc::{...}` import with `load_kicad_project_file`.

In `crates/freerouting/src/mcp/tools/schema.rs::check_drc_schema`, after the `rules_path` insert:

```rust
    object.insert("kicad_project_path".into(), json!({
        "type": "string",
        "description": "A KiCad .kicad_pro project whose board design rules (hole-to-hole, edge clearance, via minimums, netclass clearances, rule severities) are applied before checking. The CLI spelling is --kicad-project."
    }));
```

- [ ] **Step 4: Test and smoke-run**

Run: `cargo test -p freerouting && cargo run -q -p freerouting -- drc benchmark/tests/data/spike/spike.dsn --ses benchmark/tests/data/spike/routed.ses --kicad-project benchmark/tests/data/spike/stripped.kicad_pro | python3 -c "import json,sys,collections; d=json.load(sys.stdin); print(collections.Counter(v['type'] for v in d['violations']), len(d['unconnected_items']))"`
Expected: tests PASS; the command prints a counter of KiCad type strings and `1` unconnected item. The two former `hole_clearance` false positives on the stacked ESP32 pads must be gone.

- [ ] **Step 5: Commit**

```bash
git add crates/freerouting
git commit -m "feat(cli): --kicad-project supplies KiCad design rules to route and drc"
```

---

### Task 14: The `kicad-cli` oracle

**Files:**
- Create: `tests/reference/kicad-drc-fixtures.txt`, `scripts/gen-kicad-drc-reference.sh`, `crates/fr-drc/tests/kicad_oracle.rs`
- Generated: `tests/reference/kicad-*/kicad-drc.json`, `tests/reference/kicad-*/kicad-drc.meta.txt`

**Interfaces:**
- Fixture row format: `stem|dsn|ses|kicad_pcb|kicad_pro|ignore_types`. Paths are relative to the workspace root, or to the Java checkout when prefixed `java:`. `ignore_types` is a comma-separated list of KiCad type strings whose counts are not compared for that stem, or empty.

- [ ] **Step 1: Write the fixture list**

Create `tests/reference/kicad-drc-fixtures.txt`:

```
# kicad-cli DRC oracle fixtures for scripts/gen-kicad-drc-reference.sh and
# crates/fr-drc/tests/kicad_oracle.rs.
#
#   stem|dsn|ses|kicad_pcb|kicad_pro|ignore_types
#
# Paths are relative to the workspace root, or to $FREEROUTING_JAVA_DIR when prefixed `java:`.
# ignore_types lists KiCad violation types whose counts are not compared for that stem.
kicad-spike|benchmark/tests/data/spike/spike.dsn|benchmark/tests/data/spike/routed.ses|benchmark/tests/data/spike/stripped.kicad_pcb|benchmark/tests/data/spike/stripped.kicad_pro|
kicad-issue191-z80|java:fixtures/Issue191-processor.Z80/processor.Z80.dsn|java:fixtures/Issue191-processor.Z80/processor.ses|java:fixtures/Issue191-processor.Z80/processor.Z80.kicad_pcb|java:fixtures/Issue191-processor.Z80/processor.Z80.kicad_pro|
kicad-issue269-power-planes|java:fixtures/Issue269-NoViasOnPowerPlanes/Issue269-NoViasOnPowerPlanes.dsn|java:fixtures/Issue269-NoViasOnPowerPlanes/Issue269-NoViasOnPowerPlanes.ses|java:fixtures/Issue269-NoViasOnPowerPlanes/Issue269-NoViasOnPowerPlanes.kicad_pcb|java:fixtures/Issue269-NoViasOnPowerPlanes/Issue269-NoViasOnPowerPlanes.kicad_pro|
kicad-issue283-preamp|java:fixtures/Issue283-UnconnectedTracesUnderPads/Natural_Tone_Preamp.dsn|java:fixtures/Issue283-UnconnectedTracesUnderPads/Natural_Tone_Preamp.ses|java:fixtures/Issue283-UnconnectedTracesUnderPads/Natural_Tone_Preamp.kicad_pcb|java:fixtures/Issue283-UnconnectedTracesUnderPads/Natural_Tone_Preamp.kicad_pro|
kicad-issue368-corney|java:fixtures/Issue368-CorneyIslandWireless/corney_island_wireless.dsn|java:fixtures/Issue368-CorneyIslandWireless/corney_island_wireless (no-GUI).ses|java:fixtures/Issue368-CorneyIslandWireless/corney_island_wireless.kicad_pcb|java:fixtures/Issue368-CorneyIslandWireless/corney_island_wireless.kicad_pro|
kicad-issue742-tastexx|java:fixtures/Issue742-tastexx-pcb/tastexx-pcb.dsn|java:fixtures/Issue742-tastexx-pcb/tastexx-pcb.ses|java:fixtures/Issue742-tastexx-pcb/tastexx-pcb.kicad_pcb|java:fixtures/Issue742-tastexx-pcb/tastexx-pcb.kicad_pro|
```

- [ ] **Step 2: Write the generator script**

Create `scripts/gen-kicad-drc-reference.sh` and `chmod +x` it:

```bash
#!/usr/bin/env bash
# Generate kicad-cli DRC references for crates/fr-drc/tests/kicad_oracle.rs.
#
# Per stem in tests/reference/kicad-drc-fixtures.txt: strip the routing from the KiCad board,
# import the session with the benchmark's vendored importer, refill zones, copy the project next
# to the board, run `kicad-cli pcb drc --all-track-errors --format json`, and store the report as
# tests/reference/<stem>/kicad-drc.json with a kicad-drc.meta.txt beside it.
#
# Usage: scripts/gen-kicad-drc-reference.sh [stem ...]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JAVA_DIR="${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}"
KICAD_CLI="${FREEROUTING_KICAD_CLI:-/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli}"
KICAD_PY="${FREEROUTING_KICAD_PYTHON:-/Applications/KiCad/KiCad.app/Contents/Frameworks/Python.framework/Versions/Current/bin/python3}"
VENDOR="$ROOT/benchmark/vendor/kicad"
FIXTURES="$ROOT/tests/reference/kicad-drc-fixtures.txt"
WANTED=("$@")

for tool in "$KICAD_CLI" "$KICAD_PY"; do
  [[ -x "$tool" ]] || { echo "error: $tool is not executable; set FREEROUTING_KICAD_CLI / FREEROUTING_KICAD_PYTHON" >&2; exit 1; }
done

resolve() {
  local p="$1"
  if [[ "$p" == java:* ]]; then printf '%s/%s' "$JAVA_DIR" "${p#java:}"; else printf '%s/%s' "$ROOT" "$p"; fi
}

wanted() {
  [[ ${#WANTED[@]} -eq 0 ]] && return 0
  local w; for w in "${WANTED[@]}"; do [[ "$w" == "$1" ]] && return 0; done
  return 1
}

while IFS='|' read -r stem dsn ses pcb pro ignore || [[ -n "$stem" ]]; do
  [[ -z "$stem" || "$stem" == \#* ]] && continue
  wanted "$stem" || continue
  pcb_path="$(resolve "$pcb")"; ses_path="$(resolve "$ses")"; pro_path="$(resolve "$pro")"
  for f in "$pcb_path" "$ses_path" "$pro_path"; do
    [[ -f "$f" ]] || { echo "skip $stem: missing $f" >&2; continue 2; }
  done
  out_dir="$ROOT/tests/reference/$stem"
  work="$(mktemp -d)"
  mkdir -p "$out_dir"
  echo "== $stem"
  "$KICAD_PY" "$VENDOR/strip_kicad_routing.py" "$pcb_path" "$work/stripped.kicad_pcb"
  "$KICAD_PY" "$VENDOR/ses_to_board.py" "$work/stripped.kicad_pcb" "$ses_path" "$work/routed.kicad_pcb" > "$work/import.json"
  "$KICAD_PY" "$VENDOR/refill_zones.py" "$work/routed.kicad_pcb" "$work/routed.kicad_pcb" > "$work/refill.json"
  cp "$pro_path" "$work/routed.kicad_pro"
  "$KICAD_CLI" pcb drc --format json --all-track-errors --units mm -o "$work/kicad-drc.json" "$work/routed.kicad_pcb"
  cp "$work/kicad-drc.json" "$out_dir/kicad-drc.json"
  {
    echo "kicad-cli    $("$KICAD_CLI" version)"
    echo "generated    $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "dsn          $dsn"
    echo "ses          $ses"
    echo "kicad_pcb    $pcb"
    echo "kicad_pro    $pro"
    echo "import       $(cat "$work/import.json")"
    echo "refill       $(cat "$work/refill.json")"
    echo "command      kicad-cli pcb drc --format json --all-track-errors --units mm"
  } > "$out_dir/kicad-drc.meta.txt"
  rm -rf "$work"
done < "$FIXTURES"
```

- [ ] **Step 3: Generate the references**

Run: `scripts/gen-kicad-drc-reference.sh`
Expected: six `tests/reference/kicad-*/kicad-drc.json` files. If a Java fixture's import prints skipped items in `import.json`, that is recorded in the meta file; continue.

- [ ] **Step 4: Write the oracle test**

Create `crates/fr-drc/tests/kicad_oracle.rs`:

```rust
use std::collections::BTreeMap;
use std::path::PathBuf;

use fr_board::prelude::*;
use fr_drc::{DesignRulesChecker, DrcViolationKind, UnconnectedKind};
use fr_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};

struct Row {
    stem: String,
    dsn: PathBuf,
    ses: PathBuf,
    pro: PathBuf,
    ignore: Vec<String>,
    needs_java: bool,
}

fn resolve(field: &str) -> (PathBuf, bool) {
    match field.strip_prefix("java:") {
        Some(rest) => (parity::java_dir().join(rest), true),
        None => (parity::workspace_root().join(field), false),
    }
}

fn rows() -> Vec<Row> {
    let path = parity::workspace_root().join("tests/reference/kicad-drc-fixtures.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let fields: Vec<&str> = line.split('|').map(str::trim).collect();
            let (dsn, java_a) = resolve(fields[1]);
            let (ses, java_b) = resolve(fields[2]);
            let (pro, java_c) = resolve(fields[4]);
            Row {
                stem: fields[0].to_string(),
                dsn,
                ses,
                pro,
                ignore: fields
                    .get(5)
                    .map(|s| s.split(',').filter(|t| !t.is_empty()).map(str::to_string).collect())
                    .unwrap_or_default(),
                needs_java: java_a || java_b || java_c,
            }
        })
        .collect()
}

fn load(row: &Row) -> (Board, CoordinateTransform) {
    let bytes = std::fs::read(&row.dsn)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", row.dsn.display()));
    let (mut board, transform) = match fr_dsn::read_board(&bytes[..], None, None, &DsnReadOptions::default()) {
        BoardReadResult::Success { board, coordinate_transform, .. }
        | BoardReadResult::OutlineMissing { board, coordinate_transform, .. } => {
            (*board.expect("a board"), coordinate_transform.expect("a transform"))
        }
        other => panic!("{} did not read: {other:?}", row.stem),
    };
    let ses = std::fs::File::open(&row.ses)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", row.ses.display()));
    fr_dsn::ses_reader::read(ses, &mut board, &transform).expect("the session imports");
    let project = std::fs::read_to_string(&row.pro).expect("the project reads");
    fr_drc::apply_kicad_project(&project, &mut board, &transform).expect("the project applies");
    (board, transform)
}

fn port_counts(board: &mut Board) -> (BTreeMap<String, usize>, usize) {
    let mut checker = DesignRulesChecker::new(board);
    let mut counts = BTreeMap::new();
    for violation in checker.get_all_violations() {
        if violation.severity == DrcSeverity::Error {
            *counts.entry(violation.kind.kicad_type().to_string()).or_insert(0) += 1;
        }
    }
    let unconnected = checker
        .get_all_unconnected_items()
        .iter()
        .filter(|entry| entry.kind == UnconnectedKind::UnconnectedItems)
        .count();
    (counts, unconnected)
}

fn oracle_counts(stem: &str) -> Option<(BTreeMap<String, usize>, usize)> {
    let path = parity::reference(stem, "kicad-drc.json");
    if !parity::require_reference(&path) {
        return None;
    }
    let text = std::fs::read_to_string(&path).expect("the reference reads");
    let doc: serde_json::Value = serde_json::from_str(&text).expect("the reference is JSON");
    let mut counts = BTreeMap::new();
    for violation in doc["violations"].as_array().into_iter().flatten() {
        if violation["severity"].as_str() != Some("error") {
            continue;
        }
        let kind = violation["type"].as_str().unwrap_or_default();
        if DrcViolationKind::from_kicad_type(kind).is_some() {
            *counts.entry(kind.to_string()).or_insert(0) += 1;
        }
    }
    let unconnected = doc["unconnected_items"].as_array().map_or(0, Vec::len);
    Some((counts, unconnected))
}

#[test]
fn the_port_matches_kicad_cli_per_violation_type() {
    let mut failures = Vec::new();
    for row in rows() {
        if row.needs_java && !parity::require_java_dir() {
            continue;
        }
        let Some((expected, expected_unconnected)) = oracle_counts(&row.stem) else {
            continue;
        };
        let (mut board, _) = load(&row);
        let (actual, actual_unconnected) = port_counts(&mut board);
        for kind in DrcViolationKind::ALL {
            let name = kind.kicad_type();
            if row.ignore.iter().any(|t| t == name) {
                continue;
            }
            let want = expected.get(name).copied().unwrap_or(0);
            let got = actual.get(name).copied().unwrap_or(0);
            if want != got {
                failures.push(format!("{}: {name}: kicad-cli {want}, port {got}", row.stem));
            }
        }
        if expected_unconnected != actual_unconnected {
            failures.push(format!(
                "{}: unconnected_items: kicad-cli {expected_unconnected}, port {actual_unconnected}",
                row.stem
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
```

Add `serde_json.workspace = true` to `[dev-dependencies]` in `crates/fr-drc/Cargo.toml` if it is not already reachable there.

- [ ] **Step 5: Run the oracle and converge**

Run: `cargo test -p fr-drc --test kicad_oracle -- --nocapture`

Expected on the first run: a list of per-stem, per-type count differences. Work through them in this order, committing after each fix:

1. A type the port reports and KiCad does not, on a pair the spec says KiCad waives, is a port bug. Fix the check.
2. A type KiCad reports and the port does not, where the items involved have estimated pad drills or a net-tie footprint, is a known divergence. Add the type to that stem's `ignore_types` column with a one-line reason in the fixture file's comment block.
3. Anything else: inspect the specific violation in `kicad-drc.json` (positions are in mm) against the port's report for the same board via `cargo run -p freerouting -- drc <dsn> --ses <ses> --kicad-project <pro>`, and fix the check.

The task is done when the test passes with every remaining `ignore_types` entry justified in the fixture file.

- [ ] **Step 6: Commit**

```bash
git add tests/reference/kicad-drc-fixtures.txt tests/reference/kicad-* scripts/gen-kicad-drc-reference.sh crates/fr-drc/tests/kicad_oracle.rs crates/fr-drc/Cargo.toml
git commit -m "test(drc): kicad-cli oracle fixtures and per-type agreement test"
```

---

### Task 15: Spec amendment, workspace verification, and handoff notes

**Files:**
- Modify: `docs/superpowers/specs/2026-09-04-kicad-drc-port-design.md` (Architecture item 1, Testing item 4, Components item 4), `scripts/gen-drc-reference.sh` header comment

- [ ] **Step 1: Amend the spec**

In the spec's Components list, change item 1 to say `DrcConstraints` and `DrcSeverity` are defined in `fr-board` with the builders in `fr-drc`, and item 4 to say the project file is loaded by the CLI and MCP layers through `load_kicad_project_file`, with `RoutingJob` unchanged. In Testing item 4, replace the sentence about removing `tests/reference/drc-fixtures.txt` and the `drc-*` directories with: "The fixture list and its `drc-*` reference directories stay because `scripts/quality-ab.sh` defines the 29-stem quality gate over them; only the parity tests are removed."

Add a sentence to the header comment of `scripts/gen-drc-reference.sh` stating that `crates/fr-drc/tests/reference_parity.rs` no longer exists and the script now serves `scripts/quality-ab.sh` alone.

- [ ] **Step 2: Full verification**

Run:

```bash
cargo build --workspace
cargo test --workspace 2>&1 | grep -E "^test result|FAILED|panicked" | sort | uniq -c
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: zero failures, clippy clean. Any remaining failure is investigated individually as in Task 12 step 4.

- [ ] **Step 3: Smoke the CLI on the spike both ways**

```bash
cargo run -q -p freerouting -- drc benchmark/tests/data/spike/spike.dsn --ses benchmark/tests/data/spike/routed.ses -o /tmp/spike-dsn-only.json
cargo run -q -p freerouting -- drc benchmark/tests/data/spike/spike.dsn --ses benchmark/tests/data/spike/routed.ses --kicad-project benchmark/tests/data/spike/stripped.kicad_pro -o /tmp/spike-project.json
python3 -c "import json,collections; [print(f, collections.Counter(v['type'] for v in json.load(open(f))['violations'])) for f in ('/tmp/spike-dsn-only.json','/tmp/spike-project.json')]"
```

Expected: both runs succeed; the project run may show more types (edge, hole rules) than the DSN-only run, and neither shows `hole_clearance` on the stacked ESP32 pads.

- [ ] **Step 4: Commit**

```bash
git add docs scripts/gen-drc-reference.sh
git commit -m "docs(drc): record the spec amendments from the KiCad DRC port"
```

---

## Self-review

**Spec coverage.** Components 1-7 map to Tasks 1-5, 10-13. Rule resolution items 1-7 map to Task 3 (pair clearance, netclass lookup, canonical names), Task 4 (severity, units), Task 6 (same-net waiver, same logical pad), and the spec's net-tie item is recorded as out of scope with nothing to build. The Checks table maps to Tasks 6-9 and the existing connectivity code. Scoring and consumers map to Task 12. Error handling maps to Task 4 (parse errors) and Task 13 (missing file, unreadable file). Testing items 1-3 map to Tasks 5-9, 4, and 14; item 4 is amended and executed in Task 10; item 5 is Task 12 step 4 and Task 15 step 2; item 6 needs no code.

**Placeholders.** None. Every step carries its code or its exact command.

**Type consistency.** `DrcConstraints` fields are used with the same names in Tasks 1, 3, 4, 6-9. `gap_below` returns `Option<(f64, FloatPoint)>` everywhere. `run(board: &mut Board, constraints: &DrcConstraints, out: &mut Vec<DrcViolation>)` is the signature of all four check modules and `run_all` calls them that way. `severity(constraints, kind)` and `search_radius(constraints)` are free functions in `constraints.rs` as imported by the check modules. `kicad_type()` is used by the report, the oracle, and the severity lookup.
