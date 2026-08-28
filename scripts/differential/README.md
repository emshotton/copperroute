# Java-vs-Rust differential harness

This directory preserves the harness used while porting `geometry/planar`
(Tasks 14-17): a small Java driver and a Rust "twin" binary that walk the same
seeded pseudo-random inputs through corresponding Java and Rust methods and
print one line of state per call. Diffing the two outputs is far more
effective at finding porting mistakes than hand-tracing, especially for
methods with dozens of branches.

## Layout

- `java/` — driver `.java` files, copied verbatim from the scratch harness
  reviewers built during Tasks 14-17. Each declares
  `package app.freerouting.geometry.planar;` and is compiled *alongside* the
  real, unmodified sources from the sibling `../freerouting` checkout — nothing
  from that repo is copied here.
  - `T14.java` — `Simplex` (Task 14). Twin: `t14`.
  - `T15.java` — `LineSegment` (Task 15). Twin: `t15`.
  - `T16R.java` — `Polyline` (Task 16). Twin: `t16r`.
  - `D17.java` — `PolygonShape` / `PolylineArea` / `Circle` (Task 17). Twin: `d17`.
  - `E15.java` — `LineSegment.stairApproximation` edge cases (Task 15). Twin: `e15`.
  - `P2T3.java` — `ShapeTree`/`MinAreaTree`, fixed 8-box script (Plan 2 Task 3).
    Twin: `p2t3`. `crates/fr-board/tests/min_area_tree.rs` was written from this
    driver's output. Declares `package app.freerouting.datastructures;` and is
    compiled alongside the real
    `datastructures/{ShapeTree,MinAreaTree,ArrayStack}.java`.
  - `P2T3R.java` — `ShapeTree`/`MinAreaTree`, randomised (Plan 2 Task 3).
    Twin: `p2t3r`. Drives `insert(Storable)` (so the *tree* applies its bounding
    directions), `remove(Leaf[])` on arrays with deliberate `null` holes,
    in-place re-keying of live leaves the way `ShapeSearchTree` does, plus
    `overlaps`, `toArray` and `distanceToRoot`. Same `datastructures` sources as
    `P2T3.java`.
  - `support/FRLogger.java` — a minimal stand-in for
    `app.freerouting.logger.FRLogger` (the real class pulls in the Log4j/board
    stack that has nothing to do with geometry). Reproduces the handful of
    `warn`/`debug`/`trace`/`error` overloads the planar package calls.
  - `historical/` — earlier, superseded exploration scripts kept for
    provenance only. `T6.java` and `T6b.java` (Plan 2 Task 6) inline the bodies
    of `Pin.getShape`, `Pin.relativeLocation`, `Pin.getTraceExitRestrictions`,
    `Pin.nearestTraceExitCorner` and `Pin.calcNearestExitRestrictionDirection`
    over the *real* `geometry/planar` classes — `Pin` itself cannot be compiled
    standalone, since it drags in `BasicBoard` and the whole board stack — and
    print the values that `crates/fr-board/tests/drill_items.rs` asserts. Run
    them from *this* directory with
    `JD=../../../freerouting/src/main/java; javac -d out -sourcepath $JD \
      $JD/app/freerouting/geometry/planar/*.java java/support/FRLogger.java \
      java/historical/T6.java java/historical/T6b.java && \
      java -cp out app.freerouting.geometry.planar.T6`.
    The older ones: `T17.java` and `T17b.java` are fixed print-statement
    dumps written before `D17.java` existed (no seeded/diffable format, no
    Rust twin). `RD.java`, `RD2.java` were `D17.java`'s drafts. `RV17.java`
    and `R.java` pin `java.util.Random(99).nextInt(bound)` sequences used
    elsewhere in the Java tree; nothing in the Rust port re-implements
    `java.util.Random`, so there is nothing to diff against. None of these
    are wired into `run.sh`.
