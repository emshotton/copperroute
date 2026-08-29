# Plan 3 hand-off — Specctra DSN/SES/rules text I/O (`fr-dsn`)

Branch `plan-3-dsn`, 31 commits on top of `main` (plus the plan document itself,
`16516e8`) before this task's commit. The whole-branch final review has not yet
run; this task closes out the plan's own documentation obligations first.

**Read this before Plans 4–8.** Every later plan touches `fr-dsn`: Plan 4 owns
the `RouterSettings` this crate stands in for, Plan 5 consumes
`BoardMetadata`, Plans 6/7 inherit two open hazards this plan found and
deliberately did not close, and Plan 8 wires this crate's public surface plus
the KiCad/Eagle paths that were deferred out of it.

## Delivered

- **`fr-dsn`**: 19.6k lines across `format` (Java `Double.toString`/
  `Float.toString`, `IdentifierType`, `IndentFileWriter`), `lexer` (the
  transcribed JFlex DFA plus the hand-rolled `nextString`/`nextStringList`/
  `nextDouble` bypass), `keyword` (`Keyword`/`ScopeKeyword`),
  `coordinate_transform`, `parser` (the twelve scope families: `dsn_file`,
  `header`, `geometry`, `structure`, `autoroute_settings`, `library`,
  `part_library`, `placement`, `network`, `wiring`, `scope_parameter`),
  `dsn_reader`, `dsn_writer`, `ses_reader`, `ses_writer`, `rules_reader`,
  `rules_writer`, `error`. Dependencies are exactly the three the plan allowed:
  `fr-board`, `fr-geometry`, `thiserror` (dev: `parity`). No `serde`, no
  `regex`, no `tracing`.
- **Byte parity**: all seven `tests/reference/*/roundtrip.dsn` **and** all seven
  `tests/reference/*/unrouted.ses` are byte-for-byte identical to the pinned
  2.3.0 jar's output.
- **Corpus parity**: 105 fixtures read to the same `BoardReadResult` variant and
  the same complete warning list as the jar (104 `Success`, 1 `ParseError`); a
  530-pair `p3t15` sweep (106 fixtures × 5 modes) over the reader and all three
  writers has **0 unexpected diffs** and 5 documented expected ones.
- **`docs/java-quirks.md`**: 30 new pinned quirks (**#84–#113**), 55 new
  totalization rows, and four new obligation-register rows.
- **Differential harness**: three new driver pairs — `p3t2` (number
  formatting), `p3t3` (the token stream), `p3t15` (reader + all three writers) —
  plus `scripts/differential/sweep-p3t15.sh`.
- **`scripts/audit-port.sh` per-class matching** (the Plan 2 obligation) and
  `scripts/audit-map/fr-dsn.map`; all four `fr-dsn` invocations exit 0 with no
  `MISSING` and no `UNMAPPED` line.
- **Three `fr-board` additions**, all additive (no existing signature changed):
  the `*_checked` stop-check family (`normalize_all_traces_checked`,
  `normalize_traces_checked`, `normalize_trace_checked`, `split_trace_checked`,
  `connection_items_checked`), `BoardRules::replace_via_info_renumbering_rules`
  / `replace_via_rule_renumbering_net_classes`, and the `SpecctraParserInfo` /
  `WriteResolution` fields on `Communication`.
- **257 tests in `fr-dsn`** (unit + fourteen integration suites; 2 `#[ignore]`d
  in debug), **1119 across the workspace** (3 ignored in total) — measured on
  commit `9cdf4f7` and unchanged at `b85e674`, the branch tip (that commit is
  documentation-only). (At `b0e8e38`, two commits earlier, the same counts were
  256 and 1118; Task 15's review round added ruling I's non-ignored sibling
  test. Re-measure rather than trusting either figure.)

## Public API surface (what Plan 8 wires up)

Every entry point takes `impl Read` / `&mut W` rather than a path, and every
writer takes a `&CoordinateTransform` explicitly (see the deviation below).

```rust
// crates/fr-dsn/src/dsn_reader.rs
pub fn read_board(
    input: impl Read,
    id_generator: Option<ItemIdGenerator>,
    design_name: Option<&str>,
    options: &DsnReadOptions,
) -> BoardReadResult;

pub fn read_metadata(input: impl Read) -> BoardReadResult;

// crates/fr-dsn/src/dsn_writer.rs      (re-exported as `fr_dsn::write`)
pub fn write<W: Write>(
    board: &Board,
    ct: &CoordinateTransform,
    out: &mut W,
    design_name: &str,
    compat_mode: bool,
) -> io::Result<()>;

// crates/fr-dsn/src/ses_writer.rs      (only as `fr_dsn::ses_writer::write`)
pub fn write<W: Write>(
    board: &Board,
    ct: &CoordinateTransform,
    out: &mut W,
    design_name: &str,
) -> io::Result<()>;

// crates/fr-dsn/src/ses_writer.rs      (only as `fr_dsn::ses_writer::snapped_endpoint`)
pub fn snapped_endpoint(board: &Board, wire_id: ItemId, start_side: bool) -> Option<FloatPoint>;

// crates/fr-dsn/src/ses_reader.rs      (only as `fr_dsn::ses_reader::read`)
pub fn read(
    input: impl Read,
    board: &mut Board,
    ct: &CoordinateTransform,
) -> Result<SesImportSummary, DsnError>;

// crates/fr-dsn/src/rules_reader.rs    (only as `fr_dsn::rules_reader::read`)
pub fn read(
    input: impl Read,
    design_name: &str,
    board: &mut Board,
    ct: &CoordinateTransform,
    target_settings: Option<&mut DsnRouterSettings>,
) -> Result<bool, DsnError>;

pub fn read_router_settings(input: impl Read) -> Result<Option<DsnRouterSettings>, DsnError>;

// crates/fr-dsn/src/rules_reader.rs    (only as `fr_dsn::rules_reader::discover_layer_structure`)
pub fn discover_layer_structure(text: &str) -> Result<DsnLayerStructure, DsnError>;

// crates/fr-dsn/src/rules_writer.rs    (only as `fr_dsn::rules_writer::write`)
pub fn write<W: Write>(
    board: &Board,
    ct: &CoordinateTransform,
    settings: Option<&DsnRouterSettings>,
    out: &mut W,
    design_name: &str,
) -> io::Result<()>;
```

`BoardReadResult` is Java's sealed four-variant result verbatim —
`Success { board, metadata, warnings, coordinate_transform }`,
`OutlineMissing { … }`, `ParseError { … }`, `IoError(_)` — with **one added
field** (`coordinate_transform: Option<CoordinateTransform>`, `None` when
`create_board` never ran; ruling A). `read_board` returns `Success` with
`metadata: None`; only `read_metadata` fills it (quirk-family note, see the
rulings). Everything else the crate exports is in `fr_dsn::prelude`.

**Naming note for Plan 8.** Only the DSN writer reached the crate root as
`fr_dsn::write`; the SES and rules entry points are module-qualified
(`fr_dsn::ses_writer::write`, `fr_dsn::ses_reader::read`,
`fr_dsn::rules_reader::read`, `fr_dsn::rules_writer::write`). If a surface wants
all four at the root they need distinct names (`write_dsn` / `write_ses` / …),
which is a rename of an existing public item — a Plan 8 decision, not a defect.

### The `CoordinateTransform` deviation (controller ruling A)

Java hangs the transform off the board-reading side and reaches it through
`WriteScopeParameter`. This port **threads it explicitly** through every
signature above. `fr-board` cannot depend on `fr-dsn` (spec §7 puts
`CoordinateTransform` in `fr-dsn`), so `Board` cannot carry one. Consequence for
Plan 8: whatever holds a `Board` between a read and a write must also hold the
`CoordinateTransform` the read produced, or DSN/SES output will be in the wrong
units. `read_board`'s `Success`/`OutlineMissing` hand it back for exactly that
reason. The KiCad reader (Plan 8) shares the same type and inherits the same
obligation.

## Rulings

### Plan rulings 1–11 (made while writing the plan) — and what execution did with them

1. **The fifteen camelCased `Keyword` names at the clone's HEAD are a
   regression; the port emits the 2.3.0 Specctra tokens.** *Held, and it is the
   single most load-bearing constraint in the plan.* Task 3's DFA transcription
   and Task 4's `Keyword::name()` table both use the snake_case strings, and
   Tasks 11–14's writer literals were transcribed one by one against them. Pinned
   as **quirk #92** with the JVM evidence (HEAD cannot read back its own output:
   re-reading HEAD's `tutorial_board` round trip drops `host_cad`/`host_version`,
   every via rule, every per-type clearance rule and 26 wires) and restated in
   the known-diffs table below so nobody "updates" the references to HEAD's
   spelling. **Cost if wrong:** the port emits DSN no tool can read — loud and
   immediate, which is why the ruling is cheap in the safe direction.
   *(`tests/reference/README.md` carries a restatement of this ruling next to
   the references themselves, added in commit `b85e674` —
   §"Why the 2.3.0 jar, not the clone's HEAD (Plan 3 ruling 1)".)*

   In full, so the reference set cannot be "corrected" by someone who only reads
   that directory: HEAD spells fifteen `Keyword` constants in camelCase
   (`autorouteSettings`, `clearanceClass`, `hostCad`, `hostVersion`,
   `logicalPart`, `pullTight`, `shoveFixed`, `snapAngle`, `startRipupCosts`,
   `stringQuote`, `useLayer`, `useVia`, `viaCosts`, `viaRule`,
   `writeResolution`), and the rename leaked into the **writers' string
   literals** (`Parser.java:102` writes `"(stringQuote "`). The DFA is unchanged,
   so HEAD cannot read back its own output. Running
   `scripts/gen-reference/RefWriter.java` against the clone's own jar produces
   output whose diff against the committed 2.3.0 references is **exactly those
   fifteen tokens and nothing else — zero remaining diff lines** on all four of
   the original fixtures. `tests/reference/` is the spec §3 acceptance target;
   do not regenerate it from HEAD.
