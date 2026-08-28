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
  - `support/FRLogger.java` — a minimal stand-in for
    `app.freerouting.logger.FRLogger` (the real class pulls in the Log4j/board
    stack that has nothing to do with geometry). Reproduces the handful of
    `warn`/`debug`/`trace`/`error` overloads the planar package calls.
  - `historical/` — earlier, superseded exploration scripts kept for
    provenance only. `T17.java` and `T17b.java` are fixed print-statement
    dumps written before `D17.java` existed (no seeded/diffable format, no
    Rust twin). `RD.java`, `RD2.java` were `D17.java`'s drafts. `RV17.java`
    and `R.java` pin `java.util.Random(99).nextInt(bound)` sequences used
    elsewhere in the Java tree; nothing in the Rust port re-implements
    `java.util.Random`, so there is nothing to diff against. None of these
    are wired into `run.sh`.
- `rust/` — a standalone Cargo package, `fr-geometry-differential`, **not** a
  member of the repo's workspace (see the root `Cargo.toml` `exclude` and this
  package's own `[workspace]` table). It depends on `fr-geometry` by path and
  builds one `[[bin]]` per twin: `t14`, `t15`, `t16r`, `e15`, `d17`.
- `run.sh <driver> [seed]` — compiles the requested Java driver against the
  real sources, builds the matching Rust binary, runs both, and diffs stdout.

## Running it

Requirements:
- JDK ≥ 23 (`javac`/`java`); set `JAVA_HOME`, or edit the default at the top
  of `run.sh` (it defaults to a Homebrew JDK 23 install).
- A sibling checkout of the Java repo at `../freerouting` relative to this
  repo's root (override with `FREEROUTING_JAVA_DIR`).

```sh
./scripts/differential/run.sh t15 42     # LineSegment, seed 42
./scripts/differential/run.sh d17        # PolygonShape/PolylineArea/Circle
```

`run.sh`:
1. Compiles the driver together with the real `geometry/planar/*.java`, the
   three `datastructures/{Signum,BigIntAux,Stoppable}.java` files it needs,
   and the local `FRLogger` stand-in, into `build/classes/`.
2. Builds the matching Rust binary (`cargo build --release --bin <driver>`
   inside `rust/`).
3. Runs both (fixed argument conventions per driver — see `run.sh`; the
   `seed` argument only applies to `t15` and `t16r`, which take a runtime
   seed on both sides) and diffs `build/<driver>.j.out` against
   `build/<driver>.r.out`.

`build/` and `rust/target/` are gitignored scratch output.

## Known, expected diffs

`d17` matches exactly. `t14`, `t15`, `t16r`, and `e15` still report a handful
of diff lines — all of them already-documented, deliberate divergences from
`docs/java-quirks.md`'s `totalized` section (e.g. `Simplex.EMPTY` edge cases,
`LineSegment.stairApproximation` on a non-positive width) plus one purely
cosmetic difference: the Java drivers tag a caught exception with its real
class name (`EXC:ArithmeticException`), while the Rust drivers catch a panic
via `catch_unwind` and can only report `EXC:panic` (Rust panics don't carry a
Java-style exception type). Neither is a regression; re-run after any future
`geometry/planar` change and compare new diff lines against
`docs/java-quirks.md` before treating them as bugs.

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