- `rust/` — a standalone Cargo package, `fr-geometry-differential`, **not** a
  member of the repo's workspace (see the root `Cargo.toml` `exclude` and this
  package's own `[workspace]` table). It depends on `fr-geometry` and
  `fr-board` by path and builds one `[[bin]]` per twin: `t14`, `t15`, `t16r`,
  `e15`, `d17`, `p2t3`, `p2t3r`.
- `run.sh <driver> [args...]` — compiles the requested Java driver against
  the real sources, builds the matching Rust binary, runs both (passing
  `args` through unchanged to each side, or a per-driver default smoke run
  if none are given), and diffs stdout.

## Running it

Requirements:
- JDK ≥ 23 (`javac`/`java`); set `JAVA_HOME`, or edit the default at the top
  of `run.sh` (it defaults to a Homebrew JDK 23 install).
- A sibling checkout of the Java repo at `../freerouting` relative to this
  repo's root (override with `FREEROUTING_JAVA_DIR`).

```sh
./scripts/differential/run.sh t15               # LineSegment, default smoke run (200 iters, seed 42)
./scripts/differential/run.sh t15 2000 12345    # LineSegment, 2000 iters, seed 12345
./scripts/differential/run.sh d17               # PolygonShape/PolylineArea/Circle, mode 0
./scripts/differential/run.sh d17 200 2         # ...mode 2 (Circle)
```

Extra arguments **replace** the whole default list, not just one field — for
`t15` that means passing a bare iteration count on its own
(`run.sh t15 2000`) leaves the required `seed` argument missing and Java
exits with `ArrayIndexOutOfBoundsException`. Pass every positional argument
the driver expects, or none at all.

`run.sh`:
1. Compiles the driver together with the real `geometry/planar/*.java`, the
   three `datastructures/{Signum,BigIntAux,Stoppable}.java` files it needs,
   any driver-specific extra sources (`p2t3`/`p2t3r` add
   `datastructures/{ShapeTree,MinAreaTree,ArrayStack}.java`), and the local
   `FRLogger` stand-in, into `build/classes/`.
2. Builds the matching Rust binary (`cargo build --release --bin <driver>`
   inside `rust/`).
3. Runs both, passing through whatever arguments follow `<driver>` on the
   command line to *both* sides unchanged, and diffs `build/<driver>.j.out`
   against `build/<driver>.r.out`. With no extra arguments each driver falls
   back to a fixed default — a **smoke run**, not full coverage — defined at
   the top of `run.sh`: `t14 200`, `t15 200 42`, `t16r 200 42 0`, `e15` (no
   arguments), `d17 200 0`.

`build/` and `rust/target/` are gitignored scratch output.

### Per-driver arguments and mode coverage

- `t14 <iters>` — `Simplex`. No mode/seed argument (the seed is a fixed
  constant baked into both the Java and Rust generators).
- `t15 <iters> <seed> [c]` — `LineSegment`. `seed` and the optional shape
  constant `c` are both read on the Java and the Rust side.
- `t16r <iters> <seed> <mode>` — `Polyline`. Modes `0`, `1`, and `4` all
  exercise the same general polyline-fuzzing path with different point-count
  profiles (verified: all three reproduce the same category of diffs, see
  below); mode `3` exercises a distinct, smaller "pool of shared points"
  scenario and — verified — matches Java exactly (0 diffs over 400 lines at
  seed 42).
- `e15` — `LineSegment.stairApproximation` edge cases. No arguments; it's a
  fixed sequence of edge-case calls, not seeded generation.
- `p2t3` — `ShapeTree`/`MinAreaTree`. No arguments; a fixed 8-box insert/query/
  remove script plus tie-break and edge cases.
- `p2t3r <ops> <seed> <mode> [dumpEvery] [insertPct]` — `ShapeTree`/`MinAreaTree`,
  randomised. `mode` is `0` for orthogonal (`IntBox`) bounds and `1` for
  45-degree (`IntOctagon`) bounds — run **both**, since only mode `1` proves the
  tree applies its own bounding directions rather than storing the box it was
  handed. `dumpEvery` (default 1) prints the whole tree every N ops; `insertPct`
  (default 62) is the share of insert ops, the rest split between re-keying and
  removal.