2. **Read lexemes and write literals are two independent tables, and neither is
   `Keyword.getName()` alone.** *Held exactly.* Three separate things: the DFA's
   lexeme→`Keyword` map (`lexer/tables.rs`), `Keyword::name()` (`keyword.rs`),
   and each writer's literal strings. Task 11's fix round found the practical
   consequence — several citations pointed at the wrong `writeScope` — and
   re-derived every one.
3. **The lexer's DFA is ported by transcribing JFlex's packed tables.**
   *Held.* `scripts/gen-lexer-tables.py` decodes the five packed strings into
   `src/lexer/tables.rs` (3,360 lines, generated); only the driver loop and the
   action switch are hand-written. `p3t3` matches Java's token stream on all 105
   `.dsn`, all 12 `.ses`, all 7 `.rules` and all 14 reference files.
4. **`normalize_all_traces` at the end of a DSN read runs under a `StopCheck`.**
   *Held in principle, **corrected in placement**.* The plan named
   `split_trace`'s entry walk, `normalize_trace`'s recursion and the
   `normalize_traces` passes as the stop-check sites. Task 10 found that the
   actual non-terminating site is **`Item.getConnectionItems`' walk along the
   contacts, which has no visited set** — pinned as **quirk #106** — so
   `connection_items_checked` had to be added too. The warning string
   (`"Wiring: normalization of traces failed"`, Wiring.java:349) and the
   configurable limit (`DsnReadOptions::normalize_time_limit`, default 60 s) are
   as planned. The plan's mitigation held: **no fixture in the 105-file corpus
   trips the limit**, asserted directly by
   `every_fixture_in_the_corpus_matches_javas_result_and_warnings`.
   **What it did not close:** ruling F, below.
5. **`AutorouteSettings` gets a `fr-dsn`-local `DsnRouterSettings`, not a
   forward reference to Plan 4's `RouterSettings`.** *Held.* Two
   `// obligation: settings/RouterSettings.java` markers
   (`parser/autoroute_settings.rs:34`, `:278`) and a register row in
   `docs/java-quirks.md`. Two things Plan 4 must not assume: `DsnRouterSettings`
   has **no `Eq`** (it holds `f64`s), and `apply_new_values_from` copies only the
   fields this type carries — the two per-layer `double[]` cost arrays are *not*
   copied onto a target that already has them. **Cost if wrong:** bounded — one
   `From`/`Into` pair.
6. **Java's two `static` lexer fields become instance state.** *Held.*
   `scope_identifier` is an instance field; `nf` became the stateless free
   function `java_number_format_parse`. Recorded as a deviation row in the
   obligation/candidate table, not as a bug fix. **Cost if wrong:** none
   observable — Java's own warning text already gets the interleaved case wrong.
7. **`PartLibrary` is ported only as far as DSN write needs.** *Held* (Task 7:
   `read_scope` populating `BoardLibrary::logical_parts`, `write_scope` in full).
   **Caveat found:** no corpus fixture has a `part_library` scope at all, so
   the writer has no reference round trip behind it — only a synthetic
   `RefWriter` comparison. See Residuals.
8. **`Circuit`/`PlaceControl`/`KiCadNetClassNames` are ported at the scope level,
   in full.** *Held.* `readBoardMergesKicadDefaultIntoFreeroutingDefault` passes.
9. **Bit-parity tests live in `crates/fr-dsn/tests/parity_{dsn,ses}.rs` with
   `parity` as a dev-dependency.** *Held.*
10. **Plan 3's differential drivers link against `tools/freerouting-2.3.0.jar`,
    not the clone-HEAD jar Plan 2's `p2t*` drivers use.** *Held* — and it is
    what makes ruling E's expected diff legible instead of mysterious.
