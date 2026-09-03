# Plan 9 — pre-built fixture ground truth

Read-only pre-build for the five highest-cost Plan 9 tasks. Nothing here was written into
`/Users/em/Development/freerouting/freerouting-rs`; every measurement comes from a read-only
clone of that repo at tag **`v1.0.0` (`ecc0abf`)**, built in this scratchpad at
`../port-v100`.

| dir | task | rows with ground truth |
|---|---|---|
| `task-8/`  | rooms, doors and expandable identity | **9 / 9** |
| `task-11/` | board geometry corrections | **10 / 10** (the #26 tail: 8 / 8 sites) |
| `task-12/` | polygon and circle implementations | **2 / 2** (8 / 8 named tests) |
| `task-13/` | airlines, incompletes and board history | **5 / 5** |
| `task-19/` | DRC report accuracy | **11 / 11** |

Each directory holds:
* `expected-outcomes.md` — **the deliverable**: the derivations, with the arithmetic shown.
* `current-port-behavior.txt` — what v1.0.0 does today, with the probe source named.
* `fixtures/` — hand-written DSN fixtures (validated against the v1.0.0 binary) and, for
  Task 19, KiCad 10's own DRC verdicts as independent ground truth.
* `READY.md` — a one-page summary of what the task's implementer gets and what they still owe.

## Probe sources (in `../port-v100`, the read-only v1.0.0 clone)
* `crates/fr-geometry/examples/p9probe.rs`   — Task 11 / 12 geometry
* `crates/fr-geometry/examples/p9poly.rs`    — Task 12 polygon + circle
* `crates/fr-board/examples/p9delaunay.rs`   — Task 13 Delaunay edge counts
* `crates/fr-board/examples/p9delaunay2.rs`  — Task 13 coordinate sensitivity + witness draws
* `crates/fr-router/examples/p9stats.rs`     — Task 13 #194 / Task 19 #195 + #196

## `kicad-cli`
Installed but **not on `PATH`**: `/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli`,
version **10.0.3**. It was not needed — KiCad's own DRC verdicts for the two Issue575 boards
are already committed in the Java clone and are copied into `task-19/fixtures/`.

## Five corrections to the plan / survey that the derivations turned up
1. **Task 8 #159** — `[4, 4, 4]` does not follow from the fix as sketched. `complete_shape_90`
   never calls `divide_large_room`, so restoring only the fallthrough gives `[4, 4, 1]`.
2. **Task 19 #152** — `an_smd_pad_is_not_a_hole` cannot assert on BBD Mars-64: all 64 rows are
   `(SMD Pin, Via)` and the classification is `is_hole(a) || is_hole(b)`. The stems that move
   are `drc-dev-board` (2/0 → 0/2) and `drc-issue753-cpu85` (78/0 → 76/2).
3. **Task 13 #82** — the survey's `5×5 → 50/56` and `6×6 → 75/85` do not reproduce (v1.0.0
   gives 53 and 82) and the deficit is coordinate-dependent. Only the post-fix numbers
   (5, 56, 85, 120 = `(3k−1)(k−1)`) are stable.
4. **Task 13 #82** — `…leaves_no_witness_pad_edgeless` already passes today (0/2000). The
   discriminating assertion is the edge count: **17 / 2000 → 0 / 2000**.
5. **Task 11 #15** — the survey's mechanism sentence is backwards. `0.0 < Double.MIN_VALUE` is
   true, so distance-0 is the only case that fires; the defect is that every non-zero distance
   is skipped and the answer is always corner 0.
