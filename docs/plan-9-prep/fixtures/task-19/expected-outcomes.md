# Task 19 — DRC report accuracy: independent ground truth

The DRC report is the artefact a KiCad user actually reads, so this task has an oracle no other
task has: **KiCad 10's own DRC verdict on two of the corpus boards**, already committed to the
Java clone as `fixtures/Issue575-*-kicad_drc.json`. Those files are used below as independent
ground truth wherever they apply.

`kicad-cli` **is** available locally at
`/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli` (version **10.0.3**) — it is simply not
on `PATH`. It was not needed for the rows below because the committed KiCad verdicts already
cover the two Issue575 boards, but it is there if a third board's verdict is wanted.

All port numbers are the committed D goldens at tag **`v1.0.0` (`ecc0abf`)**. Raw tables:
`current-port-behavior.txt`.

---

## #152 — `isHole` calls every `Pin` a hole — **THE PLAN'S BINDING TEST IS WRONG**

### The classification, exactly (`crates/fr-drc/src/report/build.rs:207-211, 331-337`)

    fn is_hole(board, id) -> bool { matches!(item.kind(), Via | Pin) }

    let kind = if is_hole(first) || is_hole(second) { "holeClearance" } else { "clearance" };
                                ^^ OR, not AND

The fix sketch is `Via || (Pin whose padstack.fromLayer() != toLayer())`. It changes `is_hole`
**only**. The `||` stays. **Therefore any violation involving a `Via` keeps its
`holeClearance` classification no matter what the fix does to pins.**

### Every `holeClearance` violation in the corpus, classified

| stem | count | the two item kinds | can #152 move it? |
|---|---|---|---|
| drc-bbd-mars-64 | 64 | `(Pin, Via)` ×64 | **NO** — the Via is a hole under `\|\|` |
| drc-dev-board | 2 | `(Pin, Pin)` ×2 | **YES** if both pins are SMD |
| drc-issue110-relay | 2 | `(Via, Via)` ×2 | NO |
| drc-issue110-relay | 2 | `(BoardOutline, Pin)` ×2 | **YES** if the pin is SMD |
| drc-issue753-cpu85 | 64 | `(Via, Via)` ×64 | NO |
| drc-issue753-cpu85 | 12 | `(Pin, Via)` ×12 | **NO** |
| drc-issue753-cpu85 | 2 | `(BoardOutline, Pin)` ×2 | **YES** if the pin is SMD |

### Are those pins SMD? — resolved from the DSN padstacks

Every candidate pin was located by matching the violation's reported position against
`(place …)` + `(pin …)` offsets, and its padstack's `(shape (… <layer> …))` entries counted.
KiCad's DSN export also encodes the answer in the padstack **name** — `[T]` top only,
`[B]` bottom only, `[A]` all layers — and the two agree in every case:

| stem | pin | padstack | layers | hole? |
|---|---|---|---|---|
| drc-dev-board | `U2-41@4`, `U2-41@5` (at 125.5, −49.31) | `Rect[T]Pad_900.000000x900.000000_um` | `F.Cu` only | **SMD — not a hole** |
| drc-dev-board | `U2-41@2`, `U2-41@3` (at 124.1, −52.11) | `Rect[T]Pad_900.000000x900.000000_um` | `F.Cu` only | **SMD — not a hole** |
| drc-issue110-relay | `FU1-1` (at 113.25, −221.75) | `Oval[A]Pad_2500x3500_um` | `F.Cu` **and** `B.Cu` | **through — IS a hole** |
| drc-issue753-cpu85 | `J1-1` (at 109.5106, −185.1587) | `RoundRect[T]Pad_3401.174×6753.291…` | `F.Cu` only | **SMD — not a hole** |
| drc-issue753-cpu85 | `J1-51` (same x,y, back side) | `RoundRect[B]Pad_3401.176×6737.500…` | `B.Cu` only | **SMD — not a hole** |
| drc-bbd-mars-64 | all 64 `TPnnn-Pad1` | `Rect[T]Pad_4000.000000x4000.000000_um` | `F.Cu` only | SMD — but paired with a **Via** |

### **The binding expectation, per stem**

| stem | today `holeClearance / clearance` | **after #152** | total |
|---|---|---|---|
| **drc-bbd-mars-64** | 64 / 12 | **64 / 12 — UNCHANGED** | 96, unchanged |
| **drc-dev-board** | 2 / 0 | **0 / 2** | 10, unchanged |
| **drc-issue110-relay** | 4 / 0 | **4 / 0 — UNCHANGED** | 26, unchanged |
| **drc-issue753-cpu85** | 78 / 0 | **76 / 2** | 107, unchanged |
| the other four stems | 0 / 0 or 0 / n | unchanged | unchanged |

