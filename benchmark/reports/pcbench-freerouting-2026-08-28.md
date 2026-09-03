# Freerouting (HEAD, 2.3.1-snapshot) on PCBench — 2026-08-28

Host: workbench (16-core x86-64, NixOS, KiCad 10.0.4), `--jobs 12`, 1 seed (router is deterministic),
`--router.max_threads=1`, 5-minute job timeout. Referee: KiCad DRC on the board's own design rules
(legacy net classes + `(setup)` floors rounded down to 1 µm, zones refilled, routing-type checks only).

Corpus: PCBench 1182 boards → 605 admitted (reference routing-DRC-clean *and* fully connected;
2 unrouted references kept without ratio ground truth). Splits by net count (PCBWorld): d3-a 2–13,
d3-b 14–42, d3-c 43–451.

## PCBWorld-comparable setting (`-mp 10`)

| Tier | Boards | Clean Pass | Fully connected | Zero routing DRC | Median CPU | WL / human | Vias / human | PCBWorld Freerouting CP |
|---|---|---|---|---|---|---|---|---|
| d3-a | 143 | **0.59** | 0.76 | 0.73 | 34 s | 1.02× | 0.79× | 0.80 (99 boards) |
| d3-b | 260 | **0.40** | 0.65 | 0.60 | 81 s | 1.07× | 0.72× | 0.78 (10-board subset) |
| d3-c | 200 | **0.16** | 0.34 | 0.37 | 313 s | 0.97× | 0.74× | — (unscored) |
| all  | 603 | 0.37 | 0.57 | 0.55 | 93 s | 1.02× | 0.75× | — |

Failure mix (382 non-clean boards): 259 with nets unrouted, 123 fully connected but DRC-dirty,
136 missing by only 1–2 connections. Dominant DRC failure: `track_width` on 235 boards — the
router's neck-down retry (75 % width) violates the board's minimum track width.

## Budget: `-mp 10` vs `-mp 500` (same boards, 602)

| Tier | CP mp10 | CP mp500 | connected mp10 | connected mp500 | zero-DRC | median CPU mp10 / mp500 |
|---|---|---|---|---|---|---|
| d3-a | 0.59 | 0.59 | 0.76 | 0.77 | 0.73 / 0.73 | 34 s / 32 s |
| d3-b | 0.40 | 0.42 | 0.65 | 0.69 | 0.60 / 0.60 | 81 s / 79 s |
| d3-c | 0.16 | 0.16 | 0.34 | 0.35 | 0.37 / 0.37 | 313 s / 268 s |
| all  | 0.37 | 0.38 | 0.57 | 0.60 | 0.55 / 0.55 | 93 s / 94 s |

Transitions mp10→mp500: clean-pass +7 / −2; connected +17 / −2. 44 boards hit the 5-minute router
timeout at `-mp 500`; the rest stopped on stagnation (no score gain for 10 passes).

## Observations
- More passes do not help: the router stagnates long before 500 passes; the remaining gap is
  algorithmic (unrouted nets on medium/large boards), not budget.
- Neck-down is the single largest source of DRC failures and turns "1 net unrouted" into a
  DRC-violating board.
- The Java self-report disagrees with its own DRC-only mode on ~45 % of boards; referee numbers
  are authoritative.
