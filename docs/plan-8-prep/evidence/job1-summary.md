# Job 1 — `HeadlessBoardManager`'s three overrides on the Plan 8 parity corpus

**Verdict up front: 15 of the 16 corpus boards are MUTATED on a plain `-de <dsn> -do <ses>` run,
with default settings and no flags.** The mutation changes the routed SES bytes. Plan 8's survey
§5.7 and ruling "Risk 2" assumed these methods were "not a no-op in general" but effectively inert
on the corpus; that assumption is **false**. See the plan-boundary verdict at the bottom.

Raw evidence: `job1-overrides.txt`. Probe source: `job1/P8J1Probe.java`.

## What actually fires

| method | Java lines | reachable outcome on the corpus |
|---|---|---|
| `applyCopperToEdgeClearanceOverride` | `:466-552` | **FIRES and mutates on 15/16 boards.** Early-returns on 1 (`router-rpi-splitter`). |
| `applyHoleClearanceOverride` | `:346-396` | **FIRES on 16/16** (it cannot early-return once `holeClearanceUm` is non-null and >= 0), but with the default `0.0 um` it is a **no-op in effect**: `setHoleClearance(0)` over an existing 0, `changed == false`, so no `reinsertTreeItems()`. |
| `assignHoleKeepoutClearanceClass` | `:404-464` | **Never reached on the corpus** — it is gated on `configuredClearanceBoardUnits > 0` and the merged default is `0`. But the corpus *has* the input it wants: 6 boards carry 4-40 circular component keepouts each (see table). |

Merged values, identical on every board (merge #1, no `.rules` override observed — the
`drc-issue593-rules` row with `Issue593-BBD_Mars-64.rules` in the merger produces the same pair):

```
router.copper_to_edge_clearance_um = 500.0   (DefaultSettings.java:78)
router.hole_clearance_um           = 0.0     (DefaultSettings.java:81)
```

## Why the copper override fires

`:501-507`'s guard is:

```java
boolean usesFallbackOutlineClass    = outline.clearanceClassIndex() == defaultAreaClassNo;
boolean usesDefaultEdgeClearanceValue = |cfg - 500.0| < 1e-9;
if (usesDefaultEdgeClearanceValue && !usesFallbackOutlineClass) return;   // the ONLY early return
```

`usesDefaultEdgeClearanceValue` is always true (nothing on the corpus overrides 500.0), so the
early return depends entirely on `usesFallbackOutlineClass`. **On 15 of 16 boards the DSN reader
gives the outline the *default* class (index 1, `"default"`) and `defaultAreaClassNo` is also 1**,
so `usesFallbackOutlineClass == true` and the method proceeds. The survey's premise — that a DSN
outline "carries an explicit DSN clearance class" — holds for exactly one fixture.

When it fires it does four things, all of them board mutations:
1. `matrix.appendClass("board_edge")` — the clearance matrix grows by one class (row+column, every layer);
2. writes `configuredClearanceBoardUnits` (= **5000** board units at the corpus resolution) into the whole `board_edge` row **and** column, for every layer, unconditionally (no `Math.max` — unlike the hole path);
3. `searchTreeManager.remove(outline)` → `outline.setClearanceClassIndex(board_edge)` → `clearDerivedData()` → `searchTreeManager.insert(outline)`;
4. logs at `FRLogger.debug` only — **invisible at the default log level**, which is why this was never noticed.

## Board × override × outcome

`copper` = `applyCopperToEdgeClearanceOverride`; `hole` = `applyHoleClearanceOverride`;
`keepouts` = circular component keepouts, i.e. what `assignHoleKeepoutClearanceClass` *would* reclassify.

