# fr-geometry

A behavioral Rust port of `app.freerouting.geometry.planar` (freerouting v2.3.0):
exact-arithmetic planar geometry — points, vectors, directions, lines, and the
shape hierarchy (boxes, octagons, simplices, polygons, polylines, circles)
used by the autorouter. Every method is ported in Java's source order under
its `snake_case` name; deliberate Java bugs and edge-case crashes are
reproduced rather than fixed (see `docs/java-quirks.md`). Use
`fr_geometry::prelude::*` to bring in every public type.

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