- Runs: `results/pcbench-mp10`, `results/pcbench-mp500` + `results/pcbench-mp500-b` (the latter
  resumed after an ssh drop killed the first job's pull-back; cells are disjoint).

## Version comparison on d3-a (143 boards, `-mp 10`, run `d3a-versions-mp10`)

| Version | Clean Pass | Fully connected | Zero routing DRC | Boards with `track_width` violations | Median CPU |
|---|---|---|---|---|---|
| 2.1.0 (PCBWorld's version) | **0.76** | 0.83 | 0.92 | 0 | 19 s |
| 2.3.0 release | 0.70 | 0.76 | 0.94 | 0 | 27 s |
| 2.3.1 HEAD | 0.59 | 0.76 | 0.73 | 31 | 34 s |

2.1.0 reproduces PCBWorld's 0.80 within corpus-slice noise. Two regressions since:
2.1.0→2.3.0 lost connectivity (0.83→0.76) and speed; 2.3.0→HEAD introduced the neck-down
retry, which alone drops zero-DRC from 0.94 to 0.73 (31/143 boards) without recovering any
connectivity.

## Six versions on d3-a (143 boards, `-mp 10`, runs `d3a-versions-mp10`, `d3a-versions2-mp10`)

| Version | Clean Pass | Fully connected | Zero routing DRC | `track_width` boards | Median CPU | Vias / human |
|---|---|---|---|---|---|---|
| 1.9.0 | 0.71 | 0.75 | 0.94 | 0 | 22 s | 0.38× |
| 2.0.1 | 0.71 | 0.75 | 0.93 | 0 | 13 s | 0.50× |
| 2.1.0 | **0.76** | **0.83** | 0.92 | 0 | 19 s | 0.40× |
| 2.2.4 | 0.64 | 0.67 | 0.94 | 0 | 20 s | 0.37× (8 cells failed: 2.2.4's CLI splits `-de` paths containing spaces) |
| 2.3.0 | 0.70 | 0.76 | 0.94 | 0 | 27 s | 0.71× |
| 2.3.1 HEAD | 0.59 | 0.76 | 0.73 | 31 | 34 s | 0.79× |

2.1.0 is the high-water mark. Connectivity regressed in the 2.2 series (2.2.4: 0.67; 0.71 on the 135 boards its CLI could load), partially recovered in 2.3.0 (0.76), and the neck-down retry after 2.3.0 costs DRC
cleanliness without regaining connectivity. Via usage roughly doubled between 2.2.4 and 2.3.0.

## Root cause — neck-down (`track_width` violations)

Bisected on workbench with per-run isolated settings (see below): first bad commit
`f31a0c84` (2026-05-19, in 2.3.0) "Add micro-neckdown fanout fallback for trace insertion":
failed 2-point fanout insertions are retried at the pin neckdown width, then 75 % and 50 % of
the class width, with no check against the board's minimum track width. The opt-in
`neck_width_um` retry (`9b782275`) is off by default and not involved.

**Caveat on all cross-version tables above:** every freerouting process loads and re-saves a
shared `freerouting.json` (XDG config dir), copying its fields over the built-in defaults, so
runs of different versions on one host leak settings into each other. The version tables were
recorded before the suite isolated HOME/XDG per cell (commit 6ee98be) and are being re-run.

## Ten versions on d3-a — isolated settings (run `d3a-all-iso`, 138 common boards, `-mp 10`)

| Version | Clean Pass | Fully connected | Zero routing DRC | `track_width` boards | Median CPU |
|---|---|---|---|---|---|
| 1.9.0 | 0.70 | 0.73 | 0.94 | 0 | 23 s |
| 2.0.1 | 0.62 | 0.66 | 0.92 | 0 | 13 s |
| **2.1.0** | **0.75** | **0.81** | 0.93 | 0 | 19 s |
| 2.2.0 | 0.71 | 0.75 | 0.93 | 0 | 22 s |
| 2.2.1 | 0.72 | 0.76 | 0.93 | 0 | 21 s |
| 2.2.2 | 0.71 | 0.75 | 0.93 | 0 | 21 s |
| 2.2.3 | 0.71 | 0.75 | 0.93 | 0 | 22 s |
| 2.2.4 | 0.71 | 0.75 | 0.93 | 0 | 21 s |
| 2.3.0 | 0.66 | 0.80 | 0.77 | 25 | 25 s |
| 2.3.1 HEAD | 0.60 | 0.76 | 0.73 | 30 | 26 s |

(2.2.x cannot load DSN paths containing spaces — 8 boards per version excluded from the common set.)
Two regressions: connectivity 0.81 → 0.75 between 2.1.0 and 2.2.0 (17 boards 2.1.0 completes and
every 2.2.x leaves unrouted, 10 the other way); and DRC cleanliness 0.93 → 0.77 in 2.3.0 from the
micro-neckdown fallback (`f31a0c84`), which also recovered some connectivity (0.75 → 0.80).

## Root cause — connectivity drop 2.1.0 → 2.2.0

Bisected (isolated settings, 25 discriminating boards, fraction-of-boards verdict): first bad commit
`cbca03ee` (2026-01-10) "Improve unconnected items detection in DRC" — `DrillItem`/`Trace` contact
detection switched from exact corner equality to a Manhattan tolerance of `half_width + 1`
(`isWithinTolerance`). Failure fraction on the probe boards jumps 24 % → 92 % at that commit.
Because the same contact test feeds the router's own connectivity model, items that merely come
within a trace half-width of each other are treated as connected and the router stops routing
them, leaving real gaps that KiCad (and HEAD's own DRC) report as unconnected.

## Complexity ladder — d3-b and d3-c samples, ten versions, isolated (run `ladder-bc-iso`, `-mp 10`)

d3-b (30 boards, 14–42 nets):

| Version | Clean Pass | Connected | Zero DRC | `track_width` boards | Median CPU |
|---|---|---|---|---|---|
| 1.9.0 | 0.70 | 0.73 | 0.93 | 1 | 47 s |
| 2.0.1 | 0.43 | 0.47 | 0.87 | 1 | 24 s |
| **2.1.0** | **0.77** | **0.80** | 0.93 | 1 | 43 s |
| 2.2.0–2.2.4 | 0.70 | 0.73 | 0.93 | 1 | 45–49 s |
| 2.3.0 | 0.57 | 0.77 | 0.70 | 8 | 59 s |
| HEAD | 0.60 | 0.77 | 0.73 | 7 | 60 s |

d3-c (30 boards, 43–385 nets):

| Version | Clean Pass | Connected | Zero DRC | `track_width` boards | Median CPU | 5-min timeouts |
|---|---|---|---|---|---|---|
| 1.9.0 | 0.13 | 0.13 | 0.93 | 0 | 69 s | 10 |
| 2.0.1 | 0.13 | 0.13 | 0.87 | 0 | 47 s | 9 |
| **2.1.0** | **0.20** | 0.23 | 0.93 | 0 | 260 s | 4 |
| 2.2.0–2.2.4 | 0.13–0.17 | 0.13–0.17 | 0.93–0.97 | 0 | 190–230 s | 0–3 |
| 2.3.0 | 0.13 | **0.33** | 0.40 | 16 | 335 s | 1 |
| HEAD | 0.13 | **0.33** | 0.40 | 16 | 292 s | 0 |

The pattern holds at every size: 2.1.0 is the best release; 2.2.0 lost connectivity (cbca03ee);
2.3.0's micro-neckdown (f31a0c84) raises connectivity on large boards (0.17 → 0.33) but halves
DRC cleanliness, so Clean Pass does not improve.

## Ablation at HEAD (run `d3a-ablation`, 146 d3-a boards, `-mp 10`, isolated)

| Variant | Clean Pass | Connected | Zero DRC | `track_width` boards |
|---|---|---|---|---|
| 2.1.0 (reference) | 0.76 | 0.82 | 0.92 | 0 |
| HEAD | 0.60 | 0.76 | 0.73 | 31 |
| HEAD − micro-neckdown (`f31a0c84` disabled) | **0.72** | 0.76 | **0.95** | 0 |
| HEAD − via-endpoint tolerance (`04c0478f` exact) | 0.60 | 0.76 | 0.73 | 31 |
| HEAD − both | 0.72 | 0.76 | 0.95 | 0 |

HEAD is deterministic (two independent runs agree on every board). Disabling the micro-neckdown
fallback recovers 12 of the 16 lost clean-pass points and makes HEAD the cleanest version on DRC;
the remaining 4–6 points are the connectivity loss introduced with 2.2.0 (`cbca03ee` lineage),
which the surviving via-endpoint tolerance does not explain.

## Narrowed connectivity bisect (tolerance neutralised at every step; run on workbench)

With the `cbca03ee` contact tolerance forced to exact-match at every commit, the connectivity loss
still appears — first bad commit `933d2980` (2026-01-14) "Remove useSlowAlgorithm parameter from
autorouter": probe-board failure fraction 24 % (parent `5d6dde53`) → 60 %. Besides removing the
parameter it (a) stops running the exhaustive "slow" search tree on every 4th pass and (b) removes
the airline-distance sort of the items to route (`autoroute_item_list.sort(comparingDouble(
calculateItemDistance))`) — i.e. the connectivity regression is a change in net ordering /
search strategy, not in DRC. `cbca03ee` (four days earlier) only masked it while its tolerance was
active.

## Ablation 2 — restoring 2.1.0's airline-distance ordering at HEAD (run `d3a-ablation2`, 146 boards)

| Variant | Clean Pass | Connected | Zero DRC | `track_width` boards | Median CPU | Vias / human |
|---|---|---|---|---|---|---|
| 2.1.0 (reference) | 0.76 | 0.82 | 0.92 | 0 | 18.7 s | 0.42× |
| HEAD | 0.60 | 0.76 | 0.73 | 31 | 25.9 s | 0.79× |
| HEAD − micro-neckdown | 0.72 | 0.76 | 0.95 | 0 | 26.2 s | 0.87× |
| HEAD + airline sort | 0.64 | 0.82 | 0.75 | 29 | 23.7 s | 0.78× |
| **HEAD − micro-neckdown + sort** | **0.77** | 0.79 | **0.96** | 0 | 23.4 s | 0.94× |

Restoring the shortest-first ordering alone brings connectivity back to 2.1.0's level (0.76 → 0.82);
combined with the neck-down guard it gives the best clean-pass of any build measured (0.77 vs
2.1.0's 0.76), with the cleanest DRC. Two one-line changes. HEAD still uses ~2× the vias of 2.1.0.

## Ablations on the harder tiers (run `ladder-ablation`, 30 d3-b + 30 d3-c boards, `-mp 10`)

| Variant | d3-b Clean Pass / Connected / Zero DRC | d3-c Clean Pass / Connected / Zero DRC |
|---|---|---|
| 2.1.0 | 0.77 / 0.80 / 0.93 | 0.20 / 0.23 / 0.93 |
| HEAD | 0.60 / 0.77 / 0.73 | 0.13 / 0.33 / 0.40 |
| HEAD − micro-neckdown | 0.77 / 0.80 / 0.93 | **0.30** / 0.30 / 0.90 |
| HEAD + airline sort | 0.60 / 0.70 / 0.70 | 0.13 / 0.30 / 0.37 |
| HEAD − micro-neckdown + sort | **0.80** / 0.80 / **0.97** | 0.27 / 0.27 / 0.83 |

The neck-down guard is the dominant fix at every size and on large boards it makes HEAD better
than any release (0.30 vs 2.1.0's 0.20) — HEAD's post-2.1.0 search work does pay off there once
its output is DRC-clean. The airline-distance ordering helps on small boards, is neutral-to-slightly
negative on medium/large ones at n=30 (within noise), and should be re-evaluated on the full tiers.

## Rust port (plan-8, e821bcd) vs Java HEAD — run `rs-vs-java`, 206 boards, `-mp 10`, 2026-09-02

| Tier | n | Clean Pass rs / java | Connected rs / java | Zero DRC rs / java | Median CPU rs / java | Speedup | RSS rs / java |
|---|---|---|---|---|---|---|---|
| d3-a | 146 | 0.60 / 0.60 | 0.76 / 0.76 | 0.73 / 0.73 | 0.30 s / 24.8 s | ×85 | 9 MB / 1050 MB |
| d3-b | 30 | 0.60 / 0.60 | 0.77 / 0.77 | 0.73 / 0.73 | 1.20 s / 51.6 s | ×43 | 11 MB / 1214 MB |
| d3-c | 30 | 0.13 / 0.13 | 0.33 / 0.33 | 0.43 / 0.40 | 17.4 s / 305.6 s | ×12 | 28 MB / 1421 MB |

191/206 boards have identical (clean-pass, unrouted, violations); 9 lexicographic losses (5 on hard
metrics — largest divergence: 74Logic_SA_ADC 62→127 unrouted), 197 wins (time). Total CPU for the
run: 31 min (Rust) vs 259 min (Java). Formal compare: `reports/rs-vs-java-mp10.{md,html,json}`.

## FINAL: Rust port vs Java HEAD, full corpus — run `full-rs-vs-java` (605 boards, `-mp 10`, paired, isolated, 2026-09-02)

| Tier | n | Clean Pass rs / java | Connected rs / java | Zero DRC rs / java | Median CPU rs / java | Speedup | RSS rs / java |
|---|---|---|---|---|---|---|---|
| d3-a | 143 | 0.59 / 0.59 | 0.76 / 0.76 | 0.73 / 0.73 | 0.41 s / 34.1 s | ×94 | 9 / 949 MB |
| d3-b | 260 | 0.41 / 0.40 | 0.65 / 0.64 | 0.60 / 0.60 | 1.97 s / 77.2 s | ×40 | 13 / 1245 MB |
| d3-c | 202 | 0.18 / 0.16 | 0.37 / 0.33 | 0.42 / 0.37 | 27.3 s / 293.0 s | ×11 | 30 / 1477 MB |
| all | 605 | **0.38 / 0.37** | 0.58 / 0.57 | 0.57 / 0.55 | 2.6 s / 95.9 s | ×33 | 13 / 1274 MB |

Totals: 218 CPU-min (Rust) vs 1460 CPU-min (Java), ×6.7 overall. Identical outcomes on 513/605
boards (85 %); 92 divergent — the port better on 70 (mostly cap-bound large boards where its speed
buys extra passes), worse on 22 (pulled with both routers' artefacts to ~/Development/freerouting/
outlier_pcbs/ for review; largest: can_firewall 2→138 unrouted, 1Bitsy 0→63 — in both the port
trades connectivity for zero violations). Formal compare: `reports/full-rs-vs-java.{md,html,json}`;
lexicographic verdict "worse" (563 W / 42 L / 0 T, hard losses on the 22) per the suite's
no-board-left-behind rule.