| stem | dsn | outline class (idx) | defaultAreaClassNo | copper | hole | keepouts | board MUTATED by a real run |
|---|---|---|---|---|---|---|---|
| router-rpi-splitter | Issue143-rpi_splitter.dsn | `boundary` (2) | 6 | **EARLY-RETURN** | fired, no effect | 8 | **no** |
| router-dac2020-bm01 | Issue508-DAC2020_bm01.dsn | `default` (1) | 1 | **MUTATED** | fired, no effect | 0 | **YES** |
| router-j2-reference | Issue026-J2_reference.dsn | `default` (1) | 1 | **MUTATED** | fired, no effect | 4 | **YES** |
| router-tutorial-board | tutorial_board.dsn | `default` (1) | 1 | **MUTATED** | fired, no effect | 0 | **YES** |
| router-ecc83-input | Issue649-kicad_ecc83-pp_input_board_v1.dsn | `default` (1) | 1 | **MUTATED** | fired, no effect | 0 | **YES** |
| drc-dev-board | Issue575-drc_dev-board_4_hole_clearance_violations.dsn | `default` (1) | 1 | **MUTATED** | fired, no effect | 4 | **YES** |
| drc-bbd-mars-64 | Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn | `default` (1) | 1 | **MUTATED** | fired, no effect | 8 | **YES** |
| drc-natural-tone-preamp | Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.dsn | `default` (1) | 1 | **MUTATED** | fired, no effect | 0 | **YES** |
| drc-issue593-rules | Issue593-BBD_Mars-64.dsn + .rules | `default` (1) | 1 | **MUTATED** | fired, no effect | 8 | **YES** |
| drc-issue593-ses | Issue593-BBD_Mars-64.dsn | `default` (1) | 1 | **MUTATED** | fired, no effect | 8 | **YES** |
| drc-issue753-cpu85 | Issue753-CPU-85_r104.dsn | `default` (1) | 1 | **MUTATED** | fired, no effect | 12 | **YES** |
| drc-issue110-relay | Issue110-RelayModule.dsn | `default` (1) | 1 | **MUTATED** | fired, no effect | 0 | **YES** |
| drc-tutorial-board | tutorial_board.dsn | `default` (1) | 1 | **MUTATED** | fired, no effect | 0 | **YES** |
| batch-fanout-bm11 | Issue730-DAC2020_bm11.dsn | `default` (1) | 1 | **MUTATED** | fired, no effect | 0 | **YES** |
| batch-strict-drc-cnh | Issue555-CNH_Functional_Tester_1.dsn | `default` (1) | 1 | **MUTATED** | fired, no effect | 40 | **YES** |
| batch-empty-board | empty_board.dsn | `default` (1) | 1 | **MUTATED** | fired, no effect | 0 | **YES** |

## Exactly what changes (before → after)

Every mutated board follows the same shape. Two representative rows, layer 0 of the clearance
matrix (`getValue(i, j, 0, addSafetyMargin=false)`):

**`router-dac2020-bm01`** — classes `[null, default, smd]` → `[null, default, smd, board_edge]`,
outline class `1 (default)` → `3 (board_edge)`, matrix sum `13000` → `63000`:

```
null@L0       0 0 0            ->  0 0 0 0
default@L0    0 2000 2000      ->  0 2000 2000 5000
smd@L0        0 2000 500       ->  0 2000 500  5000
board_edge@L0 (absent)         ->  0 5000 5000 5000
```

**`batch-empty-board`** — an all-zero matrix becomes a 500 um edge keep-out:
classes `[null, default]` → `[null, default, board_edge]`, matrix sum `0` → `30000`.

**`batch-strict-drc-cnh`** — 10 classes → 11, outline `1` → `10`, sum `1772800` → `2152800`
(4 layers × the new row/column). This is the "16 pre-existing violations, must add none" fixture,
so the 500 um edge clearance is directly load-bearing for its DRC assertion.

Boards with a named non-default class present (`drc-dev-board`'s `Power,Default`,
`drc-issue110-relay`'s `HighVoltage`, `batch-strict-drc-cnh`'s eight net classes) get `board_edge`
appended **after** them, so the new class index differs per board (3, 4 or 10). A port must
append, not assume index 3.

## The mutation reaches the output bytes (independent confirmation)

```
jar -de fixtures/Issue026-J2_reference.dsn -do def.ses  -mp 3                                     -> 15254 bytes
jar -de fixtures/Issue026-J2_reference.dsn -do zero.ses -mp 3 --router.copper_to_edge_clearance_um=0 -> 14644 bytes
cmp: differ at char 741, line 35
```

Both runs exit 0. `--router.copper_to_edge_clearance_um=0` still *fires* the override (the value is
no longer the default, so the `:501-507` guard cannot early-return) but installs a 0-valued
`board_edge` class, so the diff isolates the 500 um edge keep-out. **610 bytes and 45 connections'
worth of routing move.** This is not cosmetic.

## Cross-check against the port and against the existing references

- `grep -rn "board_edge\|hole_edge" crates/ scripts/ docs/` in `freerouting-rs` returns **nothing**.
  The port installs neither class; `fr_settings::resolve_headless` takes `&Board`, not `&mut Board`.