- `d17 <cases> <mode>` — `PolygonShape`/`PolylineArea`/`Circle`. Java's
  `D17.java` only branches explicitly on mode `0` (polygon) and `1`
  (polyline area); every other mode value, including `2`, falls through to
  its final `else`, which is the `Circle` case (`D17.java`'s own comment
  reads `// 0 = polygon, 1 = polyline area, 2 = circle`, but `2` isn't a
  distinct branch — it's just what falls through). `d17.rs` mirrors that
  exact structure: explicit `mode == 0`/`mode == 1` branches, then an
  unconditional `else` that is `Circle`. **Verified** by running
  `run.sh d17 200 <mode>` for `mode` in `0`, `1`, `2`: all three **match
  exactly**. The one mode that does *not* match is `3` — see "Harness
  maintenance note" below; Java's placeholder for the never-real
  `splitPiecesForDiff` branch and Rust's `Circle` fallthrough disagree, so
  avoid `mode 3` when running `d17` by hand.

## Known, expected diffs

Verified at HEAD, default smoke-run arguments, JDK 23:

| driver | lines | diff lines | classification |
|---|---|---|---|
| `d17` (mode 0) | 200 | 0 | exact match |
| `e15` | 16 | 3 | `LineSegment.stairApproximation`/`45` on a non-finite width (quirk #21) |
| `t15` (seed 42) | 11747 | 44 | 42 cosmetic (`EXC:ArithmeticException` vs `EXC:panic`, see below) + 2 sign-of-zero in `LineSegment.startPointApprox`/`endPointApprox` (quirk #14) |
| `t16r` (mode 0, seed 42) | 9200 | 72 | all `lineSegment`/`offsetBox` fields: `LineSegment(Polyline, no)` with `no` out of its valid range (Java constructs a degenerate object with null internal lines that later NPEs; Rust's `LineSegment::from_polyline`/`Polyline::offset_box` return `None` up front — the `offsetBox` case is the `Polyline.offsetBox(halfWidth, no)` row in the `totalized` table) |
| `t14` | 17997 | 145 | `Simplex.EMPTY` / degenerate-shape edge cases already in the `totalized` table |
| `p2t3` | 132 | 0 | exact match |
| `p2t3r` (400 42 0) | 109940 | 0 | exact match |
| `p2t3r` (2000 42 1) | 2898938 | 0 | exact match (45-degree/`IntOctagon` bounds) |

Every diff line traces to an already-documented, deliberate divergence in
`docs/java-quirks.md`'s `pinned`/`totalized` tables, plus one purely cosmetic
difference that recurs across every driver: the Java drivers tag a caught
exception with its real class name (`EXC:ArithmeticException`,
`EXC:NullPointerException`, ...), while the Rust drivers catch a panic via
`catch_unwind` and can only report the generic `EXC:panic` (a Rust panic
doesn't carry a Java-style exception type). Neither is a regression; re-run
after any future `geometry/planar` change and compare new diff lines against
`docs/java-quirks.md` before treating them as bugs — and re-run with a
different seed/iteration count (not just the smoke-run default) for anything
touching the classes above, since the default only samples a few hundred
cases.

## Harness maintenance note (Task 18)

`D17.java`'s `mode == 3` branch originally called a `PolygonShape.splitPiecesForDiff()`
that never existed on the real, unmodified class (apparently a debug hook a
reviewer meant to add and never did, or reverted). It doesn't compile against
`../freerouting` as checked in, and `d17.rs` never implemented that mode
either, so it has been replaced with a short placeholder string rather than
inventing the method on the Java side. `T16R.java`'s Rust twin (`t16r.rs`)
needed a handful of call sites updated for the `Polyline` methods that now
return `Result<Polyline, PolylineError>` instead of `Polyline`/`Option` (see
`docs/java-quirks.md` quirk #22) — mapped `Err` to the same `"EXC"` tag the
Java side already used for the equivalent exception.
