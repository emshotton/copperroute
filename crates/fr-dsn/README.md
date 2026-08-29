# fr-dsn

A behavioral Rust port of freerouting's Specctra text-I/O layer (v2.3.0):
`io/specctra/**` (the JFlex scanner, the thirty parser scope classes,
`DsnReader`, `DsnWriter`, `SesReader`, `SesWriter`, `RulesReader`,
`RulesWriter`), `io/{CoordinateTransform,BoardReadResult,BoardMetadata,
FileFormat,KiCadNetClassNames}.java`, and
`datastructures/{IdentifierType,IndentFileWriter}.java`. It reads a Specctra
`.dsn` design into a `fr_board::Board` and writes `.dsn`, `.ses` and `.rules`
back out **byte for byte** as Java writes them. Use `fr_dsn::prelude::*` to
bring in every public type.

The crate sits directly on `fr-board`: it builds a board through Plan 2's
public insert API and reads one back out through `get_traces`/`get_vias`/
`get_connectable_items`/`items_in_board_order`. It owns no board state of its
own, and depends only on `fr-board`, `fr-geometry` and `thiserror` — no
`serde`, no `regex`, no `tracing`.

Diagnostic `FRLogger` calls from the Java source either push onto
`ReadScopeParameter::warnings` exactly where Java does, or vanish
(`global-constraints.md`). Deliberate Java bugs and edge-case crashes are
reproduced rather than fixed; every one carries a `// Java bug:` or
`// totalized:` marker at the site and a row in `docs/java-quirks.md`.

## Two token tables, not one

Plan 3 ruling 2: **read lexemes and write literals are independent.** Three
things are kept separate and must not be conflated:

1. the DFA's lexeme → `Keyword` mapping (`lexer/tables.rs`, transcribed
   mechanically from JFlex's five packed strings by
   `scripts/gen-lexer-tables.py`);
2. `Keyword::name()` (`keyword.rs`), used *only* where Java calls
   `getName()`;
3. the writers' raw string literals, transcribed one by one from each
   `writeScope` — `Rectangle.writeScope` writes `"(rect "` while
   `Keyword.RECTANGLE.getName()` is `"rectangle"`.

Plan 3 ruling 1: the Java source authority for this crate is the clone's
HEAD **except** for fifteen `Keyword` name literals, where the pinned
`tools/freerouting-2.3.0.jar` wins. HEAD renamed `autoroute_settings`,
`clearance_class`, `host_cad`, `host_version`, `logical_part`, `pull_tight`,
`shove_fixed`, `snap_angle`, `start_ripup_costs`, `string_quote`,
`use_layer`, `use_via`, `via_costs`, `via_rule` and `write_resolution` to
camelCase in `Keyword.java` *and in the writers' string literals*, without
touching the DFA — so HEAD cannot read back its own output. This port emits
the snake_case Specctra tokens.

## What is *not* here

- **The KiCad JSON path.** `io/kicad/**` (1,574 lines) is Plan 8.
- **`SessionToEagle`.** `io/specctra/parser/SessionToEagle.java` (627 lines)
  turns a session file into an Eagle CAD command script. Deferred to Plan 8;
  its one caller, `SesReader.saveSpecctraSessionSesAsEagleScriptScr`, carries
  a `// not ported:` marker in `ses_reader.rs`, and
  `scripts/audit-map/fr-dsn.map` maps the class to the `added in Plan 8:`
  marker in `lib.rs` so the audit checks the deferral rather than skipping
  it.
- **`RouterSettings`.** `AutorouteSettings.readScope` returns
  `settings.RouterSettings` in Java; Plan 4 owns `fr-settings`, so this crate
  defines a local `DsnRouterSettings` holding exactly the fields the DSN and
  rules scopes read and write, with an `obligation:` marker naming
  `settings/RouterSettings.java` (ruling 5).
- **No GUI, no observers, no `BoardHandling`.** `ReadScopeParameter`'s
  `boardHandling`/`observers` become a plain `Option<Box<Board>>` plus the
  `ItemIdGenerator` Java threads through `Communication`.
- **No static mutable state.** Java's `SpecctraDsnStreamReader.scopeIdentifier`
  and `.nf` are `public static` fields; both are instance fields on
  `DsnScanner` here. A documented deviation (ruling 6), not a bug fix.

## Type mapping