**Totals are unchanged everywhere**, which is the plan's own statement — *"Changes counts per
`type`, not the total."* ✔

> ### ⚠ `an_smd_pad_is_not_a_hole` must not assert on BBD Mars-64
> The plan binds: *"asserts the BBD Mars-64 **64/12 split** moves to the corrected split and
> that the total is unchanged."* **On BBD Mars-64 nothing moves.** All 64 are
> `(SMD Pin, Via)` pairs — a test-point pad sitting *exactly on* a via of the same net
> (`actual: 0.0000 mm`, same `pos`), and the Via alone keeps `is_hole(a) || is_hole(b)` true.
>
> Point the test at **`drc-dev-board` (2/0 → 0/2)** and **`drc-issue753-cpu85` (78/0 → 76/2)**,
> which are the two stems that actually move, and keep the "total unchanged" assertion on
> BBD Mars-64 as a **no-change** guard.

### Is `||` the right semantics? — KiCad says yes, and that settles it
KiCad's own rule is that `hole_clearance` fires between a hole and *any* copper, not only
between two holes. So `is_hole(a) || is_hole(b)` is correct and should **not** be changed to
`&&` to make BBD Mars-64 move. Record that decision.

For completeness, KiCad 10's verdict on the same board is `hole_clearance: 1, clearance: 6`
against the port's `64 / 12`. The gap is **not** #152 — it is net-awareness (KiCad does not
report a pad and a via of the *same net* at the same point as a violation at all) and is a
different question from the one #152 asks. Do not tune toward it here.

---

## #146 — the dangling-trace dedup guards on `firstItem` alone

### Mechanism
Each candidate is guarded on `firstItem` **only**, so a trace that is a net entry's
`secondItem` — or merely a member of its `allItems` — is reported **twice**. The **via** phase
has **no guard at all**. 17 of 112 rows reach it.

### The independent evidence
`drc-natural-tone-preamp` at v1.0.0: **111 `track_dangling` + 4 `via_dangling` = 115**.
**KiCad 10 on the same board: 5 `track_dangling` + 4 `via_dangling` = 9.**

The two definitions are not identical — KiCad's "dangling" is stricter — so **9 is not the
target**. But a 111-vs-5 ratio on `track_dangling` while `via_dangling` matches exactly (4 = 4)
is a strong independent signal that the trace phase is over-reporting and the via phase is not
over-*counting* (though it is under-*guarded*).

### The expectation
> **Invariant (the real one):** every reported dangling item appears **exactly once** in the
> report. Assert `report.violations.iter().filter(dangling).map(item_uuid).all_unique()`.
>
> The plan's `a_dangling_trace_is_reported_once` and `the_via_phase_dedups_too` are exactly
> this, split by phase.

**Fix:** build the `allItems` set **once before the phase** and test membership of it — which
also removes an O(n²) rescan — **or** drop the guard and let the two phases be independent, as
the via phase already is. **Not a number to tune toward any particular jar run.**

**The headline:** the port already emits a *stable* count where the jar wavers across hash
modes, so `drc-natural-tone-preamp` carries the tree's **one permanent XDIFF** — the only
artefact that exists purely because the jar disagrees with itself. **Closing #146 deletes it**,
and that deletion is the commit's headline (`p8t3`).

---

## #153 — `Item.smallestClearance` is a monotone lifetime minimum, never reset

**Mechanism:** the field is initialised to `−1.0` **once, at construction**, and only ever
lowered. `getAllClearanceViolations` runs **once per report** *and* **once per
`BoardStatistics` built with violations**, so by the end of a run every item reports the worst
clearance any earlier call saw — even if routing improved it since.

**Ground truth — a two-call invariant, no literal needed:**

> 1. Compute violations. Note `item.smallest_clearance()` = `c₁`.
> 2. Improve the board (move the offending copper further away, or delete it).
> 3. Compute violations again. `item.smallest_clearance()` must now be the **improved**
>    number, `c₂ > c₁` — or, with the violation removed entirely, must report *no* violation
>    rather than the stale `c₁`.
>
> Today step 3 answers `c₁`. That is the whole bug and it needs no oracle.

