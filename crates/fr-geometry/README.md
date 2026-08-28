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

Invariants: no `f64` on exact paths — coordinate arithmetic stays in `i64`
(promoting to `BigInt` only where Java does); `f64` is only for genuine
approximations (`…Approx`, `distance`, `area`, `length`). Java's
`Math.round` is reproduced as `java_round` (round-half-up), not Rust's
round-half-away-from-zero.
