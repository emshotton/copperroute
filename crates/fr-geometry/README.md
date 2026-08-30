# fr-geometry

A behavioral Rust port of `app.freerouting.geometry.planar` (freerouting v2.3.0):
exact-arithmetic planar geometry — points, vectors, directions, lines, and the
shape hierarchy (boxes, octagons, simplices, polygons, polylines, circles)
used by the autorouter. Every method is ported in Java's source order under
its `snake_case` name; deliberate Java bugs and edge-case crashes are
reproduced rather than fixed (see `docs/java-quirks.md`). Use
`fr_geometry::prelude::*` to bring in every public type.

**Naming.** Java resolves overloads by argument type, Rust does not, so a
Java method with several overloads becomes several Rust methods distinguished
by a suffix naming the argument:

- `_box` / `_octagon` / `_simplex` for the `TileShape` subclasses —
  `IntBox.intersection(IntBox|IntOctagon|Simplex)` becomes
  `intersection`, `intersection_octagon`, `intersection_simplex`; likewise
  `union_*`, `intersects_*`, `is_contained_in_octagon`, `compare_octagon`,
  `cutout_from_*`. The overload taking the receiver's own type keeps the bare
  name.
- `_int` / `_rational` for the `Point`/`Vector`/`Direction` subclasses —
  `side_of_int`/`side_of_rational`, `difference_by_int`/`difference_by_rational`,
  `scalar_product_int`/`scalar_product_rational`, `translate_by_int`/
  `translate_by_rational`, `projection_int`/`projection_rational`,
  `compare_y_rational`.
- `_any` where Java takes the *abstract* base class and dispatches at run time:
  `Line::from_direction_any`, `Line::translate_by_any`.
- `_geometric` for `Line::equals_geometric`, Java's `Line.equals(Object)`. The
  derived `PartialEq` on `Line` is the structural end-point comparison, so the
  two tests are kept apart (see the type-level note in `line.rs`).

Two members are renamed outright rather than suffixed, each marked
`// renamed:` at the definition:
`Direction.getInstanceApprox` → `Direction::from_angle_approx`
(`direction.rs`) and `FortyfiveDegreeDirection.getDirection` →
`FortyfiveDegreeDirection::to_int_direction` (`bounding_directions.rs`).

| Java | Rust |
|---|---|
| `Point` (abstract) | `enum Point { Int(IntPoint), Rational(RationalPoint) }` |
| `Vector` (abstract) | `enum Vector { Int(IntVector), Rational(RationalVector) }` |
| `Direction` (abstract) | `enum Direction { Int(IntDirection), Big(BigIntDirection) }` |
| `TileShape` (abstract) | `enum TileShape { Box(IntBox), Octagon(IntOctagon), Simplex(Simplex) }` |
| `RegularTileShape` | `enum RegularTileShape { Box(IntBox), Octagon(IntOctagon) }` |
| `Shape` (interface) | `trait ShapeOps` + `enum Shape { Tile(TileShape), Polygon(PolygonShape), Circle(Circle) }` |
| `Area` (interface) | `enum Area { Shape(Shape), Polyline(PolylineArea) }` |
| `PolylineShape` (abstract) | `trait PolylineShapeOps` |
| `BigInteger` | `num_bigint::BigInt` |
| `java.util.Random` (JDK) | `struct JavaRandom` (`java_random.rs`) |

One JDK class is ported here too. `JavaRandom` (`java_random.rs`) is
`java.util.Random` reproduced bit for bit — the 48-bit truncated LCG,
`nextInt`'s power-of-two fast path and rejection loop, and `nextDouble`'s two
draws. It was private inside `polygon_shape.rs` until plan-6 ruling 5 promoted
it: `PolygonShape.splitToConvexRecu` starts its concavity scan at
`randomGenerator.nextInt(corners.length)` from the fixed seed 99
(`docs/java-quirks.md` #30), and the maze router's ripup resolver draws from a
`Random` seeded with `ctrl.ripupCosts`. `rand`'s `StdRng` diverges on the first
draw, so it is not a dependency; `crates/fr-geometry/tests/java_random.rs`
pins the stream against a JDK 25 `jshell` run whose command it records.

Invariants: no `f64` on exact paths — coordinate arithmetic stays in `i64`
(promoting to `BigInt` only where Java does); `f64` is only for genuine
approximations (`…Approx`, `distance`, `area`, `length`). Java's
`Math.round` is reproduced as `java_round` (round-half-up), not Rust's
round-half-away-from-zero.

## Conventions this crate shares with the workspace

**`#![forbid(unsafe_code)]`** sits in the crate root (Plan 6 Task 18, at the
user's request). It holds for every workspace crate — `fr-geometry`, `fr-board`,
`fr-dsn`, `fr-settings`, `fr-drc`, `fr-router`, `tests/parity` and the
`freerouting` binary's `main.rs`. The only `unsafe` left in the repository is the
`static mut` PRNG in `scripts/differential/rust/src/bin/p2t13.rs`, a differential
driver rather than a crate; `scripts/differential/README.md` names it.

Deliberate divergences stay greppable: `// not ported:`, `// renamed:`,
`// added in Plan N:`, `// totalized:` and `// Java bug:`, each matching a row in
`docs/java-quirks.md` where the divergence is behavioural.