**Fix:** **delete the field** and let `ClearanceViolation::smallest_clearance(&[…])` fold over
the returned list. It exists only because the GUI wanted a number the compute had thrown away.
Deleting it also kills the repeated whole-board recompute — a free `cpu_s` win that partly pays
for Task 12's cost.

Note the port already documents a coupled side effect: `Board::clearance_violations` is
`&mut self` **because** it lowers `smallestClearance` and advances the search tree's entry
counter (`crates/fr-board/src/board/clearance.rs:37`, cited from
`statistics.rs:505-512`). Deleting the field may let that signature become `&self` — check,
and if it can, take it: it removes a `&mut` from a hot read path.

---

## #271 — `initializeDrc` returns `true` unconditionally

**Today:** `-drc` exits **0** whatever it finds. A missing `.rules` warns and carries on; a
missing session warns and carries on; a failed quality score warns; and **the violation count
never reaches the exit code at all**. A board with 107 violations
(`drc-issue753-cpu85`) exits exactly like `drc-tutorial-board`, which has 0.

**Fix (recommendation 5, adopted): add `--fail-on-violations`** rather than silently redefining
exit 0 — every existing CI script reads today's code.

> **Two binding cases, both exact:**
>
> | invocation | board | violations | exit |
> |---|---|---|---|
> | `drc <board>` | `Issue753-CPU-85_r104.dsn` | 107 | **0** (unchanged) |
> | `drc --fail-on-violations <board>` | `Issue753-CPU-85_r104.dsn` | 107 | **1** |
> | `drc --fail-on-violations <board>` | `tutorial_board.dsn` | 0 | **0** |
>
> `drc-tutorial-board` and `drc-issue593-rules` are the two clean stems (0 violations);
> `drc-issue753-cpu85` (107) and `drc-bbd-mars-64` (96) are the dirtiest. Use one of each.

Document the flag in the help **and** in the hand-off's divergence table.

---

## #272 — the DRC path uses a different settings merge from the router's

**Mechanism:** the DRC merge is `0, 10, 20, 55, 60` — with **no `RulesFileSettings` at 40**, no
between-merges pass, no second merge, and no post-merge `.rules` re-apply. So

    -de b.dsn -dr weights.rules -drc r.json

reports violations computed **with** those clearances and a `qualityScore` computed with
weights **that file never influenced**.

> **Binding expectation:** run `drc-issue593-rules` — the one stem that carries a `.rules` file
> (`fixtures/Issue593-BBD_Mars-64.rules`) — twice, once with `-dr` and once without.
> Today the reported `qualityScore` is **identical**; after the fix it must **differ**, because
> the rules file's weights reach the score.
>
> This needs no external number: it is a *before/after within one run pair*. The plan's test
> `the_quality_score_uses_the_rules_file_weights` is exactly this shape.
>
> Sanity floor: the *violation* counts must **not** change (they already used the rules file's
> clearances) — `drc-issue593-rules` stays at **0 violations, 74 unconnectedItems**.

**Fix:** use the job's own merged `routerSettings` — the DRC path already has one and throws it
away. **One line**, and it **discharges `crates/fr-settings/src/resolve.rs`'s obligation
marker**; strike the marker in the same commit.

---

## #151 — the report's coordinate unit is hard-coded `"mm"`

**Mechanism:** the only CLI call site passes `"mm"`, so **four of the five `convertCoordinate`
branches are dead — including the fallback, the only arm that honours a board's declared
`(unit …)`**. Every Issue575 fixture declares `(unit um)` and is silently rescaled by 1/1000.

**Ground truth — an exact conversion table.** Take one violation whose position the goldens
already carry, e.g. `drc-bbd-mars-64`'s first `holeClearance`, `pos = (96.52, −119.38)` mm:

| `--unit` | expected `coordinateUnits` | expected `pos.x` | expected `expected:` clearance |
|---|---|---|---|
| `mm` (default) | `"mm"` | `96.5200` | `0.2000 mm` |
| `um` | `"um"` | `96520.0000` | `200.0000 um` |
| `inch` | `"inch"` | `3.8000` | `0.0079 inch` |
| `mil` | `"mil"` | `3800.0000` | `7.8740 mil` |

(`96.52 mm / 25.4 = 3.8 inch` exactly; `0.2 mm / 25.4 = 0.007874015… → 0.0079` at `%.4f`.)

**Note the resolution trap the register flags:** `%.4f` means a `"um"` report prints **four
decimals of a micrometre** — 0.1 nm resolution, from a board whose grid is 0.1 µm. The unit
flag changes the *resolution* of the text output, not just its label. Consider widening the
format for `um`/`mil`, or document it.