- **The existing Plan 6 router references are safe.** `scripts/differential/java/P6T1.java:173`
  loads via `DsnReader.readBoard(in, null, null, designName)` — it never constructs a
  `HeadlessBoardManager`, so those references are on the *pristine* board and the port matches them
  legitimately. `scripts/differential/java/P4T1.java:307-311` is the only probe that transcribes
  `HeadlessBoardManager.java:739-748`, and its comment covers only the layer-count and
  `applyBoardSpecificOptimizations` half.
- **Plan 7 Task 16's references are NOT safe.** Its generator drives the real CLI
  (`jar -de … -do "$REF/$stem/batch.ses"`), and `--verify-driver` requires `P7T9.java`'s SES to be
  byte-identical to that bare-jar output. Both sides of that check therefore *include* the
  `board_edge` mutation on 7 of the 8 batch stems — every stem except `router-rpi-splitter`.

## Plan-boundary verdict — READ THIS

**Plan 7 Task 16 needs the port of `applyCopperToEdgeClearanceOverride` BEFORE its references are
valid.** Concretely:

1. Ruling "Risk 2"'s escape condition has triggered: *"UNLESS Plan 7 Task 16 finds they fire on the
   batch corpus — then Plan 7 gains an insert task and Plan 8 inherits only the audit."* They fire
   on **7 of the 8** batch stems.
2. Without the port, `crates/fr-router/tests/batch_parity.rs` cannot reach acceptance rung (c)
   (byte-identical SES) on any of those 7 stems, and will most likely miss (a) and (b) too — the
   pass tuples and item sets diverge as soon as a connection routes near the outline. Every failure
   would be a false XDIFF pointing at the router, not at the loader.
3. **Recommended:** insert a Plan 7 task (before Task 16, after the pipeline lands) that ports
   `applyCopperToEdgeClearanceOverride` `:466-552` plus the surrounding
   `applyRouterSettingsForLoadedBoard` `:739-749` wiring, and applies it in whatever code path
   `gen-batch-reference.sh`'s Rust side uses to load a DSN. `applyHoleClearanceOverride` `:346-396`
   and `assignHoleKeepoutClearanceClass` `:404-464` can stay with Plan 8 Task 3 — they are provably
   inert at the default `0.0 um` on all 16 boards — **but** the hole path is what the two
   `Issue575-drc_*_hole_clearance_violations` fixtures are named after, so Plan 8 Task 3's probe
   must still exercise it at `--router.hole_clearance_um ∈ {100, 500}` where 6 corpus boards have
   4-40 circular keepouts waiting to be reclassified.
