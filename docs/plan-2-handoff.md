# Plan 2 hand-off — board model (`fr-board`)

Branch `plan-2-board`, 41 commits on top of `main` before this task's commit. Final
whole-branch review is still **pending** (this task closes out Plan 2's own
obligations first, per the brief).

## Delivered

- `fr-board`: 40 source modules / 25.6k LOC across `ids`, `error`, `structure`
  (`Layer`/`LayerStructure`/`Component(s)`/`BoardOutline`/`ShapeAndEntrySide`),
  `library` (`Padstack(s)`/`Package(s)`/`LogicalPart(s)`/`BoardLibrary`), `rules`
  (`ClearanceMatrix`/`Net(s)`/`NetClass(es)`/`ViaInfo(s)`/`ViaRule`/`BoardRules`),
  `datastructures` (`ShapeTree`/`MinAreaTree` arena, `TimeLimit`,
  `PlanarDelaunayTriangulation`), `items` (the `Item` enum, `ItemHeader`,
  `Connectable`, drill items, areas/outlines, `PolylineTrace` + normalisation),
  `searchtree` (`ShapeSearchTree`, `SearchTreeManager`, `ShapeTraceEntries`) and
  `board` (`Board` — the non-shove half of `BasicBoard`/`RoutingBoard`: insert/
  remove protocol, connectivity queries, `normalize_all_traces`, `deep_copy`,
  structural hash, `ChangedArea`, `Communication`).
- `docs/java-quirks.md`: 83 pinned quirks (36-83 added in this plan; 1-35 were
  Plan 1's), the totalizations table, and a candidate/obligation table whose
  bolded "(Plan N obligation)" rows are the forward-obligation register.
- Differential harness extended with six board-level driver pairs:
  `p2t3`/`p2t3r` (`ShapeTree`/`MinAreaTree`), `p2t10` (`ShapeSearchTree`/
  `SearchTreeManager`, 9 modes), `p2t11` (`Board`, modes 0-11), `p2t13`
  (`PlanarDelaunayTriangulation`, 8 modes) and `p2t15` (a randomised board-scale
  sweep, 20 runs across 10 seeds × 2 sizes). All are exact matches against the
  JVM except `p2t11` mode 11's one documented, deliberate divergence (see
  Evidence).
- `scripts/audit-port.sh` (generalised from `scripts/audit-geometry-port.sh`)
  and a clean audit across all nine Java source directories this plan ports.
- 575 tests in `fr-board` (301 unit + 274 across ten integration-test files;
  1 deliberately `#[ignore]`d — the non-terminating ladder-hang reproduction),
  854 across the whole workspace.

## Rulings made during execution (cost if wrong noted where it applies)

1. **`BTreeMap<ItemId, Item>` keyed by Java id**, not `slotmap` as the spec's
   §6 suggested (plan ruling #1). Ids are load-bearing (`Leaf` ordering, `getId`
   at 55 call sites, the SES writer, hash detection); a slotmap key would need a
   parallel id field anyway. Cost if wrong: O(log n) lookups instead of O(1) —
   negligible at PCB scale. **Amended after Task 10** — see Corrections, below.