**Default stays `mm`**, so **no committed D golden moves for this row alone.** That is what
makes this row cheap: it is pure CLI wiring plus one flag, and the port has already ported all
five arms.

---

## #154 — the document does not validate against the schema it names — **PROVEN BY ARTEFACT**

Both files claim the **same** schema:

    port  $schema : https://schemas.kicad.org/drc.v1.json
    KiCad $schema : https://schemas.kicad.org/drc.v1.json

and their keys disagree:

| the port writes (camelCase) | KiCad 10 writes (snake_case) |
|---|---|
| `coordinateUnits` | `coordinate_units` |
| `kicadVersion` | `kicad_version` |
| `schematicParity` | `schematic_parity` |
| `unconnectedItems` | `unconnected_items` |
| `holeClearance` (violation `type`) | `hole_clearance` |

**And the port is internally inconsistent about it**: it writes `holeClearance` (camel) next to
`track_dangling` and `via_dangling` (snake) in the *same* `type` field. KiCad writes
`via_dangling` too — so three of the port's four violation types already match KiCad and only
`holeClearance` does not.

Port-only keys with no KiCad counterpart: `freeroutingVersion`, `qualityScore`. Those are
extensions and are fine; the schema's `additionalProperties` decides, and 2.3.0 shipped them.

> **Binding expectation:** after the fix, every key the port emits that KiCad also emits is
> spelled the way KiCad spells it. The five rows above are the complete list, extracted from
> KiCad 10's own output, not guessed.

**Fix:** make `DrcJsonFlavor::KiCad` the only flavour **written** (recommendation 6, adopted;
ruling W already made it the CLI default). **Keep the `FreeroutingHead` reader** for one
release — its test stays — and delete only the writer arm. With parity gone, "the parity choice
the crate's tests pin" is no longer a reason for the writer to exist.