11. **`Double.toString` is emulated as "shortest digits from Rust's `{:e}`,
    reformatted by Java's rules".** *Held and over-verified.* `p3t2` ran
    10 M doubles (mode 0, seed 42) plus 3 M in modes 1–3 plus a 13 M second-seed
    pass: **0 mismatches**. Task 2's own caveat stands: this rests on Rust's
    `{:e}` being a correct shortest-round-trip formatter, so **re-run `p3t2`
    after any toolchain bump that touches float formatting**.

### Controller rulings A–I (made during execution)

**A. `CoordinateTransform` is threaded explicitly.** Task 1 declares
`BoardReadResult` without it (the type did not exist yet); Task 10 adds
`coordinate_transform: Option<CoordinateTransform>` to `Success` and
`OutlineMissing`; `DsnWriter::write` and `SesWriter::write` take it as a
parameter. *Reason:* `fr-board` cannot depend on `fr-dsn`, and spec §7 puts
`CoordinateTransform` in `fr-dsn`. *Cost if wrong:* one extra parameter to plumb
in Plan 8.

**B. Plan ruling 1 (2.3.0 keyword tokens over HEAD's camelCase regression) is
accepted, with a required quirks row.** *Reason:* `tests/reference/` is the spec
§3 acceptance target and the snake_case tokens are what the Specctra format
uses. *Cost if wrong:* every bit-parity test fails on fifteen token names —
immediate. **Discharged, both halves.** The quirks row is **quirk #92** — the
camelCase name list, the "HEAD cannot read back its own output" evidence and the
fix ("revert the fifteen literals; the rename was an IDE refactor that leaked
into string literals"). The second half — a restatement in
`tests/reference/README.md`, so nobody who reads only that directory regenerates
the references from HEAD — landed as commit `b85e674`
(§"Why the 2.3.0 jar, not the clone's HEAD (Plan 3 ruling 1)").

**C. `SpecctraParserInfo`/`WriteResolution` live on `fr-board`'s
`Communication`, as in Java.** They had been placed in `fr-dsn` by Task 5, which
would have left a `fr-board` obligation permanently open; `write_parser_scope`
now takes them from `Communication`, `ReadScopeParameter` keeps flat fields and
Task 10 assembles them. *Reason:* Java's own home for the data, and it closes
two `// added in Plan 3:` markers `fr-board` was carrying. *Cost if wrong:*
`Communication` grows four fields nothing else reads — cheap, and it is the
faithful layout.

**D. `Keyword::Pn`'s dead branch is documented, not written.**
`Component.readPlaceScope` tests `nextToken == Keyword.PN` by identity against a
keyword the DFA never returns (**quirk #98**); `Pn` is deliberately absent from
the port's enum, so the branch is recorded with a marker and a quirks row rather
than reproduced. *Reason:* writing an unreachable arm for a variant that does
not exist would be fiction. *Cost if wrong:* if some input could produce a `PN`
token, the port would skip a branch Java takes — ruled out by the DFA tables
themselves.

**E. The 2.3.0 jar and the clone's HEAD differ in `DsnFile.readStringScope`;
the port follows HEAD, and `Issue229-display-8-digit-hc595.dsn` is an *expected*
differential diff.** HEAD added a resync loop (`while (nextToken != null &&
nextToken != CLOSED_BRACKET)`) that 2.3.0 lacks. On that one fixture — whose
`(PN "DISPLAY 7-SEG 0.5"")` has an unbalanced string quote — 2.3.0 desynchronises
and drops everything after the `structure` scope (1 item, 0 components, 0 nets)
while HEAD and this port read on (22 components, 30 nets). *Reason:* the plan's
Java source authority is HEAD except for ruling 1's fifteen literals. *Cost if
wrong:* one corpus fixture builds a different board than the pinned jar — which
is why it is listed in the known-diffs table rather than hidden. **Positive
evidence that the divergence is in the parser and not the scanner: `p3t15`
mode 4 (the raw token stream) MATCHes on that file.**

**F. The `insert_via → split_traces` unchecked stop path is an obligation for
Plan 7/hardening, not a Plan 3 fix.** `Wiring.readViaScope`'s `board.insertVia`
(Wiring.java:706) reaches quirk #76's machinery by a second route, outside the
`try`/`catch` ruling 4's `StopCheck` lives in. *Reason:* closing it changes
`Board::insert_via`'s signature, whose other callers are the router; no fixture
trips it. *Cost if wrong:* a hostile or unlucky DSN whose vias sit on a
four-rung ladder can still wedge the reader. `// obligation:` marker at
`crates/fr-dsn/src/parser/wiring.rs:596`. **OPEN.**

**G. Task 15 adds trace/via/plane-bearing fixtures to `tests/reference` so
writer coverage is durable.** Task 11 had proved six extra fixtures byte-exact
*ad hoc*, but that evidence was not in the repository. Three stems
(`Issue413-test`, `Issue110-RelayModule`, `Issue753-CPU-85_r104`) were added to
`fixtures.txt` and generated with the committed script. *Reason:* the original
four fixtures have zero `polyline_path`, zero wiring vias, zero fixed states and
zero `(plane …)` between them. *Cost if wrong:* ~450 KB of committed
references. Discharged in Task 15.

**H. The via-info / via-rule *re-pointing* divergence is real and observable;
keep the index model and re-file it as an OPEN Plan 6/7 obligation.** Task 14's
renumbering fix keeps every index resolvable, but it changes *which object* a
rule reaches: Java's `ViaRule` keeps the detached original after a
`RulesReader.applyViaInfo` replacement, the port's index necessarily reaches the
replacement. JVM-verified on `Issue593-BBD_Mars-64.dsn` plus a one-line
`.rules`: the jar's rules reach `attach=false`, the port's `attach=true`.
*Reason:* keeping a detached object is not expressible in the index model, and
reverting the model for one reachable-but-rare case is disproportionate.
*Cost if wrong:* **writer-visible: never** (both entries share a name and every
writer emits names, so no `.dsn`/`.ses`/`.rules` byte differs);
**router-visible: yes** — `ViaInfo::attach_smd_allowed`, `get_padstack`,
`get_clearance_class_index` and a via rule's via list can all differ. Also
ruled in the same round: the three clone-HEAD-only APIs
(`readRouterSettings`, `discoverLayerStructure`, `applyNewValuesFrom`) are not
jar-pinned and must be **disclosed** rather than silently shipped. **OPEN.**

**I. The corpus test keeps its debug-only `#[ignore]`, and gains a skip-with-
message plus a non-ignored sibling.** `every_fixture_in_the_corpus_matches_javas_
result_and_warnings` takes ~90 s in debug and 11.7 s in release, so it stays
`#[cfg_attr(debug_assertions, ignore)]`; it now goes through
`parity::require_java_dir()` / `parity::java_dir()`, so it skips with a printed
message instead of panicking when the fixture directory is missing and honours
`FREEROUTING_JAVA_DIR`, and the non-ignored sibling
`the_corpus_golden_parses_and_the_named_fixtures_read_to_its_variant` runs in
every configuration. Landed in `9cdf4f7`.
*Reason:* a 90 s test in the default `cargo test` loop gets disabled by whoever
is in a hurry, and a silently-skipping suite is worse than a
slow one. *Cost if wrong:* **the full corpus check does not run in a plain
`cargo test`** — release/CI must run `cargo test -p fr-dsn --release --test
dsn_reader`, and a green debug run is not evidence of corpus parity. Stated here
and in the test inventory because it is the easiest gate in this plan to lose.

## Corrections to the plan discovered during execution

1. **Ruling 4's stop-check placement was insufficient** (Task 10). The hang site
   is `Item.getConnectionItems`, not `PolylineTrace.split`'s entry re-walk —
   quirk #106. `connection_items_checked` was added to close it.
2. **`Rule.java`'s readers moved once** (Tasks 6→8, commit `ecee8b0`).
   `Structure.readScope` cannot work without a rule reader, so `DsnRule` and
   friends first landed in `structure.rs` and were relocated to `network.rs`,
   which the plan assigns `Rule.java` to. The `network.rs → structure.rs`
   dependency (`read_via_padstacks`) is real and one-way.
3. **`Library.java`'s `instanceof Path` is `PolygonPath` *or* `PolylinePath`**
   (Task 15). Java's `Path` is the shared base of both, and the port had
   narrowed it to `PolygonPath`. Corrected for fidelity, but the
   `DsnShape::PolylinePath` arm is **not reachable in the port**, and that is
   documented at the site: `transform_to_board_rel` answers `None` for a
   `PolylinePath` (Java stores that `null` in the package's `Shape[]`, where
   `Package.writeScope` later NPEs on it — a totalized row), so the `continue`
   one line earlier drops such an outline before the width/closed branch runs.
   It is therefore **not** an open coverage gap; the Task 15 report's concern
   that it is "the only behaviour change / unpinned" is **withdrawn**.
4. **`SesReader` does not refuse an out-of-range wire layer; Java clamps it**
   (Task 13 fix round). `Trace`'s constructor clamps on both sides
   (`Math.min(Math.max(p_layer, 0), layerCount - 1)`, Trace.java:45-47), so a
   `pcb`-layer path lands on **layer 0** and counts as an imported wire.
   Measured on the jar, not argued.
5. **`Wiring.tryCorrectNet` takes the *highest*-id contact** (Task 10 fix
   round), because Java merges the contacts into a `TreeSet<Item>` ordered by
   `Item.compareTo` — descending id (quirk #44). The port's `BTreeSet` needed
   `.rev()`. This is Plan 2's correction #1 reappearing in a new collection.
6. **`read_integer_scope`/`read_float_scope` totalize to `0`/`0.0`, they do not
   error** (Task 4 fix round, Java-wins over the plan's own test list) —
   including the token-consumption asymmetry between the two. Quirk #90.

## Quirks pinned in Plan 3 (`docs/java-quirks.md` #84–#113)

| # | One line |
|---|---|
| 84 | `nextString`'s skip sets are `{8, 32}` etc. — **8 is backspace, tab (9) is absent**, though the comment says "spaces, tabs" |
| 85 | Two number grammars in one file: the DFA's exponent-aware `DecFloatLiteral` vs `nextDouble`'s lenient `NumberFormat`, so `1e5` is 100000.0 or 1.0 by call site |
| 86 | `zzBuffer` is a fixed 16 MiB array `nextString` indexes with no refill — an undocumented file-size cap, plus an unguarded read past the skip loop |
| 87 | `Keyword.GENERATED_BY_FREEROUTING` is unreachable from the scanner: action 118 exists, no table path reaches it |
| 88 | `PolygonPath.boundingBox` adds the offset **inside** the running `Math.max` on the x axis, so the upper x bound grows once per coordinate |
| 89 | `CoordinateTransform.boardToDsn(double)` with a zero scale factor gives `±Infinity`/`NaN` instead of throwing |
| 90 | `DsnFile.readIntegerScope`/`readFloatScope` warn and return `0`/`0.0` for a wrong-kind token, and the integer path leaves the closing bracket unread |
| 91 | `ScopeKeyword.skipScope` returns `false` (not an exception) at end of file, and every caller discards the boolean |
| 92 | **Ruling 1**: fifteen `Keyword` names are camelCase at HEAD and the rename leaked into the writers' string literals, so HEAD cannot read back its own output |
| 93 | `Circle.boundingBox` treats `coor[0]` as a **radius** where every other member treats it as a diameter — the box is 2× too large, and board sizing/scaling depends on it |
| 94 | `Structure.createBoard`'s `int scaleFactor /= 10` overflow loop truncates to 0 → division by zero → `Infinity` |
| 95 | `Structure.readScope` reads `autoroute_settings` only when it is the **first** layer-structure consumer in the scope |
| 96 | `Structure.setClearanceRule`'s two-entry branch ignores the entry it is iterating and computes the same pair twice |
| 97 | `PartLibrary.readLogicalPart`'s `readOk` flag is never assigned `false` — the guard is dead |
| 98 | `Component.readPlaceScope` compares a token against `Keyword.PN` by **identity**, against a keyword the DFA never returns (ruling D) |
| 99 | `Rule.readLayerRuleScope` accepts exactly one `(rule …)` per `layer_rule` |
| 100 | `NetClass.readClassClassScope` has no `skipScope` fallback, unlike its sibling |
| 101 | `Network.readScope` assigns a net class's `useVia` list **by reference** when the structure scope named no via padstacks |
| 102 | `Network.insertClassPairs` reuses the outer iterator for the inner loop |
| 103 | `Network.insertComponent` `return`s instead of `continue`ing, abandoning the whole component when one pin's padstack is missing |
| 104 | `Network.createViaRule` takes an `attachAllowed` it never reads |
| 105 | `Wiring.readViaScope`'s net-number loop never increments its index (**still unpinned by a test** — see Residuals) |
| 106 | `Item.getConnectionItems`' contact walk has no visited set — the real ladder-hang site (correction 1) |
| 107 | `Rule.writeNonDefaultClearanceRules`' outer loop runs one class past the end of the clearance matrix |
| 108 | `Net.writePin`'s "component not found" branch dereferences the null it just tested for |
| 109 | Two `Wiring` calls that cannot fail in Java but can in the port (`Polyline` normalisation and `insertTraceWithoutCleaning`'s null) |
| 110 | `SesWriter.writeConductionArea` mixes integer and floating-point coordinates in one SES scope (boundary int, holes double) |
| 111 | `RulesWriter.writeRules` writes the design name raw where `DsnWriter.writePcbScope` quotes the same string |
| 112 | `RulesReader.applyRules` applies a layer rule to **every** layer when the layer name is unknown |
| 113 | `Structure.setClearanceRule` splits at the string-quote character with a **regex**, not a literal — a live deviation, see Residuals |

Plus **55 new rows in the totalization table** naming `crates/fr-dsn/…` (Java crashes, the port returns a
value), of which one is *reachable*: `RulesWriter.writeRules` reads
`board.library.padstacks.count()` with no null check and `BoardLibrary.padstacks`
is `null` on any board read from a DSN with no `(library …)` scope, so the 2.3.0
jar throws `NullPointerException` on `fixtures/empty_board.dsn` while the port
writes a valid 20-line `.rules`. That is the one unexpected diff in the corpus
sweep; the `// totalized: RulesWriter.writeRules` marker sits on the loop bound
in `crates/fr-dsn/src/rules_writer.rs`.

Two rows in the candidate/deviation table record Plan 3's own deliberate
divergences: the `static` lexer fields (ruling 6) and `DsnScanner` converting
the whole input at construction instead of refilling a moving buffer window
(which loses only quirk #86's garbage; verified identical over 132 files).

## Test and fixture inventory

**Bit-parity references** — `tests/reference/`, seven stems in `fixtures.txt`,
each with `roundtrip.dsn` + `unrouted.ses` + `java.log`:

| stem | why it is in the set |
|---|---|
| `tutorial_board` | the plan's original set: structure/placement/library/network writers |
| `Issue026-J2_reference` | ditto |
| `Issue103-Board-Unrouted` | ditto |
| `Issue143-rpi_splitter` | ditto |
| `Issue413-test` | ruling G: 11 `polyline_path`, 4 wiring vias, 3 fixed states — the **only** SES reference with `(wire` entries |
| `Issue110-RelayModule` | ruling G: 22 wiring vias, 22 fixed states |
| `Issue753-CPU-85_r104` | ruling G: 3 `(plane …)`, 65 `polyline_path` |

Regenerate with `JAVA=/opt/homebrew/opt/openjdk@25/bin/java
scripts/gen-reference.sh` (idempotent for existing stems — only `java.log`'s
timestamp changes). **Never hand-edit a generated file.** The gate is
`parity_dsn.rs::every_reference_is_byte_for_byte_identical_to_java` and its
`parity_ses.rs` twin — raw bytes, not whitespace-normalised.

**Corpus test** — `crates/fr-dsn/tests/dsn_reader.rs`,
`every_fixture_in_the_corpus_matches_javas_result_and_warnings`: every `.dsn` in
`../freerouting/fixtures` (105 files) compared against
`tests/data/corpus-read-results.txt` (captured with `CProbe.java`) for the
`BoardReadResult` variant **and** the complete warning list, message for
message. 104 `Success`, 1 `ParseError`. It also asserts ruling 4's "no fixture
trips the normalisation stop-check" under a 30 s budget, half the default.

**The full corpus check runs only in release (controller ruling I).** The test
carries a debug-only `#[cfg_attr(debug_assertions, ignore)]` — ~90 s in debug,
11.7 s in release — so a plain `cargo test -p fr-dsn` does **not** run it.
`cargo test -p fr-dsn --release --test dsn_reader` does, and that is what CI and
any pre-merge gate must run; a debug-only run is not evidence of corpus parity.
It skips with a printed message when the sibling `../freerouting` checkout is
absent, and a non-ignored sibling test runs in every configuration so the suite
cannot silently become a no-op.

**Per-fixture goldens** — `crates/fr-dsn/tests/data/*.txt`, each captured from
the pinned 2.3.0 jar (compared verbatim below its `#` header). Four probe
programs live beside them. Most goldens carry their own regeneration command
in that header; the four `*-rules.txt` goldens are the exception — their
header only points at `tests/rules_round_trip.rs` (`RProbe.java`'s `dump` /
`write` / `writesettings` modes), whose module docs carry the actual
regeneration commands for all four.

| probe | what it dumps | consumed by |
|---|---|---|
| `NProbe.java` | board items, nets, net classes, via infos/rules, components | `network_scope.rs`, `structure_scope.rs`, `placement_scope.rs`, `dsn_reader.rs` |
| `CProbe.java` | every `.dsn` in a directory → `BoardReadResult` + warnings | the corpus golden |
| `SProbe.java` | a `.dsn` + `.ses` through `SesReader` → items + import summary | `ses_round_trip.rs` |
| `RProbe.java` | a `.dsn` + `.rules` through `RulesReader` (`dump`/`write`/`divergence` modes) | `rules_round_trip.rs` |

Typical shape (from any golden's header):

```sh
export PATH=/opt/homebrew/opt/openjdk@25/bin:$PATH
javac -cp tools/freerouting-2.3.0.jar -d /tmp/nprobe crates/fr-dsn/tests/data/NProbe.java
java -Djava.awt.headless=true -cp tools/freerouting-2.3.0.jar:/tmp/nprobe \
     NProbe crates/fr-dsn/tests/data/via_order.dsn
```

**Synthetic fixtures** (`tests/data/*.dsn`) exist for branches no real design
reaches: `network_via.dsn` (the skipped `setViaPadstacks` branch),
`wiring_try_correct_net.dsn`, `wiring_warnings.dsn`,
`wiring_via_warnings.dsn`, `wiring_via_padstack_missing.dsn`, `alias.dsn`,
`class_pair.dsn`, `via_order.dsn`.

**Ported Java tests.** `DsnReaderTest`/`DsnReadResultTest`/
`DsnReaderMetadataTest` (Task 10), `DsnWriterTest` (11), `SesRoundTripTest`
(12+13), `RulesRoundTripTest` (14), `BoardMetadataTest` (1).
`io/SpecctraPackageArchTest` is **deliberately not ported**: it is a
Java-package-dependency test whose Rust analogue is the crate's own dependency
list, enforced at compile time (`fr-dsn` may only see `fr-board`,
`fr-geometry`, `thiserror`).

`tests/parity` is a helper library crate with no `fr-dsn` dependency. Only
`parity_dsn.rs`, `parity_ses.rs` and the two corpus tests in `dsn_reader.rs`
skip with a printed message when `../freerouting` (or `$FREEROUTING_JAVA_DIR`)
is missing, by calling `parity::require_java_dir()` first and returning. Every
other fixture-reading suite fails instead: `library_scope.rs`,
`network_scope.rs`, `placement_scope.rs`, `structure_scope.rs`,
`rules_round_trip.rs`, `ses_round_trip.rs` and the rest of `dsn_reader.rs` read
through `tests/common/mod.rs`'s `fixture()`, which resolves via
`parity::java_dir()` (so it *does* honour `FREEROUTING_JAVA_DIR`) but has no
skip guard, so it panics on a missing file rather than skipping. Extending the
guard to those suites means a call site per test, not a helper change.

## Differential drivers

All three `p3t*` pairs link against **`tools/freerouting-2.3.0.jar`** (ruling
10), not the clone-HEAD jar Plan 2's `p2t*` drivers use, because that is the jar
`tests/reference/` was generated with. All need a JDK 25 (`JAVA25_HOME`). The
jar is gitignored — a fresh checkout fetches it on first run.

| Driver | Covers | Baseline run | Result |
|---|---|---|---|
| `p3t2` mode 0 (seed 42) | `Double.toString`/`Float.toString` over random bit patterns | 10,000,000 | 0 diff |
| `p3t2` mode 1 (seed 42) | DSN-coordinate-shaped decimals + `formatPlacementRotation` | 1,000,000 | 0 diff |
| `p3t2` mode 2 (seed 42) | integers in ±10^7 | 1,000,000 | 0 diff |
| `p3t2` mode 3 (seed 42) | rotations in [0, 360), three decimals | 1,000,000 | 0 diff |
| `p3t2` all four modes (seed 7) | second seed | 13,000,000 | 0 diff |
| `p3t3` | the scanner's whole token stream (index, tag, value, lexical state) over one file | 105 `.dsn` + 12 `.ses` + 7 `.rules` + 14 reference files; `tutorial_board/roundtrip.dsn` = 80,308 lines | 0 diff on every file |
| `p3t15` mode 0 | reader → item dump (id, kind, layers, nets, clearance class, fixed state, bbox, tile count) + warnings | `tutorial_board` 441 / `Issue413-test` 39 lines | 0 diff |
| `p3t15` mode 1 | `DsnWriter.write` verbatim | 38,869 / 378 lines | 0 diff |
| `p3t15` mode 2 | `SesWriter.write` verbatim | 27 / 130 lines | 0 diff |
| `p3t15` mode 3 | `RulesWriter.write` verbatim | 94 / 42 lines | 0 diff |
| `p3t15` mode 4 | delegates to `P3T3` — the token stream | 76,602 / 1,097 lines | 0 diff |
| `sweep-p3t15.sh` | all 5 modes × all 106 fixtures | **530 pairs** | 525 MATCH, **0 unexpected diffs**, 5 `XDIFF` |

```sh
./scripts/differential/run.sh p3t2 100000 42 0     # 100k doubles, seed 42
./scripts/differential/run.sh p3t3 <file>          # any .dsn / .ses / .rules
./scripts/differential/run.sh p3t15 <file.dsn> 1   # one fixture, DsnWriter
./scripts/differential/sweep-p3t15.sh              # all 5 modes × all 106 fixtures (~25 min)
```

**`p3t15` is the only coverage of the scanner's hand-rolled bypass.** `p3t3`
exercises `next_token` only; `nextString`/`nextStringList`/`nextDouble` walk
`zzBuffer` directly instead of running the DFA, so nothing but a parser-level
driver reaches them. A wrong string surfaces as a wrong item name, a wrong
string list as a wrong net list, a wrong double as a wrong coordinate.

### Known diffs (both expected, neither hidden)

| Fixture | Modes | Why |
|---|---|---|
| `Issue229-display-8-digit-hc595.dsn` | 0–3 (mode 4 **MATCHes**) | Ruling E: the 2.3.0 jar's `DsnFile.readStringScope` has no resync loop; the port follows HEAD. Java 3/59/18/0 lines, Rust 502/3907/1922/55. Mode 3 is 0 on the Java side because the collapsed board has no library, so `RulesWriter` hits the same NPE as the row below |
| `empty_board.dsn` | 3 only (0–2 MATCH) | The reachable totalization: no `(library …)` scope → `BoardLibrary.padstacks` is `null` → the jar throws `NullPointerException` out of `RulesWriter.writeRules` (RulesWriter.java:70) while the port writes a valid 20-line `.rules` |

`sweep-p3t15.sh`'s `EXPECTED_DIFFS` array is keyed per **`(fixture, mode)`**,
not per fixture — deliberately, so a regression that broke `Issue229`'s *tokens*
would still surface as a `DIFF` on a mode nobody excused.

## Parked residuals, by task

- **Task 1** — `DsnError` grew past its original three variants as later tasks
  needed them; `UnexpectedEof` was added and later removed (verified gone at
  `238bb5d`). The `IndentFileWriter` no-arg-`start_scope` test vector in the
  brief disagreed with the JVM; Java won.
- **Task 2** — `format_placement_rotation` lives in `format/double.rs`, not with
  `SesWriter`; the audit map would need both paths if Java ever made it public.
  Three items were re-deferred by the Task 15 review and are still open minors:
  (a) `p3t2` mode 0 omits rotation formatting above 1e7 — a coverage gap, not a
  known divergence; (b) `point` is shadowed twice in `format/double.rs` (~:148);
  (c) `JavaRandom` is duplicated across two differential binaries
  (`p2t13.rs`, `p3t2.rs`) and wants a shared module.
- **Task 3** — `lexer/mod.rs` is 1,088 lines (the action switch could be split).
  The `p3t3` Rust twin prints a fixed `ERROR java.lang.Error` line where Java
  prints the real exception class, so a scan error on a future fixture would show
  as a diff rather than a match — deliberate, since no fixture should reach it.
- **Task 4** — `ScopeKeyword::name()` duplicates `Keyword::name()`'s literals for
  twelve keywords rather than delegating.
- **Task 5** — `ReadAreaScopeResult::shape_list` is `Vec<Option<DsnShape>>` and
  consumers must keep the `Option` (flattening would silently drop a hole Java
  NPEs on). Quirk #87's knock-on: `dsn_file_generated_by_host` can never become
  `false` from a read, so any later branch on it reads a constant — do not "fix"
  the lexer, the references depend on the current behaviour.
- **Task 6** — quirk #95 has corpus-wide consequences (a keepout before
  `autoroute_settings` moves everything after it one level up); faithful, but a
  differential reader should expect it rather than diagnose it.
- **Task 7** — `write_package_scope`'s outline uses `boardToDsnRel` while its
  keepouts use `boardToDsn` (Package.java:201 vs :237), so package keepouts are
  written in *absolute* DSN coordinates. Currently **unobservable**:
  `Structure.createBoard` is the only producer of a transform and always passes
  `baseX = baseY = 0`. Revisit if any later task builds a transform with a
  non-zero base. Also: no corpus fixture has a `part_library` scope, so
  `write_part_library_scope` has no reference behind it.
- **Task 8** — `read_via_padstacks` is `pub(crate)` in `structure.rs` and
  `network.rs` depends on it one-way. Note for anyone comparing ordering with
  Java: the port's `BTreeSet<PinRef>` / `BTreeMap<String, …>` order by Rust
  **code point** where Java's `String.compareTo` orders by **UTF-16 code unit**;
  the two disagree only above U+FFFF, and the ordering-sensitive sites
  (`Net.Id`, `Pin`) use `java_string_cmp` explicitly for that reason.
- **Task 9** — `network.rs` is **3,298 lines** and is the obvious candidate for a
  `parser/network/` split; deferred deliberately, since splitting it mid-plan
  would have churned every citation.
- **Task 10** — seven of the eight warning sites are corpus-unreachable (covered
  by three synthetic fixtures only); `read_metadata` is not differential-tested
  over the corpus, only on the three fixtures Java tests; `read_wire_scope`
  clones `p.layer_structure` once per sub-scope token (behaviour-neutral).
- **Task 11** — `write_non_default_clearance_rules` is `pub` only to satisfy
  `-D warnings`; `layer_rule`, `part_library` and that function have **no
  parity evidence at all** (no fixture produces them and the Java call site is
  commented out at Rule.java:193). `fr_dsn::write` is a very general name for a
  glob import.
- **Task 12** — `write_conduction_area` has no byte-level coverage anywhere;
  `snapped_endpoint`'s tie-break is reasoned from quirk #63, not measured (no
  fixture puts two drill items on one trace endpoint).
- **Task 13** — `errors_encountered > 0` is untested against a real fixture (all
  six corpus pairs import cleanly); multi-subnet nets are ported-from-source.
- **Task 14** — the two renumbering methods change `fr-board` behaviour for
  future callers: a `ViaRuleId` is now stable only *between* `add_via_rule`
  calls. `String.isBlank()` vs `char::is_whitespace` differ on U+00A0 and
  U+001C–1F in `discover_layer_structure` — noted at the site, not emulated,
  since no Specctra layer name can contain them.
- **Task 15** — the sweep takes ~25 minutes (530 JVM starts); `p3t15` mode 0
  does not dump the board's *rules* directly (modes 1–3 write them out, so a
  rules regression still fails the sweep, but through the writers).

## Obligations for later plans

### Plan 4 (`fr-settings`) — all three DISCHARGED; see `docs/plan-4-handoff.md`

- ~~**`DsnRouterSettings` → `RouterSettings` (plan ruling 5).**~~ **Discharged in
  Plan 4 Task 6 (`6e742bf`), with the gap it left closed in that task's fix round
  (`826756a`, controller ruling L).** The `From`/`Into` pair lives in
  `crates/fr-settings/src/sources/mod.rs`; the surfaces carry
  `fr_settings::RouterSettings`, and `DsnRouterSettings` stays `fr-dsn`'s internal
  subset. Both traps were handled: no `Eq` (the round trip compares field by
  field) and `apply_new_values_from`'s DSN-subset scope (superseded — the sources
  call `RouterSettings::apply_new_values_from`, the full one). **The trap this
  hand-off did not foresee:** `DsnRouterSettings` stored its four scalar defaults
  *eagerly*, so it could not express *absence*, and a `.rules` file that omitted
  `(via_costs …)` pushed `1` over `DefaultSettings`' `50`. Ruling L fixed it at
  the root — `Option` fields, `board_specific_trace_costs_applied`, `*_raw`
  accessors, a conditional `apply_new_values_from` — with the coalescing getters
  kept so no emitted byte moved (`p3t15` mode 3 and the full 530-pair sweep
  re-run MATCH). **Lesson: a subset type that a merge engine will consume must
  carry absence, not defaults.**
- ~~**Three ported APIs are not jar-pinned (ruling H).**~~ **Discharged in Plan 4
  Task 9 (`9bfab5e`).** All three are now backed by a JVM differential against the
  clone's HEAD jar, not by Rust tests alone: `scripts/differential/java/P4T1.java`
  builds the real `RulesFileSettings`, whose `getSettings()` calls
  `RulesReader.readRouterSettings` (`RulesFileSettings.java:83`), which calls
  `discoverLayerStructure` (`RulesReader.java:198`); and the real
  `SettingsMerger.merge` calls `RouterSettings.applyNewValuesFrom`. Every
  `.rules`-carrying row of the 84-case table exercises all three, at **0 diffs**
  in all three modes. This is exactly what found controller ruling N (the file is
  parsed **twice**, quirk #142) — 13 of the 84 rows diverged until it was fixed.
- **Legacy-CLI value normalisation — the `fr-settings` half is discharged, the
  wiring half is Plan 8's.** **Discharged in Plan 4 Task 7 (`1c81176`):**
  `crates/fr-settings/src/sources/cli.rs` carries the whole table —
  `apply_command_line_arguments` → `LegacyBridge` with `-oit /100`, the `-mp`/`-mt`
  clamps, `-us`/`-is` folding and `-inc`'s missing trim — plus `CliSettings` for
  the two flags that actually reach the router and `classify_de_arguments` for the
  `-de` rule. JVM-verified, with one correction to
  `docs/cli-legacy-flags.md`: **`-oit -5` keeps the previous value**, because the
  blanket rule never consumes an argument starting with `-` (quirk #135). **Still
  open (Plan 8):** `crates/freerouting/src/legacy.rs` still forwards raw values
  and still reproduces the `-de` rule itself — plan 4 ruling 10 kept the binary
  untouched, so the rewire is Plan 8's one-call change. Plan 8 must also decide
  whether to make the five dead flags live (see `docs/plan-4-handoff.md` §10).

### Plan 5 (`fr-drc`) — all three answered; see `docs/plan-5-handoff.md`

- **`BoardMetadata::router_settings` consumers.** `read_board` returns `Success`
  with `metadata: None`; **only `read_metadata` fills it** (`DsnReader.readBoard`
  does the same in Java). Any Plan 5 code that wants metadata from a board read
  must call `read_metadata` on a second pass over the same bytes, or accept
  `None` — it is not a bug to be "fixed" in the reader. — **Plan 5 never needed
  it, and the note stays open for Plans 6-8.** Nothing in `fr-drc`, in its test
  suites or in the `p5t1`/`p5t2` drivers calls `read_metadata`: the DRC surface
  takes `DrcCoordinates`/`DrcReportOptions` as *parameters* (plan-5 ruling 5),
  so the only thing it wants from the read is the board and the
  `coordinate_transform`, both of which `Success` already carries. `grep -rn
  read_metadata crates/fr-drc scripts/differential` is empty. The obligation
  therefore lands on whoever wires `-drc`'s router settings — Plan 8.
- ~~**The DRC report should *not* reuse `IndentFileWriter`.**~~ — **honoured in
  Plan 5 Task 8** (`f73d5fb`, `e6a788d`). Decided there so it is
  not re-litigated: `IndentFileWriter` is a faithful port of Java's
  S-expression indenter (`start_scope`/`start_scope_nl`/`new_line`, two-space
  indent, no escaping), and spec §3's DRC clause asks for JSON. Bending it into
  a JSON emitter would put a parity-critical type on a non-parity path where a
  future edit could silently change `.dsn` output. Plan 5 wrote its own
  serialiser: `crates/fr-drc/src/report/json.rs` renders through
  `fr_dsn::format::json::to_gson_string_pretty` — Plan 4's Gson-compatible
  formatter, which plan-5 ruling 7 moved *down* into `fr-dsn` (Task 1,
  `1a8f0a9`) so `fr-settings` and `fr-drc` share one copy instead of two.
  `IndentFileWriter` is untouched and no `.dsn`/`.ses` byte moved (`p3t15`
  stays MATCH over the whole sweep).
- **Quirk #82 (Delaunay in-circle degenerate on axis-aligned input) must not be
  fixed while porting `NetIncompletes`** — carried unchanged from Plan 2, and
  **still open after Plan 5**: Task 5 ported `NetIncompletes` on top of the
  unfixed triangulation, which is what makes `p5t2` mode 3 match the jar's
  airline endpoints on all 112 rows. Plans 6/7 inherit it unchanged.

### Plans 6/7 (`fr-router`)

- **`insert_via → split_traces` is an unchecked path into quirk #76's
  machinery (ruling F).** Thread a `StopCheck` through `Board::insert_via` /
  `Board::split_traces`. Marker at `parser/wiring.rs:596`.
- **Via-info / via-rule re-pointing (ruling H).** Decide whether router
  behaviour on a re-declared via must match Java (then `ViaInfos` needs
  tombstoned entries, or `ViaRule` must own its `ViaInfo`s) or whether
  re-pointing is the better semantics (then say so and close the register row).
  Pinned as the *port's* behaviour by
  `rules_round_trip.rs::re_declared_via_info_re_points_the_existing_via_rule_unlike_java`,
  whose comment carries the jar's answer.
- **Quirk #106 (`Item.getConnectionItems` has no visited set) affects the router,
  not just the importer.** `connection_items_checked` exists; nothing outside the
  DSN import path uses it yet.
- **Write `scripts/audit-map/fr-board.map`.** The per-class mechanism now exists
  and `fr-dsn` uses it; `fr-board`'s nine directories are still audited
  crate-wide, so Plan 2's 357-of-750 collective-match caveat still stands there.
  Mechanical work now — do **not** relax the script instead.
- Everything Plan 2 already filed for these plans (quirk #74's early return,
  `equals_geometric` at the `TraceTightener` call sites, `RoutingBoardExt`,
  `changed_area`'s reset, `catch_unwind` recovery boundaries, room ordering)
  remains open and unchanged.

### Plan 8 (`fr-core` + surfaces)

- **Wire this crate's seven entry points** (above). Whatever holds a `Board`
  between a read and a write must also hold the `CoordinateTransform` (ruling A).
  Decide then whether the SES/rules entry points should be renamed to the crate
  root.
- **`io/kicad/**` (1,574 lines) and `SessionToEagle` (627 lines) land here.**
  Both traffic in the **same** `BoardReadResult`/`BoardMetadata` this plan
  defines and both share `CoordinateTransform`, so build on `fr-dsn`'s types
  rather than parallel ones. `SessionToEagle`'s only caller anywhere in the Java
  tree is `SesReader.saveSpecctraSessionSesAsEagleScriptScr`
  (SesReader.java:105-109), which `ses_reader.rs` carries a `// not ported:`
  marker for; `lib.rs` carries the `// added in Plan 8:` marker the audit map
  checks.
- **The KiCad reader must not "fix" quirk #83.** `ClearanceMatrix.setValue`/
  `getValue` index J-then-I rather than I-then-J, which makes the matrix
  asymmetric in a way KiCad input can observe. Writing both orders to
  "correct" it changes clearances on every KiCad-sourced board and breaks parity.
- MCP concurrency (progress sink, cancel token, reader thread) — carried from
  Plan 1, untouched by this plan.
- **The four zero-coverage Plan 3 paths** (register row): `SesWriter.writeWasIs`'s
  swap body, `Component.readLockType`'s `(lock_type position)` arm,
  `SesWriter.writeConductionArea` (quirk #110's mixed int/double output) and
  quirk #105. Each needs a synthetic fixture plus JVM ground truth. If a wider
  corpus ever arrives, these are the rows to check first. (The
  `instanceof Path` → `PolylinePath` arm is **not** on this list: it is
  unreachable in the port and documented as such — see Correction 3.)
- **Quirk #113 is a live deviation, not a reproduction.**
  `Structure.setClearanceRule` splits at the string-quote character with
  `String.split`, i.e. a **regex**, and `Parser.readQuoteChar` accepts any
  string token — so `(string_quote .)` diverges. Closing it needs a regex engine
  (forbidden by the dependency rule) or a hand-rolled single-character regex
  emulation. Out of proportion to an input no exporter writes, but recorded.

## Evidence

Verified on the committed tree, not taken from a report:

Test counts are from commit **`9cdf4f7`**, unchanged at **`b85e674`** (the
branch tip, documentation-only); the working tree carried nothing but this
document's own edits when they were taken:

```
cargo fmt --all                                   clean
cargo clippy --workspace --all-targets -D warnings clean
cargo test --workspace                            1119 passed, 0 failed, 3 ignored
cargo test -p fr-dsn                               257 passed, 0 failed, 2 ignored
cargo test -p fr-dsn --release --test dsn_reader    31 passed (corpus test 12.0 s)

./scripts/audit-port.sh io/specctra        crates/fr-dsn/src '*.java' scripts/audit-map/fr-dsn.map   -> 0
./scripts/audit-port.sh io/specctra/parser crates/fr-dsn/src '*.java' scripts/audit-map/fr-dsn.map   -> 0
./scripts/audit-port.sh io                 crates/fr-dsn/src '<5 files>' scripts/audit-map/fr-dsn.map -> 0
./scripts/audit-port.sh datastructures     crates/fr-dsn/src '<2 files>' scripts/audit-map/fr-dsn.map -> 0
   (no MISSING line, no UNMAPPED line; the nine fr-board dirs and geometry/planar also 0)

./scripts/differential/run.sh p3t2 …              0 diff over 26 M values
./scripts/differential/run.sh p3t3 …              0 diff over 138 files
./scripts/differential/sweep-p3t15.sh             530 pairs, 0 unexpected diffs, 5 XDIFF
```

The three ignored tests are `fr-board`'s non-terminating ladder reproduction
(quirk #76 — it exists precisely because it cannot pass) and two in
`fr-dsn/tests/dsn_reader.rs` gated on `debug_assertions` (the 90 s corpus test
and a release-only lexer timing assertion).

**Read the audit zero precisely.** For `fr-dsn` it *is* per-class evidence: the
map pins each of the 52 ported Java classes to its Rust file(s) (56 mapping
lines in a 74-line file — a class whose Rust home spans two files gets one line
per file, the rest is comments), and an unmapped
class prints `UNMAPPED`. For `fr-board` and `fr-geometry` the older crate-wide
form still applies, with Plan 2's caveat.

**Nothing in Plan 3 changes `docs/geometry-library-survey.md`.** `fr-dsn` adds
no geometry dependency (`fr-board`, `fr-geometry`, `thiserror` only), and no
scope in `io/specctra/**` needs polygon booleans; the survey's conclusions stand
as Plan 2 left them.

## Open items for the user

- **The DSN round trip is not idempotent in Java, and nothing here depends on
  it.** Re-reading `tests/reference/*/roundtrip.dsn` through the 2.3.0 jar and
  writing again loses the per-type clearance rules and the per-pin clearance
  classes, and grows the outline bounding box by the boundary clearance on every
  pass (JVM-verified on all four original references while writing the plan).
  The acceptance target is **one** round trip from the original fixture. Do not
  add an idempotence test.
- **Two documented divergences from the pinned jar exist on purpose** (ruling E's
  `Issue229` and the `empty_board` `RulesWriter` NPE). Both are in the
  known-diffs table above and in `scripts/differential/README.md`; neither is a
  port bug, and neither should be "fixed" by changing the port.
- **Plan 3's final whole-branch review has not run.** This task's own
  verification is not a substitute for it.