| Java | Rust |
|---|---|
| `SpecctraDsnStreamReader` (JFlex scanner) + `IJFlexScanner` | `lexer::DsnScanner` (+ `lexer::tables`, the transcribed DFA) |
| the scanner's return `Object` | `enum Token { Open, Close, Kw, Str, Int, Float }` |
| `Keyword` (a class of `public static final` singletons) | `enum Keyword` |
| `ScopeKeyword` | `enum ScopeKeyword` + the free `read_scope`/`skip_scope` |
| `ReadScopeParameter` / `WriteScopeParameter` | `ReadScopeParameter` / `WriteScopeParameter` |
| `DsnFile`, `DsnReader`, `DsnWriter` | `parser::dsn_file`, `dsn_reader`, `dsn_writer` (free functions — the Java classes are private-constructor static holders) |
| `SesReader`, `SesWriter`, `RulesReader`, `RulesWriter` | `ses_reader`, `ses_writer`, `rules_reader`, `rules_writer` |
| `Shape` (abstract) + `Rectangle`/`Circle`/`Polygon`/`Path`/`PolygonPath`/`PolylinePath` | `enum DsnShape { Rect, Circle, Polygon, Path, PolylinePath }` + `DsnRectangle`/`DsnCircle`/`DsnPolygon`/`DsnPolygonPath`/`DsnPolylinePath` |
| `Layer`, `LayerStructure` (the DSN-side ones) | `DsnLayer`, `DsnLayerStructure` |
| `Structure`, `Plane`, `Library`, `Package`, `PartLibrary`, `Placement`, `Component`, `Network`, `Net`, `NetClass`, `NetList`, `Circuit`, `Rule`, `Wiring`, `Parser`, `PlaceControl`, `AutorouteSettings`, `KiCadNetClassNames` | one module each under `parser/` (`scripts/audit-map/fr-dsn.map` is the class → file map) |
| `BoardReadResult` (sealed interface, 4 records) | `enum BoardReadResult` — plus a `coordinate_transform` field the port adds |
| `BoardMetadata`, `FileFormat` | `BoardMetadata`, `FileFormat` (`error.rs`) |
| `CoordinateTransform` | `CoordinateTransform` |
| `IdentifierType`, `IndentFileWriter` | `format::IdentifierType`, `format::IndentFileWriter` |
| `Double.toString`, `Float.toString`, `String.format("%.Nf", …)`, `Math.rint`/`Math.round` | `format::double::{java_double_to_string, java_float_to_string, java_format_fixed, java_rint, java_round, java_round_to_int}` |
| `util/gson/GsonProvider`'s Gson configuration (write half) | `format::json::{JavaNumberFormatter, to_gson_string_pretty}` — moved here from `fr-settings` in Plan 5 (ruling 7) so `fr-drc` can reuse it without depending on that crate; `fr-settings::RouterSettings::to_json_string_pretty` is a thin wrapper over `to_gson_string_pretty` |
| `SessionToEagle` | not here — Plan 8 |
| `settings.RouterSettings` | `parser::DsnRouterSettings` (crate-local, ruling 5) |

### `CoordinateTransform` is threaded explicitly

Controller ruling A. Java's writers re-derive the transform from
`board.communication`; `fr-board` cannot depend on `fr-dsn`, and
`CoordinateTransform` lives here, so `BoardReadResult::Success`/`OutlineMissing`
carry the transform `Structure.createBoard` built (`None` if it never ran) and
every writer takes `ct: &CoordinateTransform` as a parameter. That transform is
the only one that round-trips a file's coordinates unchanged.

## Invariants

- **`normalize_all_traces` runs under a stop check** (ruling 4). Quirk #76 is
  a 4+-rung ladder on one net that makes `PolylineTrace::split`'s entry
  re-walk non-terminating, and `Wiring.readScope` ends every DSN read with
  `board.normalizeAllTraces()`. Java already wraps that call in
  `try`/`catch (Exception)` and, on a throw, pushes
  `"Wiring: normalization of traces failed"`. The port passes a
  `TimeLimit`-backed closure (`DsnReadOptions::normalize_time_limit`, default
  60 s) and, when it trips, pushes exactly that string and continues — the
  divergence lands inside a branch Java already has. `Duration::ZERO` means
  "give up before the first check", a deterministic opt-out. No fixture in
  the 105-file corpus comes near the limit; `dsn_reader.rs`'s corpus test
  asserts it.
- **Java string order, not Rust string order.** Several DSN structures are
  keyed by a Java `TreeSet`/`TreeMap` of `String`, whose comparator is
  UTF-16-code-unit order. Rust's `str: Ord` is code-point order, and the two
  disagree above the BMP. Where the order is observable in the output, the
  port uses `java_string_cmp` (`parser/part_library.rs`) rather than the derived
  `Ord`.
