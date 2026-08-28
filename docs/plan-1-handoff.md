# Plan 1 hand-off — foundation & `fr-geometry`

Branch `plan-1-foundation`, 36 commits on top of `main` (`e1145d1`). Final
whole-branch review: **merge with fixes**; fix wave applied and re-reviewed clean.

## Delivered
- Cargo workspace; `freerouting` binary (clap `route|drc|info|mcp`; stubs exit 3)
  with a Java-faithful `-de/-do` legacy shim; native stdio MCP skeleton.
- `parity` helper crate + `scripts/gen-reference.sh` (references NOT generated —
  needs JDK 25; see "Open items").
- `fr-geometry`: all 34 `geometry/planar` classes; 244 tests; audit script
  (`scripts/audit-geometry-port.sh`, exit 0 against 651 Java declarations);
  Java-vs-Rust differential harness (`scripts/differential/`, baseline in its
  README, reproduced exactly by the final review).
- `docs/java-quirks.md` (34 pinned quirks, totalizations, obligations),
  `docs/cli-legacy-flags.md`, `docs/geometry-library-survey.md`.

## Rulings made during execution (all reversible)
1. Work in place on a branch, not a nested worktree (harness resolves `../freerouting`).
2. Implementers may not install system packages; Java-25 references deferred to the user.
3. Java is the authority over plan test data; changes must cite the Java line; never bend a test to fit a bug.
4. Tautological test placeholders must be pinned before commit.
5. MCP `id: null` treated as a notification — deferred (MCP spec forbids null ids).
6. `IntDirection`/`Direction`/`Line` have **no `Ord`**; Java `compareTo` is exposed as inherent `compare_to` because Java's ordering is not antisymmetric at NULL / degenerate lines. `PartialEq` = Java `equals` (collinear ∧ positive projection, structural shortcut).
7. Lengths use `sqrt(x²+y²)` (`FloatPoint::size`), never `hypot`.
8. `getId`/`hashCode` ports use `wrapping_*`.
9. Interior points give `Line::side_of == OnTheRight` for every border line (differential-tested).
10. `Math.min/max` on doubles → NaN-propagating `java_min/java_max`; `Double.MIN_VALUE` = `f64::from_bits(1)`.
11. `docs/java-quirks.md` is mandatory: every reproduced quirk/totalization gets a row.
12. Totalizations (Rust value where Java crashes) are allowed only when no reachable Java caller observes the difference; otherwise surface the failure (`Result`/panic). Applied: `Polyline::from_lines -> Result<_, PolylineError>`; border-line-free `TileShape::distance` family panics (Task 14's `f64::MAX` withdrawn).
13. The differential harness is preserved in-repo.
14. `PolylineArea` border/holes are `PolylineShapeRef { Tile, Polygon }` (Java passes `IntBox` borders from `DrillPage`/`BoardOutline`).
15. `-de` with a `.json` part maps to a new `--kicad-json` option — a deliberate divergence (Java overloads the design/session slots); documented in `docs/cli-legacy-flags.md`.
16. `gen-reference.sh` disables routing with `--router.enabled=false` (`-mp 0` means *unlimited* passes in Java).

## Parked residuals (final review, no second fix wave)
- Legacy shim: bare `-drc` (no path) errors; Java accepts it and enters DRC-only mode. Plan 8.
- Legacy shim matches flags exactly; Java uses `startsWith` (`-mpx 5` ≡ `-mp 5`). Stricter; documented here only. Plan 8 decides.
- `gen-reference.sh` has never been executed: first run on JDK 25 must inspect the `(routes …)` section of each `unrouted.ses` (README says how).
- `docs/cli-legacy-flags.md:73-75`: `-mt` consumer list incomplete (`BatchOptimizerMultiThreaded.java:41`, `GlobalSettings.java:853`); headless path `BatchOptimizer.createForHeadless` never reads `optimizer.maxThreads`.
- `docs/java-quirks.md` process note over-claims `grep "obligation:"` coverage (only the two MCP notes carry the token).

## Obligations for later plans (details in `docs/java-quirks.md`)
- Plan 2: `PolylineError` → pass abort; memo cache for convex pieces; `equals_geometric` not `==` for `Line`; `border_line_index == None` on box/octagon is a Java stub, not "not a border line"; marker-hygiene sweep (`// Java bug:` literal prefix).
- Plans 6/7: `catch_unwind`/`Result` boundaries at `AutoroutePassRunner.java:144` (per pass) and `BatchAutorouterThread.java:537` (per item); Java `catch (Exception)` does not catch `StackOverflowError`.
- Plan 5: apply Java flag normalisation (`-oit /100`, `-mp`/`-mt` clamps, `-us`/`-is` folding) in `fr-settings`.
- Plan 8: MCP handler shape (progress sink, cancel token, reader thread); `id: null`; bare `-drc`.
- Before `fr-dsn` formats floats: `FloatPoint::Display` differs from Java `NumberFormat` above 2^53 and at 4th-digit ties.

## Open items for the user
- Install JDK 25 (`brew install openjdk@25`), run `scripts/gen-reference.sh`, inspect outputs.
- Post-parity improvement candidates are listed in `docs/java-quirks.md` (`i128` fast path in `Line::intersection` is the big one) and `docs/geometry-library-survey.md` (`i_overlay` for copper pours).