4. If the insert is rejected, the only alternative is to generate Task 16's references from a
   driver that bypasses `HeadlessBoardManager` — which contradicts ruling AV ("the HEAD jar is what
   routes") and would make `--verify-driver` a check of the driver against itself.

## Notes for whoever ports this

- `applyHoleClearanceOverride` has **no** early return once `holeClearanceUm` is non-null and >= 0.
  `board.rules.setHoleClearance(...)` runs unconditionally; only the `reinsertTreeItems()` and the
  `FRLogger.info` are gated. A port that early-returns "because nothing changed" diverges the moment
  the value is non-zero.
- The copper path writes its row/column **unconditionally**; the hole path writes
  `Math.max(holeClearance, matrix.getValue(defaultAreaClassNo, classNo, layer, false))` — "never
  reduce an existing requirement". The two are not symmetric; do not factor them together.
- Both `appendClass` calls are `matrix.getNo(name) < 0` guarded, so a board that already declares a
  `board_edge`/`hole_edge` class in its DSN reuses it. No corpus board does.
- ~~Both run **twice** per DSN load — once at `createBoard:342-343` on the freshly built, itemless
  board, and once at `applyRouterSettingsForLoadedBoard:746-747` on the fully loaded one. The
  second run is idempotent for the copper path (the class already exists, the outline already points
  at it) but re-runs the whole matrix write. A port that runs it once at the end produces the same
  state on the corpus; whether that holds in general is unverified.~~
  **FALSE — struck by Plan 8 Task 3 (`7ef2f57`). This line is the origin of survey ruling AD and of
  quirk #232's falsified text; it was read from `HeadlessBoardManager.java` alone, without checking
  that `createBoard` is reachable.** It is not, and the proof is at the *type* level rather than in
  any constructor: `Structure.java:1268` dispatches through `ReadScopeParameter`'s
  `final BoardParserCallback boardHandling` (`ReadScopeParameter.java:27`), while
  `HeadlessBoardManager implements BoardManager` (`HeadlessBoardManager.java:82`) and
  `public interface BoardManager` (`BoardManager.java:79`) has **no `extends`** — a
  `HeadlessBoardManager` is not assignable to that field, so no code path can route the call site
  into its `createBoard`. `BoardParserCallback.java:10-17`'s own javadoc says as much: *"The only
  production implementation is the package-private `MinimalBoardManager` nested inside
  `ReadScopeParameter`"* — and `MinimalBoardManager.createBoard` (`ReadScopeParameter.java:139-166`)
  builds the `RoutingBoard` and returns, calling neither override, with
  `getCurrentRoutingJob()` returning `null`. Measured on the HEAD jar: a counting subclass of the
  real `HeadlessBoardManager` driving a real `loadFromSpecctraDsn` reports
  `headless_create_board_calls=0 board_loaded=true` on all three probe fixtures
  (`crates/fr-core/tests/data/p8t3-clearance-overrides.txt`'s `[createboard]` rows).
  **So both overrides run exactly ONCE, at `applyRouterSettingsForLoadedBoard:746-747`**, and the
  port's single `fr_router::pipeline::prepare_board` call is Java's single call — a second call
  would be the divergence. See quirk **#253** (the dead method) and the rewritten **#232**.
- The class index of `board_edge` is board-dependent (3, 4 or 10 across the corpus).

---

## Addendum — the hole path at non-zero `--router.hole_clearance_um`

Measured with `copperToEdgeClearanceUm` forced to `null` so the two paths do not interfere
(`job1/P8J1Hole.java`; full transcript at the end of `job1-overrides.txt`). Values are layer-0
clearance-matrix rows.

| board | keepouts | `holeClearance` @100 um | @500 um | `hole_edge` class no |
|---|---|---|---|---|
| batch-strict-drc-cnh | 40 | 1000 | 5000 | 10 |
| drc-issue753-cpu85 | 12 | 1000 | 5000 | 3 |
| drc-bbd-mars-64 | 8 | 1000 | 5000 | 3 |
| router-rpi-splitter | 8 | **10000** | **50000** | 7 |
| router-j2-reference | 4 | 1000 | 5000 | 3 |
| drc-dev-board | 4 | 1000 | 5000 | 4 |

Findings a port must reproduce:

- **The um → board-unit conversion is board-resolution dependent.** 100 um is `1000` units on five
  boards and `10000` on `router-rpi-splitter` (an imperial-unit DSN). The Java expression is
  `round(Unit.scale(um * max(1, communication.resolution), UM, communication.unit))`.
- `-1.0` (negative) early-returns cleanly: no `hole_edge` class, `rules.holeClearance` stays `0`,
  keepouts keep their original class. Confirmed on all six boards.
- `assignHoleKeepoutClearanceClass` reclassifies **every** circular component keepout
  (`ObstacleArea`, `getComponentId() > 0`, `getArea() instanceof Circle`) to the new class —
  40/12/8/8/4/4 items respectively. It is a floor, not a set: the row value is
  `max(holeClearanceBoardUnits, matrix.getValue(defaultAreaClassNo, classNo, layer, false))`.
  At 100 um the existing copper clearance always wins, so the `hole_edge` row comes out **identical
  to the default-AREA row** and only the item reclassification is observable. At 500 um the floor
  bites on the lower-valued columns.
- The write loop runs `for (classNo = 1; classNo < matrix.getClassCount(); classNo++)` **after**
  `appendClass`, so `getClassCount()` already includes `hole_edge`: the diagonal
  `[hole_edge][hole_edge]` is written, and the symmetric write `setValue(classNo, holeEdgeClassNo, …)`
  also **mutates the default-AREA row's new last column**. Both are visible in the transcript
  (e.g. cnh's `defaultAreaClass(1)@L0` gains a trailing `2288` at 100 um, `5000` at 500 um).
- Class index 0 (`null`) is never written — the loop starts at 1 — so column 0 of `hole_edge`
  stays `0`.
- `hole_edge`'s class index is board-dependent (3, 4, 7 or 10 here) and, when both overrides are
  live, depends on whether `board_edge` was appended first. The call order at both call sites is
  **copper then hole** (`:342-343` and `:746-747`), so `board_edge` always takes the lower index.