**Cost:** all 8 D stems re-cut on this row alone (every key's spelling changes).

---

## #195 + #196 — the statistics block

### #195 — the three lengths do not sum to the total

**Mechanism** (`crates/fr-router/src/score/statistics.rs:247-266`): the loop walks
`polyline.lines()` and measures each **infinite line's two defining points** — not the trace's
corners — and over **all** the lines, including the two bounding lines that carry **no
segment**. `total_segment_count`, computed four lines above from `corner_count − 1`, already
counts the right thing.

**A minimal witness, measured at v1.0.0** — far smaller than the survey's
`121 606.75 / 130 610.65` pair, and reproducible from a hand fixture
(`../task-13/fixtures/p9t13-multi-net-smd-pin.dsn`, one straight protected horizontal trace):

    traces.total_count         = 1
    traces.total_segment_count = 1
    traces.total_length        = 1200
    h / v / a                  = 1200 / 0.002 / 0
    h + v + a                  = 1200.002        <-- exceeds total_length by 0.002

The `0.002` is exactly the polyline's **two bounding lines**: each is defined by two points
1 board unit apart, and the unit conversion at `:449-457` scales by 1/1000. A perfectly
straight horizontal trace is credited with 0.002 of *vertical* length.

On a real board the error is the other sign and much larger, because a bounding line's two
defining points bear no relation to the segment: the survey's measurement on
`Issue143-rpi_splitter` at k=8 is `121 606.75` against a `totalLength` of `130 610.65` — a
**6.9 % shortfall**.

> **Binding invariant (the plan's `the_three_lengths_sum_to_the_total`):**
> `h + v + a == total_length`, and the number of terms summed equals `total_segment_count`.
>
> **⚠ Floating-point caution.** `total_length` is a **Kahan/Neumaier-compensated `f64`** sum
> (`java_double_stream_sum`, `:201-206`) cast to `f32`, while `h`/`v`/`a` are naive `f32`
> accumulations. Exact equality will not hold on a board with thousands of segments.
> Either (a) accumulate the breakdown with the *same* compensated `f64` summation and then
> assert exact `f32` equality, or (b) assert a relative tolerance and **state it in the test**.
> (a) is better and is a one-line change since `java_double_stream_sum` already exists.

### #196 — the bounding box holds a corner, not extents

**Mechanism** (`statistics.rs:125-131`): `Rectangle2D.Float`'s `w`/`h` parameters are handed the
board's **lower-left corner**.

    stats.board.bounding_box = { x: bb.ur.x, y: bb.ur.y, width: bb.ll.x, height: bb.ll.y }
                                                          ^^^^^^^^^^^^^  ^^^^^^^^^^^^^^^

**Measured at v1.0.0:**

| board | `bounding_box` | `board.size` |
|---|---|---|
| `p9t13-multi-net-smd-pin.dsn` | x=2001 y=2001 **width=−1 height=−1** | width=2002 height=2002 |
| `Issue143-rpi_splitter.dsn` | x=54025.8 y=106451.4 **width=−25.4 height=−25.4** | width=54051.2 height=106476.8 |

`board.size` (`:132-137`) is `|ll − ur|` and already carries the real extent. So:

> **Binding invariant (`the_bounding_box_holds_extents_not_a_corner`) — no literal needed:**
>
>     bounding_box.width  == board.size.width
>     bounding_box.height == board.size.height
>     bounding_box.width  >  0   and   bounding_box.height > 0
>
> Today `width` and `height` are the lower-left corner and are **negative on every corpus
> board**. Post-fix: `ur − ll`.

**The gate consequence:** from this commit on, **G2 may use the length breakdown and the
bounding box** — BL2's forbidden-input list shrinks. That is a **gate-version bump to `g3`**
(ruling BP8): the same commit re-cuts Task 12's affected baseline columns under the new gate,
commits both, and names the bump.

---

## #110 — a conduction area's holes emit `Double.toString` coordinates

**Mechanism:** `SesWriter.writeConductionArea` writes the boundary with `writeScopeInt` and each
**hole** with plain `writeScope`, so a conduction area with holes emits float coordinates inside
an otherwise all-integer SES file. **A session file's grid is integral by construction.**

**The fixture already exists**: `crates/fr-dsn/tests/data/p8t13-conduction-area.dsn` (Plan 8
Task 13), which carries

    (wire (polygon F.Cu 0  100000 100000  200000 100000  200000 200000  100000 200000)
          (window (polygon F.Cu 0  130000.5 130000.5  170000.5 130000.5
                                   170000.5 170000.5  130000.5 170000.5))
          (net NPLANE))

The window's coordinates are deliberately `.5` values, so the float/int distinction is visible
in the output rather than hidden by round numbers. **The roadmap's "needs a new fixture" is
discharged.**

> **Binding expectation (`a_conduction_area_hole_is_written_as_integers`):** every coordinate
> token in the emitted SES matches `^-?[0-9]+$` — no `.`, no `E`. Applied to the whole file,
> not only the hole, so a regression anywhere in the writer is caught.
>
> The hole's expected integer coordinates are the `writeScopeInt` rounding of the `.5` inputs:
> `130000.5 → 1300005` at resolution 10 (the DSN declares `(resolution um 10)`, so board units
> are 1/10 µm and the `.5` is exactly representable) — **verify the scale factor against the
> actual writer rather than assuming**; the invariant "no non-integer token anywhere" is the
> assertion that does not depend on it.

**Fix:** give `Shape.writeHoleScope` an **`int` variant**.

---

## #111 — `RulesWriter.writeRules` writes the design name raw

**Mechanism:** `DsnWriter` quotes the design name; `RulesWriter` does not. A name holding a
space produces a `.rules` header **the reader's own `NAME` lexeme cannot read back as one
token**.

> **Binding expectation (`a_design_name_with_a_space_round_trips`):**
>
>     write_rules(design_name = "my board")  ->  first line contains  "my board"  (quoted)
>     read it back                           ->  design name == "my board"        (one token)
>
> Today the header is `(rules PCB my board` and the reader lexes `my` as the name and `board`
> as a stray token. The round trip is the whole test and needs no external oracle.

**Fix:** `identifierType.write(designName, file)`. **Changes the first line of every `.rules`
file** — the committed `.rules` goldens
(`Issue029-hw48na-written.rules`, `Issue593-BBD_Mars-64-written.rules`) re-cut on this row.
Check whether their design names contain a space: if not, the goldens do **not** move and the
row is golden-neutral; if they do, they move and it must be named.

---

## #81, #212, #201 — three diagnostics that lie

### #81 — `PlanarDelaunayTriangulation.validate()` answers `true` unconditionally
`crates/fr-board/src/datastructures/delaunay.rs:388-400`: the inner-node branch calls
`currentChild.validate()` and **discards the return value** (line 957 in Java), so the anchor —
an inner node as soon as the first corner splits it — always answers `true`, and logs
*"check passed ok"*.

**Measured at v1.0.0:** `validate()` returns **`true`** on a 7×7 grid triangulation that is
**provably missing an edge** (119 where `3n−3−h = 120`). The diagnostic cannot see the very
defect Task 13 spends a commit fixing.

**Fix:** `result &= child.validate();`.
> **Binding expectation (`validate_fails_on_an_inconsistent_inner_node`, replacing
> `validate_is_vacuous_on_an_inner_node`):** construct a triangulation, corrupt one **leaf**
> below an inner node (swap two edge references), and assert `validate() == false`.
> Today it is `true` for **every** corruption below the anchor.

**Ordering (ruling BP6):** Task 13 rewrites this file's in-circle predicate and bounding
triangle and does **not** touch `validate()`; Task 24 deletes the private PRNG afterwards.
Re-read the file; do not rely on line numbers.

### #212 — `ItemRouteResult.improvementPercentage` divides two `int`s
The via term truncates, so a re-route halving the via count scores like one removing **every**
via.

    vias before = 10, vias after = 5
        int arithmetic : (10 - 5) / 10        -> 5 / 10        -> 0   (integer division)
        ... and the surrounding expression then treats 0 or 1 as the whole term
    correct        : (10.0 - 5.0) / 10.0      -> 0.5

**The tell that makes this unambiguous:** the optimizer's own recomputation **has** the
`(float)` cast. So **the field is wrong and the number actually used is right** — the two
disagree today, and agreeing is the assertion.

> **Binding expectation (`improvement_percentage_does_not_truncate_the_via_term`, replacing its
> pinning twin):** for a result that halves the via count, the field equals the optimizer's own
> recomputed value. No external number: the two computations in the same codebase must agree.

**Fix:** the `(float)` cast.

### #201 — `getHash`'s javadoc says "trace" where the body hashes the whole item graph
A **comment** fix whose *consequence* was already corrected: the wrong comment is what
justified this port's original trace-and-via-only hash, under which **every trace-free board
hashed alike**. The port's hash has since been fixed; the row exists to record why.

> **Deliverable:** reword the three comments, and correct
> `crates/fr-board/tests/snapshot.rs`'s hash comment. No behaviour change, no golden moves.
> Say so explicitly in the commit message so the row is not mistaken for a no-op.

---

## Rows with no work owed — the record to write, not the code

* **#144 / #145** are already answered *better* by the port (`BTreeMap`/`BTreeSet` with a
  lowest-id representative; always `.`). Add a note to their register rows: **#144 is why
  `-XX:hashCode=2` was mandatory on `p8t3`, and retiring the jar comparison retires that
  requirement.**
* **#149 / #150** (the hard-coded `focusNets = {98, 99}` debug block; the
  array-instead-of-count log line) are **NOT-A-FIX** — not ported, nothing owed. Mark
  `candidate` with that reason.
