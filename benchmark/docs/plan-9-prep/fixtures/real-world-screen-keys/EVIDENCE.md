# Real-world case: screen-keys — the invisible top-edge corridor

A 2-layer, 34-net keyboard/screen board (from the user's Room-Service-2026
project; DSN exported via pcbnew after stripping routes). Both freerouting-rs
v1.0.0 AND the Java HEAD jar leave the same single connection open at -mp 40:

    5V: C1.1 @ (13.5, 8.0) -> J1.2/J1.4 @ (55.9-58.4, 12.7)

## Why this is an algorithm case, not a rules case

The straight line crosses two both-layer copper keepouts (H1, an M3 mounting
hole at (31.5, 8.8), keepout r~1.85mm; grazing H9, the M2.5 screen standoff at
(51.0, 14.0), r~1.6mm) — so the human's direct path is illegal. BUT the strip
along the top edge (y ~ 1..6.95mm, below the edge clearance and above nothing)
is ~6mm of open legal corridor for a 0.2mm-width/0.2mm-clearance net:
up from C1.1 to y~5, straight to x~56 (passing UNDER H1's keepout, which spans
y in [6.95, 10.65]), drop to J1.2 passing right of H9 (51+1.6 = 52.6 < 55.9).
Neither implementation finds it. Identical failure in both = shared algorithmic
limitation, parity-consistent.

## Hypothesis (to confirm when Task 8 lands)

The door-drop family: #160/#161 (comparator silently drops expansion doors —
measured 481/2000 room completions) and #163 (the missing eighth door). An
edge-and-keepout-pinched corridor is exactly where dropped doors sever the
search graph. Prediction: after Task 8's fixes, this board routes 71/71.
If it does, this is a measurable quality win over Java HEAD on a real board;
if not, escalate to T9/T10 (pass loop / shove decisions) with this file.

## Files
- unrouted.dsn — the stripped board (34 nets)
- routed.ses / result.json — rs v1.0.0 at -mp 40: 70/71, 0 violations, score 985.9
- result-java.json — Java HEAD at -mp 40: same 1 incomplete
- Also unreached: GND pad J1.9 (pour didn't connect it after refill — separate,
  minor; check thermal-relief/pour reach when touching T19).

## Acceptance hook
Task 8's stem A/B should append this board as an extra directed case:
route unrouted.dsn at -mp 10 and assert incomplete_count == 0 post-fix
(pre-fix: 1 on both implementations, jar-confirmed 2026-09-03).