- **Descending item iteration.** Every writer that mirrors a Java
  `board.itemList` walk goes through `Board::get_items`/
  `items_in_board_order`, which iterate *descending* by id (quirk #63) — the
  order Java's `UndoableObjects.startReadObject` produces, and the order the
  output bytes depend on.
- **The lexer buffer is finite.** Java allocates `zzBuffer` once as a
  `char[16 * 1024 * 1024]` and its hand-rolled `nextString` indexes that
  buffer with no refill, so a larger file is silently mis-lexed. The port
  refuses it with `DsnError::InputTooLarge` instead.
- **Never edit a generated reference by hand.** `tests/reference/<stem>/`
  is produced only by `scripts/gen-reference.sh` against the pinned 2.3.0
  jar.

## Tests

`cargo test -p fr-dsn` runs the unit tests plus fourteen integration suites
(`tests/`): the lexer, the number formatters, `IdentifierType`, each scope
family (`structure`, `library`, `placement`, `network`, `geometry`), the DSN
reader, the SES round trip, the rules round trip, and the two bit-parity
suites.

- **`tests/parity_dsn.rs` / `tests/parity_ses.rs`** compare the writers'
  output against the committed Java references in `tests/reference/`.
  `every_reference_is_byte_for_byte_identical_to_java` is the real gate: it
  asserts raw bytes, not the whitespace-normalised comparison the per-fixture
  cases use. Seven references, generated from the pinned 2.3.0 jar by
  `scripts/gen-reference.sh` — four original (`tutorial_board`,
  `Issue026-J2_reference`, `Issue103-Board-Unrouted`, `Issue143-rpi_splitter`)
  and three added under controller ruling G because those four have
  `polyline_path = 0`, wiring `(via  = 0`, `(type shove_fixed|fix|protect) = 0`
  and `(plane  = 0` between them: `Issue413-test` (11 `polyline_path`, 4
  wiring vias, 3 fixed states, and the only SES reference with `(wire`
  entries), `Issue110-RelayModule` (22 vias, 22 fixed states) and
  `Issue753-CPU-85_r104` (3 planes, 65 `polyline_path`).
- **The corpus test** (`tests/dsn_reader.rs`,
  `every_fixture_in_the_corpus_matches_javas_result_and_warnings`) reads
  every `.dsn` in `../freerouting/fixtures` and compares the
  `BoardReadResult` variant and the complete warning list, message for
  message, against a golden captured from the same jar. 105 files: 104
  `Success`, one `ParseError`.

  **It is `#[ignore]`d in a debug build** (~90 s) and runs unconditionally in
  release — `cargo test -p fr-dsn --release`, or `--ignored` in debug. The
  Plan 3 brief asked for a non-`#[ignore]`d corpus test; controller ruling I
  kept the debug-only `ignore` because 90 s is unreasonable in the default
  suite. **The consequence is real and worth stating: the full 105-file
  corpus check runs only in a release build or under `--ignored`, i.e. in CI
  or on request, not in a bare `cargo test`.** Its always-on sibling,
  `the_corpus_golden_parses_and_the_named_fixtures_read_to_its_variant`,
  costs milliseconds and covers what would otherwise rot unnoticed: that the
  golden parses, that it still names all 105 files including the five the
  brief calls out by name, and that each of those five reads to the variant
  the golden records.
- **The `p3t15` sweep** (`scripts/differential/sweep-p3t15.sh`) runs the DSN
  reader and all three writers, plus the raw token stream, against the 2.3.0
  jar over all 106 fixtures — see below.

### What needs the sibling checkout

`tests/reference/` travels with this repository, but the fixtures those
outputs were generated from do not. Two different behaviours when
`../freerouting` (or `$FREEROUTING_JAVA_DIR`) is missing:

- **`parity_dsn.rs`, `parity_ses.rs` and the two corpus tests in
  `dsn_reader.rs` skip with a printed message** — they call
  `parity::require_java_dir()` first and return.
- **Every other fixture-reading suite fails**: `library_scope.rs`,
  `network_scope.rs`, `placement_scope.rs`, `structure_scope.rs`,
  `rules_round_trip.rs`, `ses_round_trip.rs` and the rest of `dsn_reader.rs`
  read through `tests/common/mod.rs`'s `fixture()`, which resolves via
  `parity::java_dir()` (so it *does* honour `FREEROUTING_JAVA_DIR`) but has no
  skip guard, so it panics on a missing file rather than skipping. Extending
  the guard to those suites means a call site per test, not a helper change.

Everything that reads only `tests/data/` or `tests/reference/` — the lexer,
the number formatters, `IdentifierType` — needs no checkout at all.

## Differential drivers

`scripts/differential/` holds Java-vs-Rust driver pairs that print one line
of state per call and are diffed byte-for-byte. Unlike the Plan 1/2 drivers,
the `p3t*` pair links against the **pinned `tools/freerouting-2.3.0.jar`**
(ruling 10), because that is the jar `tests/reference/` was generated with.
Both need a **JDK 25** (`JAVA25_HOME`).

| Driver | Covers | Twin |
|---|---|---|
| `P3T2.java` | `Double.toString`, `Float.toString` and `SesWriter.formatPlacementRotation` (which is `String.format("%.Nf", …)` underneath) over random bit patterns, DSN-shaped decimals, integers and rotations | `p3t2` |
| `P3T3.java` | the Specctra scanner's whole token stream over one file — index, tag, value, lexical state | `p3t3` |
| `P3T15.java` | one fixture through the reader and all three writers: mode 0 the item dump (id, kind, layers, nets, clearance class, fixed state, bounding box, tile-shape count) plus the warnings, mode 1 `DsnWriter.write`, mode 2 `SesWriter.write`, mode 3 `RulesWriter.write`, mode 4 the `p3t3` token stream | `p3t15` |

`p3t3` exercises `next_token` only. `p3t15` modes 0-3 go through the whole
parser, so they are the only coverage of the scanner's **hand-rolled
`nextString`/`nextStringList`/`nextDouble` bypass** — those three methods walk
`zzBuffer` directly instead of running the DFA, and nothing but a parser-level
driver reaches them.

```sh
./scripts/differential/run.sh p3t2 100000 42 0        # 100k doubles, seed 42
./scripts/differential/run.sh p3t3 <file>             # any .dsn / .ses / .rules
./scripts/differential/run.sh p3t15 <file.dsn> 1      # one fixture, DsnWriter
./scripts/differential/sweep-p3t15.sh                 # all 5 modes × all 106 fixtures
```

`scripts/differential/README.md` has the baseline line counts and the
known-diffs table. The sweep is 530 pairs with **0 unexpected diffs** and two
expected sites:

- `Issue229-display-8-digit-hc595.dsn`, modes 0-3 (controller ruling E): the
  2.3.0 jar's `DsnFile.readStringScope` has no resync loop where HEAD's does,
  and the port follows HEAD, so the two readers legitimately build different
  boards. Mode 4 — the raw token stream — still matches, which is what
  localises the divergence to the parser rather than the scanner, so the
  sweep script excuses only modes 0-3 by name.
- `empty_board.dsn`, mode 3: the file has no `(library …)` scope, so Java's
  `BoardLibrary.padstacks` stays `null` and `RulesWriter.writeRules` throws a
  `NullPointerException` on `padstacks.count()`. The port's field is a value,
  so it writes a complete `.rules` file. Modes 0-2 match, which localises it
  to `RulesWriter`. See `docs/java-quirks.md`'s totalization table.

## Audit

`scripts/audit-port.sh` verifies the crate has zero unaccounted-for public
Java methods. Unlike the Plan 1/2 invocations, these pass the **per-class
map** added for this crate, so a `readScope` in `structure.rs` no longer
satisfies `Network.readScope`:

```sh
./scripts/audit-port.sh io/specctra        crates/fr-dsn/src '*.java' scripts/audit-map/fr-dsn.map
./scripts/audit-port.sh io/specctra/parser crates/fr-dsn/src '*.java' scripts/audit-map/fr-dsn.map
./scripts/audit-port.sh io                 crates/fr-dsn/src \
    'CoordinateTransform.java BoardReadResult.java BoardMetadata.java FileFormat.java KiCadNetClassNames.java' \
    scripts/audit-map/fr-dsn.map
./scripts/audit-port.sh datastructures     crates/fr-dsn/src \
    'IdentifierType.java IndentFileWriter.java' scripts/audit-map/fr-dsn.map
```

All four exit 0 with no `MISSING` and no `UNMAPPED` line. A class the map does
not mention still falls back to the crate-wide search *and* prints
`UNMAPPED <Class>`, so the map cannot silently rot as new classes come into
scope.