2. **`TreeObject { Item(ItemId), Room(RoomId) }` enum** in the search tree
   (plan ruling #2), because Java's autoroute inserts
   `CompleteFreeSpaceExpansionRoom` into the same `ShapeTree`. `RoomId` is
   reserved; Plan 6 populates it. **Corrected by Task 5** — see Corrections.
3. **`ConductionArea`'s `java.awt.geom.Area` fill cache is not ported**
   (plan ruling #3); the geometry-library survey confirmed it is renderer-only
   (no caller outside the paint path), so `i_overlay` is not a Plan 2
   dependency. `docs/geometry-library-survey.md` §(vi) amended by this task.
4. **`RoutingBoard` is split** (plan ruling #4): its non-shove state
   (`changed_area`, `check_trace_segment`, the *removal* half of
   `remove_items_and_pull_tight`, the failure-log hook, `max_trace_half_width`
   bookkeeping) is `Board`, here; the shove/forced-via/pull-tight entry points
   arrive in Plan 7 as an extension trait `RoutingBoardExt` in `fr-router`.
5. **Dispatch order** `1,2,3,4,5,6,7,8,10,11,9,12,13,14,15,16` (plan ruling #5)
   — Task 9 (trace normalisation) needed `Board` (Task 11) to exist first, so
   it ran after it despite its lower task number.
6. **User authorization carried forward** (2026-08-28, verbatim): "keep
   running the implementation, assume I say yes to planning/starting all the
   future phases." Applies to writing Plans 3-8, executing them
   subagent-driven, and merging each plan to `main` locally after a clean
   final review. Still stops for destructive/irreversible ops,
   security-sensitive actions, pushes to a remote (none configured), or a plan
   so broken every path is a guess.
7. **`java_to_lower`/`java_to_upper` are Java's *simple* case mappings**, not
   Rust's full mapping (Task 2, plan-wide ruling): U+0130 must special-case to
   `'i'` and the 27 Greek-ypogegrammeni characters must special-case on the
   uppercase side, or `"İ".equalsIgnoreCase("ı")`-style comparisons diverge.
   The helper lives at `crates/fr-board/src/rules/mod.rs`. `java_to_lower` and
   `java_to_upper` are **`pub`**, and re-exported from `lib.rs` and from
   `fr_board::prelude`, so Plan 3's `fr-dsn` can `use fr_board::java_to_lower`
   directly and reuse them as the ruling intended. (They were plain private
   `fn`s as first committed; the final-review fix wave widened them. Their
   siblings `equals_ignore_case`/`compare_to_ignore_case` stay `pub(crate)` —
   widen those too if `fr-dsn` needs the string-level fold rather than the
   per-character one.)
8. **`MinAreaTree::remove_leaf` panics on a double removal** where Java
   silently corrupts the tree (Task 3, confirmed by review; quirk #39). No
   Java caller removes the same leaf twice today (callers do the bookkeeping
   that prevents it), so the panic is unreachable in every port so far — but
   Task 10 and Plan 6 must re-examine this if a path emerges where two owners
   can hold the same `LeafId` (autoroute rooms leaving `completeExpansionRooms`
   are the likely candidate).
9. **The tree owns the bounding-shape step** (Task 3, confirmed by review):
   `insert_tiles(&[TileShape]) -> Vec<Option<LeafId>>` applies
   `ShapeTree.insert`'s implicit `boundingShape` call itself, rather than
   trusting callers to pre-bound shapes (as a raw `insert` signature would
   require Task 10 to remember for every caller, on pain of a 45°-regime tree
   silently storing box bounds).
10. **`is_obstacle` takes no `&BoardRules`** (Task 5): Java's own
    `isObstacle` never reads `ignoreConduction` directly —
    `RoutingBoard.changeConductionIsObstacle` instead pushes it into each
    `ConductionArea`'s own field (quirk #50). Task 11 ports that push
    verbatim, including its two-line "latch, not mirror" defect.
11. **Item memo caches use `std::sync::OnceLock`, not `Cell`/`RefCell`**
    (Task 6, plan-wide ruling, confirmed by review). The four Java memo
    fields (`precalculatedFirstLayer/LastLayer/MinWidth`,
    `precalculatedShapes`) are real interior mutability in Java too, and
    dropping them would change behaviour (quirk #51: `minWidth` survives
    `clearDerivedData`). `Cell`/`RefCell` were the first attempt but make
    `Via`/`Pin` `!Sync`, conflicting with `Board: Send + Sync` (needed for
    Plan 7's `rayon` workers); `OnceLock` keeps both. `ItemCtx<'a>{ library,
    components, rules }` replaces Java's `Item.board` back-pointer, threaded
    through the dispatch methods that reach a drill-item body; `Board` builds
    it inline in `&mut self` contexts via the `item_ctx!` macro, not a
    `fn ctx(&self)` that would borrow the whole board.
12. **`fr-geometry`'s per-call `Random` in `split_to_convex` (quirk #30) is
    kept; the memo moved to the item level** (Task 7): Java memoises
    `precalculatedConvexPieces` once per shape lifetime, which this port's
    geometry layer deliberately does not (reproducibility under concurrent
    calls, Plan 1's improvement). The obligation — restoring Java's amortised
    cost without losing that property — was discharged by adding
    `OnceLock<Option<Vec<TileShape>>>` caches to `ObstacleAreaData` and
    `BoardOutline`, cleared at the same five invalidation points as the
    absolute-area memo.
13. **Deferral-marker format is line-based** (Task 8, plan-wide ruling): the
    Java method name must sit on the same line as `added in Task/Plan N:`,
    because `audit-port.sh`'s regex is line-based; a marker split across two
    lines is invisible to the audit and gives a false "zero missing".
14. **Three plan-wide rules for `Board`, all from Task 11's review** (dispatch
    order ran Task 11 before Task 9, so this predates ruling #15 below):
    tree-shape reads must always **recompute on a cold cache**, the way
    Java's `Item.getTreeShape` does — never panic or silently skip a shape
    that has not been computed yet, because every real query needs an
    answer, not a hole; a `checkPolylineTrace`-style temporary still
    **consumes an item id** even though it is thrown away immediately,
    because Java's `Item` constructor calls `newId()` unconditionally — ids
    are load-bearing (quirk #44's ordering, the SES writer, hash detection),
    so a port that only assigns ids to items it keeps would desynchronise
    id sequences from Java's; and every loop over a Java `TreeSet<Item>`
    query result iterates **descending**, ported as `BTreeSet<ItemId>.rev()`,
    the same correction as Correction #1 below applied to a
    different collection. Cost if wrong: the first two silently produce
    wrong-answer obstacles or off-by-one id sequences that only surface much
    later as a mis-routed board; the third silently reverses visit order in
    exactly the queries the differential harness was built to catch.
15. **Quirk #74's early return and quirk #76's ladder hang are Plan 7/Plan 3
    decisions, not defects to silently carry** (Task 9, plan-wide ruling).
    `PolylineTrace.change` comparing `Line`s by reference (quirk #74) is
    board-observable — the port's value comparison lets it take an early
    return Java's identity comparison never can — and `change_trace` has no
    production caller yet, so **Plan 7 must decide** (recommended: drop the
    early return) before wiring `TraceShover`/`correctConnectionToPin`. A
    ladder of four or more rungs on one net (quirk #76, the mechanism behind
    quirk #71) hangs a single `normalize` call in both Java and the port,
    inside one `normalize`/`normalizeTraces` pass where neither depth nor
    iteration cap reaches it — **Plan 3 must decide** how the DSN reader
    (`Wiring.java:347` ends every read with `normalizeAllTraces()`) handles
    that hang before it can safely import arbitrary designs. Cost if wrong:
    Plan 7 wiring the shove machinery over the port's early return would
    silently under-normalise traces relative to Java; Plan 3 shipping a DSN
    reader with no decision here means a crafted (or just unlucky) design
    hangs the import with no way to recover.
16. **`deep_copy` keeps the cloned tree arena rather than rebuilding it**
    (Task 12, JVM-verified): Java's `readObject` rebuilds the search tree by
    reinserting every item in descending-id order (quirk #77), which gives a
    *different* physical tree shape than the original — `deep_copy` derives
    `Clone` on the arena instead, which is strictly more faithful (nothing
    ported ever observes raw tree shape; every real query canonicalizes into
    an ordered set). It **must** reset every Java-transient field regardless:
    `revision = 0`, `changed_area = None`, `shove_failing_obstacle = None`,
    `shove_failing_layer = 0` (not `-1` — quirk #79, Java's own field
    initializer is skipped by deserialization too).
17. **Quirk #82 (Delaunay in-circle vacuous on axis-aligned input) is a real,
    user-visible Java bug and must not be fixed pre-parity** (Task 13,
    JVM-verified: square grids lose edges at every tested size, and a 7×7
    grid disconnects in ~1.4e-4 of random 20-point draws). Plan 5 inherits
    this as a hard constraint on `NetIncompletes`, not a free improvement.
    **Honoured, and it stays unfixed:** Plan 5 Task 5 built `NetIncompletes`
    on the unfixed triangulation, and that is what makes the port's ratsnest
    reproduce the jar's edge set exactly once the seed order is pinned. Plans
    6/7 inherit it unchanged.

## Corrections to the plan discovered during execution

1. **Item iteration order is descending id, not JVM-dependent hash order**
   (Task 10, JVM-verified, amending plan ruling #1). `UndoableObjects.objects`
   —what `board.itemList` actually is — is a `ConcurrentSkipListMap` keyed by
   the *reversed* `Item.compareTo` (quirk #44's subtraction bug), so every
   Java walk of the board's items runs in **descending** item id: deterministic,
   but the opposite of the "hash order" the plan first assumed. This is not
   cosmetic — `MinAreaTree`'s insertion heuristic is order-dependent, so it
   decides the physical shape of every search tree Java builds. Every port of
   a Java `itemList`/`getItems()` walk must therefore be
   `items.values().rev()` (`Board::items_in_board_order`), never the ascending
   default a `BTreeMap` gives for free. Verified directly on the JVM (`p2t10`'s
   board: both `itemList` and `getItems()` give `5 4 3 2 1`) and the resulting
   tree structures are byte-identical to Java in all four `p2t10` modes that
   exercise it. The plan document itself was amended (commit
   `5b4e10a docs(plan-2): amend ruling 1 — Java item iteration is descending
   id`).
2. **`TreeObject`'s `Ord` is Java's `Item.compareTo` exactly, not the
   "obvious" ascending reading** (Task 5, corrects plan ruling #2,
   JVM-verified). Java's subtraction is backwards — `result = item.id - id`
   with `item` the *argument* — so items sort by **descending** id, and
   because `CompleteFreeSpaceExpansionRoom.compareTo` returns `-1` against a
   non-room argument, **rooms sort before items**, descending among
   themselves too. The port's `impl Ord for TreeObject` is hand-written to
   match (the derived ordering would give ascending ids with items first).
   The `p2t3`/`p2t3r` differential goldens had themselves been transcribed
   with the subtraction the "obvious" way round in Task 3 — which is what
   made the port's *own* ordering ascending in the first place and hid the
   defect until Task 5's from-scratch reading of `Item.compareTo` caught it;
   both goldens were regenerated from unmodified Java and re-run to confirm.
3. **The `BTreeMap<ItemId, Item>` decision (ruling #1) gained a second,
   independent justification.** It was originally justified only by id
   load-bearing-ness and O(log n) cost. Task 10's finding (correction #1,
   above) means the map's ordering is not just a convenience: every
   consumer of a board-wide item walk must reverse it to match Java, and
   getting that backwards would silently reshape every search tree built
   from a fresh `Board`, not just return items in a cosmetically different
   order.
4. **`Item.compareTo` (Item.java:93-103) is a genuine Java bug**, and the two
   corrections above are downstream of it, not independent design choices:
   the subtraction being backwards is what makes both `itemList` iteration
   and `TreeSet<Leaf>`/search-tree result order run descending. It is pinned
   as quirk #44, with the full JVM-verified caller list (`Leaf.compareTo`
   delegates to it) and the `p2t3`/`p2t10`/`p2t11` differential coverage that
   depends on getting it right.

## Parked residuals (grouped by task; Task 14's citation sweep already
resolved the items marked ✓ below — verified against the committed tree)

- **Task 1:** `FixedState` doc comments were rewritten relative to Java's
  (swapped-looking) comment text; `LayerStructure::count()` was added and
  labelled as not a Java method.
- **Task 2:** `Net` exposes both a `contains_plane` field and a method of the
  same name; Java-final fields are mutable (crate-wide convention, not
  Task 2-specific). Two `Network.java` line citations in
  `clearance_matrix.rs` are off by one. 16 UCD-version-skew code points
  between Rust's Unicode tables and JDK 23's are enumerated in code but
  unreachable from any DSN identifier.
- **Task 3:** `ShapeTree`'s `PartialEq` compares the whole arena, including
  `Free` slots — not semantic (per-live-leaf) equality. Doc wording ("skip
  the index" → "leave a `None` at that index", `shape_tree.rs:532`); the
  `Node` generation field is `pub` rather than crate-private; `free()` is not
  double-free-guarded; the generation counter wraps at 2^32 (unreachable in
  practice); the per-node release `assert!` in the `overlaps` hot loop has an
  uncharacterised cost — **flagged for Plan 6 to measure** once real
  autoroute-scale trees exist; the `p2t3` Java twin emits stderr noise
  (cosmetic, harness-only).
- **Task 4:** Java-final library fields are `pub` mutable (the crate-wide
  convention, noted again here since Task 4 is where library fields start);
  `drill_radius`'s cache-drop justification relies on padstack name
  immutability, which is asserted but not enforced by the type system.
- **Task 7:** ✓ quirks rows #46/#50's stale `mod.rs` citations, corrected to
  `items/area.rs` (Task 14 §4 items 1-2). ✓ `BOARD_OUTLINE_HALF_WIDTH`
  narrowed from `pub` to `pub(crate)` with the intra-doc link replaced by
  plain text (Task 14 §4 item 3). Still open: `ItemCtx::bounding_box` could
  be passed by value (`IntBox` is `Copy`) rather than by reference;
  `get_trace_connection_shape`'s doc comment overstates cold-cache fidelity;
  `BoardOutline`'s `PartialEq` includes the keepout memo, a deliberate
  asymmetry with the absolute-area memo that wants an explicit note.
  *Resolved/superseded by Task 14:* Task 7's own report flagged its
  Task-11-marker running count as 24, self-corrected to 25 before commit;
  Task 14's whole-crate audit (§1) is now the authoritative count (0 missing
  everywhere), which supersedes any interim snapshot number either way.
- **Task 8:** `split_polyline_at_point`'s granularity was deferred to Task 9's
  re-implementation of Java's per-candidate loop (done); `trace_geometry_
  characterization` passes `layer_count: None`. ✓ `trace.rs:87`'s "five"
  Plan-7-marker count corrected to six (Task 14 §4 item 4). *Resolved/
  superseded by Task 14:* Task 8's report likewise carried its own running
  Task-11-marker count (28) as a point-in-time snapshot; Task 14's §1
  whole-crate audit (0 missing across all nine directories) is the count
  that matters now, not any task's interim tally.
- **Task 9:** `normalize_suppressed_net_nos` is `pub` where Java's field is
  private; `split_trace` inlines its clip filter instead of calling
  `clip_intersects_segment`; `split_traces` silently skips a dead trace (a
  totalization, not a bug — recorded in the totalizations table).
- **Task 10:** `calculate_board_outline_tree_shapes` should call
  `polyline.offset_shape` (non-virtual in Java, `ShapeSearchTree.java:982`),
  not the virtual `self.offset_shape`; a `changeOrder == true` merge test/
  mode is still missing; `validate_entries`'s `None`-skip needed (and now
  has) a quirks row (#64); quirks #61/#63 said "four modes" where there are
  eight (wording, not a data error); a `usize` underflow exists in
  `change_entries` where Java's `int` arithmetic clamps; per-insert shape
  clones are a perf cost, not a correctness one; a stray `.gitignore` entry;
  `run.sh`'s `JAVA25_HOME` path is version-pinned.
- **Task 11:** `pick_nearest_routing_item`'s tie-break order is unverified
  against Java beyond the driver's coverage; the `ShapeAndEntrySide`
  reference-vs-value `!=` and `store_trace`'s self-compare are both now
  fully documented as quirks (#68, #69) rather than open questions. ✓ quirk
  #63's stale reference, the `DrillItem.getTileShapeOnLayer` citation
  (`:308-315` → `:252-260`), and two further off-by-1-2 citations
  (`connectivity.rs`, `ConductionArea::normal_contacts`) were all corrected
  in Task 14 §4 (items 5-7). Still open: `remove_items`'s totalization
  behaviour (documented in the totalizations table, not re-verified against
  a second Java path); the `connection_items` order test doesn't actually
  pin order (needs a two-branch fixture); a redundant re-borrow at
  `query.rs:531`. *Resolved/superseded by Task 15:* Task 11's report also
  noted, as a minor, that its characterization board's outline shape
  (`PolylineShapeRef::Tile(TileShape::Box(...))`, matching
  `BoardServiceCharacterizationTest.java:101`) was worth double-checking
  against a wider fixture; Task 15's `p2t15` sweep (10 seeds × 2 sizes,
  0 diff in all 20 runs) now exercises far more outline/board-shape
  combinations than that one fixture, superseding the narrower note.
- **Task 12:** `DefaultHasher` is a same-process contract only, not
  cross-Rust-version stable — acceptable per quirk #78's own reasoning
  (nothing compares this hash across processes or languages), but worth
  restating since a naive reader might expect hash stability. Java's
  `BoardHistoryTest` was not ported as a standalone test file; it needs
  Plans 3/5/6 machinery (DSN import, DRC, autoroute) to be meaningful.
  > **Status (2026-09-01): both halves COMPLETE — Plan 7 Task 17's tick.**
  > (a) `BoardHistoryTest` **is** ported, by name, method for method:
  > `crates/fr-router/tests/board_history.rs:915-1060` (Plan 7 Task 2, ruling AF).
  > (b) The same-process reasoning was **re-audited rather than inherited**, under
  > controller ruling AH: `Board::structural_hash` was widened in Plan 7 Task 3 to the
  > field set `BasicBoard.serialize(true)` actually reaches — the whole item graph, not
  > the traces its javadoc claims (quirk #201) — with a row-per-field audit table in
  > `crates/fr-board/src/board/snapshot.rs`'s module doc and **no `?` cells**. The
  > acceptance is *decision* parity, never digest parity: `p7t10` runs the three Java
  > sites that compare two hashes (`BatchFanout:152-156`, `BoardHistory.contains`,
  > `BoardHistory.getRank`) over 10 boards x 8 scripts x 2000 scripted mutations and
  > reports **0 decision diffs**, hash-mode independent. Quirk #78's port column carries
  > the widened description. The five deliberately-skipped fields are the only judgement
  > calls and each is argued in the table.
- **Task 13:** A report file-length figure, a `p2t13` README row's
  placement/JDK header, and a test helper misnamed `Lcg` are all cosmetic.
- **Task 14:** "not ported: lives in fr-geometry" wording for
  `BigIntAux`/`Signum` could be sharper; the `saveForUndo` marker label
  undersells its actual callers; a report §5 sub-count has an arithmetic
  slip (report-only, not a code issue).
- **Task 15:** The deep-copy replay in `p2t15` cannot observe the quirk-#77
  tree-layout difference by design (no `toArray()` call in the driver); the
  round-trip test's overlap assertions are inert (no obstacle in the fixture
  actually hits the probe box); the entry-counter snapshot in the driver's
  output is output-neutral (present but never differs). (The coverage-hole
  notes that used to be duplicated across two `scripts/differential/README.md`
  sections are now written once, in the `p2t15` sweep section.)

## Obligations for later plans

**Plan 3 (DSN/KiCad import):**
- ~~**Ladder hang on import (quirk #76).**~~ **Partly discharged in Plan 3
  Task 10** (`82a647f` `feat(board): stop-checked normalisation for the DSN
  import path`, `d45676f` `feat(dsn): wiring scope, DsnReader
  read_board/read_metadata, stop-checked import normalisation`); **a second
  half is DISCHARGED in Plan 6 Task 10b and Plan 8 owns its last line.** Plan 3 took the second option, per its
  ruling 4: `fr-board` gained `normalize_all_traces_checked` /
  `normalize_traces_checked` / `normalize_trace_checked` /
  `split_trace_checked` / `connection_items_checked`, and `fr-dsn` runs
  `Wiring.java:347`'s `normalizeAllTraces()` under a `TimeLimit`-backed
  `StopCheck` (`DsnReadOptions::normalize_time_limit`, default 60 s), pushing
  Java's own `"Wiring: normalization of traces failed"` (Wiring.java:349) and
  continuing when it trips. **Ruling 4's stop-check placement turned out to be
  insufficient**: the real non-terminating site is not `split`'s entry re-walk
  but `Item.getConnectionItems`' walk along the contacts, which has no visited
  set — found in Task 10 and pinned as **quirk #106**. No fixture in the
  105-file corpus trips the limit, asserted directly by
  `every_fixture_in_the_corpus_matches_javas_result_and_warnings`.
  **Still open (Plans 6/7, controller ruling F):** `Wiring.readViaScope`'s
  `board.insertVia` (Wiring.java:706) reaches the same machinery by a second
  route — `BasicBoard.insertVia` walks `fromLayer..toLayer` calling
  `splitTraces` → `PolylineTrace.split` — and it sits *outside* the
  `try`/`catch` Java wraps `normalizeAllTraces` in, so the `StopCheck` does
  not reach it. Closing it means threading a `StopCheck` through
  `Board::insert_via`/`Board::split_traces`, whose other callers are the
  router. `// obligation:` marker at `crates/fr-dsn/src/parser/wiring.rs:596`.
  **Plan 6 Task 10b built exactly that** (`c4648d8`): `Board::insert_via_checked`,
  `Board::insert_escape_via_checked` and `Board::split_traces_checked`, with the
  three original names kept as delegating `|| false` wrappers so no existing
  caller moved. `insert_stops_when_the_stop_check_trips`
  (`crates/fr-router/tests/forced_via.rs`) pins a tripping check on a four-rung
  ladder answering `BoardError::Stopped`. **What remains is one line in `fr-dsn`
  and is Plan 8's** by controller ruling (accepted in Task 10b's review):
  `read_via_scope` still calls the unchecked wrapper.
- ~~**`ViaInfoId` renumbering across `ViaInfos::remove`.**~~ **Discharged in
  Plan 3 Task 14.** Java's `ViaRule` holds `ViaInfo` object references, so
  removing one from the middle of the list disturbs no rule. This port
  addresses via infos by index, so the same removal shifts every later index
  and can silently re-point a rule at the wrong via.
  `io/specctra/RulesReader.java:340-350` is the one non-GUI Java caller
  (remove-then-add-with-the-same-name); it now goes through
  `BoardRules::replace_via_info_renumbering_rules`, which removes, appends and
  renumbers in one step. Porting `RulesReader` also surfaced the **same hazard
  one level up** — `Network.addViaRule` removes a same-named `ViaRule` while
  `NetClass::via_rule` is a `ViaRuleId` — fixed alongside it as
  `BoardRules::replace_via_rule_renumbering_net_classes`. See the
  `docs/java-quirks.md` obligation-register row for the mapping.

  **Both methods have since been renamed, and the hazard has gone with them:**
  Plan 7 Task 0 made `ViaRule` own its `ViaInfo`s (`replace_via_info`) and Plan 7
  Task 11 made `NetClass` own its `ViaRule` (`replace_via_rule`), so neither
  method renumbers anything any more — there is no index left to renumber, and
  the "what it left open" paragraph below is **closed**.

  **What it left open, for Plans 6/7:** keeping every index resolvable also
  changes *which object* a rule reaches. Java's rule keeps the detached
  original after a replacement; the port's index necessarily reaches the
  replacement. This is reachable — `RulesReader` runs on a board that already
  holds via infos *and* via rules from the `.dsn` — and JVM-verified: on
  `Issue593-BBD_Mars-64.dsn` plus a one-line `.rules` re-declaring its `(via …)`
  with `attach`, the jar's via rules still reach `attach=false` while the port's
  reach `attach=true`. No Plan 3 writer can see it (both entries share a name,
  and the writers emit names), but `attach_smd_allowed`/`get_padstack`/
  `get_clearance_class_index` and a via rule's via list are router inputs.
  Filed as the open `docs/java-quirks.md` obligation row "Via-info / via-rule
  re-pointing".
- ~~**`Board::new` requires via-padstack population before any via lookup.**~~
  **Discharged in Plan 3** — Task 6 (`Structure.createBoard`, commit `cf888f3`)
  builds the board and Task 9 (`Network.readScope`/`readViaInfo`, commit
  `88edbbb` + `658e3c9`) populates the via padstacks before any via lookup,
  reproducing `Network.java:1286`'s `if (scopeParameter.viaPadstackNames !=
  null)` guard exactly: when the `structure` scope named no via padstacks the
  field stays `None` and `setViaPadstacks` is **skipped**, so the padstacks
  `readViaInfo` appended through `addViaPadstack` survive. That branch is pinned
  by `crates/fr-dsn/tests/data/network_via.dsn` and
  `a_network_only_via_padstack_list_survives_because_set_via_padstacks_is_skipped`,
  against a JVM golden. The KiCad reader (Plan 8) inherits the same obligation.
- ~~**`java_to_lower`/`java_to_upper` visibility gap.**~~ **Discharged in the
  final-review fix wave**, not carried into Plan 3: both are `pub` in
  `crates/fr-board/src/rules/mod.rs` and re-exported from `lib.rs` and the
  prelude, so `fr-dsn` can `use fr_board::{java_to_lower, java_to_upper};`
  exactly as ruling #7 intended. Left listed here so the trail from the ruling
  to the fix is readable.
- **Scope `audit-port.sh`'s `fn` match per Java class.** — **mechanism built and
  used in Plan 3 (Task 1's map support, Task 15's zero run); the `fr-board` half
  is still OPEN.** `audit-port.sh` now takes an optional 4th argument, a
  `<JavaClass> <rust-path-glob>` map; a mapped class is searched only under its
  mapped path(s), and an unmapped class falls back to the crate-wide search
  **and prints `UNMAPPED <Class>`**. `scripts/audit-map/fr-dsn.map` maps all 52
  classes Plan 3 ports (56 mapping lines) and all four `fr-dsn` invocations exit 0 with no
  `MISSING` and no `UNMAPPED`. **What Plan 3 did not do:** write a
  `scripts/audit-map/fr-board.map` and re-run Plan 2's nine directories under it
  — those are still audited with the 3-argument crate-wide form, so the caveat
  below still applies verbatim to `fr-board`. It is mechanical now; Plans 6/7
  should do it. — **DONE in Plan 6 Task 18**: `scripts/audit-map/fr-board.map`
  maps all 77 classes of Plan 2's nine directories, and all nine invocations run
  under the 4-argument form at zero `MISSING` and zero `UNMAPPED`. Writing it
  exposed seven places where the crate-wide search had been satisfied by a
  *different* class's `fn` or marker, all seven now fixed
  (`docs/plan-6-handoff.md` §5.6). The script was **not** weakened. The original text follows. The script's positive
  branch is `grep -rqE "fn <snake>…" <crate src>` over the *whole* crate, so
  for `Foo.getBar` any `fn get_bar…` anywhere satisfies the check — including
  one on an unrelated type. 357 of the 750 distinct `class`/`method` pairs in
  Plan 2's nine directories share a method name with another class in the same
  set, so for those the audit is collective, not per class. (The marker
  branches — `not ported:`/`renamed:`/`added in Task N:`/`added in Plan N:` —
  are exact and unaffected, and a wholly unported name still fails.) Plan 3
  should carry a class → Rust-type map (or a per-file `impl` scan) into the
  script so the positive match lands on the right type, and re-run the nine
  Plan 2 directories under the tightened script before adding `fr-dsn`'s own.
  **Do not weaken the script to make this go away.**

**Plan 5 (DRC/autorouter core)** — the two DRC bullets are settled by Plan 5
(`docs/plan-5-handoff.md`): the second is discharged, the first stays open by
design:
- **Quirk #82 (Delaunay in-circle degenerate on axis-aligned input) must
  not be fixed while porting `NetIncompletes`.** — **STILL OPEN, and now
  binding on Plans 6/7 as well.** It is a real, JVM-verified
  Java bug (square grids lose edges at every size; a 7×7 grid can
  disconnect), reproduced byte-for-byte by this port. Fixing it changes
  which airlines the ratsnest emits and therefore what the autorouter
  routes — a post-parity change only, once DRC parity against the Java
  engine is proven, with the ratsnest expectations re-baselined in the same
  commit. **Plan 5 honoured it**: `NetIncompletes` (Task 5, `16e573c`) was
  built on the unfixed triangulation, which is *why* the port's ratsnest
  matches the jar's edge for edge once the seed order is pinned (`p5t2`
  mode 3, 112/112 rows). The bug is now load-bearing in two crates rather
  than one, so the re-baseline it eventually needs is larger, not smaller.
- ~~**`Board::clearance_violations`/`clearance_violation_count` are markers,
  not implementations.**~~ — **discharged in Plan 5 Task 2** (`0275ad7`,
  with fix round `e527626`). `Item.clearanceViolations`
  (Item.java:363-469, `Via`'s override at Via.java:88-112,
  `calculateClearanceBetweenTwoShapes` at Item.java:471-493) and
  `clearanceViolationCount` (Item.java:357-361) were explicitly deferred
  (`items/mod.rs`, `connectivity.rs:992`) because they build
  `drc.ClearanceViolation` objects, which was Plan 5's DRC layer to define.
  They now live in `crates/fr-board/src/board/clearance.rs` with
  `ClearanceViolation` itself in `crates/fr-board/src/items/clearance_violation.rs`
  (plan-5 ruling 9 — `fr-board` cannot depend upward on `fr-drc`, so the type
  is defined here and re-exported as `fr_drc::ClearanceViolation`). All four
  take `&mut self` (plan-5 ruling 8): Java's method lowers `smallestClearance`
  and advances the search tree's entry counter, and hiding either behind
  interior mutability would make quirk #153 invisible. **Both
  `// added in Plan 5:` markers are consumed**, and `grep -rn "added in Plan 5"
  crates/` now returns nothing.
- ~~Apply Java's flag normalisation (`-oit /100`, `-mp`/`-mt` clamps,
  `-us`/`-is` folding) in `fr-settings`~~ — **discharged in Plan 4 Task 7**
  (`crates/fr-settings/src/sources/cli.rs`; see `docs/plan-4-handoff.md` and the
  register row in `docs/java-quirks.md`). What remains open is the *wiring*:
  `crates/freerouting/src/legacy.rs` still forwards raw values, which is Plan 8's
  one-call rewire. Note the plan numbering moved after Plan 2 was written:
  `fr-settings` is **Plan 4** and `fr-drc` is Plan 5, so this bullet was Plan 4's,
  not Plan 5's.

**Plan 6 (autoroute expansion rooms):**
- ~~**`RoomId`/`TreeObject` room ordering must be re-checked once rooms are
  real.**~~ — **discharged in Plan 6 Task 2.** `TreeObject::Ord` (`ids.rs`)
  already encoded Java's `CompleteFreeSpaceExpansionRoom.compareTo` (rooms sort
  before items, descending among themselves — quirk #44), verified only against
  the *absence* of rooms. Plan 6 Task 2 populated it:
  `ShapeSearchTree::insert_room`/`remove_room` put a
  `TreeObject::Room` leaf into the shared tree
  (`autoroute/maze/AutorouteEngine.java:534`,
  `CompleteFreeSpaceExpansionRoom.java:56-59`), and the ordering is asserted
  against a **real mixed tree** by
  `crates/fr-board/tests/expansion_room_tree.rs::a_room_goes_into_the_default_tree_and_sorts_before_every_item`
  and by `rooms_sort_before_items_and_descending_among_themselves` /
  `a_room_and_an_item_with_the_same_numeric_id_do_not_collide` /
  `a_room_enters_the_boards_own_compensated_tree_before_its_items` in
  `crates/fr-router/tests/expansion_rooms.rs`. Two caveats were **recorded, not
  fixed**: `compareTo`'s `instanceof`/cast mismatch (quirk #157) and the fact
  that room ids and item ids collide numerically, the order being total only
  because the type discriminator is the primary key. One obligation remains:
  `ShapeSearchTree`'s `tree_shape_of` and `ignore_object` still panic on a
  `TreeObject::Room`, so a room in the tree must not be reached by
  `overlapping_tree_entries` until Plan 6 Task 4 teaches those queries to
  resolve a room's shape and layer. — **that last obligation is discharged in
  Plan 6 Task 4**: the compensated queries resolve a room's shape and layer
  through the `ExpansionRoomStore`, and
  `a_room_bearing_tree_answers_queries_through_the_room_lookup`
  (`crates/fr-router/tests/sorted_neighbours.rs`) is the test.
- ~~**`ShapeSearchTree::complete_shape`/`divide_large_room` are not written at
  all**~~ — **discharged in Plan 6 Task 3** (`131211f`): both are
  `fr_router::AutorouteSearchTreeExt` methods over `ShapeSearchTree`, in all
  three angle regimes, and the two markers named below became `renamed:` markers
  naming that trait. `./scripts/audit-port.sh board/searchtree
  crates/fr-board/src` still exits 0. The original text follows. There were two
  deferral markers at the end of
  `searchtree/shape_search_tree.rs` (~:1625-1635) recording what belongs there
  and why it cannot land yet (both take and return
  `IncompleteFreeSpaceExpansionRoom`, which is `autoroute/expansion`). The
  markers name `ShapeSearchTree.java:580-693`,
  `ShapeSearchTree45Degree.java:95-281`, `ShapeSearchTree90Degree.java:38-191`
  and `ShapeSearchTree.java:1095-1118`/`ShapeSearchTree45Degree.java:288-298`.
- ~~**`AutorouteInfo` is an opaque placeholder**~~ — **discharged in Plan 6
  Task 1** (`9b63cb1`): it now holds `start_info: bool`,
  `precalculated_connection: Option<ConnectionId>` and
  `expansion_rooms: Vec<Option<ObstacleRoomId>>` (plan-6 ruling 15 — ids, not
  objects, because `fr-board` cannot name `fr-router`'s types), with the two new
  id newtypes beside `RoomId` in `ids.rs`. The original text follows.
  `AutorouteInfo` was an opaque placeholder (`items/header.rs`) standing
  in for `autoroute.ItemAutorouteInfo`; Plan 6 gives it a real body. It is
  reachable only through `ItemHeader::autoroute_info`/`get_autoroute_info`
  so that `Board::deep_copy` can drop it wholesale (`clear_autoroute_info`),
  matching Java's per-run scratch-data semantics.
- **`ShapeSearchTree::get_tree_shape`'s `&self` cold-cache recompute path
  cannot replicate Java's `clearDerivedData()` side effect** (dropping
  `autorouteInfo`), because it only has `&self` (Task 11 NOTE). Re-check this
  once autoroute scratch is a real, populated field rather than the empty
  `AutorouteInfo` placeholder.
- **Per-node release `assert!` in the `overlaps` hot loop has an
  uncharacterised cost** (Task 3 minor, deferred): measure once Plan 6's
  autoroute-scale trees exist; soften to `debug_assert!` only if profiling
  shows it matters and Java's own crash-on-corruption behaviour (quirk #39)
  is preserved some other way.

**Plan 7 (shove/tighten/forced-via routing):**
- ~~**Quirk #74's early return is a decision, not a default.**~~ — **decided,
  and the opposite way round: quirk #74 is now REPRODUCED** (Plan 6 Task 17b,
  controller rulings AD and AE, `ead7902`). This bullet's premise — "the port's
  `Line` is a `Copy` type with no identity to base Java's reference comparison
  on" — was the thing that had to change. `fr_geometry::Line` now carries a
  private identity token drawn from a process-wide `AtomicU64`, and
  `Board::change_trace` compares with `Line::is_same_object`. It is not
  cosmetic: comparing by value kept a different number of lines, which changed
  the search-tree shape, which made the DAC2020 board diverge from the jar at
  `ripupPassNo >= 2`. With the token the acceptance ladder is 15/15 at passes 1,
  2 and 4. **The contract Plan 7 must keep** is in `docs/plan-6-handoff.md` §9:
  a new token wherever Java allocates a new `Line`, the same token wherever Java
  passes the same reference on. The original text follows. `Board::
  change_trace` compares polylines by *value* (the port's `Line` is a
  `Copy` type with no identity to base Java's reference comparison on),
  which lets it take an early return Java's identity comparison never can.
  `change_trace` has no production caller in Plan 2 — Java's are
  `correctConnectionToPin` and `TraceShover`, both Plan 7. **Recommended
  in the quirk's own text:** drop the port's early return so `changeEntries`
  + `normalize` always run, since Java only takes its early return for an
  array identity-identical to the one already stored — not a case that can
  arise for a `Copy` value type.
- ~~**`equals_geometric` at the `TraceTightener` call sites remains entirely
  open**~~ — **discharged in Plan 6 Task 15a** (`cd202d0`, controller ruling AB
  moved the whole tightener family into Plan 6): all four `Line.equals` sites use
  `equals_geometric`, never `==`. The original text follows (see ruling — not a
  correction — above): `Line.equals`
  (quirk #34) has four Java call sites; one (`Simplex::border_line_index`)
  predates Plan 2 in `fr-geometry`; the other three
  (`TraceTightener.repositionLine`, `TraceTightenerAnyAngle.repositionLine`
  ×2) are untouched and must use `equals_geometric`, never `==`, when
  ported — using `==` there would silently disable the guard those methods
  exist for.
- ~~**`RoutingBoardExt` is the shove/pull-tight extension trait**~~ —
  **built in Plan 6, not Plan 7** (plan-6 ruling 3, Task 9, `a3bb81b`), because
  the maze search reaches `checkForcedTracePolyline` directly; controller rulings
  AA and AB then brought the mutating halves and the whole tightener family down
  with it. What is left for Plan 7 is `opt_changed_area` and the pull-tight tail
  of `removeItemsAndPullTight` (`docs/plan-6-handoff.md` §10).
  > **Status (2026-09-01): COMPLETE — Plan 7 Task 17's tick.** `RoutingBoardExt` gained
  > `opt_changed_area` and `opt_changed_area_with_keep_point` (Task 5, `ae8d003`),
  > `remove_items_and_pull_tight` (Task 8, `b855b4c`) and `fanout` (Task 11, `a19d4c5`).
  > Nothing of the trait is deferred: the `fr-board` markers that pointed here are all
  > `// renamed:` lines now, and `grep -rn "added in Plan 7" crates/*/src` is empty.
  The original text
  follows (plan ruling #4, `fr-router`): `Trace.pullTight`/`PolylineTrace.pullTight`
  and the `TraceShover` family are marked `// added in Plan 7:` in
  `items/trace.rs` and land there, not in `fr-board`.
- **`changed_area`'s reset depends on `deep_copy`, not on being cleared
  independently.**
  > **Status (2026-09-01): honoured — Plan 7 Task 17's tick.** Both dereferencing callers
  > exist now (`TraceShover::insert`, `Board::change_trace`) and neither clears
  > `changed_area` independently. Plan 7 Task 5 added a third class of caller,
  > `RoutingBoardExt::opt_changed_area`, which *does* clear it — that is Java's own
  > `RoutingBoardOperations.optChangedArea:78` (`board.changedArea = null`), pinned by
  > `crates/fr-router/tests/opt_changed_area.rs`'s
  > `the_changed_area_is_cleared_after_the_sweep`. Note also plan-6 §10's louder warning:
  > every `route_connection` caller must `start_marking_changed_area()` first (quirk #177).
  `Board::deep_copy` resets `changed_area` to `None`
  because a stale non-`None` value surviving a copy would corrupt the next
  autoroute pass's bookkeeping (`board/snapshot.rs` module doc); Plan 7's
  `TraceShover`/`PolylineTrace.change` are the two dereferencing callers, and
  neither exists yet — this is a documented dependency for Plan 7 to build
  against, not a bug.
- **`catch_unwind`/`Result` recovery boundaries** at
  `AutoroutePassRunner.java:144` (per pass) and
  `BatchAutorouterThread.java:537` (per item) —
  > **Status (2026-09-01): DISCHARGED, and both citations were wrong — Plan 7 Task 17's
  > tick.** `:144` closes the **dead** `runMultiThread` (`:40-149`); `runSingleThread`
  > (`:151-336`) has its own `try` at `:156` and `catch (Exception e) { … return false; }`
  > at `:331-335`, which Plan 7 Task 9 ported as one `catch_unwind` around the whole body,
  > degrading to `Ok(false)`. `BatchAutorouterThread.java:537` is on a class with **zero
  > live callers in the entire Java tree** (quirk #143, extended by Task 17), so that
  > boundary does not exist on any live path and there is nothing to build. Plan 7 added a
  > second boundary the plan did not anticipate: `RouterError::NoRoutableLayer`
  > (`AutorouteBatchLoop.java:44-56`, Task 10), the only *propagating* one. Table in
  > `crates/fr-router/README.md`.
  Java's `catch (Exception)`
  recovers and continues routing where several ported panics (Java NPE
  equivalents: `PolygonShape.intersects` stack overflow, `Polyline`
  normalisation, `TileShape.rotateApprox`) would otherwise abort the whole
  run. Both Java boundaries catch `Exception`, not `Throwable`, so they do
  **not** recover from a `StackOverflowError` either (quirk #27) — that one
  crashes the whole run in both languages, and a Rust panic there is
  correctly fatal, not a gap.

**Plan 8 (MCP/CLI polish, carried from Plan 1, unaffected by Plan 2):**
- MCP concurrency (progress sink, cancel token, reader thread) and legacy-CLI
  value normalisation — both already tabulated in `docs/java-quirks.md`'s
  candidate/obligation table and untouched by this plan.

## Evidence

**Differential harness** (`scripts/differential/`, `JDK 25` for these six
drivers — see `scripts/differential/README.md` "Requirements"):

| Driver | Coverage | Modes | Result |
|---|---|---|---|
| `p2t3` | `ShapeTree`/`MinAreaTree`, fixed script | — | 132 lines, 0 diff — exact match |
| `p2t3r` (400 ops, seed 42, mode 0) | randomised, orthogonal (`IntBox`) bounds | mode 0 | 109,940 lines, 0 diff |
| `p2t3r` (2000 ops, seed 42, mode 1) | randomised, 45°/`IntOctagon` bounds, `insert_tiles`/`remove_opt`/in-place re-keying | mode 1 | 2,898,938 lines, 0 diff |
| `p2t10` | `ShapeSearchTree`/`SearchTreeManager` | 9 modes (0-8) | every mode exact match, 17-88 lines each |
| `p2t11` | `Board` — insert/remove, connectivity, normalisation, snapshots | modes 0-11 (12) | modes 0-10 exact match (5-66 lines each); **mode 11** (20 lines) has 2 documented diff lines — `treeArrayCopy`/`treeArraysEqual` only, the deliberate tree-rebuild-vs-clone divergence (ruling #16/quirk #77); every other line (`transientBefore`/`transientOriginalAfterCopy`/`transientCopy`/`overlappingObjects`/`hashEqual`/`diffTraces`) matches |
| `p2t13` | `PlanarDelaunayTriangulation` | 8 modes (0-7) | mode 0 (50 points): 141 lines, 0 diff; modes 1-7 (30 points, seed 7): 7-172 lines each, 0 diff — includes quirk #82's square/grid reproduction |
| `p2t15` | randomised board-scale sweep: pin/via/trace insertion, `normalizeAllTraces`, 100 `overlappingObjects` + 50 clearance queries, `deepCopy` + re-dump + replay, `hashEqual` | 10 seeds × {30, 120} = 20 runs | every run 0 diff, 499-1111 lines each; re-confirms `p2t10` (9 modes)/`p2t11` (11 modes)/`p2t13` (mode 0) unregressed alongside it |

The one non-zero-diff cell across all six drivers (`p2t11` mode 11's two
lines) is the single place this plan's own choice diverges from Java on
purpose (ruling #16), not an unexplained gap.

**Test counts** (verified by running `cargo test` on the committed tree,
not taken from any report):
- `fr-board`: 575 passed, 0 failed, 1 ignored (`a_four_rung_ladder_never_
  finishes_normalizing`, the quirk-#76 non-terminating reproduction — the
  test exists precisely because it cannot pass), across 1 unit-test binary
  and 10 integration-test files.
- Whole workspace (`fr-board`, `fr-geometry`, `freerouting`, `parity`): 854
  passed, 0 failed, 1 ignored.
- `scripts/audit-port.sh` runs clean (exit 0, zero missing members) against
  all nine Java source directories this plan ports: `board/model/items`,
  `board/model/structure`, `board/facade`, `board/searchtree`, `board/trace`,
  `board/state`, `rules`, `core/library`, `datastructures`.
  **Read that zero precisely.** The script's positive match is *crate-wide by
  name*: for `Foo.getBar` it accepts any `fn get_bar…` anywhere under
  `crates/fr-board/src`, with no check that the `fn` sits on the Rust type
  that stands in for `Foo`. Java method names repeat heavily — 357 of the 750
  distinct `class`/`method` pairs these nine directories declare share a
  method name with at least one other class in the same set — so for those the
  audit proves the name is ported *somewhere*, collectively, not per class.
  The `not ported:`/`renamed:`/`added in Task N:`/`added in Plan N:` marker
  branches *are* exact (they match the Java method name verbatim), and a name
  nobody ported at all still fails the audit; the per-class evidence is the
  Java citation carried in every ported body's doc comment plus the
  differential drivers. Scoping the `fn` match per Java class is a Plan 3
  obligation (below).

## Open items for the user

- ~~**`java_to_lower`/`java_to_upper` have no visibility modifier at all**~~ —
  **done in the final-review fix wave.** Both are now `pub` in
  `crates/fr-board/src/rules/mod.rs` and re-exported from `lib.rs` and from
  `fr_board::prelude`, so Plan 3's `fr-dsn` can reuse them
  (`use fr_board::{java_to_lower, java_to_upper};`) instead of duplicating the
  ~40-line helper, which is what the plan-wide ruling that created them
  (Task 2, ruling #7) intended. Their string-level siblings
  `equals_ignore_case`/`compare_to_ignore_case` are still `pub(crate)`; widen
  them the same way if `fr-dsn` wants the whole-string fold.
- **`p2t11` mode 11's tree-rebuild-vs-clone divergence (quirk #77) was
  accepted, not defaulted into**, and is worth the user's own read: this
  port's `deep_copy` is *more* faithful to the pre-copy board than Java's own
  round trip (which changes tree shape on every clone via reinsertion), but
  it means a raw `ShapeTree.toArray()` comparison between the two engines
  will never match after a copy. Nothing ported observes that order today.
- Post-parity improvement candidates remain listed in `docs/java-quirks.md`
  (Delaunay's exact in-circle predicate, `i128` fast paths inherited from
  Plan 1) and `docs/geometry-library-survey.md` (now amended: `i_overlay` is
  off the table entirely unless a renderer is ever built).
- Plan 2's final whole-branch review has not yet run; this task's own
  verification (fmt, `cargo doc`, full test suite, audit script, evidence
  cross-check against the committed tree) is not a substitute for it.
