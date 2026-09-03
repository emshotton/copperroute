# Two routing regressions in freerouting between v2.1.0 and HEAD (2.3.1-SNAPSHOT)

Measured with the benchmark suite in `benchmark/` on 605 PCBench boards (real, human-routed
KiCad designs whose references are routing-DRC-clean and fully connected under their own design
rules), judged by KiCad 10 DRC with each board's own rules, `-mp 10`, 5-minute cap, one isolated
config dir per run, single-threaded. The router is deterministic, so every delta below is exact,
not statistical. Full data: `reports/pcbench-freerouting-2026-08-28.md`; per-board artefacts for
both regressions were verified by ablation builds at HEAD.

v2.1.0 is the high-water mark: on the 138-board small tier it fully connects 0.81 of boards with
0.92 DRC-clean (clean pass 0.76). HEAD measures 0.76 / 0.73 (clean pass 0.60). The loss decomposes
into exactly two independent changes.

## Regression 1 — connectivity: airline-distance net ordering removed (first bad: `933d2980`)

**What changed.** Commit `933d2980` ("Remove useSlowAlgorithm parameter from autorouter",
2026-01-14, shipped in v2.2.0) removed two behaviours in one commit:

1. the shortest-airline-first ordering of the items to route —
   `autoroute_item_list.sort(Comparator.comparingDouble(this::calculateItemDistance))` was
   commented out with the note "Disabled in v2.3 because it negatively impacts convergence
   compared to v1.9 (natural order)";
2. the exhaustive ("slow") search tree that previously ran on every 4th pass
   (`useSlowAlgorithm = p_pass_no % 4 == 0` → `false`).

**Measured effect.** Fully-connected rate on the small tier drops 0.81 → 0.75 at v2.2.0 and never
recovers; 17 probe boards that v2.1.0 completes are left unrouted by every 2.2.x release. The
commit's own justification is contradicted by the data: restoring the sort alone at HEAD (a
one-line ablation; `calculateItemDistance` still exists in `AutorouteAirlineCalculator`) brings
connectivity back to exactly v2.1.0's level (0.76 → 0.82) and is slightly faster. On medium/large
boards the sort is neutral within noise, so a conservative fix is to enable it at least for boards
under ~50 nets — or unconditionally, since no tier measured worse with it.

**Bisect note.** A first bisect landed on `cbca03ee` ("Improve unconnected items detection in
DRC", 2026-01-10), which changed Trace/DrillItem contact detection to a Manhattan tolerance of
`half_width + 1`. That tolerance broke the same probe boards but was reverted before v2.2.0
shipped (`6182f236`); re-bisecting with the tolerance neutralised at every step isolates
`933d2980` as the change that ships in releases. `cbca03ee` is still worth review: its
tolerance-based contact test briefly made the router treat nearly-touching items as connected
(and its DRC-side grouping logic survives at HEAD).

## Regression 2 — DRC: micro-neckdown fanout fallback ignores minimum track width (first bad: `f31a0c84`)

**What changed.** Commit `f31a0c84` ("Add micro-neckdown fanout fallback for trace insertion",
2026-05-19, shipped in v2.3.0) retries a failed 2-point fanout insertion at reduced widths — the
pin's neckdown width, then 75 % and then 50 % of the class width — "keeping the same clearance
class", with no check against the board's minimum track width. (The opt-in `neck_width_um` retry
from `9b782275` is off by default and not involved.)

**Measured effect.** On any board whose net-class width equals its minimum width (very common),
the fallback produces sub-minimum traces: `track_width` violations on 31/146 small boards,
DRC-clean rate 0.94 → 0.73 (small tier) and 0.93 → 0.40 (large tier). Because a clean-pass metric
weighs a DRC violation like an unrouted net, the fallback converts "one net open" into "board
fails DRC" — and on the small tier it recovers no connectivity at all (0.76 with or without it).
Its benefit is real only on large boards (fully-connected 0.17 → 0.33), so the fix is a guard,
not a revert: skip retry widths below the board's minimum track width (`min_track_width`, or the
class width when the class is at the minimum).

**Ablation proof.** HEAD with the fallback disabled: DRC-clean 0.95, best of any build measured.
HEAD with the fallback disabled **and** the airline sort restored: clean pass 0.77 / connected
0.79 / DRC-clean 0.96 on the small tier — better than v2.1.0 (0.76 / 0.81 / 0.92) — and on the
large tier clean pass 0.30 vs v2.1.0's 0.20. Two one-line changes recover, and slightly exceed,
four releases of regression.

## Secondary findings (not root-caused, worth issues of their own)

- **Via inflation:** between v2.2.4 and v2.3.0 the router's via usage roughly doubles
  (0.37× → 0.71–0.94× of the human reference count) alongside a ~25 % slowdown — the "recovery"
  work after the ordering regression appears to try harder with more vias.
- **Self-report vs DRC disagreement:** the in-process statistics (`normalized_score`,
  incomplete/violation counts) disagree with the router's own DRC-only mode (`-de in.dsn out.ses
  -drc`) on ~45–63 % of boards (e.g. per-pin vs per-pair hole-clearance counting); external
  scoring should not trust the manifest numbers.
- **`passes_completed` misreports 1** in `result.json` even when logs show 18–30 passes.
- **2.2.x CLI splits `-de` paths on spaces**, failing to load any file whose path contains one
  (fixed by v2.3.0).
- **Crash on non-DSN input:** a binary file passed to `-de` dies with an NPE in
  `RouterSettings.applyBoardSpecificOptimizations` instead of a parse error.