* **#155** is a settings row and lands in **Task 21**, after #115.

---

## Summary — what each row is grounded on

| # | ground truth | source | needs the jar? |
|---|---|---|---|
| #152 | per-stem reclassification table; padstack layer spans read from the DSNs | port goldens + DSN parse | no |
| #146 | uniqueness invariant; KiCad's 5 vs the port's 111 `track_dangling` | KiCad 10 verdict | no |
| #153 | two-call improvement invariant | invariant | no |
| #271 | 107-violation stem exits 1 under the flag, 0 without | port goldens | no |
| #272 | `qualityScore` differs with/without `-dr`; violations unchanged at 0/74 | before/after pair | no |
| #151 | exact mm/um/inch/mil conversion table for one known position | arithmetic | no |
| #154 | **five key spellings extracted from KiCad 10's own output** | KiCad 10 verdict | no |
| #195 | minimal witness `1200 vs 1200.002`; sum invariant + the f32 caution | measured at v1.0.0 | no |
| #196 | `bounding_box.{width,height} == size.{width,height}`; measured −1 / −25.4 | measured at v1.0.0 | no |
| #110 | existing `p8t13-conduction-area.dsn`; "no non-integer token" invariant | existing fixture | no |
| #111 | quote-and-round-trip a name with a space | invariant | no |
| #81 | `validate()` = `true` on a triangulation missing an edge | measured at v1.0.0 | no |
| #212 | field must equal the optimizer's own recomputation | internal agreement | no |
| #201 | comment reword; no behaviour change | — | no |

**Not one row in this task needs the jar.** That is the point of the task: the jar is not the
oracle for a report the jar itself gets wrong.
