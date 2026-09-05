# Plan 9 survey — the consolidated post-parity fix catalogue

**Status:** survey input for the Plan 9 implementation plan. Docs-only; no code, no register edits.
**Sources reconciled:** `docs/plan-8-prep/post-parity-roadmap.md` (721 lines, written against a
~213-row register), `docs/java-quirks.md` (292 rows today), `docs/plan-8-handoff.md` §6/§7/§9
(the roadmap import, the two post-merge errata, and controller ruling BI's quirk-#105 Tier-1 row),
and the git log through `864e76a`.

**Binding direction (user, unchanged):** *better routing beats speed.* Tiers are ranked by routing
quality; performance work is last and is not scheduled here.

**Binding direction (user, new — supersedes the roadmap's §1.1):** **there is no `Compat` switch.**
Byte-level compatibility with the Java jar may break from now on; fixes land directly, unconditionally.
Everything the roadmap says about `Compat::{Java, Fixed}`, dual goldens, `--mode=fixed` generators and
per-fix gating is **withdrawn**. §7 of this document replaces it with the harness transition: which
committed references get regenerated from the fixed port as its own goldens, which differential
drivers retire, and what acceptance replaces byte parity.

---

## 1. How to read the catalogue

One row per **candidate fix**, not per register row: paired and same-root-cause register rows are
collapsed into the row that names the whole change, because the register's own standing rule is that
half a paired fix is worse than none (#48/#57, #164's two halves, #89/#93/#94, #9/#82, #126/#128,
#7/#68, #160/#161).

| column | meaning |
|---|---|
| **#** | register row id(s) in `docs/java-quirks.md`. A `+` joins rows that are one fix. |
| **T** | tier. T1 crash / hang / OOM / silent data loss. T2 routing quality. T3 DRC and report accuracy. T4 settings and CLI predictability. Ranked within tier. |
| **mechanism** | one line: what goes wrong, in terms a user would recognise. |
| **Java site** | the file:line the register pins. |
| **port site(s)** | where the port reproduces it. |
| **fix sketch** | from the register's "Suggested fix" column where it is right, corrected where the roadmap or this survey corrects it. |
| **status** | one of the five below. |
| **Δrefs** | which committed reference families the fix moves — see §7.1 for the codes. `—` = no committed artefact moves. |

**Status values**

* **FIX** — a real behavioural improvement to implement. (With the switch gone, this is simply "do it".)
* **KEEP** — deliberate behaviour the port must retain on its merits: load-bearing for determinism,
  or a wire/JSON contract. **Not** "keep because the jar does it" — byte-parity alone is no longer a
  reason, so every roadmap row that was kept only for parity has been re-judged as FIX or NOT-A-FIX.
* **ALREADY-FIXED** — no Plan 9 work owed: the port already answers the better way (a totalization,
  a deliberate divergence, or a defect already closed on a branch that has merged).
* **NOT-A-FIX** — records a port-side divergence, a Java-side-only cleanup with no observable
  consequence here, a dead-code note, or an unported class. Upstream-PR material at most.
* **SUPERSEDED** — a roadmap row the register has since refined; the superseding row is named.

**Δrefs codes** (reference families, defined in §7.1): **G** geometry/DSN (`tests/reference/fixtures.txt`),
**R** router per-connection (`router-*.jsonl`), **B** batch SES (`batch.ses`, 8 stems),
**C** CLI end-to-end (`cli-*`, 13 stems), **D** DRC documents (`drc-*`, 8 stems),
**X** differential drivers (`scripts/differential/`), **U** unit/directed tests only.
`likely` marks a row whose corpus reach is unproven — the corpus does not currently contain an input
that discriminates, and the fix's own directed fixture is the evidence instead.

---

## 2. What changed since the roadmap was written

The roadmap is still the best analysis of rows #1-#213 and this survey does not restate it. Six
things have moved under it, and they are why a reconciliation was needed at all:

1. **The `Compat` switch is gone** (user directive above). §1.1, §1.2's five-point evidence bar,
   §7.1-§7.2's release split and every "inverts in `Fixed` mode" note are re-read as "inverts", full
   stop. The evidence bar survives minus the mode axis: register id, observable effect, a directed
   test that fails before the fix, a green full run, and an A/B number.
2. **79 register rows the roadmap never saw** — #214-#292. Twenty-two of them (#214-#235) are Plan 7
   pipeline rows; fifty-seven (#236-#292) are Plan 8's core/CLI/MCP/KiCad rows. They add **one row
   that outranks most of Tier 2** (#227, the optimizer stage is inert), **three T1 data-loss rows**
   (#265, #268, #289) and a **whole KiCad-JSON reader family** (#280-#288) the roadmap could not have
   tiered.
3. **The blockers the roadmap's §7.3 named are gone.** `route` exists (Plan 8 Task 12), so does the
   `p8t1`/`p8t2 e2e` whole-program harness, `gen-batch-reference.sh`, the eight-stem `batch_parity`
   byte gate, and — new at `ecc0abf` — the **`benchmark/` comparison suite** with an independent
   referee and a noise-aware verdict. The A/B harness the roadmap sketched no longer has to be built;
   it has to be *pointed at* the fixed port.
4. **Four fixture-debt items closed** (Plan 8 Task 13): `SesWriter.writeWasIs`, `Component.readLockType`'s
   position arm, #110 and #105 all have directed fixtures with JVM ground truth. The roadmap's "needs a
   new fixture" note on #110 and #105 is discharged.
5. **Two port defects were found and fixed after the merge** (handoff Errata, commits `374058c`,
   `c3a7ee0`): quirk #44's fanout tie-break (`.rev()` on `sorted_unconnected_targets`) and the
   in-pass deadline observation. Both are parity repairs, not post-parity fixes; they are recorded
   here as ALREADY-FIXED so nobody re-opens them, and #44's *post-parity* question (should the id
   order be ascending at all?) is untouched by them.
6. **The register's own strike-in-place corrections** have retired two roadmap premises: #232
   (the clearance overrides do **not** run twice) and #270's second clause (the KiCad-JSON output
   keeps the **first** event's board, not the last — which is what makes #289 a data-loss row).

---

## 3. Tier 1 — crashes, hangs, OOM, silent data loss

### 3.1 The output file (new — the roadmap has none of these)

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#289** | T1 | **`-do out.json` writes the board as it was BEFORE routing.** `setJobOutput` is a board-updated listener *and* runs once after the pipeline; the first write re-sniffs its own bytes to `KICAD_DESIGN_JSON`, so every later write is a no-op and the file keeps the loaded board. Measured identical at `-mp 1`, `-mp 2`, `-mp 8`, and with the router off. | `RoutingJobSchedulerActionThread.java:100, :168, :259-295`; `BoardFileDetails.java:105-119` | `crates/freerouting/src/commands/route.rs` step 12b (takes the pre-routing snapshot deliberately, to match) | Write the **final** board: serialise once, after the pipeline, and stop re-sniffing (`setData(bytes, format)`). Delete the pre-routing snapshot and `resolved_output_format`'s shared-snapshot dance. | **FIX** | C (2 KiCad stems), X (`p8t7`, `cli_e2e`) |
| **#268** | T1 | `tryToSetOutputFile`'s return is discarded, so `-do out.dsn`/`.scr` **writes a 0-byte file and exits 1** — over the previous result #265 has already deleted — while `-do out.txt` silently receives SES bytes and exits 0. | `Freerouting.java:123, :196-213`; `RoutingJob.java:377-397` | `crates/freerouting/src/commands/route.rs` (`_accepted`, `set_job_output`, `write_cli_output_if_available`) | Test the return value and refuse the run **at the argument**, naming the accepted formats; never write a zero-byte file. | **FIX** | C, X (`cli_e2e`), U |
| **#265** | T1 | The desired output file is **deleted before the run starts** — before input validation, before the board load, before the router. A run that then fails, hangs or is killed has destroyed the previous result and written nothing. `File.delete()` also unlinks an empty directory. | `Freerouting.java:116-121` | `crates/freerouting/src/commands/route.rs::delete_existing_output` | Delete nothing. `Files.write`/`std::fs::write` truncates; a failed run must leave the previous result on disk. | **FIX** | X (`cli_e2e`), U |

### 3.2 Non-termination and OOM

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#71 + #76 + #106** | T1 | `overlappingTreeEntries` **appends** to the caller's collection and never clears it; `PolylineTrace.split` re-reads into the same list and restarts the iterator, and `Item.getConnectionItems` walks the contacts **with no visited set** — so a two-rail four-rung ladder on one net makes `normalizeAllTraces` never return, and every DSN read ends with that call. | `ShapeSearchTree`/`PolylineTrace.split`; `Item.getConnectionItems` | `crates/fr-board/src/board/trace_normalize.rs`, `crates/fr-board/tests/trace_normalize.rs` (an `#[ignore]`d unbounded reproduction) | Return a **fresh** collection from `overlapping_tree_entries` (makes the aliasing unrepresentable — the roadmap's correction of the register's "either/or"), then add the visited set to the contact walk. Do #71 first and re-measure whether #76 needs anything more. The ignored test becomes a terminating assertion with a literal answer. | **FIX** | B/R likely (re-walk order feeds every shove), X (`p2t11` mode 8) |
| **#162** | T1 | `calculateNewIncompleteRooms` does not terminate and dies with `OutOfMemoryError` when a room's shape has more border lines than its `toSimplex()` does. **Reachable from production at 0.4 % of room completions.** The port does not guard it either (handoff §7, "closed with reason"), so the generators carry a `timeout(1)`. | `SortedRoomNeighbours.complete` | `crates/fr-router/tests/sorted_neighbours.rs` (pins the trigger without running the loop) | Compute `roomSimplex` **once, in the constructor**, and derive every `touchingSideNo` from it. Not the loop bound — that terminates on the wrong side. `p6t3` mode 5 currently skips these calls; the fix makes mode 5 full-coverage and that is the acceptance. | **FIX** | R, B likely; X (`p6t3` mode 5) |
| **#86** | T1 | The DSN scanner's fixed 16 MiB `char[]` has no refill, so a design over 16 MiB cannot be scanned at all. The port's lexer is already correct; it reproduces the ceiling on purpose as `DsnError::InputTooLarge`. | `SpecctraDsnStreamReader` | `crates/fr-dsn` `DsnScanner::new`; `tests/lexer.rs::input_larger_than_the_java_buffer_is_rejected` | Delete the deliberate limit; accept a 20 MiB input and lex it to the same token stream a 15 MiB one gives. | **FIX** | U |
| **#27** | T1 | `PolygonShape.intersects(Shape)` binds to itself for polygon-vs-polygon and recurses to `StackOverflowError` — not recoverable in either language, because the handlers catch `Exception`, not `Throwable`. Polygon keepouts are ordinary in KiCad exports. | `PolygonShape.intersects` | `crates/fr-geometry` `polygon_against_polygon_reproduces_the_java_stack_overflow` (a panic assertion) | Add an `intersects(PolygonShape)` overload that splits both sides to convex; assert against a hand-computed answer. Not the type test. | **FIX** | U, G likely |
| **#105** *(ruling BI)* | T1 | `Wiring.readViaScope`'s net-number loop omits its `++currentIndex`, so a multi-subnet via's net array is padded with `0` — and `calculateAllIncompletes`' `nets.get(0)` is then `Vector.get(-1)`, so **every autoroute pass throws and `AutorouteBatchLoop.run` retries for ever.** The jar hangs; one changed token separates it from a 1 995-byte routed SES. | `Wiring.java:684-687`; `DesignRulesChecker.java:558` | `crates/fr-dsn/src/parser/wiring.rs`; fixtures `p8t13-via-net-numbers.dsn` + control | Add the `++currentIndex`. Fixture and control already exist (Task 13), so this is the cheapest T1 row in the catalogue. Upstream-PR candidate. | **FIX** | U, X (`p8t1` loses its one XDIFF row) |
| **#241, #244, #250, #252, #257** | T1 | Five Java hangs/crashes the port already answers with a value: `getFileFormat`'s never-refilled shift loop (spins at 0 % CPU on six leading CR/LFs), `isCliTerminalState` omitting `INVALID` (a CLI that parks for ever), `BoardStatistics`' backwards `(hostCad)` slice, `countOccurrences("")`, and `fromJob` NPEing on a root input path. | `RoutingJob.java:180-187`; `Freerouting.java:151-158`; `BoardStatistics.java:491-502, :578-586`; `RoutingResultManifest.java:105-108` | `fr_core::FileFormat::sniff_bytes`, `RoutingJobState::is_cli_terminal`, `stats_from_bytes::slice_totalized`/`count_occurrences`, `manifest::from_job` — all `// totalized:` with `p8t1probe`/`p8t2probe` XDIFF rows on both sides | No port work. Upstream-PR candidates; the XDIFF rows become plain rows when the differential retires (§7.3). | **ALREADY-FIXED** | X (rows change shape) |

### 3.3 Silent data loss and silent input corruption

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#95** | T1 | `Structure.readScope` reads `autoroute_settings` **only when it is the first layer-structure consumer in the scope**. Any earlier `keepout`/`plane` makes the whole scope go unread *and unskipped*, so its closing bracket ends the **structure** scope one level early and the tail is misparsed. Exporters conventionally write keepouts first. | `Structure.readScope` | `crates/fr-dsn/tests/structure_scope.rs::an_autoroute_settings_scope_after_a_keepout_is_never_read` | Hoist the `AutorouteSettings.readScope` call out of the `if`. Add a corpus sweep counting, per fixture, whether the settings were read. | **FIX** | G, B/R/C (boards gain their file's router settings) |
| **#90** | T1 | `readIntegerScope`'s failure branch returns `0` and does not consume a second token, so the scope's closing bracket desyncs `AutorouteSettings.readScope`'s depth-unaware loop, which misreads it as ending its own scope. The `0` reaches `RouterSettings` and is written back out. | `DsnFile.readIntegerScope` | `crates/fr-dsn` | Fix the token consumption **and** the caller loop together — one alone makes a malformed `autoroute` scope parse worse. Land after #95. | **FIX** | G likely, U |
| **#94 + #89 + #93** | T1 | `createBoard`'s overflow loop divides an **`int`** `scaleFactor` by 10 until it truncates to **0**, for any boundary coordinate ≥ 6 710 886 DSN units; `CoordinateTransform(0,0,0)` is then built without complaint, every written coordinate becomes `Infinity`/`NaN`, every read one `0`, and the outline degenerates to the bare box — **JVM-verified to report `Success`**. #93 (a `(circle …)` bounding box 2× too wide and tall) halves the threshold. | `Structure.createBoard`; `CoordinateTransform`; the circle bounds | `crates/fr-dsn/tests/structure_scope.rs`, `tests/geometry_scopes.rs` | **One change:** `scaleFactor` a `double` (or clamped ≥ 1), a rejected zero/non-finite scale in the constructor, and `coor[0] / 2` on all four circle bounds. Needs a ≥ 6.71 M-unit fixture — there is still none. | **FIX** | G, C (`cli-large-outline` exists — check first), U |
| **#112** | T1 | `applyRules` warns "layer not found" and does **not** return, leaving `layerIndex = -1` — the sentinel the branches below read as **"all layers"** — so a stale `.rules` file silently overwrites the **default trace width on the whole board**. | `RulesReader.applyRules` | `crates/fr-dsn/src/rules_reader.rs` `apply_rules` (`Option<usize>`, `None` = all layers) | Make the sentinel unrepresentable: a two-variant enum, not an `Option`. Directed test: a `.rules` naming `B.Cu` against a 4-layer board leaves the default width untouched. | **FIX** | U, C likely |
| **#91** | T1 | `skipScope` returns `false` at EOF, every caller discards it, and `readScope`'s loop then returns `true` — so a DSN truncated inside an unrecognised scope is reported as a **successful** parse of a partial board. | `skipScope`/`readScope` | `crates/fr-dsn/tests/scopes.rs::read_scope_generic_returns_ok_true_when_truncated_inside_an_unknown_scope` | A **third `BoardReadResult` variant** carrying the partial board *and* the diagnostic — not a hard failure (the roadmap's correction of the register's binary framing). API change on `BoardReadResult`. | **FIX** | U |
| **#103** | T1 | `Network.insertComponent` `return`s — not `continue`s — on a pin naming an absent padstack, so the package contributes its first *n-1* pins and **none** of its keepouts or outlines, and every later item id shifts. | `Network.insertComponent` | none today (no directed test) | Reject the component wholesale with a diagnostic (the honest answer; the register offers `continue` as an equal and it is not). Hard to reach from DSN, **live for the KiCad-JSON reader**, which has no such guard. | **FIX** | U |
| **#211 + #45** | T1 | The trace arm of `reduceNetsOfRouteItems` puts its `break` **outside** the net loop where the via arm's is inside, so one visit can strip **every** net from a route item; `assignNetNo` on a multi-net item overwrites only `netNumbers[0]`, and `removeFromNet`'s loop has no `break` so a duplicate is removed at its **last** occurrence. | `reduceNetsOfRouteItems`; `Item.assignNetNo`/`removeFromNet` | `crates/fr-board/tests/board.rs` (`one_visit_reduces_two_nets_…`, three `assign_net_no_*`) | Move the break inside the net loop; replace the whole array or refuse the call; add the missing `break`. Land together — #211 is what creates multi-net items. Note the port reproduces Java's placement *deliberately* (a Plan 7 Task 8b parity repair), so this moves it back. | **FIX** | U (latent: no corpus item is on two nets) |

### 3.4 Reachable crashes on a real board

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#185** | T1 | `insertForcedTracePolyline` dereferences a possibly-`null` `newTrace` at `:756` and guards **the same variable** at `:791`; no `catch` covers `:756`, so the nearest handler is a bare `FAILED` — **the whole connection is abandoned where the guard would have skipped one segment.** | `RoutingBoard:756` vs `:791` | `crates/fr-router/src/board_ext/routing_board_ext.rs` (`expect`) | Move `:791`'s null test up to `:756`. Latent on all 1 621 rows of `p6t15b-insert-forced.txt` → needs a **new synthetic fixture** (a resample bringing a polyline's two ends together). | **FIX** | U (new fixture) |
| **#181** | T1 | `calculateNextTraceCorners` builds a `FloatLine` from a possibly-null corner and dereferences it one line later; the author guarded the same value 40 lines down. `autorouteConnection` catches it and degrades the whole connection to `FAILED`. | `FoundConnectionLocatorAnyAngle:287` | `crates/fr-router/src/autoroute/path/locator_any_angle.rs` (panic) | Hoist the `resultCorner != null` test to `:287` and skip the correction loop. Needs a **new synthetic fixture** (two parallel lines whose intersection is null). | **FIX** | U (new fixture) |
| **#169** | T1 | `removeIncompleteExpansionRoom` dereferences a lazily-created list where its three siblings guard; `ExpansionDrill.calculateExpansionRooms` uses the bare constructor, so the NPE is swallowed by #166's `catch` and read as "blocked". **On an engine that has never had an incomplete room added, no drill can ever be built** — JVM-verified: 0 drills vs 13, 28 swallowed NPEs. | `AutorouteEngine.removeIncompleteExpansionRoom` | `crates/fr-router/tests/drill.rs::a_virgin_engine_yields_no_drills_at_all` | Guard the field **and** have `ExpansionDrill` call `addIncompleteExpansionRoom`. Both. | **FIX** | U, R likely |
| **#168** | T1 | A cancelled `splitToConvex` makes `DrillPage.getDrills` throw **and** leaves the page memoised as having **no drills**, so a page interrupted once answers "no drills here" for the rest of the connection — **silently removing every via candidate on it.** | `DrillPage.getDrills` | `crates/fr-router/tests/drill.rs::split_to_convex_stops_when_the_stop_check_trips` | Install the list only after the split succeeds. | **FIX** | U |
| **#173** | T1 | `AutorouteControl.initNet`'s null-net arm completes only for `netNumber <= 0`; a **positive unknown net** throws two lines later, and `RoutingBoard.java:1023` builds a control from a pin's net number — so a stale net number kills the connection. | `AutorouteControl.initNet` | `crates/fr-router/tests/control.rs::a_positive_net_the_board_does_not_have_throws_like_java` | Give the null-net arm its own half-width fallback (better than moving the test: the current arm reads net 1's widths, which is itself arbitrary). | **FIX** | U |
| **#67, #123, #47, #49, #42, #43, #52, #22, #24, #25** | T1 | Ten unguarded dereferences and indices on paths a real design reaches: `Integer.parseInt` on `host_version`'s first digit run (KiCad-facing, thrown out of a predicate every board load calls); two cost accessors with no null guard where their two siblings answer `1.0`; `changeSide` on an unplaced component (NPE **after** flipping `onFront`, leaving it half-mutated); `Components.get(0)` on the "no component" sentinel; `Packages.get`/`LogicalParts.get` with no bounds check where `Padstacks.get` has one; `removeViaPadstack` before any via padstack exists; `Pin.getTraceExitRestrictions` dereferencing the component before its own null guard; `Polyline.removeOverlaps` reading index −1 (~11 % of random small-pool line arrays; six lines `h,v,h,v,h,v` is minimal) which **aborts the routing pass**; `TileShape.rotateApprox`'s two-corner branch at an invalid index; `Polyline.cornerCount()` returning **−1**, which flows into `new IntPoint[-1]`. | ten sites, all pinned | ten `#[should_panic]`/`expect` reproductions in `fr-board`, `fr-settings`, `fr-geometry` | Add the guard the sibling already has, at the defect — never a `catch_unwind` wrapper (Plan 7 ruling 7 / scan ruling 9 still binds: do not add recovery Java lacks; a guard is not recovery). #47 also flips `onFront` **after** the guard. #22 changes what a degenerate polyline normalises to, so it is the one with blast radius. | **FIX** | U ×10; #22 R/B likely |

### 3.5 The KiCad JSON reader — a real design cannot be told it is malformed

New family; the roadmap predates the KiCad reader entirely.

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#282 + #283 + #287** | T1 | A `null` name or a `null` **array element** is stored verbatim and crashes hundreds of lines later, inside a JDK collection: a null layer name dies at `:545` only if the board also has pads with a non-empty `layers` list; a null net name dies inside `Nets.get`'s walk; a null component `reference` dies inside `ConcurrentSkipListMap.put` via `Component.compareTo`. One tolerant site (`equalsIgnoreCase(null)` is `false`) against three throwing ones, in one reader. | `KiCadJsonReader.java:114, :131, :543-549, :625-634, :678, :288, :940` | `crates/fr-dsn/src/kicad/{reader.rs,dto.rs}` — `Option<Vec<Option<String>>>`, `net_name_is_null`, `java_nets_get`'s NPE, the `:625` early return | **Validate at the DTO boundary**: reject a null name and a null array element with a diagnostic naming the file, the section and the offending object. That deletes the null side-tables, the NPE emulation and half the XDIFF rows. | **FIX** | U, X (`p8t8` stems become rejections) |
| **#286** | T1 | `DrillItem.tileShapeCount` returns a **negative** number for a padstack with no shape on any layer (`toLayer - fromLayer + 1` = `-layerCount`), and the insert then allocates `new TileShape[-n]`. Reachable from a pad whose `layers` match nothing and from a via with `startLayerIndex > endLayerIndex`. The message is the bare number: `Exception occurred: -2`. On the `importSession` path Java keeps a via that is in the item list and in **no** search tree. | `KiCadJsonReader.java:555-557, :705-707`; `DrillItem.java:203-207`; `Padstack.java:137-152` | `crates/fr-dsn/src/kicad/reader.rs::java_drill_item_tile_shape_count`; `kicad_writer.rs`'s XDIFF row | Reject an all-`null` shape array in `Padstacks.add`, with a message naming the pad. Then the half-inserted-via XDIFF disappears rather than being pinned. | **FIX** | U, X (`p8t10` XDIFF retires) |
| **#284** | T1 | The generated padstack key encodes **neither the layer span nor the drill**, and the lookup is case-insensitive — so the second pad to ask for a name **silently inherits the first pad's shapes**. Measured: a middle-layer 1×1 pad gets a three-layer padstack; a drilled and an undrilled pad of the same size share one padstack and the first one's `attachAllowed`; `Round` names drop `size.y` entirely. It also **masks #286**. | `KiCadJsonReader.java:560-564, :857-890`; `Padstacks.java:25-32` | `crates/fr-dsn/src/kicad/reader.rs` `:561-564`, `get_by_name` | Key the padstack on the **shape array + layer set + drill**, and keep the generated name as a display artefact. The name reaches the SES, so this re-baselines every KiCad-sourced output. | **FIX** | C (2 stems), X (`p8t7`, `p8t8`, `p8t10`) |
| **#285** | T2 | The per-component `catch (Exception)` is a **package-deduplication fallback**, not a component skip: a `null` pad name throws inside `arePackagePinsIdentical` and the ladder then adds a package under the raw `footprint` string — so a board whose pads have no `name` gets **one duplicate package per component** (three components → three `NONAME` packages), silently. | `KiCadJsonReader.java:580-619, :892-924` | `crates/fr-dsn/src/kicad/reader.rs::are_package_pins_identical` (`Result<bool, JavaNpe>`), the `package_pin_names` side table | Falls out of #282's DTO validation: with pin names non-null the fallback is unreachable and the side table goes. Null-check the pin name in the comparison as well. | **FIX** | U, X (`p8t8`) |
| **#280** | T2 | The **net numbers** of every auto-registered net are `java.util.HashSet` iteration order, i.e. a function of `String.hashCode` — and that numbering is what every `netNumbers[]`, every DSN `(net …)` scope and every SES wire carries. `Issue649`'s thirteen pad nets come back in nothing like declaration order. Deterministic, but arbitrary; the port reproduces it by **rebuilding `HashMap`'s bucket layout**, with a `debug_assert!` guarding treeification (peak bucket 4 against the 9 a treeify needs). | `KiCadJsonReader.java:456, :490-496`; `Nets.java:89` | `crates/fr-dsn/src/kicad/reader.rs::java_hash_iteration_order`, `JavaStringSet` | Use insertion order (`LinkedHashSet`). One word in Java; in the port it **deletes the whole `HashMap` emulation** and the treeification hazard the handoff parks at §6 task 9. Re-baselines every KiCad-JSON board's net numbers and therefore its SES. | **FIX** | C (2 stems), X (`p8t7`, `p8t8`) |
| **#279, #277** | — | `readBoard` catches **`Throwable`** (so a `StackOverflowError` is reported as a malformed file) and its `ParseError.detail` is the JSON parser's own prose, including the JVM's helpful-NPE string. `DsnReader` by contrast has **no** blanket catch at all. | `KiCadJsonReader.java:76, :78, :746-750` | the port has no blanket catch; five explicit `ParseError` sites, one `XDIFF` row for the syntax-error prose | No port work; the detail-prose divergence is already recorded. Upstream: catch `Exception`, and give `ParseError` a machine-readable cause. #282's fix makes four of the five sites diagnostics rather than emulated NPEs. | **NOT-A-FIX** | — |

---

## 4. Tier 2 — routing quality

### 4.1 The two measured Java regressions — the strongest quality evidence in the catalogue

**Evidence: `benchmark/reports/java-regressions-2026-09.md`** — 605 PCBench boards (real,
human-routed KiCad designs whose references are routing-DRC-clean and fully connected under their
own rules), judged by KiCad 10 DRC with each board's own rules, `-mp 10`, 5-minute cap,
single-threaded, one isolated config dir per run. The router is deterministic, so every delta is
exact, not statistical, and both root causes were confirmed by **ablation builds at HEAD**.
v2.1.0 is the high-water mark (small tier: connected 0.81, DRC-clean 0.92, clean pass 0.76);
HEAD measures 0.76 / 0.73 / 0.60. The loss decomposes into exactly the two rows below, and the
two-ablation build — sort restored **and** fallback disabled — measures **better than v2.1.0**
(0.77 / 0.79 / 0.96 small; clean pass 0.30 vs 0.20 large).

These two carry more quality evidence than every other row in this document combined, and neither
needs a new fixture, a new harness or a discovery phase. **They should be the plan's first two fix
tasks.** Neither has a register row yet — they are Java *regressions* found by the benchmark suite,
not quirks found by reading the source — so Plan 9 allocates them the next two free register ids.

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **new — R1** | **T2 rank 0** | **The shortest-airline-first ordering of the items to route was deleted.** Commit `933d2980` ("Remove useSlowAlgorithm parameter from autorouter", 2026-01-14, shipped in v2.2.0) removed `autoroute_item_list.sort(Comparator.comparingDouble(this::calculateItemDistance))`, with the note "Disabled in v2.3 because it negatively impacts convergence compared to v1.9 (natural order)". **The data contradicts the note.** Fully-connected drops 0.81 → 0.75 at v2.2.0 and never recovers; 17 probe boards v2.1.0 completes are left unrouted by every 2.2.x. Restoring the sort alone at HEAD returns connectivity to **0.76 → 0.82** and is *slightly faster*. Neutral within noise on medium/large. The port transcribed HEAD's state faithfully: `getAutorouteItems` returns the list in `board.itemList` **descending-id** order (quirk #63) and nothing sorts it. | `BatchAutorouter.getAutorouteItems` (`:345-409` — the sort is gone from HEAD, not commented); `AutorouteAirlineCalculator.calculateItemDistance:162-177` still exists and has **no caller** | `crates/fr-router/src/pipeline/batch_autorouter.rs:657-781` (`autoroute_items` — returns unsorted at `:781`); `crates/fr-router/src/pipeline/airline.rs:23-25`, three `// not ported: … no caller` lines for `calculateItemDistance`, `calculateMinDistance`, `getItemReferencePoint` — **caller-less precisely because this sort was deleted**; consumed at `pipeline/pass_runner.rs:168` | Port the three caller-less `AutorouteAirlineCalculator` methods (`airline.rs`'s own roster names them) and sort the work list ascending by `calculateItemDistance` before the pass runner walks it. **Unconditionally** — no tier measured worse with it — with the tie order left as the descending-id walk so the sort stays a stable refinement of today's order. The three `// not ported:` lines become ported methods and their greps in the roster must be re-stated. | **FIX** | **B, R, C** (every routed stem), X (`p6t1`, `p7t1`, `p7t2`, `p7t9`, `p8t1`) |
| **new — R2** | **T2 rank 0** | **The micro-neckdown fanout fallback ignores the board's minimum track width.** Commit `f31a0c84` ("Add micro-neckdown fanout fallback for trace insertion", 2026-05-19, shipped in **2.3.0**, i.e. the port carries it) retries a failed 2-point fanout insertion at the pin's neckdown half-width, then `3/4`, `3/5` and `1/2` of the class half-width, "keeping the same clearance class" and checking nothing against the design rule. On any board whose net-class width **equals** its minimum width — very common — it emits sub-minimum traces: `track_width` violations on **31/146** small boards, DRC-clean **0.94 → 0.73** (small) and **0.93 → 0.40** (large). Because a clean-pass metric weighs a violation like an unrouted net, the fallback converts "one net open" into "board fails DRC", and on the small tier it recovers **no** connectivity at all. Its benefit is real only on large boards (connected 0.17 → 0.33). | `FoundConnectionInserter.insertFanoutMicroNeckdown:455-520` (the candidate set at `:462-471`, the loop guard at `:473` — `candidateHalfWidth <= 0 \|\| >= baseHalfWidth`, and nothing else) | `crates/fr-router/src/autoroute/path/inserter.rs:488-…` (`insert_fanout_micro_neckdown`, the candidate list at `:509-…`, transcribed including Java's `wrapping_mul`), called from `:394` | **Guard, do not revert** — the benefit on large boards is real. Skip every candidate half width below the board's minimum: `fr_board::BoardRules::get_min_trace_half_width()` (`BoardRules.java:37`, the minimum over the declared net-class widths) — **not** `Board::get_min_trace_half_width`, which is a running minimum over the traces already inserted and would ratchet itself down. When the class width already **is** the minimum, the whole fallback is skipped and the connection fails honestly. | **FIX** | **B, R, C** (every stem with SMD fanout), X (`p6t1`, `p7t5`, `p8t1`) |
| **new — I1** | T2 (investigation) | The **same commit** `933d2980` also removed the exhaustive ("slow") search tree that ran on every 4th pass (`useSlowAlgorithm = passNo % 4 == 0` → `false`). **The report does not implicate it** — the sort ablation alone recovers v2.1.0's connectivity — but the flag's remains are still in both trees, and the "slow" tree is the *base* `ShapeSearchTree`, i.e. the one whose `completeShape` **keeps** the room the 90-degree override drops (**#159**). So an every-4th-pass slow tree was, among other things, a periodic workaround for #159. | `BoardRules.java:48, :395, :404` — `useSlowAutorouteAlgorithm` has **no reader** in the whole tree; `SearchTreeManager.java:148-160`'s third arm is the tree it selected | `crates/fr-board/src/rules/board_rules.rs:61, :514, :519` — the same dead accessor pair, ported | **Investigation row, not a fix.** After #159 lands, measure whether a periodic exhaustive tree still buys anything; if not, delete the flag in both trees. Do not restore it blind — it is a 4× cost on every 4th pass with no measured benefit. | **FIX** (investigation) | — |
| **new — I2** | T2 (investigation) | **Via inflation.** Between v2.2.4 and v2.3.0 via usage roughly **doubles** (0.37× → 0.71-0.94× of the human reference count) alongside a ~25 % slowdown — the report reads it as the "recovery" work after R1's ordering regression trying harder with more vias. Not root-caused. | not localised | — | Re-measure **after** R1 lands: if the inflation is the ordering regression's downstream compensation, restoring the sort should deflate it on its own. If it survives R1, bisect v2.2.4→v2.3.0 with the suite. Via count is a first-class metric in the acceptance table (§7.4), so this is measured either way. | **FIX** (investigation) | — |
| **bisect note** | — | The first bisect landed on `cbca03ee` ("Improve unconnected items detection in DRC"), whose Manhattan `half_width + 1` contact tolerance broke the same probe boards; it was reverted before v2.2.0 shipped (`6182f236`), and re-bisecting with the tolerance neutralised isolates `933d2980`. **Its DRC-side grouping logic survives at HEAD** and is worth a read against the DRC cluster (§5). | `cbca03ee`, `6182f236` | — | No fix; a pointer for whoever takes §5's unconnected-items rows. | **NOT-A-FIX** | — |

**The report's four secondary findings, cross-referenced rather than duplicated:**

* **Self-report vs DRC disagreement on ~45-63 % of boards** (e.g. per-pin vs per-pair hole-clearance
  counting) — *"external scoring should not trust the manifest numbers"*. That is this catalogue's
  DRC cluster (**#152** counts every `Pin` a hole, **#146** double-reports dangling traces, **#153**'s
  never-reset minimum) meeting the statistics cluster (**#82**/**#147** under-report incompletes,
  **#194**'s net-index-0 fanout count, **#195**/**#196**'s non-summing lengths and negative box).
  No new row; it is corpus-scale confirmation that §5 and §6.2 are worth doing, and it is the reason
  the acceptance gate in §7.4 uses the **referee's** numbers and never the manifest's.
* **`passes_completed` misreports 1** while the logs show 18-30 passes — that is **#267** exactly,
  same field and same mechanism (the optimizer's `setCurrentPass(1)` overwrites the router's count),
  now confirmed at corpus scale. **The port reproduces it**, so the port's own manifests are equally
  wrong; #230 is the neighbouring off-by-one, a different disagreement in the same field.
* **Crash on non-DSN input** — a binary file at `-de` dies with an NPE in
  `RouterSettings.applyBoardSpecificOptimizations` instead of a parse error. **The port already
  answers this**: `commands/route.rs` step 9a parses first and exits 1 with the loader's message
  (quirk #244's totalization). The residue is #274 — the message names a format the user never
  typed, three steps after the mistake.
* **2.2.x splits `-de` paths on spaces** — fixed by 2.3.0, which is the port's baseline. No row.

### 4.2 The optimizer stage, and the pass loop that stops it (highest impact — new)

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#227** | **T2 rank 1** | **The optimizer stage runs, visits every item, and changes nothing** after any ordinary router run. Both stages share one stop flag and nothing lowers it; every ordinary exit from the pass loop raises `AUTO_ROUTER_ONLY` (#214's five arms), so `runOptimizationStage`'s `isStopRequested()` (ALL) lets the stage start while `autoroutePassesForOptimizingItem`'s `isStopAutoRouterRequested()` runs **zero** passes per item. Every item rips, measures worse, and is restored. The stage costs one whole-board deep copy per item and produces the board it was given. Measured: six `OPT-ITEM` lines, all `improved=false`, board shape identical. | `RoutingPipeline.java:81-119`; `AutorouteBatchLoop.java:271, :311, :318, :474, :505`; `BatchAutorouter.java:268`; `BatchOptimizer.java:171` | `crates/fr-router/src/pipeline/optimizer.rs::run_batch_loop`, `pipeline/run.rs`; `tests/optimizer.rs::an_auto_router_only_stop_leaves_every_item_rejected` | Give the optimizer a **stage-scoped stop**: reset the flag in `runOptimizationStage`, or have `autoroutePassesForOptimizingItem` read a stage flag rather than the job's. This is the single largest quality change in the catalogue — an entire optimization stage begins working. It **must** land with #202 (which decides whether the stage runs at all after `--max-items`) and it changes the run time of every board. | **FIX** | **B, C, R** — every routed stem; X (`p7t9`, `p7t8`, `p8t1`, `p8t2`) |
| **#202** | T2 | `--max-items` calls `requestStop()` (**ALL**) where `--max-passes` calls `requestStopAutoRouter()` (**AUTO_ROUTER_ONLY**), and `runOptimizationStage` returns early on the former — so a `--max-items` run **skips the optimizer entirely** and writes an unoptimised board, while `--max-passes` optimises normally. The log line says "Stopping auto-router"; the optimizer is not the auto-router. | `Freerouting`/`RoutingPipeline.runOptimizationStage` | `crates/fr-router/tests/stop_and_progress.rs`, `tests/pass_runner.rs` | Have the `maxItems` site call `requestStopAutoRouter()`. **Worth nothing on its own until #227 lands** — today both paths produce the same board. Keep the three-state stop (`CancelToken`→`RouterStop`); do not collapse to a bool. | **FIX** | B/C on `--max-items` runs only |
| **#213** | T2 | `getAutorouteItems` appends an item **once per qualifying net**, and the pass runner then runs a fresh `0..netCount()` walk per appearance — so a two-net item is routed **four times in one pass**, on net indices unrelated to the ones that qualified it, each against the board the previous one left. | `BatchAutorouter.java:390`; `AutoroutePassRunner.java:202, :207` | `crates/fr-router/tests/pass_runner.rs::a_two_item_is_routed_four_times` | Append outside the net loop and carry the qualifying net with the item (`Vec<(ItemId, NetIndex)>`). The roadmap deferred it as "a different program"; with parity gone it is simply correct. Latent on the corpus (no multi-net items). | **FIX** | U; B/C likely |
| **#214** | T2/T4 | A **normal** end of routing reports `CANCELLED`, not `FINISHED`: `:571` fires `FINISHED` only when the stop flag is still clear, and every ordinary exit raises it first. A CLI run that does exactly what it was asked ends `CANCELLED`, and no consumer can tell it from a user cancellation. | `AutorouteBatchLoop.java:571-585` | `crates/fr-router/src/pipeline/batch_loop.rs`; `tests/batch_loop.rs::a_normal_finish_reports_cancelled_not_finished` | Carry the **exit reason** out of the loop (`BatchLoopResult` is already shaped for it) and report `FINISHED` for `maxPasses`/completion. Prerequisite for #227's stage-scoped flag — both are the same "one flag means five things" defect. | **FIX** | X (`p7t9` RESULT line), C (log) |
| **#215** | T2 | The `else if` that resets the stagnation counter for a fully-routed board hangs off the `currentPass >= 8` guard, so it fires on passes 1-7 — where the counter cannot yet be incremented — and never afterwards. The counter first reaches its ten-pass window at pass 17 and the global tracker fires at 18. | `AutorouteBatchLoop.java:422, :509-517` | `crates/fr-router/src/pipeline/batch_loop.rs::stagnation_guard`; `tests/batch_loop.rs` | Move `:509-517` inside the `>= 8` arm as a third branch of the score test, which is what its own comment describes. Changes which passes reset the counter on every board that completes early → changes where long runs stop. | **FIX** | B/C on ≥ 8-pass runs |
| **#230 + #267** | T3/T4 | `job.currentPass` is written by **two** loops (router from 1, optimizer from 0) and the manifest reports whichever wrote last under a key naming the **autorouter** — so one optimizer pass overwrites a three-pass routing stage with `1`. And on a `maxPasses`-capped exit the field is one behind the loop's own local, which the final event carries, so the two APIs disagree by one at the exact moment the run stops. | `AutorouteBatchLoop.java:270-276, :521, :572-584`; `BatchOptimizer.java:196`; `RoutingResultManifest.java:124-126` | `fr_router::pipeline::{BatchLoopResult, OptimizerResult}::last_reported_pass`, `PipelineResult`; `commands/route.rs` | Give the two stages **separate fields** and fill `phases.optimizer.passes_completed` (the key already exists and is always `{}` — #254). Land with #254. | **FIX** | C (manifest), X (`p8t2 e2e`) |
| **#217, #216, #225, #226, #228** | — | Five dead or unreachable arms in the same two files: the board-rank break whose limit **is** the history's own cap (`rank > 30` has no solution, and the constant's comment has the reason backwards); a `HashSet` whose only two readers are commented out but which is still allocated and cleared twice; an empty `if` with an impossible third conjunct; a user-fixed guard that cannot fire because `getConnectionItems` already filters non-routable items; and a `-1` sentinel a real pass improvement can equal (latent — needs a pass that drives a positive score to hard zero). | `BatchAutorouter.java:38-40, :271-273`; `AutorouteBatchLoop.java:249, :315-320`; `BatchOptimizer.java:212-230, :434-440` | `batch_loop.rs::rank_limit_exceeded` (kept with a marker), `batch_autorouter.rs` (omitted), `optimizer.rs` | Delete the dead arms and their constants. **#217 is a product question, not a cleanup:** making the rank break reachable would *stop runs earlier*, so either set the limit strictly below the cap **with an A/B** or delete the branch. #228 gets its own `bool` instead of a magic double. | **NOT-A-FIX** (#216/#225/#226) / **FIX** (#228, #217 as a decision) | U |

### 4.3 Rooms and doors the maze never gets

Unchanged from the roadmap's §3.1 ranking; the register has not moved these.

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#159** | T2 | `ShapeSearchTree90Degree.completeShape` **drops** a room it decided to ignore where the base class and the 45-degree override keep it — and `completeExpansionRoom` passes exactly that pair on **every** room completion. On a 90-degree board a room whose only overlap is the door it came through **expands to nothing**. Measured `[4, 4, 0]` across the three regimes. | `ShapeSearchTree90Degree.completeShape` | `crates/fr-router/tests/tree_ext.rs::only_the_90_degree_override_drops_a_room_it_ignores_by_shape` | Add the base class's fallthrough verbatim — the 45-degree sibling is the specification. Needs a **90-degree fixture**; nothing in the corpus exercises the regime. | **FIX** | R/B on 90° boards (none committed yet), X (`p6t2`) |
| **#160 + #161** | T2 | `SortedRoomNeighbour.compareTo` is **not a total order** and the `TreeSet` it feeds silently drops elements it calls equal — **a door the room really has is never built.** Measured **481 drops in 2 000 cases**. #161 is the same comparator's final tie-break subtracting a **room** id from an **item** id. | `SortedRoomNeighbour.compareTo` | `crates/fr-router/tests/sorted_neighbours.rs` (two tests) | Make it a total order: compare last corners whenever the first-corner distances tie, apply or drop `compareFrom` consistently, and compare the object **kind** before the id. Then `JavaTreeSet` → `BTreeSet` and no neighbour is lost. **One fix, both rows.** | **FIX** | **R, B** (large), X (`p6t3` modes 1-3) |
| **#164** | T2 | `removeCompleteExpansionRoom`'s `otherRoom(room)` binds the **narrowing** overload at compile time, so the `null` check below skips **every door whose far side is an incomplete room — which is most of them**. The skip is what keeps the method alive: the code past it indexes `touchingSides[1]` with no length check. | `AutorouteEngine.removeCompleteExpansionRoom` | `crates/fr-router/tests/engine_rooms.rs::init_connection_on_a_new_net_drops_the_net_dependent_rooms` | Take an `ExpansionRoom` parameter **and** length-check `touchingSides`. **Both** — fixing only the overload turns a silent skip into an `ArrayIndexOutOfBoundsException`. | **FIX** | R, B, X (`p6t3`) |
| **#163** | T2 | `Sorted45DegreeRoomNeighbours…OfObstacleExpansionRoom` never advances `currentCorner`, so the degenerate-side guard compares every side's end corner against the corner the walk **started** at, and the **last side is always skipped** — eight-sided obstacle rooms get seven doors. JVM-verified. | `Sorted45DegreeRoomNeighbours` | `crates/fr-router/tests/sorted_neighbours_regimes.rs` | `currentCorner = nextCorner;` at the foot of the loop (7 → 8). | **FIX** | R, B on 45° boards, X (`p6t3` mode 8) |
| **#171 + #170** | T2 | A four-key tie in `MazeListElement.compareTo` answers `0` and `TreeSet.add` **discards the newcomer whole** — a different backtrack path at the same cost is lost, not merged — and `door.getId()` is a hash, so two *different* doors collide into the tie. #170: a `NaN` `sortingValue` falls through to the next key and the relation stops being transitive. | `MazeListElement.compareTo` | `crates/fr-router/tests/maze_list_element.rs` (two tests) | Add the remaining fields to the comparison (or keep the cheaper backtrack explicitly); **reject** a non-finite `sortingValue` at the three `add` sites rather than ordering it (the roadmap's correction — a NaN cost is an upstream bug, not a thing to sort). Land **after** #160/#161, which changes which elements reach the queue at all. | **FIX** | R, B | 
| **#165 + #166** | T2 | `completeExpansionRooms` is **not** the set of complete rooms that exist: rooms are constructed before they are known to survive, and two paths abandon them **still wired to live doors** — never validated, never invalidated, never removed from the tree, and `completeExpansionRoom`'s scan can hand `completeShape` a room that is not in the tree it is querying. #166's `catch` returns an **empty** collection for rooms already committed to the database. | `SortedRoomNeighbours`; `AutorouteEngine.completeExpansionRoom` | `crates/fr-router/tests/engine_rooms.rs` (two tests) | Take the room id after the commit; give `addCompleteRoom`'s `null` path a `removeAllDoors`; hoist `result` out of the `try` and return it from the `catch`. | **FIX** | R, B |
| **#178** | T2 | `MazeSearchEngine.init` ignores the `boolean` its own overridden `add` returns, so `startOk` can be `true` with an **empty** queue — the caller pays for a whole engine construction, a `reduceTraceShapesAtTiePins` pass and a full round of room completion to learn the queue was empty. | `MazeSearchEngine.init` | `crates/fr-router/tests/maze_search.rs` | `if (mazeExpansionList.add(newListElement)) { startOk = true; }` — makes `getInstance` answer `null` for a fanout window that is too tight, which is what "initialisation failed" means. | **FIX** | U; R likely |
| **#156 + #167 + #158** | T2 | Three room/page identities that are **hashes over mutable state**: `ObstacleExpansionRoom.getId` packs the shape index into the item id with an **or** (index ≥ 1024 aliases, item id ≥ 2²¹ overflows) and that id is the third sort key of `MazeListElement.compareTo`; `DrillPage.getId` hashes a field `getDrills` overwrites, so a queued page silently changes its own sort key; `IncompleteFreeSpaceExpansionRoom.getId` NPEs on the whole-plane room and moves when its shape is replaced. | three `getId`s | `crates/fr-router/tests/{expansion_rooms.rs,drill.rs,incomplete_room.rs}` | **One fix:** every expandable object gets a stable, injective id — a per-engine counter, as `CompleteFreeSpaceExpansionRoom` already has. Ids feed the maze tie-breaks, so routing moves; the current ids are provably wrong, not merely arbitrary. | **FIX** | R, B |
| **#192** | T2 | `DrillPageArray.overlappingPages` mixes an `int` lower bound with a `double` upper bound, so a shape whose upper edge lands exactly on a page boundary **stops one page short** — a coverage *and* an ordering input, because page ids order the maze queue. | `DrillPageArray.overlappingPages` | `crates/fr-router/tests/maze_drills.rs` | Compute both bounds the same way and decide deliberately whether a boundary-touching shape reaches the next page. | **FIX** | R on layer-changing connections |
| **#193** | T2 | Three HEAD-only guards that each admit an item's tree-shape indices go **stale while the search is running**; they silently `continue`, and one **resizes** `expansionRoomArr` mid-search. The corpus reaches all three. *The guards convert a corrupted search into a quietly worse route.* | three guards, HEAD-only | ported with the guards intact | **A discovery workstream, not a fix.** First deliverable: an instrumented run recording *which* mutation invalidated *which* index, over the six router stems. Start it early — its answer may subsume several §4.3 rows. Until it is answered the guards stay. | **FIX** (discovery) | — |

### 4.4 Wrong obstacle and clearance decisions

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#50** | T2 | `changeConductionIsObstacle`'s guard is `if (getIgnoreConduction() != value) return;` — it only does anything when the two are already out of step — and it ends by storing the **negation** of what it just wrote into every signal-layer conduction area, so the flag is a **latch that alternates** rather than a mirror. Non-signal-layer areas are skipped, so a power plane's `isObstacle` never changes. **This flag decides whether copper pours obstruct foreign-net routing** — the most user-visible boolean on a KiCad board with ground pours. | `RoutingBoard.changeConductionIsObstacle` | `crates/fr-board` `Board::change_conduction_is_obstacle` | Decide what the flag means and make it mean it: guard `==`, store `value`. Needs a **new fixture with a signal-layer pour and a foreign-net trace**. The roadmap is right that the register under-ranks it. | **FIX** | B/C likely, U (new fixture) |
| **#65** | T2 | `ShapeTraceEntries.storeItems`' precedence bug (`&&` binds tighter than `\|\|`) skips a `ComponentObstacleArea` **unconditionally**, so **a component keepout can never block a via placement**. | `ShapeTraceEntries.storeItems` | `crates/fr-board/src/board/shape_trace_entries.rs` (`// Java bug:` note) | `!isPadCheck && (a \|\| b)`. For a KiCad user this is the difference between a via landing inside a courtyard keepout and not. | **FIX** | B, R |
| **#69** | T2 | `storeTrace`'s three-way block test compares `contactItem.clearanceClassIndex() != contactTrace.clearanceClassIndex()` where `contactItem` **is** `contactTrace` — always false. **A contact whose clearance class differs never blocks a shove**, so the router shoves copper across a clearance-class boundary. | `ShapeTraceEntries.storeTrace` | same file | Third disjunct becomes `trace.clearanceClassIndex()` (symmetry with the second gives the intent). New directed test: two contacting traces in different clearance classes. | **FIX** | B, R |
| **#72** | T2 | `splitInsideDrillPadProhibited`'s precedence bug tests the `lastCorner` half for **this** trace too, and for a foreign trace whose first corner did not match — either answers "split allowed" even when a pad was found. **So a trace may be cut inside a pin pad.** | `PolylineTrace.splitInsideDrillPadProhibited` | `crates/fr-board/src/board/trace_normalize.rs` | `currentTrace != this && (first \|\| last)`. Directed test: a trace crossing a pin pad. | **FIX** | B, R likely |
| **#174** | T2 | `TraceShover.check`'s via arm returns `false` **without setting `shoveFailingObstacle`**, where every other refusal records the culprit — so `MazeRipupResolver` is handed a **stale item, possibly from a different `check` call on a different net**. The field is never cleared on entry, so on a fresh board it can be null. **The router rips the wrong copper today.** | `TraceShover.check` | `p6t9` mode `inst`, whose `failing=` column carries leftovers | Set it to `currentShoveVia` **and clear the field on entry** (the register mentions the staleness but does not make it a fix; it should). | **FIX** | R, B |
| **#179** | T2 | `checkNeckDownAtDestPin` **never asks whether the pin is a destination pin** and returns on the first `Pin` target door the room lists, start pin or not — so **a start pin's neckdown silently shrinks the trace the search plans through a room it is only passing through**. | `MazeSearchEngine.checkNeckDownAtDestPin` | `crates/fr-router/tests/maze_expand.rs` | Test `isDestinationDoor()` inside the loop and `continue` rather than `return`. Then the name, the javadoc and the body agree. Widens traces the router was needlessly necking down — **read together with R2**, which is the other half of "the router necks down where it should not". | **FIX** | R, B |
| **#231 (+ #233)** | T2 | `applyCopperToEdgeClearanceOverride`'s guard makes the **default** `router.copper_to_edge_clearance_um` the one value that can be ignored: `=500` leaves a board with an explicit outline class alone while `=500.000001` or `=0` rewrites a whole clearance row and column and re-points the outline. On 15 of 16 corpus boards the guard cannot fire at all, so the plain `-de/-do` run every reference is generated from **does** carry a 500 µm board-edge keep-out. It reaches the output bytes (15 254 B vs 14 644 B on one stem) and is logged only at `debug`. #233 is the consequence for Java's own fixture suite, whose `TestingSettings` constructor zeroes the value — so **the Java suite measures a board no real run produces**. | `HeadlessBoardManager.java:466-552`, `:501-507`; `TestingSettings.java:20-26` | `crates/fr-board/src/board/clearance_override.rs`; `fr_router::pipeline::prepare_board`; `crates/fr-router/tests/fixtures.rs` passes `=0.0` deliberately | Make the option **continuous**: apply the configured value uniformly and drop the equals-the-default special case; key "leave an explicit class alone" on whether a source actually supplied the value (the ladder knows — the field is `null` until one does), not on numeric equality. Log at `info`. **Decide deliberately whether the 500 µm default should apply at all** — it is the single largest silent constraint on the corpus. | **FIX** | **B, C, R** (nearly every stem), U |
| **#35, #46, #175** | T2 | `LayerStructure.getSignalLayer(n)` out of range returns the **last layer of the whole stack**, signal or not; `ConductionArea.copy` returns **`null`** whenever `netCount() != 1`, including zero, where every other `Item.copy` produces an item; `DrillItemMover.check` — a *check* — **mutates its caller's** `ignoreItems` and the recursion re-enters. | three sites | `fr-board`; `crates/fr-router/tests/board_ext.rs` | Return `Option`; implement at least the zero-net copy (it needs no new logic); copy the collection unconditionally as `shoveVias` already does. | **FIX** | U |

### 4.5 Wrong geometry on the finished board

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#177** | T2 | `TraceShover.insert` dereferences `board.changedArea` with no null check and its own `catch` **hides the NPE**, so on a board not marking its changed area the substitute traces are inserted **un-normalized** — JVM-pinned: three traces where a marked board leaves one. `ForcedPadRouter.forcedPad` — the same loop — guards the identical call, so the two mutating halves of the shove disagree about the same board state. | `TraceShover.insert` vs `ForcedPadRouter.forcedPad:439-444` | `crates/fr-router/tests/forced_via.rs` (both sides pinned as literals) | Compute `optArea` the way `forcedPad` already does **and** narrow the `catch`. | **FIX** | R, B, X (`p6t10b`) |
| **#186** | T2 | `FoundConnectionInserter` hands `connectToTrace` a `Trace` **the insert has already split away**, so the stub is inserted against a polyline the board no longer holds and the two tail removals then **delete both halves of the split trace**. Measured: both halves of trace 4 gone, its line surviving only inside a combined trace. | `FoundConnectionInserter` | `crates/fr-router/tests/inserter.rs` (fails today on the id lookup and the post-loop snapshot — which is the fixed behaviour) | Look the trace up **by id** after the insert. The most visible geometry change in the register: it changes what the board looks like where a connection lands mid-trace. | **FIX** | R, B |
| **#187** | T2 | Each `connectToTrace` stub is sized from the **other** end's layer — the stub onto the target trace is inserted on the target's layer and sized from the *start* layer's `traceHalfWidth`. There is no reading under which the width belongs to the layer the copper lands on. Latent only because every corpus board shares a width across layers. | `FoundConnectionInserter` | `crates/fr-router/tests/inserter.rs`'s `diag` | Let `connectToTrace` take the width for the layer it has just computed. Needs a **new fixture with per-layer trace widths** — also the fixture #128 needs. | **FIX** | U (new fixture) |
| **#55** | T2 | All four `BoardOutline` transforms assign to the **loop variable** of an enhanced `for`, so `this.shapes` is never written: **the outline does not move, turn, rotate or mirror.** Only the lazily-built keepout follows, so the outline's curves and its outside-keepout disagree, and `boundingBox`/`lineCount`/`getShape` and the tree line bands keep answering from the untransformed shapes. | `BoardOutline`'s four transforms | `crates/fr-board/tests/areas_and_outlines.rs` | Write back into the array. *A board whose outline finally moves has a different routable region*, so the corpus re-baselines. | **FIX** | G, B likely |
| **#5** | T2 | `RationalPoint.perpendicularProjection` uses `add` where `IntPoint` uses `subtract` — a sign bug giving a **wrong projection for any line not through the origin**, reachable from `ShapeTraceEntries` via `TileShape.nearestBorderPoint`, i.e. the shove entry path. | `RationalPoint.perpendicularProjection` | `crates/fr-geometry/src/rational_point.rs` | One character. Highest value-per-character fix in the register; land it **before** #7/#68 (the entry point must be right before the entry side is). | **FIX** | B, R, X (`p2t11`) |
| **#7 + #68** | T2 | `IntBox`/`IntOctagon.borderLineIndex` are stubs that log and return `-1`, and the live caller `ShapeAndEntrySide` receives it; in the same file both dog-ear cuts are guarded by an **always-true reference comparison**, so a cut that removed nothing still sets `cutOffAtStart`/`cutOffAtEnd` and the `fromSide` search hunts for a border line that is not there. **One fix** — #68's search is exactly what #7's `-1` breaks; the register lists them separately and does not connect them. | two files | `p2t11` mode 5 pins all four combinations | Implement `borderLineIndex` geometrically against `borderLine(i)`; compare the shapes **by value**. | **FIX** | B, R, X (`p2t11`) |
| **#183** | T2 | `TraceTightenerAnyAngle.smoothenEndCornerAtTrace` reads `prevLineDirection` from the **same** line as `lineDirection`, so the `bend` arm needs two directions that two equal directions cannot satisfy — **the whole `bend` branch of the any-angle end-corner smoothener is unreachable.** | `TraceTightenerAnyAngle` | `crates/fr-router/tests/tightener.rs`'s `smooth` fixture | Read `lines[endLineNo - 1]`. Any-angle boards only, but every end corner on them. | **FIX** | R, B (any-angle stems) |
| **#48 + #57** | T2 | For a back-side item under `flipStyleRotateFirst`, `Component.rotate` adds `360 - angle` to the stored rotation and rotates the **location** by `angle`, so rotation and location disagree by `360 - 2·angle`; `ObstacleArea.rotateApprox` and `ComponentOutline.rotateApprox` have the identical split. **The component's outline and its pads disagree after a back-side rotation.** | three methods | `crates/fr-board`'s three `rotate*` tests | Decide which angle is intended and use it in **all three at once** — fixing one makes them disagree instead. | **FIX** | U, G likely |
| **#15, #16, #9, #13** | T2 | Four small geometry defects with real reach: `indexOfNearestCorner` seeds with `Double.MIN_VALUE` (the smallest **subnormal**), so a corner at distance exactly 0 is never nearest; `nearestBorderPointsApprox`' upward insertion shift copies the wrong element; `FloatPoint.circleCenter` divides by zero on horizontal input, giving `(x, NaN)` — the mechanism of #82; `LineSegment.stairApproximation45` calls a function of *x* with a **y**-coordinate. | four sites | pinning tests/comments in `fr-geometry` | `Double::MAX`; fix the shift and check `count > 1` callers; swap the point roles for the horizontal case (**lands with #82**); use `functionInYValueApprox` and verify against 45° output. | **FIX** | U; #9 with #82 |
| **#26, #88, #23, #11, #17, #18, #32, #188** | T2 | The long tail: `PolygonShape.area()` **always returns 0** (its guard is `<= 2` where `dimension()` never exceeds 2) and is reached from `DsnFile`, so plane autoroute settings derive from a zero board area; `PolygonPath.boundingBox` adds `+ offset` to the running maximum on every even index, growing the upper x bound by `width/2` per coordinate; `Polyline(Point,Point)` repeats the start's closing direction, giving the opposite closing line from `Polyline(Polygon)`; `Simplex.cutoutFrom`'s `prevDivisionLine` is never assigned so both merge branches are dead; `IntOctagon.contains(FloatPoint)` is inclusive on the border where `IntBox`'s is exclusive; `IntBox.divideIntoSections` skips the base class's `dimension()==2` filter; `Circle.translateBy(RationalVector)` returns **`this` unchanged** where every sibling throws; `new Polyline(Line[])` normalises **the caller's array in place**. | eight sites | pinning tests in `fr-geometry`/`fr-dsn` | As the register's columns state. #26 additionally needs `corners[len-2]` guarded for a 1-corner polygon. #26 and #93 change derived board-level numbers, so they re-baseline more than a unit test. | **FIX** | U; #26 G/B |

### 4.6 Polygon and circle implementations — a workstream, not a fix

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#28 + #29** | T2 | `PolygonShape.containsOnBorder` is a stub returning `false`, so `containsInside(p) == contains(p)` for every polygon; `cutout`, `enlarge`, `borderDistance`, `distance` and `Circle.nearestPointApprox`/`cutout` are unimplemented stubs returning `null`/`0`, so **`smallestRadius()` always answers 0 for a polygon** — and that feeds the clearance heuristics. KiCad exports polygon keepouts and zones routinely. | `PolygonShape`, `Circle` | `stubs_match_the_java_stubs` in `crates/fr-geometry` | **Implementations, not corrections** — budget them as their own workstream with their own directed geometry tests. Every polygon keepout's clearance behaviour changes. Pairs with #27 (the polygon-vs-polygon intersect) and #26 (`area()`), which are the same class's other three holes. | **FIX** | G, B likely, U |

### 4.7 Airlines, incompletes, score, and which board the run keeps

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#82 + #9** | T2 | Delaunay's bounding triangle is finite with two corners **exactly on the axes**, and every edge flip is decided by a float circumcentre that silently degenerates to `NaN` for three of six coordinate coincidences — answering "legal, do not flip". Ordinary axis-aligned input triggers it: **edges are lost at every grid size** (2×2 → 4/5, 5×5 → 50/56, 6×6 → 75/85), i.e. on every pad row, column and BGA field. Usually the MST just picks a longer airline, but on a 7×7 grid straddling the origin ~0.5 % of dense draws come apart and a witness pad gets **no incident edge at all** — so **incompleteness is under-reported.** | `PlanarDelaunayTriangulation` | `crates/fr-board/src/datastructures/delaunay.rs` (`square_pins_javas_four_edges`) | An **exact** `inCircle` determinant over `Point` (the `BigInt` machinery `sideOf` already uses) and a bounding triangle pushed out where no input can be collinear with it. #9's horizontal case is part of it. **Highest-value Tier-2 fix for a KiCad user**; re-baseline in the same commit, and expect the honest incomplete count to *rise*. | **FIX** | **D, C** (incomplete counts), X (`p2t13`, `p5t1`, `p5t2` + `AIRLINE_BUDGETS`) |
| **#147** | T2 | `NetIncompletes.Edge.compareTo` is **not injective** — its own comment says the four coordinate tie-breakers exist "so that edges with the same length are not skipped in the set", and they do not achieve it. Two candidate airlines between **different** items whose corners coincide pairwise compare `0` and `TreeSet.add` drops the second: **a via stacked on a pad, or two pads at one location.** `Signum.asInt` also maps **NaN** to 0. | `NetIncompletes.Edge.compareTo` | three unit tests in `crates/fr-drc/src/net_incompletes.rs` | Break the remaining tie on the two `NetItem` indices (already to hand at the construction site) and guard the NaN with `Double.compare`. Same channel as #82; land together and regenerate `AIRLINE_BUDGETS` reading every number that grew. | **FIX** | D, C, X (`p5t1`, `p5t2`) |
| **#197 + #198** | T2 | `BoardHistory.getMaxScore` seeds with **`0`, not `-inf`**, so an empty history can never trigger a restore and a negatively-scored one only against a board below zero; `restoreBoard` **sorts the list in place** under a *read* lock and `getRank` reports the current position, so a board's rank is insertion order until the first restore and score order after — and the pass loop **breaks** when `getRank > BOARD_RANK_LIMIT`. **How many restores have happened changes when the router stops.** | `BoardHistory` | `crates/fr-router/tests/board_history.rs` (three tests) | Seed `f32::NEG_INFINITY`; keep the list in score order at insertion so `getRank` is stable. These two decide the **final output** of every multi-pass run. Note the rank break is #217's dead arm — fixing #198 does not make it reachable; that is a separate decision. | **FIX** | B, C (multi-pass stems) |
| **#194** | T2 | `BoardStatistics`' fanout block decides `pinsToEscape` from **net index 0 only** while `total` counts every SMD pin with `netCount() > 0` and `isPinEscaped` is net-blind — so a pin connected on its first net and unconnected on its second is counted as needing no escape and `BatchFanout` **skips it**. | `BoardStatistics` | `crates/fr-router/tests/score.rs` | Loop the pin's net indices, as the item loops elsewhere in the same class. Needs a **multi-net SMD pin fixture**. | **FIX** | U (new fixture), X (`p7t7`) |
| **#148** | T3 | `AirLine.compareTo` compares **the net name alone**, so a `TreeSet<AirLine>` would collapse a net's 29 airlines into one; also an NPE waiting on a null `net`. Latent — nothing in the Java tree sorts or set-collects `AirLine`s. | `AirLine.compareTo` | `crates/fr-drc/tests/net_incompletes.rs` | Drop `Comparable` entirely, which the port already effectively does — or compare name, then both item ids, then the corners. | **FIX** | U |

### 4.8 Stale caches, and identities that move

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#51, #60, #56, #58, #41** | T2 | Five memos whose input moved behind them. #51: `DrillItem.clearDerivedData` resets two layer memos but **not** `precalculatedMinWidth`, so a via that changes side keeps the minimum pad width of a padstack it no longer has **for the rest of its life** — and `minWidth()` feeds the autorouter's **neckdown** decisions. #60: `PolylineTrace.rotateApprox` alone of the four transforms does not `clearDerivedData()`, so a rotated trace keeps search-tree tile shapes for its **pre-rotation** position — *a stale tree shape is a wrong-answer obstacle.* #56: `ComponentOutline.clearDerivedData` does not chain to `super`. #58: `setFlipStyleRotateFirst` clears no caches. #41: `ShapeTree.insert` returns early for a zero-shape object **before** telling it, so a stale entry array survives an "insert". | five sites | pinning tests in `crates/fr-board/tests` | Each is the one-line addition the register names. **#51 and #60 first** — they are the two that reach routing, and #51 compounds R2 and #179 (three independent ways the router necks down wrongly). | **FIX** | R, B (#51/#60); U (rest) |
| **#218** | T2 | `ViaRule.contains` is object **identity** (`ViaInfo` declares no `equals`), so `RoutingBoard.fanout`'s fallback merge appends a via **value-equal to one it already holds** whenever a `.rules` file re-declares a `(via …)`. The duplicate widens `viaInfos`, re-runs the `viaRadii` maximum and doubles the entries the maze walks. | `ViaRule.java:55-62`; `RoutingBoard.java:1026-1041` | `fr_board::ViaRule::contains` / `ViaInfo::is_same_object` (an identity serial, ruling AE's shape); `fr_router::board_ext::combined_fallback_via_rule`; `tests/fanout_order.rs` | Give `ViaInfo` value equality on (name, padstack, clearance class, attach flag) and the dedup does what the code reads as. **Deletes an identity-token mechanism the port only carries for parity.** | **FIX** | U; B likely on rules-carrying boards |

### 4.9 Fanout ordering, stagnation, and the ripped set

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#221** | T2 | `RoutingBoard.fanout`'s two-attempt strategy hands the retry the **same** `rippedItemList`, and `autorouteConnection` removes each ripped item's whole connection from the board — so **a successful retry destroys connections only the abandoned first attempt asked to rip.** | `RoutingBoard.java:1058, :1064-1085` | `fr_router::board_ext::RoutingBoardExt::fanout`; `tests/fanout_order.rs` (asserts the single binding — the retry fires on none of the eight `p7t5` boards) | Give the retry a fresh set and union it into the caller's only on success. T1-adjacent: it deletes live copper. | **FIX** | U; B likely |
| **#222** | T2 | The fanout loop's oscillation detector packs a pass into one `long` (`routedCount << 32 ^ viaCount`) — **a summary, not a board** — so two passes that escape *different* pins in different places read as "no progress" and the run can end while it is still moving. `getHash()`, the thing that actually answers the question, is taken three lines later. And the constant is a **repeat** count, so the break fires on the **fourth** identical pass while the log says three. | `BatchFanout.java:105, :128-148` | `FanoutLoopState::{board_state, after_pass}`; `tests/fanout.rs` (three pins) | Compare the board hash; rename the constant and fix the message. | **FIX** | B, C (fanout stems) |
| **#219 + #220 + #223** | T2 | Three fanout gates that answer arbitrarily: a pin alone on its net keeps `Double.MAX_VALUE` as its `distanceToClosestOnNet` and that **sentinel is a sort key** (two such pins tie and fall through to `pinIndex`); an **unrecognised** `pinSortingOrder` silently degrades the comparator to a fifth, undocumented ordering with no warning; and `canUseVias` hangs off `net != null`, so **a pin whose net number names no net skips the via check entirely** — the case the code knows least about is the one it does not check. | `BatchFanout.java:710-723, :741-777, :238-259` | `crates/fr-router/src/pipeline/fanout.rs`; `tests/fanout_order.rs`, `tests/fanout.rs` | `Option<f64>` for "no other pin on this net", sorted explicitly; validate `pinSortingOrder` in `RouterSettings.validate` (or fall back to `outer_first`); treat an unresolvable net as "cannot use vias". | **FIX** | B/C (fanout order), U |
| **#44 + #63 + #74** | T2 | The three load-bearing orderings. #44: `Item.compareTo` subtracts the wrong way round, so every `TreeSet` the search tree returns — the order the router visits overlapping items in — is **descending id**. #63: `UndoableObjects` is keyed by that comparator, so every walk of the board's item list is descending, and that decides the *structure* of every search tree `MinAreaTree` builds. #74: `PolylineTrace.change` compares `Line`s by **object identity**, so a freshly built polyline always differs at index 0, the "no change necessary" early returns are near-unreachable, and `keepAtStart/EndCount` decides how many tree leaves are reused rather than re-inserted — a value comparison disagreed on **1 403 of 2 358** calls on one board. | `Item.java:93-103`; `UndoableObjects.java`; `PolylineTrace.java:960, :972` | `crates/fr-board/src/ids.rs`, `items/mod.rs`, `board/mod.rs::items_in_board_order` (`.rev()` everywhere); `fr_geometry::Line`'s identity token | **Measure-only, and the last thing to touch.** No correctness argument stands behind any of the three: any total order is as valid, and "fixing" them re-baselines every reference for no predicted gain. #74 alone has a cost argument (a value comparison reuses *more* leaves). Do them **after** §4.1-§4.8, as one deliberate "ordering flip" task with its own A/B, or not at all. Note `Line`'s identity token exists **only** for #74 and #218 — closing both deletes it. | **FIX** (measure-only, deferred) | **everything** |
| **#210** | T2 | `TraceTightener45.smoothenStartCornerAtTrace` keeps the **last** matching contact of a set ordered by descending item id, so which contact shapes the new corner is decided by an id ordering with no geometric meaning (measured: contacts `{496, 495, 494}`, both 496 and 494 match, Java keeps 494). Unlike #44/#63/#74 this one **does** have a correctness argument. | `TraceTightener45.java:481, :511-515` and three siblings | the port's `.rev()` at `tightener/mod.rs` — a **port defect already fixed to match Java** (ruling AY); the Java-side fix is still owed | Pick the contact on **geometry** — nearest, or smallest `translateDist`. Land it on its merits, independently of #44/#63. Ruling AY's sibling audit already cleared the other three candidate sites. | **FIX** | R, B |
| **#61, #30, #75** | — | Three orderings the port **already** answers better: a per-manager entry-id counter where Java has a `private static int` shared by every board in the JVM; a per-call `Random` where Java's `static` one makes shape division non-reproducible under concurrency; a `BTreeMap` where Java walks a bucket-ordered `HashMap`. | three sites | `searchtree/manager.rs`, `polygon_shape.rs`, `board/normalize.rs` | No work owed, and they must not be "restored". #30 and #61 stay preconditions for any future parallelism. | **ALREADY-FIXED** | — |

### 4.10 The via optimizer and the drill pages

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#206 + #207 + #205** | T2 | Three `ViaOptimizer` rows. #206: `isWithinTolerance` is a **loose Manhattan** test standing in for an **exact** connectivity rule, tested `firstCorner` first — so a trace whose first corner is within a **37 821-unit** window of a via sitting exactly on its **last** corner reads the wrong end and `repositionVia` is aimed at the wrong corner. #207: **no** `repositionVia` overload tests the board's angle restriction against the delta it produces, though the projection fallback in the calling method does. #205: `checkConnectionToPin`/`correctConnectionToPin` accept `pinEdgeToTurnDist == 0` where their only caller demands `> 0` — a dead acceptance band. | `ViaOptimizer` | `crates/fr-router/tests/via_optimizer.rs`, `connection_to_pin.rs`; `p7t4` is the live diff | Compare exactly, as `getNormalContacts` does, and delete `isWithinTolerance`; apply the delta test to overload A's answer too; make the three guards agree at `<= 0`. | **FIX** | R, B, X (`p7t4`) |
| **#192** | T2 | *(row in §4.3 — listed here because it belongs to the same drill-page task)* `DrillPageArray.overlappingPages` mixes an `int` lower bound with a `double` upper bound, so a shape ending exactly on a page boundary stops one page short. | `DrillPageArray.overlappingPages` | `crates/fr-router/tests/maze_drills.rs` | Compute both bounds the same way; decide deliberately about the boundary case. | **FIX** | R |

### 4.11 Determinism and the clock — a precondition, not a fix

With the jar no longer the oracle, **the port's own output is the golden file**, so anything that
makes the port's output depend on the wall clock has to go first or the goldens cannot be regenerated.

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#234** | **T1 (precondition)** | `optChangedArea`'s 1000 ms `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` is a `static final int` with a constant initialiser, so `javac` **inlines** it and no flag or reflection can switch it off. It abandons the pull-tight mid-way on wall-clock, and the jar is measurably **non-reproducible from pass 3** on a congested board (score 841.0115 at `-mp 3` against 835.88336 at `-mp 20`, three runs each). The **port's CLI runs `RouterBudget::default()`, i.e. Java's live 1000 ms** — a machine-speed dependency in the program whose output is about to become the reference. | four inlined declarations; `TraceTightener.java:73-77, :202-211` | `fr_core::ctx::RouterBudget` (`default()` = 1000 ms, `disabled()` = 0); `crates/freerouting/src/commands/route.rs`; parked at handoff §6 task 6 | **Make the CLI deterministic**: default `opt_changed_area_ms` to `0` (Java's own "off" value) and expose it as a real setting; if a bound is wanted, bound the pull-tight **by work**, not by clock. Then a two-run identity check on the port becomes meaningful (§7.2). Ruling AI's "time out of every measurement" survives the switch's removal unchanged. | **FIX** | **B, C** (re-baselined once, deliberately), X (all whole-program drivers) |
| **#208** | T2 | `retryConnectionNecked` **re-uses the `TimeLimit` the failed first attempt already spent** — and the connections the retry exists for are exactly the ones where the first attempt ran long, so **the remaining budget is smallest precisely where the retry is wanted.** RED/GREEN already measured: 81 ms of a 96 ms call. | `AutorouteConnectionRouter.retryConnectionNecked` | `crates/fr-router/tests/batch_autorouter.rs` | Build a fresh `TimeLimit` (the parameter then disappears). The register's alternative — "say in a comment that it is deliberately time-boxed" — is not a fix. Measure with the connection time limit **disabled**, then separately prove the retry is time-boxed. | **FIX** | R, B (hard boards) |
| **#224** | T4 | `FanoutSettings.timeout`'s two **documented** examples, `"5m"` and `"300s"`, both parse to `null` — `parseTimespanString` splits on `':'` and builds `PT5mS` / `PT300sS`, which `Duration.parse` rejects, and the exception is swallowed. So the fanout stage silently runs with **no** timeout. `optimizer.timeout` and `jobTimeoutString` go through the same method. | `FanoutSettings.java:98-101`; `TextManager.java:83-119` | `crates/fr-router/src/pipeline/fanout.rs::parse_timespan_seconds`; `tests/fanout.rs` | Accept the unit suffixes the javadoc promises, and **reject an unparseable timeout loudly** rather than running unbounded. Pairs with the in-pass deadline observation already landed at `c3a7ee0`. | **FIX** | U |
| **deadline** | — | The job deadline was observed only at pass boundaries; a `--router.job_timeout` run overshot by whatever remained of the running pass (measured +23.69 s → +0.94 s). | `AutoroutePassRunner:203` | `RouterStop::poll_deadline`'s second call site, `pipeline/pass_runner.rs` | Landed at `c3a7ee0`. Byte-invisible on an untimed run (`deadline: None` makes the call a constant `false`). | **ALREADY-FIXED** | — |

### 4.12 Policy switches — decisions, with a measurement each

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#182** | T2 | A disabled trace-tightening feature existed only as a no-op stub around unreachable Java code. | `TraceTightener` | The ordinary tightener regime tables. | **User decision:** delete the feature rather than expose or measure it. | **REMOVED before T18** | none |
| **#172** | T2/T4 | `AutorouteControl.rebuildViaInfo` relaxes **two** routing gates for a pure-SMD net, overriding what the padstacks say: it forces `attachSmdAllowed = true` while the per-via `ViaMask` keeps saying `false` (the two disagree **inside the same object**), and it multiplies `viaCostFactor` by `0.1`, making every via **ten times cheaper**. HEAD-only; JVM-pinned at `minNormalViaCost` 400 vs 4000. | `AutorouteControl.rebuildViaInfo` | ported | Make the relaxation an **explicit router setting** rather than an implicit override of the DSN. A policy row: change the default, do not delete the behaviour. Interacts with **I2** (via inflation) — a 10× via discount on SMD nets is a plausible contributor. | **FIX** (policy) | B, C (pure-SMD stems) |
| **#235** | T2/perf | `RoutingFailureLog`'s documented `FAILURE_THRESHOLD = 50` give-up policy **never runs**: `shouldSkip`'s one caller is on the dead multithreaded path and the other three methods have no callers at all. An item that fails 50 times is retried on the 51st pass exactly as on the first. | `RoutingFailureLog.java:16, :56-63, :70-78, :85-87, :104-106, :147-149` | `crates/fr-router/src/pipeline/failure_log.rs` (six `// not ported:` markers, one per caller-less member) | A **policy decision**: wiring `shouldSkip` into the item loop is a real behaviour change (faster, possibly fewer connections) and belongs behind a setting with an A/B, not in a cleanup. Otherwise delete the four methods and the constant. | **FIX** (policy) | B/C if enabled |
| **#104** | T4 | `Network.createViaRule` takes an `attachAllowed` it never reads, so **a `(via_at_smd on)` control scope has no effect on a net class's `use_via` rule** — only on the via infos. | `Network.createViaRule` | `crates/fr-dsn` | **Use** the parameter; dropping it is behaviour-preserving and is not the fix (the register does not distinguish them — the roadmap's correction). Pairs with #172: both decide whether SMD pads get vias. | **FIX** | G, B likely |

---

## 5. Tier 3 — DRC and report accuracy for KiCad users

The DRC report is the artefact a KiCad user actually reads, and the benchmark's "self-report vs DRC
disagreement on 45-63 % of boards" (§4.1) is this section measured from the outside.

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#152** | T3 | `isHole` calls **every `Pin` a hole**, surface-mount pads included — and its own comment admits it. A pad with no drill has no hole to keep clear of; KiCad reports the overlap as `clearance` and freerouting as `holeClearance`, and the description's first three words change with it. **BBD Mars-64 splits 64 `holeClearance` against 12 `clearance` on this predicate.** | `isHole` | `crates/fr-drc/tests/report.rs::smd_pins_are_classified_as_holes` | `item instanceof Via \|\| (item instanceof Pin pin && pin.getPadstack().fromLayer() != pin.getPadstack().toLayer())`. Changes counts per `type`, not the total. **This is the "per-pin vs per-pair" family the benchmark sees.** | **FIX** | **D** (all 8), C |
| **#146** | T3 | The dangling-trace dedup guards each candidate on `firstItem` alone, so a trace that is a net entry's `secondItem` — or merely a member of its `allItems` — is **reported twice**. The **via** phase has no guard at all. 17 of 112 rows reach it. The port already fixed #144's hash-dependence, so it emits a stable **112** where the jar says 113-115 across hash modes — a permanent XDIFF on `drc-natural-tone-preamp`. | the dangling dedup | `crates/fr-drc/tests/unconnected.rs` (four tests) | Build the `allItems` set **once** before the phase and test membership of it (which also removes the O(n²) rescan), or drop the guard and let the two phases be independent as the via phase already is. **Not a number to tune toward any particular jar run.** Closing this **retires the permanent XDIFF** — the one artefact in the tree that exists only because the jar disagrees with itself. | **FIX** | **D**, X (`p8t3`'s XDIFF retires) |
| **#153** | T3 | `Item.smallestClearance` is initialised to `-1.0` **once, at construction**, and only ever lowered — a monotone minimum over the **whole life of the item**, never reset, while `getAllClearanceViolations` runs once per report *and* once per `BoardStatistics` built with violations. **So by the end of a run every item reports the worst clearance any earlier call saw**, even if routing improved it. | `Item.smallestClearance` | `crates/fr-board/tests/clearance_violations.rs` | Drop the field and let `ClearanceViolation.smallestClearance` fold over the returned list — it exists only because the GUI wanted a number the compute had thrown away. Also kills the repeated whole-board recompute. | **FIX** | D, C |
| **#271** | T3/T4 | `initializeDrc` returns `true` **unconditionally**, so `-drc` exits **0** whatever it finds: a missing `.rules` warns and carries on, a missing session warns and carries on, a failed quality score warns, and the **violation count never reaches the exit code at all** — a board with 107 violations exits exactly like a clean one. Three `System.exit(1)` sites are the only failures. | `Freerouting.java:246-374` | `crates/freerouting/src/commands/drc.rs`; four `cli_e2e` tests; `p8t3 e2e`'s five argv rows | Add **`--fail-on-violations`** rather than silently redefining exit 0 — every existing CI script reads today's code. Optionally a distinct code for "ran, found violations". | **FIX** | U, X (`p8t3`) |
| **#272** | T3 | The DRC report's quality score uses a **different settings merge** from the router's: the DRC path is `0,10,20,55,60` with no `RulesFileSettings` at 40, no between-merges pass, no second merge and no post-merge `.rules` re-apply. So `-de b.dsn -dr weights.rules -drc r.json` reports violations computed **with** those clearances and a `qualityScore` computed with weights that file never influenced. | `Freerouting.java:342-352` vs `:125-146` | `crates/freerouting/src/commands/drc.rs::quality_score_settings`; the `obligation:` at `crates/fr-settings/src/resolve.rs`; `p8t3 merge` | Use the job's own merged `routerSettings` — the DRC path already has one and throws it away. **One line**, and it discharges `resolve.rs`'s obligation marker. | **FIX** | D (scores), X (`p8t3 merge`) |
| **#151** | T3 | The report's coordinate unit is hard-coded `"mm"` at its only CLI call site, so four of five `convertCoordinate` branches are dead — **including the fallback, the only arm that honours a board's declared `(unit …)`**. Every Issue575 fixture declares `(unit um)` and is silently rescaled by 1/1000, and `%.4f` means a `"um"` report would print four decimals of a micrometre — **the resolution of the text output changes with a parameter nothing can set.** | the `-drc` call site | `crates/fr-drc/tests/report.rs` already exercises all four unreachable arms; the port records a **decision not to expose a flag** | Give `drc` a unit flag (or read `board.communication.unit`) and route it through `Unit::from_string`. CLI wiring plus one flag; the port has already ported all five arms. | **FIX** | U, D if defaulted differently |
| **#154** | T3 | The report's first key advertises KiCad's **snake_case** schema and HEAD writes camelCase (`coordinateUnits`, `kicadVersion`, `unconnectedItems`, `holeClearance`) — **the document does not validate against the schema it names**, and 2.3.0 did. | the report writer | `DrcJsonFlavor::{KiCad, FreeroutingHead}`, three tests already green | Make `DrcJsonFlavor::KiCad` the **only** flavor (ruling W already made it the CLI default). With parity gone, `FreeroutingHead`'s reason to exist — "the parity choice the crate's tests pin" — is gone too: keep the reader for one release, delete the writer arm. | **FIX** | **D** (all 8 regenerate), U |
| **#291** | T3 | `items.drill_item_count` is **always 0**: `instanceof Pin` is tested before `instanceof DrillItem` and `Pin` *is* a `DrillItem`, as is `Via` — the only two concrete subclasses. A manifest reader asking "how many drilled items" is told `0` for a board of 400 pins and 90 vias. | `BoardStatistics.java:161-173` | `crates/fr-router/src/score/statistics.rs` (no `DrillItem` arm; writes `Some(0)`); every `p8t2` row | Decide which meaning the field has: test `DrillItem` first and rename `pin_count`/`via_count`, or delete the counter. Do **not** just reorder — that makes the other two unreachable instead. | **FIX** | C (manifest), X (`p8t2`) |
| **#195 + #196** | T3 | The horizontal/vertical/angled breakdown walks `polyline.lines` and measures each **infinite line's two defining points**, including the two bounding lines that carry no segment — so **the three lengths do not sum to `totalLength`** (121 606.75 vs 130 610.65). `bounding_box.width`/`height` are handed the board's **lower-left corner** and read negative. | `BoardStatistics` | `crates/fr-router/tests/score.rs`, `p7t7` | Walk `corner(i)`/`corner(i+1)`, which `totalSegmentCount` four lines above already counts; pass `ur - ll`. **Until #195 lands the acceptance table must not use the breakdown** (§7.4). | **FIX** | C, X (`p7t7`) |
| **#110** | T3 | `SesWriter.writeConductionArea` writes the boundary with `writeScopeInt` and each **hole** with plain `writeScope`, so a conduction area with holes emits `Double.toString` coordinates inside an otherwise all-integer SES file. A session file's grid is integral by construction. | `SesWriter.java:536-553` | fixture `p8t13-conduction-area.dsn` + `parity_ses.rs` (Task 13 closed the fixture debt) | Give `Shape.writeHoleScope` an `int` variant. The roadmap's "needs a new fixture" is discharged. | **FIX** | G, U |
| **#111** | T3 | `RulesWriter.writeRules` writes the design name **raw** where `DsnWriter` quotes it, so a name holding a space produces a `.rules` header the reader's own `NAME` lexeme cannot read back as one token. | `RulesWriter.writeRules` | `crates/fr-dsn/tests/rules_round_trip.rs` | `identifierType.write(designName, file)`. Changes the first line of every `.rules` file. | **FIX** | G |
| **#81, #212, #201** | T3 | Three diagnostics that lie: `PlanarDelaunayTriangulation.validate()` accumulates only on its leaf branch, so "check the consistency of the triangles" **answers `true` unconditionally** and logs "check passed ok"; `ItemRouteResult.improvementPercentage` divides two `int`s so the via term truncates (a re-route halving the via count scores like one removing every via — and the optimizer's own recomputation has the `(float)` cast, so **the field is wrong and the number actually used is right**); `getHash`'s javadoc says it hashes the **trace** state where it hashes the whole item graph — the wrong comment is what justified this port's original trace-and-via-only hash, under which **every trace-free board hashed alike**. | three sites | `validate_is_vacuous_on_an_inner_node`, `improvement_percentage_truncates_the_via_term`, `crates/fr-board/tests/snapshot.rs` | `result &= child.validate();`, the `(float)` cast, and reword the three comments. | **FIX** | U |
| **#144, #145** | — | Two the port already answers better: `getAllUnconnectedItems` iterates `HashMap`/`HashSet`s of `Item`s that override neither `hashCode` nor `equals`, so the unconnected list is **identity-hash ordered** and the emitted count moves between JVM runs; and every `%.4f` follows the JVM's default FORMAT locale, so a `de_DE` machine writes `0,0500 mm` into a machine-readable document. The port uses `BTreeMap`/`BTreeSet` with a lowest-id representative and always writes `.`. | | `crates/fr-drc/tests/unconnected.rs`, `report.rs` | No work owed; upstream-PR candidates. **#144 is why `-XX:hashCode=2` is mandatory on `p8t3` today** — retiring the jar comparison retires that requirement too. | **ALREADY-FIXED** | X (a constraint disappears) |
| **#149, #150, #155** | T3/T4 | A hard-coded `focusNets = {98, 99}` debug block walking two arbitrary nets on **every** `calculateAllIncompletes`; a log line that appends the `NetIncompletes[]` **array** where it means the count; and `DesignRulesChecker.drcSettings`, stored and **never read** — `includeWarnings`/`includeErrors` filter nothing, 12 of 14 construction sites pass `null`, and `enabled` is `transient` so it does not survive the Gson round trip the class exists for. | three sites | not ported / dropped | Nothing owed for the first two. #155 is a settings row (§6): either read the two flags in `generateReport` or delete the class — and **if a reader is added the flags must stop being primitives first**, or #115 makes `false` unreachable. | **NOT-A-FIX** (#149/#150) / **FIX** (#155, §6) | — |

---

## 6. Tier 4 — settings, CLI and manifest predictability

### 6.1 The settings merge engine

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#128** | T4 (**T2 spillover**) | `DsnFileSettings` seeds the layer count via `setLayerCount`, which also **replaces both per-layer trace-cost arrays with fresh all-`1.0` arrays** — and `copyFields`' primitive-array rule is **first writer wins**, so from priority 20 onward those `1.0`s cannot be replaced. **A `.rules` file at 40, an environment variable at 55 and a `--router.*` flag at 60 all carry per-layer trace costs that are silently dropped.** Per-layer trace costs are what steer direction preference. | `DsnFileSettings` | `crates/fr-settings` | Seed only `layers` — inline the array-sizing half, or give `setLayerCount` a variant that does not touch `scoring`. **Fixing #126 does not fix this** (here the arrays start `null`). Needs the **per-layer-width/cost fixture** #187 also needs. | **FIX** | B/C on boards with a rules file, U |
| **#119** | T4 | A property path through an array field sizes a `null` array at the **value's token count**, not the board's layer count, and writes `min(len, size)` elements with no error — and `applyRouterSettingsForLoadedBoard` then calls `setLayerCount(boardLayerCount)` whenever the counts disagree, resetting `routable`/`preferredDirectionHorizontal`/`bendCost` on **every** element. **So a `--router.layers.*` whose arity does not equal the board's layer count is silently discarded wholesale.** | the property-path navigator | `crates/fr-settings` | Size from the board and **reject** a disagreeing token count. Turns silence into an error message, which is the point. | **FIX** | U |
| **#126 + #127** | T4 | `boardSpecificTraceCostsApplied` is `private transient` and `copyFields` skips non-`public` fields, so **a merged `RouterSettings` inherits the source's tuned cost arrays with the flag back at `null`** — and `applyBoardSpecificOptimizations` re-initialises exactly the costs the merge just carried across. #126: `setLayerCount` wipes every per-layer cost even when the count is **unchanged** but only clears the flag when it reallocates. | `RouterSettings`, `copyFields` | `crates/fr-settings` | Make the flag public (it is already `transient`, so no JSON key appears) or copy it explicitly; move the cost resets inside the reallocation branch. Land **after** #128. | **FIX** | U |
| **#115 + #116** | T4 | `copyFields` can **never merge a `false` or `0` primitive**: `shouldCopy` requires `!sourceValue.equals(getDefaultValue(field))` and `getDefaultValue` returns the *type's* default — so a source that explicitly wants `include_warnings = false` cannot express it. #116: a `Set`/`List` field falls to the generic rule and recurses into `HashSet`'s own private fields — **the merge is a silent no-op**, so `DebugSettings.filterByNet` loaded from JSON never survives. | `copyFields` | `crates/fr-settings/tests/copy_fields.rs` | Box the primitives so `null` means unset — the nullable-wrapper convention the whole merger is already built on — and special-case `Collection`/`Map`. **Precondition for #155**: without it, a DRC filter flag could not be turned off. | **FIX** | U |
| **#121 + #118** | T4 | `getFieldByNameOrSerializedName` does not filter by modifier and `setFieldValue` calls `setAccessible(true)`, so **`private` and `static final` fields are both reachable from a property path** — `--router.board_specific_trace_costs_applied=true` defeats the guard #127 depends on, and `min_bend_cost` resolves to a `public static final` constant and throws. #118: the path splitter's class is `[.:\-]`, so a hyphen is a **separator** and `optimizer-max_passes` silently means `optimizer.maxPasses`. | the reflection helpers | `crates/fr-settings/tests/field_path.rs` | Skip `static` and non-`public` fields as `copyFields` already does — **a property path is external input and must not reach a private invariant flag** — and drop `-` from the class. | **FIX** | U |
| **#140** | T4 | `RouterSettings.validate` is **not idempotent**: it maps `maxPasses == 0` ("no limit") to `Integer.MAX_VALUE`, and `MAX_VALUE > 9999` so the **next** call maps it to `9999`. The headless path merges **twice**, each merge ending in an unconditional `validate()` — so `--router.max_passes=0` is a **200 000-fold** difference in the routing budget decided by how many merges ran. | `RouterSettings.validate` | `crates/fr-settings/tests/router_settings.rs` | A separate `unlimited` flag (or treat `MAX_VALUE` as in range) and stop calling `validate` once per merge. | **FIX** | U |
| **#142** | T4 | The scheduler's `.rules` file is **parsed twice against two different layer structures**: at priority 40 the structure is discovered **from the file itself**, so a file naming `F.Cu`/`B.Cu` describes a *two*-layer stack; after the merge the identical bytes are re-parsed against `board.layerStructure`, so on a four-layer board that same `B.Cu` rule lands on layer **3**. Both results are applied — 13 disagreeing rows of 84. | the scheduler | `crates/fr-settings` | Parse once, against the board when there is one, or make the discovered structure explicit in the result. Same family as #112. | **FIX** | C likely, U |
| **#124, #125, #114, #139** | T4 | Four one-liners: `validate` and `normalizeMaxThreads` disagree about `maxThreads == 0`, so a `0` that reached the field through the merge survives and the pool would be size zero; `validate` unboxes two `Integer`s that are `null` on any `RouterSettings` that did not pass through `DefaultSettings`, throwing out of `merge()`; `clone` omits `resultJsonPath`; `getRunFanout`/`isFanoutEnabled` carry the **same javadoc and opposite defaults**. | four sites | `crates/fr-settings` | The four one-liners the register names. #124 stays a precondition for any future parallelism. | **FIX** | U |
| **#258** | T4 | A `RoutingJob` still holding its field initialiser's `new RouterSettings()` makes `fromJob` **NPE** as soon as the job has a board, because `new ScoringSettings()` leaves every weight `null` and the guard tests the *object*, not the weights. Not reachable from the headless CLI; live for any API job whose JSON omits the weights. | `RoutingResultManifest.java:116-122`; `ScoringSettings.java:68-81` | `fr_router::score::BoardStatistics::maximum_score` `expect`s with Java's message | Make the weights **non-optional in the type** — the port can express what Java cannot. A silent default would change which board the pass loop keeps, which is why Plan 7 chose the panic; a required field changes nothing at runtime and removes the panic. | **FIX** | U |
| **#155** | T4 | `DesignRulesChecker.drcSettings` is stored and never read; `includeWarnings`/`includeErrors` filter nothing and `enabled` is `transient`. | `DesignRulesChecker` | dropped in the port | Either read the two flags in `generateReport` (filter by `severity`, already `"error"`/`"warning"`) or delete the class. **After #115**, or a `false` is inexpressible. | **FIX** | U |

### 6.2 The command line

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#259 + #262** | T4 | `applyCommandLineArguments` **warns but never fails**, and every short flag except `-l` is matched with `startsWith`: **`-decoy a.dsn` sets the design input** and **`-drcx r.json` enters DRC mode**; a missing value is silently ignored *and `i` is not advanced*; `-mp -5` is inexpressible; `-mp abc` produces two log lines. Nothing on this path can make the process exit non-zero. #262: Java parses argv **five times** and the passes disagree — `-dlx` disables file logging in one and not another, `-ll trace -ll error` configures log4j at `TRACE` while the settings read `ERROR`; and `--mcp_server.stdio=true` redirects stdout without starting a server. | `GlobalSettings.java:521-838`; `Freerouting.java:901-1203` | `crates/freerouting/src/legacy.rs::resolve_slots` (reproduced bug-for-bug, ruling AR); `fr_settings::CliSettings`; all **84** `p8t5` rows + the 87-row sweep | **Match flags exactly on both paths and fail on an unknown flag or a missing value.** Then #262's `-mpx` hazard disappears and the two remaining parsers can share one table. This is the largest single behaviour change in T4 and it **breaks command lines that work today** — announce it, and keep one release of "matched by prefix, warned as deprecated" if the controller wants a ramp. | **FIX** | X (`p8t5` retires as a jar comparison; becomes a port golden) |
| **#120 + #136** | T4 | `convertValue`'s boolean arm is `Boolean.parseBoolean`, i.e. `"true".equalsIgnoreCase(s)` — **every other string is `false`, silently**. So `--router.enabled=yes` **disables the router**, `--router.vias_allowed=on` **forbids vias**, `--router.strict_drc=ture` reads as an explicit `false`, and `" true "` is false too. #136: an **empty** value counts as an explicit choice from the property name alone, disarming `-de`/`-do` batch forcing and then reading as `false` — **a trailing `=` turns routing off.** | `convertValue`; the empty-value rule | `crates/fr-settings` | Accept `true`/`false`/`0`/`1`, case-insensitive and trimmed, and **throw** otherwise; treat an empty value as "not provided" or reject it. Pure predictability, no routing change on a correct command line. | **FIX** | U |
| **#131-#135 + #132** | T4 | The five legacy optimizer flags `-oit`, `-us`, `-is`, `-hr`, `-inc` and `-drc`'s router switch write a bridge **nothing reads** — in Java they have **no effect at all**. Compounded: the bridge matches by **prefix** while the live path matches exactly (so a typo reaches the dead half), `-mp` is `Integer.decode` on one and `parseInt` on the other, `-oit -5` never reaches its clamp, and one `-mt` token writes two fields with two clamps that never touch. | `GlobalSettings`, the bridge | `crates/freerouting/src/legacy.rs`; `docs/cli-legacy-flags.md` carries the normalisation table | Wire the five onto the live path, match exactly on both, one parser. **Makes the port strictly more capable than the jar** — a recorded product decision (ruling AQ hands it here), not drift. Land with #259/#262. | **FIX** | X (`p8t5`) |
| **#263 + #274 + #269** | T4 | Three "diagnosed in the wrong place" rows. A **bare `-drc`** sets two dead booleans and then dies in the CLI branch with "Both an input file and an output file must be specified" — blaming a `-do` the user did not need. `-de prev.ses -drc` is refused by the **loader**, three steps after the mistake, with a message naming a format the user never typed. And **`-dr` with a non-existent path silently disables** the adjacent `<design>.rules` discovery, so a typo loses the rules file sitting next to the design with no warning. | `GlobalSettings.java:660-669`; `BoardLoader.java:31-37`; `RoutingJobScheduler.java:113-152` | `legacy.rs`; `fr_core::load_board_if_needed`; `fr_settings::resolve_scheduler_rules_path`; four `cli_e2e` tests | Bare `-drc` means "check with the default report path" (or refuses naming the *missing path*); refuse a session file **at the argument**, since `-de` knows the extension and `setInput` knows the bytes; make the `-dr` branch fall through when the file does not exist, and warn either way. **Also close #273's asymmetry**: `initializeDrc` reads only `initialRulesFile` where the router path also probes for an adjacent `<design>.rules`, so the same directory gives the two modes different rules. | **FIX** | U, X (`p8t3 e2e`) |
| **#246 + #242** | T4 | `BoardFileDetails.setFilename` runs **Windows-only string surgery unconditionally**: the branch tests `File.separator` against the caller's raw string while every value below comes from the absolute path; `replaceAll("[/\\\\]+$", "")` turns the parent of `/board.dsn` — the **root** — into the empty string, so a file at the root reports a **relative** absolute path; and `replaceAll("\\\\.$", "")` is the regex "a backslash then **any** character", so a POSIX directory named `dir\x` becomes `dir`. #242: `changeFileExtension` NPEs on **every** bare filename and returns two different *kinds* of path depending on whether the extension already matched. | `BoardFileDetails.java:149-197`; `RoutingJob.java:352-374` | `fr_core::BoardFileDetails::set_filename` (**reproduced verbatim**), `fr_core::job::java_path` (POSIX-only, `FILE_SEPARATOR = '/'`) | Delete the two Windows rewrites, branch on `getNameCount() > 1`, keep the root as the parent it is; compute the parent lazily inside the two branches that use it and return the reconstructed absolute path from all three. **Note the asymmetry is load-bearing today** — `setInputFromFile` relies on it for an `.frb`/`.json` input's derived default output — so the derived-output rule must be written **explicitly** in the same change. This is also the module whose POSIX-only pin is what Windows support would have to unpick. | **FIX** | X (`p8t1probe`), U |
| **#260, #87, #84, #85, #137, #130, #138** | T4 | Seven documentation and lexer predictability rows: the help documents **11 of 23** accepted forms and there is no `-v`; `-ll garbage` silently means `INFO`; `Keyword.GENERATED_BY_FREEROUTING` is unreachable from the scanner, so `dsnFileGeneratedByHost` stays `true` **even for a DSN freerouting itself wrote**; the lexer's skip/stop sets contain **8 (backspace) and not 9 (tab)** although the comments say "spaces, tabs"; `nextToken`'s DFA and `nextDouble` implement **two different number grammars** in one file (`1e5` is 100000.0 as a token and 1.0 through `nextDouble`); `-de board.brd` **drops** the file rather than failing at the argument; `SesFileSettings`' javadoc claims to read SES files and `loadSettings` opens nothing; and `SettingsSource`'s javadoc puts the GUI at **50** where the constant is **65**. | seven sites | `crates/freerouting/src/{cli.rs,logging.rs}`; `crates/fr-dsn` | Add the missing lexer rule (then re-check every consumer of `dsnFileGeneratedByHost`); use `{9, 32}`; parse both numbers with the DFA's grammar; fail at the argument with the extension named; delete the SES rung; fix the one wrong number; document all 24 forms. **The lexer pair (#84/#85) changes what a DSN token *is*** and is the only one here with corpus reach. | **FIX** | G (#84/#85), U |

### 6.3 The manifest and the byte-scraping statistics

| # | T | mechanism | Java site | port site(s) | fix sketch | status | Δrefs |
|---|---|---|---|---|---|---|---|
| **#247** | T3/T4 | `countOccurrences` is a **substring** count with no token boundary, so `(layer` counts `(layer_rule`, `(net` counts `(network` **and** `(net_class`, `(via` counts `(via_rule`, `(class` counts `(class_class`. An eleven-clause DSN with one of each reports **2, 3, 2, 2**. The SES branch additionally drops a real layer whose `(path ` clause ends the file. | `BoardStatistics.java:449-473, :514-519, :578-586` | `crates/fr-core/src/stats_from_bytes.rs`; `tests/stats.rs`; every `p8t2probe` row | Match on a token boundary and read `words[0]` whenever the chunk is non-empty. **Changes every count the result manifest reports.** | **FIX** | C (manifest), X (`p8t2`, `p8t2probe`) |
| **#248 + #249 + #250** | T3/T4 | The DSN `host` scrape **can never succeed on a real Specctra file**: `searchLimit` is the first `)` after `(parser`, which in every real DSN closes `(string_quote ")`; the keywords are HEAD's own camelCase `(hostCad`, which no exporter writes; and the value slice hard-codes exactly one character after the keyword. All **fourteen** corpus DSN rows report `host` **null**. #249: the fallback that would have written `"Freerouting," + VERSION` is unreachable because a Java string concatenation of two nulls is the six characters `null,null`. #250: `(parser (hostCad))` slices backwards and throws out of a constructor whose caller has no `catch`. | `BoardStatistics.java:480-511, :113-120, :491-502` | `stats_from_bytes.rs::dsn_branch` (three `// Java bug:` markers), `statistics.rs::host_of` (`// not reachable:`), `slice_totalized` | Balance the parentheses; accept `(host_cad`/`(host_version` beside the camelCase; skip whitespace after the keyword; null-test the two fields **before** concatenating. **A fixed scrape puts a real `host` in every manifest, which no committed reference expects** — a deliberate, wanted re-baseline. #250 is already totalized. | **FIX** (#248/#249), **ALREADY-FIXED** (#250) | C, X (`p8t2`, `p8t2probe`) |
| **#254** | T3 | `phases.fanout` and `phases.optimizer` are allocated and **never written** — a manifest whose whole point is per-stage duration reports two of three stages as `{}` for ever — and `phases.autorouter.duration_seconds` holds the **whole job's** wall clock (board load, fanout, router, optimizer, SES write). *A harness reading it to compare routers is reading the process's wall clock.* | `RoutingResultManifest.java:77-86, :124-132` | `crates/fr-core/src/manifest.rs`; two named tests; every `MAN` row | Write all three stages (both stages already time themselves) and rename the autorouter's field to `total_seconds` or move it to the top level. **Land with #267/#230**, which are the same object's other two lies. | **FIX** | C, X (`p8t2`) |
| **#255 + #256** | T3 | `sha256Hex` swallows **every** failure into `null` and Gson omits the key, so "the file was deleted", "the path is a directory", "the path is unreadable" and "there was no input" produce indistinguishable JSON — for a field whose whole point is fixture identity. And `resource_usage.io_read`/`io_written` are **never assigned anywhere in the tree** yet are `float` primitives, so every manifest carries `0.0, 0.0` — two fields that state a measured quantity and are always the same lie. (The port additionally writes `0.0` for the three Java *does* fill, having no monitor thread; `p8t2`'s normaliser removes the whole object.) | `RoutingResultManifest.java:163-171`; `RouterJobResourceUsage.java:21-27` | `manifest.rs::sha256_hex` (`Option<String>`), `RouterJobResourceUsage` (five `f32`, always written) | Let the hash error out, or write an explicit `null`. **Delete `io_read`/`io_written`**, and either measure the other three or drop the object — the port cannot measure them without a forbidden dependency, so dropping is the honest answer and it deletes `normalize_manifest`'s special case. | **FIX** | C, X (`p8t2`) |
| **#251** | T4 | Port divergence: `BoardStatistics.host` is a `String`, so the port cannot tell Java's `null` from `""`. One input reaches it, and it is an XDIFF row on both sides. Ruling **BD** already schedules the fix and measured the blast radius: **1 declaration, 3 writes, 6 reads**, ~4 lines of the `p8t2` transcript. | `BoardStatistics.java:37-38` | `stats_json.rs`, `stats_from_bytes.rs` | `Option<String>`. The scan-ruling R3/R15 additive-only constraint on `fr-router` that blocked it is a Plan 8 constraint and does not survive into Plan 9. Land with #248. | **FIX** | X (`p8t2probe`) |
| **#236** | T3 | `BoardScoreBreakdown.of` reads `stats.traces.totalLength` — **raw board units** — while its javadoc and the live `calculateScore` use `totalLengthMm`, so on any board whose DSN resolution is not 1 a breakdown "explanation" does not add up to the score it explains. Both it and `ScoringWeightComparison` are **dead in `main/`** and unported (ruling AS). | `BoardScoreBreakdown.java:140` vs `BoardStatistics.java:608-609` | rostered `// not ported:` in `crates/fr-core/src/lib.rs` §9 | The roadmap wanted these ported as the A/B report renderer — *"they come with Java semantics for 'is this candidate better' that we would otherwise be guessing at"*. **That argument is weaker now**: `benchmark/`'s referee already has a corpus-scale verdict function with a noise band. Port them only if a per-stem explanation is wanted, and **fix the unit first**. | **FIX** (optional) | — |
| **#237-#240, #243, #245, #253, #264, #266, #270, #276, #279, #281, #292** | — | Eleven rows recording things the port does not have or already does better: the job-timeout monitor thread that never exits and swallows every `Throwable`; `enqueueJob`'s validate-and-discard `userId`, which is the only reason a headless run needs `SessionManager`; the scheduler's unsynchronised queue reads and static-initialiser daemon; the deferred post-load pass that **races the router** and loads a second whole board from disk to log a diff; the input-updated listener registered on the output details; `Session`'s assign-before-validate; `HeadlessBoardManager.createBoard`, unreachable at the **type** level; the unconditional Swing look-and-feel and **1 s sleep** on every headless run; the six-line donation banner on **stdout** gated on a *persisted* run counter; the SES re-serialised on every board-updated event; the DRC `date` in the JVM's default zone (the port writes UTC, deliberately, and nothing compares it); `readBoard`'s `catch (Throwable)`; six values computed and never read; and the MCP bridge stripping every `\r`/`\n` from the whole response body. | fourteen sites | rostered `// not ported:` / deliberate divergences | No port work. **Two carry a live sub-item:** #281's `OutlineJson.clearance` looks like an intended feature that `:310` hard-codes to `1` — wire it or drop the field (**FIX**, T4, small); and #276's zone could be taken from a caller, since `fr_drc::DrcReportOptions::date` is already injected. | **NOT-A-FIX** | — |

---

## 7. The harness transition — from "the jar is the oracle" to "the port is the golden"

Parity was the port's only acceptance evidence, and every fix in §3-§6 breaks some of it. The
roadmap's answer was a mode switch; the user's answer is that the evidence moves. This section says
exactly what moves, what dies, and what replaces it.

**The one thing that must not be lost:** the differential harness is what turned "the SES differs"
into "pass 6's score diverged by 0.4". A self-golden regime keeps that resolution — the goldens are
still per-pass, per-connection and byte-exact — but it can no longer tell a *fix* from a *regression*
on its own, because both look like a diff. That job passes to §7.4's quality gate, and to the rule
that **every golden churn is reviewed and named in the commit that causes it.**

### 7.1 The reference families, and what each is worth after the switch

| code | family | files | generator | today's oracle | after |
|---|---|---|---|---|---|
| **G** | geometry + DSN round trip | `tests/reference/<board>/*` for the 6 rows of `fixtures.txt`, plus `crates/fr-dsn`'s round-trip goldens | `scripts/gen-reference.sh` | **`tools/freerouting-2.3.0.jar`** (Plan 3 ruling 1 — HEAD writes Specctra its own lexer cannot read, quirk #92) | **Regenerate from the port.** Keep the 2.3.0 spelling as a *decision* (#92 is a real HEAD bug), not as a jar comparison |
| **R** | per-connection router transcripts | `tests/reference/router-*/router.jsonl`, `router-steps18.jsonl`, 6 rows | `scripts/gen-router-reference.sh` (runs the same `P6T1.java` driver `run.sh p6t1` diffs) | the clone's HEAD jar | **Regenerate from the port.** The "reference and differential describe the same run" property must be preserved by running the *port's* `p6t1` twin |
| **B** | whole-board batch SES | `tests/reference/<stem>/batch.ses` + pass transcripts, 8 stems | `scripts/gen-batch-reference.sh` | HEAD jar | **Regenerate from the port.** This is the strongest determinism gate in the tree and it survives the switch unchanged in *form* |
| **C** | CLI end-to-end | `tests/reference/cli-*/{argv.txt,route.ses,route.exit,route.log,manifest.json,drc.json,meta.txt}`, 13 stems | `scripts/gen-cli-reference.sh` (drives the **bare** jar) | HEAD jar | **Regenerate from the port**, keeping `--verify-hash-modes`' *shape* as a port-side two-run identity check (§7.2) |
| **D** | DRC documents | `tests/reference/drc-*/*`, 8 stems | `scripts/gen-drc-reference.sh` | HEAD jar | **Regenerate from the port.** A pure function of the board, so it diffs legibly — the cheapest family to re-cut |
| **X** | differential drivers | `scripts/differential/{java,rust}/**`, 39 drivers + 4 sweeps | `run.sh <driver>` | the jar, live, on every run | **See §7.3** — most retire, five convert |
| **U** | unit and directed tests | ~2 389 tests in `crates/*/tests` and `#[cfg(test)]` | — | mostly the register's pinned literals | **Invert in place**, per fix. This is where a fix's *own* evidence lives |

### 7.2 What gets regenerated, and the rule that keeps it honest

**Regenerate G, R, B, C, D from the fixed port, once per merged fix task**, not once per fix and not
once at the end:

* **Freeze the jar copies first.** Before the first fix lands, copy the whole of `tests/reference/`
  to `tests/reference/java-head-2026-09/` and mark it read-only in the README: it is the historical
  baseline the port was proven byte-identical to, and it is never again an assertion. That copy is
  what lets a future reader answer "what did the jar do here?" after the drivers are gone. It costs
  one commit and it is the only chance to take it.
* **Each generator grows a `--from-port` mode** (or, better, is rewritten to drive the port and
  keeps a `--jar` mode for the frozen baseline). `gen-router-reference.sh`'s property — that the
  reference and the differential describe *the same run* — must be preserved against the port's own
  `p6t1` twin.
* **`meta.txt` records what generated it**: the port's git sha, the fix set at that sha, and the
  `RouterBudget` in force. A golden cut before a later fix must be **loudly** invalid, which is what
  the roadmap already asked of its `Fixed`-mode meta.
* **The churn rule.** A commit that moves a golden must name, in its message, the fix that moved it
  and the direction of the movement. "The numbers went up" is not sufficient — §7.4's rule 4.
* **Determinism first, or none of this works.** #234 (§4.11) must land **before** the first
  regeneration: the CLI runs a live 1000 ms wall-clock budget today, so a golden cut from it is a
  machine-speed artefact. After #234, add the port-side analogue of
  `gen-cli-reference.sh --verify-hash-modes`: **run every stem twice and require byte-identical
  output** (there is no `-XX:hashCode` axis to sweep on the Rust side; two runs on one machine, plus
  one run on CI, is the equivalent assertion).

### 7.3 The differential drivers — what retires, what converts

Three kinds, and they have three different fates.

**(a) Retire outright** — drivers whose entire purpose is "the port reproduces a Java defect":

`p8t5` + `sweep-p8t5.sh` (87 argv rows of bug-for-bug legacy parsing — #259/#262/#131-#135 delete
the behaviour they pin), `p8t1probe` and `p8t2probe` (the totalization tables — #241/#242/#246/#250/
#252/#257 are either already-fixed or are being fixed, and their XDIFF rows are the *point* of the
drivers), `p8t6` (the eleven-row MCP delta table, asserted to be exactly itself), `p2t13`'s
Delaunay-order modes once #82 lands, `p6t2` (2 000 random seed rooms per regime) once #159 lands,
and `p4t1`'s 64-case settings matrix once §6.1 lands. **Retire means: run it once more, record the
final MATCH count in the Plan 9 hand-off, delete the pair, and keep the *Rust* half's assertions as
unit tests where they carry a literal worth keeping.**

**(b) Convert to port-golden comparison** — the five whole-program drivers, which are valuable
independently of what they are compared against, because they are the only per-pass/per-connection
instruments in the tree:

`p8t1` (SES bytes + exit code + normalised log over `cli-fixtures.txt`), `p8t2 e2e` (the manifest
field for field), `p8t3 e2e` (the DRC document, score, exit code, log), `p6t1`/`gen-router-reference`
(per-connection JSONL) and `p7t9` (the pipeline's per-pass tuple). Each keeps its Rust half, drops
its Java half, and diffs against the committed golden instead of against a live jar. **`run.sh`
keeps a `--against-jar` escape hatch** for as long as a jar is buildable — it costs nothing and it
is how a surprising golden churn gets triaged.

**(c) Keep as jar comparisons, deliberately** — nothing. Once the port's answer is allowed to be
better, a live jar diff produces a red run for every fix, and a harness that is red by design is a
harness nobody reads. The jar's remaining role is §7.4's quality baseline, through `benchmark/`.

**Two constraints that disappear with the drivers**, and should be recorded as they go: the
mandatory `-XX:hashCode=2` (quirk #144, only needed because the jar's own answer is hash-ordered)
and the `-Duser.language=en -Duser.country=US` pair (quirk #145). Both are jar-side workarounds; the
port has neither problem.

### 7.4 The new acceptance — two gates, neither of them byte parity

**Gate 1 — self-regression (every commit).** `cargo nextest run --workspace` plus the five converted
drivers against the committed goldens: **zero diffs**. A moved golden is a deliberate, reviewed,
named change, regenerated in the same commit as the fix that moved it. This is the direct successor
of "zero diffs stays the bar" and it keeps determinism honest.

**Gate 2 — quality (every fix task).** Two levels, because they answer different questions:

*Per stem, against the frozen jar baseline*, at a fixed pass count with `RouterBudget::disabled()`
on both sides (ruling AI survives the switch: time must be out of the measurement):

| metric | source | rule |
|---|---|---|
| incomplete connections | referee DRC, **not** the manifest (§4.1's 45-63 % disagreement) | must not rise on any stem; must fall on at least one |
| clearance violations | referee DRC | **0** on every stem, always |
| normalized score | `normalized_score(&ScoringSettings)` — an `f32` throughout, deliberately | must not fall by more than the `f32` noise floor |
| total trace length | `traces.total_length_mm` | reported; ↓ preferred |
| via count | `vias.total_count`, split through-hole/blind/buried | reported; ↓ preferred — and **the I2 via-inflation row is watched here** |
| bend count | `bends.total_count` | reported |
| wall time | driver-reported | reported, never optimised before the routing tiers are done |

**Do not use** `traces.total_{vertical,horizontal,angled}_length` (they do not sum to the total —
#195) or `board.bounding_box.width`/`height` (they hold the lower-left corner — #196) until those
two land. `incomplete_count` is itself a fix target (#82, #147): while those two are in flight,
report it **both ways** on the same board, or the improvement is indistinguishable from the metric
moving underneath.

*Corpus-scale, at the end of each fix task*: **`benchmark/`'s referee is the gate.**
`uv run bench run --candidates java-current,rs-main --tier <t> --seeds 3` then
`bench compare --baseline java-current --against rs-main`. The rule: **`overall.verdict` must be
`better` or `same`, with zero `hard_losses`** — the referee's hard metrics are `clean_pass_rate`,
`unrouted`, `violations`, in that order, and a `better` verdict is impossible with any hard loss.
Two properties make this the right instrument and both are already true: the referee scores the
`.ses` **independently** with KiCad DRC and each board's own rules, so it cannot be fooled by the
manifest rows §5 is fixing; and its noise band is 2× the baseline's own seed stdev, so wall-clock
jitter cannot manufacture a verdict. `rs-main` is commented out in `candidates.toml` today — **the
first plan task should uncomment it and record a baseline run**, which also makes the two §4.1
regressions measurable in the port before they are fixed.

**The acceptance rule, restated for a world without modes:** a fix lands if gate 1 is green, gate 2
shows no hard loss, and **a named, reviewed explanation exists for every golden that moved.** A fix
that improves the score but cannot explain its geometry churn does not land. A fix that makes the
score *worse* and is still correct — #82 raising the honest incomplete count is the standing example
— lands anyway, with the reason recorded: the score is a proxy, and the direction is better routing.

### 7.5 Per-fix byte impact, at a glance

Counted from the Δrefs column over the **117 FIX rows** of §3-§6 (each row is one candidate fix;
a row that carries `likely` is counted by the families it would move if the fixture existed):

| bytes move? | rows | what churns |
|---|---|---|
| **corpus-wide** — two or more of B, R, C | **38** | R1, R2, #227, #234, #231, #159, #160+#161, #164, #163, #162, #165+#166, #156+#167+#158, #171+#170, #82+#9, #147, #197+#198, #5, #7+#68, #177, #186, #65, #69, #51+#60, #71+#76+#106, #95, #213, #215, #202, #182, #172, #221, #222, #206+#207+#205, #128, #55, #50, #179, #174 |
| **one family** | **49** | D only (#152, #146, #153, #154, #272, #151); C only (the manifest cluster #247/#248/#254/#255+#256/#267+#230/#291); G only (#94+#89+#93, #110, #111, #84+#85, #90, #112); the KiCad C-stems (#289, #284, #280, #286, #282); X only (#105, #246+#242, #259+#262, #131-#135) |
| **unit/directed tests only** | **25** | the §3.4 guard cluster, #86, #91, #103, #211+#45, #185, #181, #168, #173, #187, #194, #148, #258, and most of §6.1's settings-engine rows |
| **nothing committed moves** | **5** | I1, I2, #193 (all three are investigations), #155, #236 |

Read the first row as the regeneration budget: **38 fixes each re-cut B, R and C**, so they must be
grouped (§10.3) and regenerated per group, not per fix — 38 separate whole-corpus regenerations
would make every diff unreadable.

**The five fixture gaps the roadmap named are still open** and now gate specific rows: a **90-degree**
board (#159), a board with **per-layer trace widths** (#187, #128), a **≥ 6.71 M-unit or circular**
outline (#94/#89/#93 — check `cli-large-outline` first, it may already serve), a board with
**multi-net SMD pins** (#194), and a board with **signal-layer pours** (#50). Each is a fixture-design
job with ground truth; with the jar no longer the oracle, "ground truth" now means *a hand-computed
or KiCad-DRC-verified expectation*, which is more work than reading the jar's answer and must be
budgeted as such.

---

## 8. De-Java-ification — the utility shims

These are **port conventions, not register rows**: a layer of small functions and types that
reproduce a Java library semantic so that a Rust expression and its Java original agree bit for bit.
They were load-bearing for byte parity. Most are not load-bearing for anything else, and they cost a
reader on every line they appear on. Inventory below is from `grep -rn "fn java_\|struct Java"` over
`crates/*/src` and `crates/*/tests`; **counts are total occurrences (declaration + call sites +
test references), workspace-wide**.

**Classification**

* **REPLACE-STD** — Rust's own semantic is acceptable now. Golden churn is named per row and is
  one-time.
* **FOLD-INTO-FIX** — the shim disappears as a *consequence* of a catalogue fix; do not schedule it
  separately, and do not delete it before that fix lands.
* **KEEP** — load-bearing for determinism, for numeric accuracy, or as an output/wire contract.
  Renaming away from the `java_` prefix is welcome; changing the behaviour is not.

### 8.1 The inventory

| shim | sites | files | what Java semantic it preserves | class | reasoning |
|---|---|---|---|---|---|
| `java_double_to_string` | 170 | 32 | `Double.toString`'s shortest-round-trip spelling, including the always-present `.0`, the `E` thresholds at 1e7/1e-3 and the exact digit selection | **KEEP** (contract) | This **is** the DSN/SES number format. Rust's `{}` writes `1` for `1.0` and never uses `E`, so every session file's numbers change shape and a Specctra consumer's expectations with them. Rename to `dsn_double`, do not replace |
| `java_round_to_int` | 113 | 14 | `(int) Math.round(double)` — half **up towards +∞**, then a `long`→`int` **truncation** rather than a saturation | **KEEP** (geometry) | 113 sites on the coordinate path. Replacing changes rounding at every `.5` boundary for **zero** quality gain and unbounded churn. Rename to `round_half_up_to_i32`; keep the truncation documented |
| `java_min` / `java_max` / `_f32` / `java_math_max` / `java_abs_f32` | 134 / 96 / 20 / 7 / 3 | 24 / 21 | `Math.min`/`Math.max` **propagate NaN** where Rust's `f64::min`/`max` return the non-NaN operand, and order `-0.0 < 0.0` | **KEEP** (semantics) | Rust's version *silently swallows* a NaN — exactly the failure mode #9 (`circleCenter` → NaN) and #170 (NaN `sortingValue`) are about. Swapping them would hide the defects this plan is fixing. Rename to `min_nan_propagating` / `max_nan_propagating` |
| `java_round` / `java_round_half_up` / `java_rint` | 93 / 2 / 16 | 23 / 1 / 4 | `Math.round` (half up), `Math.rint` (half **to even**) | **KEEP** (geometry) | Same argument as `java_round_to_int`. The two are genuinely different rules used at genuinely different sites; conflating them would be a regression |
| `JavaTreeSet` (+ `JavaTreeSetIter`) | 75 | 15 | `TreeSet.add` **silently discards** an element the comparator calls equal, and iteration is comparator order | **FOLD-INTO-FIX** | Its whole reason to exist is the **non-total comparators**: `SortedRoomNeighbour` (#160+#161), `MazeListElement` (#171+#170), and the maze queue. Both are FIX rows, and both fixes say "then `JavaTreeSet` can be replaced by `BTreeSet` and no element is lost". The two remaining users — `pipeline/fanout.rs` (#219/#220, whose comparator can only tie on a duplicate `pinIndex`, i.e. never) and `board_ext/routing_board_ext.rs` — convert for free once the two comparators are total. **Deleting `JavaTreeSet` is the acceptance test for #160/#161 + #171** |
| `JavaRandom` (`fr-geometry`, `pub`) | 59 | 11 | `java.util.Random`'s LCG, exactly | **KEEP-DETERMINISM** (stable named algorithm) | `PolygonShape.splitToConvex` picks its concavity-scan start from it, so shape division — and therefore every routed board — is reproducible only if the generator is. **Ruling BK, as amended: cross-version *and* cross-platform determinism is required**, so `StdRng`/`SmallRng` are excluded outright (neither offers value stability across `rand` versions) and the generator must be a **stable, named, seeded algorithm**. The port has one already — Java's LCG is exactly that, specified to the bit — so the cheapest correct answer is to **keep the implementation and rename it** (`SeededLcg`), dropping only the claim that it must match the JVM. If the plan prefers to leave Java's constants behind, a hand-rolled **splitmix64 or xorshift** is the right size for the single shuffle this actually needs; `rand_chacha`/`rand_pcg` are admissible if some later fix needs real RNG infrastructure, since both guarantee value stability. Quirk #30's per-call construction stays either way — it is already better than Java's `static` field |
| `JavaRandom` (private copy in `fr-board/src/datastructures/delaunay.rs:81`) + its `shuffle` | (of the 59) | 1 | the same LCG **plus** `Collections.shuffle`, used to randomise Delaunay insertion order | **FOLD-INTO-FIX** | This copy exists only to reproduce Java's insertion order in the triangulation that **#82 is rewriting**. Once the in-circle predicate is exact, insertion order stops deciding the edge set — the answer becomes shuffle-independent — and the private copy either goes entirely or becomes one call into `fr-geometry`'s `SeededLcg`. Do not duplicate a PRNG across two crates any longer than #82 takes, and whatever survives must satisfy the same stable-named-algorithm rule as the row above |
| `JavaStringSet` / `JavaStringMap` / `java_hash_iteration_order` / `java_string_hash` | 7 / 8 / 7 / 9 | 1 / 1 / 1 / 2 | `java.util.HashMap`'s **bucket layout** — capacity 16 doubling at 0.75, `h ^ (h >>> 16)`, `(cap-1) & hash`, lo/hi split order — and `String.hashCode` | **FOLD-INTO-FIX** | All four exist for **#280** alone (KiCad auto-registered net numbering). #280's fix is "use insertion order", which deletes the emulation *and* the treeification `debug_assert!` the handoff parks at §6 task 9. **~30 sites and one of the most intricate mechanisms in the port, removed by a one-word behaviour change** |
| `JavaNpe` | 9 | 1 | the JVM's helpful-NPE message text, reconstructed from `(invoked, receiver)` | **FOLD-INTO-FIX** | Exists to reproduce #282/#285/#287's crash *messages*. Those rows' fix is DTO-boundary validation with a real diagnostic, which retires the type and the `package_pin_names` / `net_name_is_null` side tables with it |
| `java_clone` | 23 | 7 | `RouterSettings.clone`'s field-by-field copy, **including the omitted `resultJsonPath`** (#114) | **FOLD-INTO-FIX** | #114 is a FIX row; once the omission is fixed the semantic is "copy every field", which is `#[derive(Clone)]`. Keep the explicit version only if the merger genuinely needs a *partial* clone — check at fix time |
| `java_number_format_parse` | 7 | 1 | `NumberFormat.parse`'s grammar, as used by the DSN lexer's `nextDouble` | **FOLD-INTO-FIX** | This **is** #85 — `nextToken`'s DFA and `nextDouble` implement two different number grammars in one file. #85's fix is "parse both with the DFA's grammar", which deletes this shim |
| `java_double_compare` / `java_float_compare` | 9 / 13 | 1 / 3 | `Double.compare`'s total order: NaN last, `-0.0 < 0.0` | **REPLACE-STD** | `f64::total_cmp` / `f32::total_cmp` are **exactly** this. Zero behaviour change, zero churn — a pure readability win, and #147's NaN guard is spelled with it |
| `java_trim` / `java_is_whitespace` / `java_is_blank` | 34 / 8 / 24 | 5 / 2 / 2 | `String.trim` strips `<= U+0020` (Rust's `trim` strips Unicode whitespace, NBSP included); `String.isBlank`/`Character.isWhitespace` | **REPLACE-STD** | Live at four settings/CLI/manifest sites and two KiCad-reader sites. The difference is reachable only through a value containing NBSP or an exotic space, which no corpus input has. #120's fix ("trimmed") wants ordinary `trim()` anyway. **One-time churn: none expected; verify on the `p8t2probe` table before it retires** |
| `java_to_upper` / `java_to_lower` (+ `_unit` variants) | 24 / 13 | 3 | locale-independent `String.toUpperCase()`/`toLowerCase()` and `equalsIgnoreCase` | **REPLACE-STD** | `to_uppercase`/`to_lowercase` agree with Java on everything the corpus contains (including `ß`→`SS`); ASCII sites become `eq_ignore_ascii_case`. **Caution:** the clearance-class and net-name comparisons in `fr-board/src/rules` are name matching on **user data** and a non-ASCII net name exists in the tests (`GND_é中`, quirk #290) — convert those with a directed test, not in bulk |
| `java_string_cmp` / `java_string_compare` | 9 / 6 | 2 | `String.compareTo`'s **UTF-16 code-unit** order | **REPLACE-STD** | Rust's `str` ordering is by Unicode scalar value; the two differ only for supplementary-plane characters (emoji, rare CJK), which no board name in the corpus has. Both sites are DSN library/network ordering, so the churn is bounded to G and is one-time |
| `java_split` / `java_split_literal` / `java_split_underscore` | 19 / 3 / 1 | 2 / 1 | `String.split`'s **dropped trailing empties** and regex-vs-literal split | **FOLD-INTO-FIX** / **REPLACE-STD** | `field_path.rs`'s use is #118 (the `[.:\-]` separator class, whose fix removes `-`); `stats_from_bytes.rs`'s is #247's SES scrape, whose fix rewrites the chunk walk. What is left after both is one `split` with an explicit "drop trailing empties" — which Rust spells `split(..).filter(..)`, in place |
| `java_parse_i32` / `_f64` / `_f32` / `_bool` / `java_integer_decode` / `java_parse_*_vec` | 20 / 19 / — / — / 4 | 2 | `Integer.parseInt`, `Double.parseDouble`, `Boolean.parseBoolean`, `Integer.decode` | **FOLD-INTO-FIX** (`_bool`, `decode`) / **KEEP** (the rest) | `java_parse_bool` **is** #120 (`"yes"` reads as `false`) and `java_integer_decode` **is** #131-#135's two-parser split — both are FIX rows that delete them. The numeric parsers define what a settings value *is* and should stay, renamed, because their overflow and format rules are the documented CLI contract |
| `JavaNumberFormatter` | 19 | 8 | Gson's number rendering in a JSON document | **KEEP** (contract) | The manifest, the DRC report and the settings JSON are consumed by other tools; their number spelling is a wire contract, and it is the same argument as `java_double_to_string` one format over |
| `java_format_fixed` | 40 | 8 | `String.format("%.Nf")` with **HALF_UP on the decimal value** (Rust's `{:.N}` rounds half-to-even on the binary value) | **KEEP** (contract) | The DRC report's `%.4f` (and #288's padstack names) are read by KiCad users and by the schema. Plan 5 ruling 6 already settled the locale half; keep the rounding, rename to `format_fixed_half_up`. **#288's separate defect — the whole-string `xf_`→`x` `replace` — is a FIX row and is not this shim** |
| `java_big_integer_hash_code` | 6 | 1 | `BigInteger.hashCode` for `RationalPoint`'s `Hash` | **REPLACE-STD** | Verify first that nothing observable reads it (it should be a `HashMap` key only). If so, `#[derive(Hash)]`. If any ordering or output depends on it, it becomes KEEP and the *dependency* is the bug |
| `java_double_stream_sum` | 6 | 3 | `Collectors.sumWithCompensation` — **Kahan** compensated summation | **KEEP** (numeric) | It is *more* accurate than a naive fold, and it feeds `normalized_score`, which is a threshold comparison in two loops. Keep and rename to `kahan_sum` |
| `java_name` / `java_type_name` / `java_class_name` / `java_enum_constant` / `java_item_class_name` / `java_angle_name` / `java_fixed_state_name` | 33 + ~7 | 9 | Java enum/class **names as strings** | **KEEP** (contract) | These are JSON values and log-message payloads that consumers match on. Renaming the functions is fine; changing a single string is a wire break for no gain |
| `java_shift_loop_hangs`, `java_nets_get`, `java_drill_item_tile_shape_count`, `java_parent_is_null`, `java_double_to_int`, `java_int`, `java_reverse`, `java_fixed_state`, `java_simple_uppercase_exception`, `java_static_constants_are_not_settable_fields` | 1-5 each | — | one-off reproductions of a specific quirk's mechanism | **FOLD-INTO-FIX** | Each is tied to a named row — #241, #282, #286, #242, #121 respectively — and goes when that row's fix lands. `java_shift_loop_hangs` in particular is a **public predicate whose only consumer is the `p8t1probe` driver** that §7.3 retires |

### 8.2 How to schedule it

Two tasks, both **after** the routing-quality tiers, because none of this changes a routed board:

* **Task DJ1 — "the shims that std already has"** (REPLACE-STD): `java_double_compare`/
  `java_float_compare` → `total_cmp`; `java_trim`/`java_is_blank`/`java_is_whitespace`;
  `java_to_upper`/`java_to_lower` (with the `fr-board/src/rules` name-matching sites done under a
  directed test); `java_string_cmp`; `java_big_integer_hash_code` after the read-audit. ~110
  occurrences. Expected golden churn: **none**, and the task's own acceptance is exactly that —
  gate 1 green with **no** regeneration. If a golden moves, the shim was load-bearing and the row
  becomes KEEP.
* **Task DJ2 — "the shims a fix deleted"** (FOLD-INTO-FIX sweep): run **after** #82, #160/#161,
  #171, #280, #282, #85, #114, #118, #120, #247 have landed, and delete `JavaTreeSet`,
  `delaunay.rs`'s private `JavaRandom`, `JavaStringSet`/`JavaStringMap`/`java_hash_iteration_order`/
  `java_string_hash`, `JavaNpe`, `java_number_format_parse`, `java_parse_bool`,
  `java_integer_decode`, `java_clone` and the ten one-off reproductions — **plus the side tables and
  `debug_assert!`s they carry**. ~150 further occurrences. This task writes nothing new; it is the
  bill the fixes already paid, collected.

**Do not** open a task for the KEEP set. `java_double_to_string` (170), `java_round*` (224),
`java_min`/`java_max` (260) and the name/format contracts are ~700 of the ~900 occurrences, and every
one of them is either a wire contract, a geometry rule, or a NaN-propagation semantic this plan
depends on. A blanket "remove the `java_` prefix" sweep over those is a large, risky diff for a
cosmetic gain; a **rename-only** commit (mechanical, no behaviour change, gate 1 green) is the
acceptable version of that wish and can ride along with DJ1.

---

## 9. KEEP and SUPERSEDED

### 9.1 KEEP — deliberate behaviour, on its own merits

With byte parity gone, "the jar does it" is no longer a reason for anything. These survive that test.

| # | what is kept | why it is load-bearing, not merely inherited |
|---|---|---|
| **#62** | The base `ShapeSearchTree` inflates a drill item's tree shape with `enlarge` and the 45-degree subclass with `offset`; on a diagonal they differ by √2 | **Not a bug.** The subclass's own comment gives the reason ("to avoid small corner cutoffs"). The register pins it because it is "the single easiest thing to get wrong when collapsing the three tree classes into one" — which Plan 9 may well attempt |
| **#83** | `ClearanceMatrix.setValue` indexes J-then-I, and a KiCad-sourced board's clearance matrix is genuinely **asymmetric** because `KiCadJsonReader` writes only one order | Java is internally consistent; "fixing" the asymmetry by writing both orders would change the clearances of every KiCad board for no defect. The register's standing warning to the KiCad reader stands |
| **#92** | The port's DSN/SES writer emits **2.3.0**'s snake_case Specctra tokens, not HEAD's camelCase | HEAD's own writer produces files **HEAD's own lexer cannot read back** (a re-read drops `host_cad`, every via rule, every per-type clearance rule and 26 wires). The port's spelling is the correct one, and it is what makes G regenerable |
| **#202**'s distinction | The three-state stop (`NONE` / `AUTO_ROUTER_ONLY` / `ALL`) must not collapse to a bool | It is the difference between "stop routing" and "stop everything", and #202 + #227 both turn on it. `CancelToken` → `RouterStop` already preserves it |
| **#273**'s order | The DRC path imports the session **after** re-parsing the `.rules` file | Measured both ways (structural hash 3938620220908954804 / 15 violations vs 945128192703554816 / 0); the jar takes the rules-first board and **that is the correct one** — wires and vias should be created against the rules that govern them. Only the *asymmetry* with the router path's adjacent-rules probe is a fix (§6.2) |
| **#281**'s four DTO fields | `OutlineJson.clearance`, `NetClassJson.netNames`, `NetJson.id`, `LayerJson.index` are parsed and never read by `readBoard` | They are a **wire contract**: Task 10's writer has to write them back, which is what makes the DTO a document model rather than a reader's private struct. (`OutlineJson.clearance` is separately a FIX — wire it or drop it — but it may not simply stop round-tripping) |
| **`normalized_score` as `f32`** | Java's `float` association, three roundings in the penalty term, one narrowing in the cost term | Both `AutorouteBatchLoop`'s `> lastBestScore + 0.5` and `BatchOptimizer`'s `< improvementThreshold` are **threshold comparisons** on it. Widening to `f64` for "cleaner" reporting changes which board a run keeps |
| **#12, #14, #19, #20, #10, #8** | Six accepted divergences and "none needed" rows the register's own columns close | Re-checked against the no-parity rule: none has an observable consequence, and #8 (`31*a + b` with silent overflow) is fine **as a hash** — it is only wrong as an identity, which is #156's row, not this one |
| **the `// Java bug:` markers** | 165 markers across 8 crates | They are the provenance that makes the register navigable from the code. **Keep every one**, and add `// fixed:` beside it naming the Plan 9 task — do not delete the description of what the code used to do, because that is what a future reader compares the jar against |
| **§8's KEEP shims** | `java_double_to_string`, the `java_round` family, `java_min`/`java_max`, `JavaNumberFormatter`, `java_format_fixed`, the name/enum contracts, `java_double_stream_sum` | Wire contracts, geometry rules, NaN-propagation semantics and a compensated sum — ~700 of the ~900 shim occurrences. See §8.1 for the per-row argument |

### 9.2 SUPERSEDED — roadmap statements the register or Plan 8 has overtaken

| roadmap statement | superseded by |
|---|---|
| §1.1's whole `Compat::{Java, Fixed}` design, §1.2's mode axis, §7.1/§7.2's release split | the user's no-switch directive; §7 of this document |
| §2.2's premise that both clearance overrides run **twice** per DSN load | **#232** (they run once — `HeadlessBoardManager.createBoard` is unreachable at the type level, **#253**) |
| §6.1's reading of #270 — "the file keeps whatever the last mid-run event produced" | **#289**: only the **first** event ever writes, so the file holds the **pre-routing** board. That is what turns a perf note into a T1 data-loss row |
| §3.5's ranking of **#202** as a routing-quality win | **#227**: the optimizer stage changes nothing after any ordinary run, so #202's fix buys nothing until #227 lands. #202 survives as a *predictability* row and as #227's partner |
| §6.5's "**Until `route` exists there is no A/B harness**, so §7 depends on it" | Plan 8 Task 12 (`route`), plus `p8t1`/`p8t2 e2e`/`gen-batch-reference.sh` and the `benchmark/` suite at `ecc0abf`. §7.4's gate is assembled, not designed |
| §7.3's corpus ("eight DRC stems, six router rows … that is not enough for an A/B") | still true as *stated*, and now joined by 13 CLI stems, 8 batch stems and `benchmark/`'s **605-board** PCBench corpus with an independent KiCad-DRC referee |
| §3.7's "#105 … no fixture in the 105-file corpus reaches it, so this needs a **new fixture**" | Plan 8 Task 13's `p8t13-via-net-numbers.dsn` **and its control**, plus ruling **BI**: the padded zero **hangs the HEAD jar's CLI for ever**, which promotes #105 from a Tier-4 curiosity to a T1 row |
| §4's "#110 … needs a **new fixture**" | Task 13's `p8t13-conduction-area.dsn` |
| §3.6's account of #44 in the port | the handoff **Errata** (`374058c`): the port's fanout target sort was seeding ascending where Java seeds descending — a *port defect*, now fixed and covered by `fanout_tie_break.rs`. #44's post-parity question is untouched |
| §6.1's "no threading policy … no rayon, no threads" (as it bears on **PRNGs**) | **ruling BK, as amended**: a seeded RNG dependency is admissible, but **cross-version and cross-platform determinism is required**, so `StdRng`/`SmallRng` are excluded and any generator must be a stable named algorithm (an in-tree LCG/splitmix64, or `rand_chacha`/`rand_pcg`). The **threading** half of §6.1 is *not* superseded: no rayon, no threads, and §6.1's parallelism remains last |

---

## 10. Dependencies, conflicts, and the proposed task groups

### 10.1 Hard ordering constraints

1. **#234 (determinism) before the first golden regeneration.** The CLI runs a live 1000 ms
   `optChangedArea` budget; a golden cut from it is a machine-speed artefact (§7.2).
2. **R1 and R2 before everything else that is measured.** They move the quality baseline by more
   than any other pair, so every later A/B taken against today's numbers would be measuring the
   regression rather than the fix. Land them, re-cut the goldens, re-run `bench`, *then* start T1.
3. **#71 → #76 → #106.** #71 is the mechanism; re-measure whether #76 needs anything after it.
4. **#95 → #90.** #90's caller-side desync is inside the scope #95 repairs.
5. **#93 + #89 + #94 in one commit.** #93 alone changes the board size, #94 alone changes which
   boards degenerate, #89 alone converts a silent `Infinity` into a loud error.
6. **#160 + #161 → #171 (+ #170).** Both are `JavaTreeSet` drops; fixing the neighbour comparator
   changes which elements ever reach the maze queue, so measuring #171 first measures noise.
7. **#5 → #7 + #68.** The shove entry *point* must be right before the entry *side* is.
8. **#82 + #9 with #147, one commit, one `AIRLINE_BUDGETS` regeneration**, reading every number that
   grew — the register's own standing instruction for a deliberate ratsnest change.
9. **#214 → #227 → #202.** #214 is the "one flag means five things" defect; #227's stage-scoped stop
   is the same repair one level up; #202 only becomes observable once #227 makes the stage do work.
10. **#128 → #126 + #127.** And **#115 → #155** (a primitive `false` is inexpressible until #115).
11. **#284 with #286.** #284 currently *masks* #286 — a pad whose layers match nothing silently
    borrows a valid padstack instead of crashing. Fix the identity and the crash surfaces.
12. **#282 → #285.** Once pin names cannot be null, the package-dedup fallback is unreachable.
13. **#280 before DJ2.** ~30 occurrences of `HashMap` emulation die with it.
14. **#195 before the acceptance table uses the length breakdown** (§7.4).
15. **#231 early, once.** It changes the clearance every committed reference was generated with;
    doing it late invalidates every A/B taken before it.
16. **#44 + #63 + #74 last, or never.** They re-baseline everything for no predicted gain.

### 10.2 Interactions worth naming

* **The three neckdown defects are independent and compound**: **R2** (the fanout fallback ignores
  the minimum width), **#179** (a *start* pin's neckdown shrinks a trace merely passing through) and
  **#51** (a via that changed side keeps the old padstack's minimum width for ever, and `minWidth()`
  feeds the neckdown decision). Measure them together or each will be credited with the others' gain.
* **#160/#161's restored doors vs #162's room-shape fix.** Both change the door set of the same
  rooms. #162 is the riskiest T1 row (0.4 % of completions) and #160/#161 the largest T2 one
  (481 drops per 2 000); land them in separate commits with separate A/Bs, in that order.
* **The DRC cluster #144-#148 is now three different things.** #144 and #145 are done. #146's fix
  **retires the tree's one permanent XDIFF** (`drc-natural-tone-preamp`, where the jar disagrees with
  itself across hash modes) — which is only possible because the jar stops being the oracle. #147
  belongs with #82, not with #146. #148 is latent and cheap.
* **#172 (a 10× via discount on pure-SMD nets) and I2 (via inflation doubled between v2.2.4 and
  v2.3.0)** are plausibly the same phenomenon seen from two ends. Measure #172's removal against the
  via-count column before treating I2 as unexplained.
* **#217 and #197/#198 look related and are not.** Fixing `getRank`'s stability does **not** make the
  rank break reachable — the limit *is* the cap. Enabling it is a separate product decision that
  makes runs stop **earlier**.
* **#265/#268/#289 are one code path** (`commands/route.rs`'s output half) and should be one task:
  delete-before-run, discard-the-return, and write-the-wrong-board are three ways the same function
  loses the user's result.
* **#247/#248/#254/#255/#256/#267/#291 are one object** (the result manifest) and one task: every one
  of them is a field that lies, and fixing them one at a time re-cuts the same 13 C-stems repeatedly.
* **DJ2 is downstream of ten fixes** (§8.2) and must not start before all ten have merged.

### 10.3 Proposed task groups

Twenty-four groups. Sizes are "fix rows in this catalogue", not commits; the T1/T2 groups are
one-fix-per-commit inside the group, with a single regeneration at the end of each.

| # | task group | tier | fixes | notes |
|---|---|---|---|---|
| **1** | **Baseline, determinism and the frozen jar** | T0 | 2 | #234, #224. Plus the non-fix mechanics: freeze `tests/reference/java-head-2026-09/`, uncomment `rs-main` in `candidates.toml`, record a `bench` baseline, add the two-run identity check |
| **2** | **The two measured regressions** | T2 (first) | 4 | **R1** (restore the airline sort), **R2** (guard the micro-neckdown fallback), **I1**/**I2** as investigations. Strongest evidence in the catalogue; regenerate and re-baseline after |
| **3** | **Output integrity** | T1 | 3 | #289, #268, #265 — one code path |
| **4** | **DSN read integrity: scopes, scale, size** | T1 | 7 | #95→#90, #93+#89+#94, #112, #91, #103, #86 |
| **5** | **Termination: normalisation and net numbers** | T1 | 3 | #71+#76+#106, #211+#45, #105 |
| **6** | **The reachable crash guards** | T1 | 6 | #185, #181, #169, #168, #173, and the ten-site guard cluster. Guards at the defect — never a `catch_unwind` |
| **7** | **KiCad JSON: validation, identity, numbering** | T1/T2 | 6 | #282+#283+#287, #286+#284, #285, #280, #281's outline clearance. Deletes `JavaNpe`, the hash emulation and two side tables |
| **8** | **Rooms, doors and expandable identity** | T2 | 9 | #159, #160+#161, #164, #163, #171+#170, #165+#166, #178, #156+#167+#158, #162 (from T1, because it changes the same door sets) |
| **9** | **The optimizer stage and the pass loop** | T2 | 7 | #214→#227→#202, #213, #215, #230+#267, #228; #217 as a decision |
| **10** | **Shove, obstacle and clearance decisions** | T2 | 8 | #65, #69, #72, #174, #179, #50, #231+#233, #35/#46/#175 |
| **11** | **Board geometry corrections** | T2 | 10 | #5→#7+#68, #177, #186, #187, #55, #183, #48+#57, #15/#16/#9/#13, the #26 tail |
| **12** | **Polygon and circle implementations** | T2 | 2 | #28+#29, #27. A workstream with its own directed geometry suite |
| **13** | **Airlines, incompletes and board history** | T2 | 5 | #82+#9 with #147, #197+#198, #194, #148. One `AIRLINE_BUDGETS` regeneration |
| **14** | **Stale caches and via-rule identity** | T2 | 2 | #51+#60 first, then #56/#58/#41; #218 (deletes an identity token) |
| **15** | **Fanout ordering, stagnation and the ripped set** | T2 | 3 | #221, #222, #219+#220+#223 |
| **16** | **Via optimizer and drill pages** | T2 | 2 | #206+#207+#205, #192 |
| **17** | **#193 — the stale tree-index discovery** | T2 | 1 | Discovery first, code second. Start early; its answer may subsume several group-8 rows |
| **18** | **Measured policy switches and orderings** | T2 (last) | 6 | #182, #172, #235, #104, then #210, then #44+#63+#74 — each with its own A/B, each individually abandonable |
| **19** | **DRC report accuracy** | T3 | 11 | #152, #146, #153, #271, #272, #151, #154, #195+#196, #110, #111, #81/#212/#201 |
| **20** | **Manifest and statistics truth** | T3 | 7 | #247, #248+#249, #254, #255+#256, #251, #291, #236 (optional) |
| **21** | **The settings merge engine** | T4 | 10 | #128→#126+#127, #119, #115+#116→#155, #121+#118, #140, #142, #124/#125/#114/#139, #258 |
| **22** | **CLI predictability** | T4 | 6 | #259+#262, #120+#136, #131-#135, #263+#274+#269, #246+#242, #260 + the six doc/lexer rows |
| **23** | **DJ1 — the shims std already has** | cleanup | 0 | §8.2. Acceptance: gate 1 green with **no** regeneration |
| **24** | **DJ2 — the shims a fix deleted** | cleanup | 0 | §8.2. Strictly after groups 7, 8, 13, 21, 22 |

Suggested sequence: **1 → 2 → (3, 4, 5, 6, 7 in parallel where they touch different crates) → 17
(discovery, running alongside) → 8 → 9 → 10 → 11 → 13 → 14 → 15 → 16 → 12 → 19 → 20 → 21 → 22 →
23 → 24 → 18**. Group 18 is last on purpose: every row in it is a deliberate change of policy or
of an arbitrary order, and each needs the rest of the catalogue's improvements underneath it before
its A/B means anything.

---

## 11. Open questions for the controller

Each carries a recommendation; none is blocking on its own, but 1-4 shape the plan's first commits.

1. **Do R1/R2 go first, ahead of the T1 data-loss rows?**
   *Recommend yes.* They are two one-line-class changes with 605-board ablation evidence, they move
   the baseline every later measurement is taken against, and they are the two rows a user would
   notice first. T1's data-loss rows are severe but narrow (`-do out.json`, `-do out.dsn`, a deleted
   previous result); they follow immediately.
2. **When is the jar baseline frozen, and when do the drivers retire?**
   *Recommend:* freeze `tests/reference/java-head-2026-09/` **before the first fix**, in its own
   commit — it is the last moment the port is byte-identical to the jar and the copy is otherwise
   unrecoverable. Retire each driver **in the task that deletes the behaviour it pins** (so `p8t5`
   dies with #259, `p8t2probe` with the totalization rows), not in one sweep — a driver retired
   early loses the evidence that its task is complete.
3. **Does the 500 µm board-edge keep-out apply by default at all (#231)?**
   *Recommend:* make the option continuous (apply the configured value uniformly, drop the
   equals-the-default special case) and **keep 500 µm as the default**, because that is what every
   committed reference and every benchmark number was produced with — then measure removing it as
   its own experiment. Changing the semantics and the default in one commit would make the
   regeneration unreadable.
4. **How much CLI compatibility is deliberately broken (#259/#262/#131-#135)?**
   *Recommend:* fix the **native** subcommand form immediately (exact matching, hard failure), and
   give the **legacy** flag form one release of "matched by prefix, warned as deprecated" before it
   becomes exact — `-decoy a.dsn` setting the design input is a bug, but somebody's script may be
   relying on a prefix today, and the port has no telemetry to know.
5. **`-drc`'s exit code (#271).**
   *Recommend:* add `--fail-on-violations` rather than redefining exit 0. Every CI script that
   exists today reads the current behaviour, and a silent redefinition is exactly the class of
   change this catalogue is full of.
6. **Delete the `FreeroutingHead` DRC flavour's writer (#154)?**
   *Recommend yes* — keep the *reader* for one release, make `KiCad` the only spelling written. Its
   only remaining justification was "the parity choice the crate's tests pin", which is gone.
7. **Do R1, R2, I1 and I2 get register ids?**
   *Recommend yes*, at the next free ids after #292, with `benchmark/reports/java-regressions-2026-09.md`
   cited as their evidence column — the register is the project's index and a fix with no row in it
   is a fix nobody will find. They should also be the first two **upstream PRs**.
8. **Which fixes are contributed back to the Java project, and by whom?**
   *Recommend* a standing list rather than a decision per row: R1, R2, #105 (the jar hangs), #162
   (OOM), #241/#244 (two CLI hangs), #95 (settings silently dropped), #27, #39 and #144/#145. The
   register already marks most of them "Java-side fix owed"; Plan 9 should keep that column true.
9. **Ground truth for the five missing fixtures, now that the jar is not an oracle.**
   *Recommend:* hand-computed expectations reviewed in the plan, plus KiCad DRC where the property
   is a design rule. Budget each fixture as a task-sized item, not a test-writing afterthought —
   this is the single largest hidden cost in the catalogue (§7.5).
10. **Which generator replaces `JavaRandom` (ruling BK, as amended)?**
    *Recommend:* keep the existing LCG implementation and **rename it `SeededLcg`**, dropping only
    the claim that it reproduces the JVM. The amended ruling requires cross-version **and**
    cross-platform determinism, which rules out `StdRng`/`SmallRng` and is already satisfied by a
    bit-specified LCG the port owns. If the plan would rather not carry Java's constants, a
    hand-rolled **splitmix64** is the right size for the one shuffle in question; reach for
    `rand_chacha`/`rand_pcg` only if a later fix needs real RNG infrastructure, since those two do
    guarantee value stability. Either way the generator is **in the port's own control**, so a
    `cargo update` can never move a golden.

11. **Defaults for the two remaining policy switches (#172 the pure-SMD relaxation and #235 the
    give-up policy).**
    *Recommend:* implement both as settings with the **current** behaviour as the default, then
    decide each default from its own A/B. #172's removal interacts with the via-inflation finding.
12. **Do the new capabilities reach the MCP surface too** (`--fail-on-violations`, a DRC unit flag,
    `-v`)? *Recommend:* CLI first, MCP in the same task only where the tool already exposes the
    neighbouring option — the MCP tool set is a contract with its own delta table (`p8t6`).
13. **Register discipline during Plan 9.** *Recommend:* keep every `// Java bug:` marker and add
    `// fixed:` beside it naming the task; add a **status column** to `docs/java-quirks.md` rather
    than deleting rows, so the register stays a description of the *Java* program — which is what
    makes it useful upstream — while recording what the port now does instead.

---

## 12. Counts

**Catalogue rows** (§3-§6, one row per candidate fix, deduplicated): **124**, covering **215 distinct
register ids** of the register's 292 — the remainder are rows the register itself closes as
totalizations, deliberate port divergences, GUI-only, or unported classes, and they are summarised in
the grouped rows rather than given a line each.

| status | rows | note |
|---|---|---|
| **FIX** | **117** | includes 4 investigation rows (I1, I2, #193, #236) that produce a measurement before any code, and 3 policy rows (#182, #172, #235) whose default is decided by an A/B |
| **ALREADY-FIXED** | **5** | the five totalization/divergence clusters (#241/#244/#250/#252/#257; #61/#30/#75; the deadline errata; #144/#145; #250) — no Plan 9 work owed |
| **NOT-A-FIX** | **5** | #279+#277; #216/#225/#226 (dead arms); #149/#150; the fourteen-row unported cluster (#237-#240, #243, #245, #253, #264, #266, #270, #276, #281, #292); the bisect note |
| **KEEP** | **10** | §9.1 — #62, #83, #92, #202's three-state stop, #273's order, #281's DTO contract, `normalized_score` as `f32`, the six accepted divergences, the 165 `// Java bug:` markers, and §8's KEEP shims |
| **SUPERSEDED** | **10** | §9.2 — roadmap statements the register, Plan 8 or the user's directives have overtaken |

Two rows carry a mixed status (a group where some members are dead-code cleanups and one is a real
fix); they are counted under both and named in place.

**Tiers**: T1 **26**, T2 **53**, T3 **19**, T4 **19**, plus **7** rows carrying no tier (the
record-only clusters and the bisect note). The T2 total includes §4.1's four regression rows and
§4.11's determinism rows, which are nevertheless scheduled **ahead of** T1 for the reasons in §10.1.

**Shims** (§8): ~**25** distinct kinds, ~**900** occurrences workspace-wide. **KEEP ~700**
(contracts, geometry rounding, NaN propagation), **FOLD-INTO-FIX ~150** (deleted as a consequence of
ten catalogue fixes), **REPLACE-STD ~110** (task DJ1, expected golden churn: none).

**Task groups**: **24** (§10.3), of which 2 are cleanup and 1 is discovery.
