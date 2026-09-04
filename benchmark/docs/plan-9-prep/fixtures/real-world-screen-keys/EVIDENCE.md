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

## Hypothesis (to confirm when Task 8 lands) — TESTED, NOT CONFIRMED

The door-drop family: #160/#161 (comparator silently drops expansion doors —
measured 481/2000 room completions) and #163 (the missing eighth door). An
edge-and-keepout-pinched corridor is exactly where dropped doors sever the
search graph. Prediction: after Task 8's fixes, this board routes 71/71.
If it does, this is a measurable quality win over Java HEAD on a real board;
if not, escalate to T9/T10 (pass loop / shove decisions) with this file.

### Result: the prediction is FALSIFIED. Task 8 changes this board by nothing.

Measured on the same machine, `FR_ROUTER_BUDGET=disabled`, both sides built from
source; "before" by checking the worktree out at Task 8's base commit `90ee5a3`
and rebuilding, "after" at Task 8's tip:

    before (90ee5a3)   -mp 10: 3 incomplete, 0 violations, score 957.738
                       -mp 40: 3 incomplete, 0 violations, score 957.738
    after  (Task 8)    -mp 10: 3 incomplete, 0 violations, score 957.738
                       -mp 40: 3 incomplete, 0 violations, score 957.738

Not one incomplete, not one violation, not one point of score. The `-mp 10` and
`-mp 40` runs are identical to each other as well: this board stops improving
long before pass 10.

Both of the hypothesis's mechanisms are live and both landed — the comparator
drops **481 of 8 000** neighbours before #160/#161 and **0** after, and an
eight-sided obstacle room went from seven doors to eight under #163 — so the
fixes are not the thing that failed. Neither reaches this corridor.

### Escalation to T9/T10, as the hypothesis directs

Two readings are left and this measurement does not separate them:

1. the corridor is not lost to a **missing door** at all but to a **cost or shove
   decision** — the pass loop declining to rip, or the shover refusing the
   pinched corridor — which is T9/T10's ground; or
2. the corridor's doors were never dropped here, because the drop needs a
   geometry (two neighbours of one room whose first corners are *different but
   equidistant* from the compare corner, on one wall) that this board does not
   present.

**The one probe that separates them:** ask whether the maze ever *reaches* a room
in the `y ~ 1..6.95 mm` strip under H1's keepout, which the section above already
localises the corridor to. If it does, the doors are there and the refusal is a
cost/shove decision — reading 1. If it never reaches one, the graph is severed
somewhere earlier and reading 2 stands and needs its own hypothesis. One
instrumented run answers it, and it belongs with whoever owns the next
hypothesis rather than with this file.

Note also that the board is **1 incomplete worse than its own v1.0.0 line below**
(70/71 at `-mp 40` there against 68/71 now). That regression predates Task 8 —
it is present at `90ee5a3` — and is Task 2's territory, not this hypothesis's.
**Closed at Task 9**: the board is back at 70/71 and at v1.0.0's own score, for
#227's reason rather than for anything Task 2 did. See the Task 9 section above.

### Task 9 ran the hook again: the board MOVED, the corridor did NOT open

Measured at Task 9's tip (`c135a3f`), `FR_ROUTER_BUDGET=disabled`, same machine and same
invocation as the Task 8 rows above:

    after (Task 9)     -mp 10: 1 incomplete, 0 violations, score 985.907, 1917.429 mm, 5 vias
                       -mp 40: 1 incomplete, 0 violations, score 985.907, 1917.429 mm, 5 vias

So Task 9 is the first task that moves this board at all: **3 incomplete -> 1**, score
**957.738 -> 985.907**. What moved it is #227 — the optimizer stage had a stage-scoped stop and
began doing work — and the result is that the board is back at **70/71**, i.e. exactly the
v1.0.0-rs line quoted below. The regression this file recorded in its last paragraph ("1 incomplete
worse than its own v1.0.0 line") is **closed**; `-mp 10` and `-mp 40` are still identical to each
other, so the board still stops improving long before pass 10.

**The corridor is still not found.** The one connection left open is the same one both
implementations have always left open — 5V, `C1.1 -> J1.2/J1.4` — so the section above is
untouched and readings 1 and 2 are both still live.

**The separating probe was NOT run at Task 9, and that is a deliberate hand-off rather than an
omission.** The probe this file specifies — "ask whether the maze ever *reaches* a room in the
`y ~ 1..6.95 mm` strip under H1's keepout" — needs a **room-position ledger** in the autoroute
engine: something that records each completed expansion room's shape as the maze search runs, so
the strip can be tested for occupancy. `fr_router::autoroute::instrument` has no such recorder
(its five counters are quirk #193's stale-index guards) and `P7T14B_MAZE`'s queue ledger prints
door ids and sorting values, not geometry. Adding one is production instrumentation and is outside
Task 9's fix list, which is the seven rows of survey group 9.

**What T10 needs to build, precisely.** One `LazyLock<bool>` env gate in the same shape as
`instrument::on` / `p7t14b_maze_ledger`, one recorder called where a `CompleteFreeSpaceExpansionRoom`
is first completed, and a dump of `(layer, bounding box)` per completed room for the 5V connection
of this board. Reading 1 (a cost or shove decision) is "at least one completed room's box
intersects `y in [1 mm, 6.95 mm]` on a signal layer"; reading 2 (the graph is severed earlier) is
"none does". One run answers it.

## Files
- unrouted.dsn — the stripped board (34 nets)
- routed.ses / result.json — rs v1.0.0 at -mp 40: 70/71, 0 violations, score 985.9
- result-java.json — Java HEAD at -mp 40: same 1 incomplete
- Also unreached: GND pad J1.9 (pour didn't connect it after refill — separate,
  minor; check thermal-relief/pour reach when touching T19).

## Acceptance hook — DISCHARGED at Task 8 (not met), RE-RUN at Task 9 (partly met)
Task 8's stem A/B should append this board as an extra directed case:
route unrouted.dsn at -mp 10 and assert incomplete_count == 0 post-fix
(pre-fix: 1 on both implementations, jar-confirmed 2026-09-03).

Task 8 ran it, before and after, at both `-mp 10` and `-mp 40`: **3 incomplete
either side, unchanged**. The hook is answered — with a "no" — and the escalation
it names has been written above. **Task 9 ran it again** and the hook's own
assertion (`incomplete_count == 0` at `-mp 10`) is still **not** met — but the
count fell 3 -> 1 and the score rose 957.738 -> 985.907, so the escalation carries
forward to T10 with a moved board rather than a static one. It is deliberately **not** wired into the stem
A/B as a 30th row: this board is not in `tests/reference/`, the A/B's 29 stems are
fixed by the three fixture tables, and adding one is a plan amendment rather than
a task's own change (`scripts/quality-ab.sh` refuses a count other than 29 and
says so). Whoever takes reading 1 or 2 above should decide whether it becomes a
fixture.
