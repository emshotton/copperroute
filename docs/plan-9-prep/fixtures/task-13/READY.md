# Task 13 — what the implementer gets

**Coverage: 5 of 5 fix rows have ground truth ready.**

| row | ground truth | measured at v1.0.0 |
|---|---|---|
| #82 + #9 | `E = 3n−3−h = (3k−1)(k−1)` derived from Euler; **5 / 56 / 85 / 120** confirmed coordinate-independent across six grid placements; `circle_center` order-dependence measured (3 of 6 orders find `(500,500)`) | yes |
| #147 | stacked via-on-pad witness: **2** airlines must survive where 1 does today; NaN-vs-finite invariant | — |
| #197 + #198 | three constructible invariants (empty history restores; `−3 > −5` restores; ranks `1,0,2` stable across a restore) | — |
| #194 | **`fixtures/p9t13-multi-net-smd-pin.dsn` — built, routed, validated**; `pins_to_escape` **2 → 3** | **yes (2)** |
| #148 | 29-airlines-collapse-to-1 argument; drop `Comparable` | — |

## The three things to read first

1. **The survey's per-grid deficits do not reproduce, and the plan's post-fix literals do.**
   `5×5 → 50/56` and `6×6 → 75/85` are not what v1.0.0 produces (53/56 and 82/85), and the
   deficit is coordinate-dependent — six placements were measured and no two agree. The
   *post*-fix numbers **5, 56, 85, 120** are coordinate-independent because they follow from
   Euler's formula, and those are the only literals that belong in the test. Measure any
   fail-before number on the exact grid the test uses, in this task.

2. **The plan's `…leaves_no_witness_pad_edgeless` test is vacuous.** Over 2 000 dense random
   draws of 49 points straddling the origin, v1.0.0 already produces **0 edgeless pads** — but
   **17 / 2 000 (0.85 %)** draws are short of `3n−3−h`. The survey's "~0.5 % come apart" is the
   edge-count rate, not the edgeless-pad rate. Replace the assertion with the edge-count
   invariant (17 → 0) or the test cannot fail before the fix. A deterministic companion is
   available: the regular 7×7 grid straddling the origin gives **119/120**.

3. **`#194`'s fixture is done and its number is confirmed**: `pins_to_escape = 2` today, must
   be **3**. But note the second obligation — `escaped_count` uses `is_pin_escaped`, which is
   **net-blind**, so it stays at 2 unless it is *also* made net-aware. Decide and record which
   reading the fix takes; the plan's test name reads as "fix `pins_to_escape` only".

## Bonus finding, already measured for Task 19
* **#196** needs no board-specific literal: `bounding_box.width == board.size.width` and
  `…height == …height` is the whole invariant. Measured today: `bounding_box.width = −1`
  on the new fixture and `−25.4` on `Issue143-rpi_splitter`, against `size.width` of
  `2002` and `54051.2`.
* **#195** has a **minimal** witness far smaller than the survey's `121 606.75 / 130 610.65`
  pair: a board with **one straight horizontal trace** already gives `h+v+a = 1200.002`
  against `total_length = 1200` — the 0.002 is precisely the polyline's two bounding lines,
  which carry no segment.
* **#81** measured: `validate()` answers `true` on a triangulation that is provably missing an
  edge (119 of 120).

## What was NOT built here
* No `AIRLINE_BUDGETS` regeneration (needs the tree and the harness).
* The `121 606.75 / 130 610.65` pair was not re-measured — it needs a *routed* rpi_splitter at
  k=8. The minimal witness above is stronger evidence for a directed test anyway.
* `#197`/`#198`/`#148` fixtures are invariant-shaped and need no data file.

## Provenance
Tag **`v1.0.0` (`ecc0abf`)**, cloned read-only to the scratchpad and built there. Probe sources
live at `$SCRATCH/port-v100/crates/fr-board/examples/p9delaunay{,2}.rs` and
`$SCRATCH/port-v100/crates/fr-router/examples/p9stats.rs`. The `plan-9-post-parity` working
tree was never written to.
