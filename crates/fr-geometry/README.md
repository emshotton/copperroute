# fr-geometry

Exact-arithmetic planar geometry for the autorouter: points, vectors,
directions, lines, and the shape hierarchy (boxes, octagons, simplices,
polygons, polylines, circles). Every other crate in the workspace sits on
this one. Use `fr_geometry::prelude::*` to bring in every public type.

The crate depends on `num-bigint`, `num-traits` and `num-integer`, and on
nothing else.

## Type families

| Family | Types |
|---|---|
| Points | `IntPoint`, `RationalPoint`, `enum Point { Int, Rational }`, `FloatPoint` (approximate) |
| Vectors and directions | `IntVector`, `RationalVector`, `enum Vector`, `IntDirection`, `BigIntDirection`, `enum Direction`, `FortyfiveDegreeDirection` |
| Lines | `Line` (two integer points), `LineSegment`, `FloatLine` (approximate) |
| Tile shapes (convex, integer-bounded) | `IntBox`, `IntOctagon`, `Simplex`, `enum TileShape { Box, Octagon, Simplex }`, `enum RegularTileShape { Box, Octagon }` |
| Other shapes | `PolygonShape`, `Circle`, `Ellipse`, `enum Shape { Tile, Polygon, Circle }` |
| Areas (shapes with holes) | `PolylineArea`, `enum Area { Shape, Polyline }` |
| Polylines | `Polyline` (a sequence of lines whose consecutive intersections are the corners), `Polygon` |
| Traits | `ShapeOps` (the operations every shape answers), `PolylineShapeOps` |

The enums replace run-time dispatch over a class hierarchy: a `Point` is
either integer or rational, a `TileShape` is one of three convex forms, and
callers match on the variant.

**Naming of overloads.** Where an operation exists for several argument
types, the Rust methods carry a suffix naming the argument:

- `_box` / `_octagon` / `_simplex` for the tile-shape operations —
  `IntBox::intersection`, `intersection_octagon`, `intersection_simplex`;
  likewise `union_*`, `intersects_*`, `is_contained_in_octagon`,
  `compare_octagon`, `cutout_from_*`. The variant taking the receiver's own
  type keeps the bare name.
- `_int` / `_rational` for the point, vector and direction operations —
  `side_of_int`/`side_of_rational`, `difference_by_int`/`difference_by_rational`,
  `scalar_product_int`/`scalar_product_rational`, `translate_by_int`/
  `translate_by_rational`, `projection_int`/`projection_rational`,
  `compare_y_rational`.
- `_any` where the argument is the enum and dispatch happens at run time:
  `Line::from_direction_any`, `Line::translate_by_any`.
- `_geometric` for `Line::equals_geometric`. The derived `PartialEq` on `Line`
  is the structural end-point comparison; the geometric test asks whether two
  lines coincide, which is a different question (see the type-level note in
  `line.rs`).

## Invariants

- **No `f64` on exact paths.** Coordinate arithmetic stays in `i64`,
  promoting to `BigInt` only where the intermediate can overflow.
  `bigint_aux` and `Signum` are the helpers for that. `f64` appears only in
  genuine approximations (`…Approx` methods, `distance`, `area`, `length`) and
  in the `Float*` types.
- **`CRIT_INT`** (`limits.rs`) is the coordinate magnitude above which a
  product of two coordinates no longer fits an `i64` without promotion.
- **Rounding is round-half-up** (`java_round`), not round-half-away-from-zero.
  Every place that converts an approximate value back to an integer coordinate
  goes through it, so a coordinate rounds the same way everywhere.
- **`Polyline::from_lines` returns `Result<_, PolylineError>`.** A line list
  that cannot be normalised into a polyline (parallel consecutive lines, fewer
  than two lines) is an error, never an empty polyline. Callers propagate it.
- **`Line` carries a private identity token.** Two lines with equal end points
  are `==`; a trace's change detection additionally asks whether a line is *the
  same object* it started with (`Line::is_same_object`), because a rebuilt
  polyline can be value-equal to its input without being the same polyline.
  The token is not part of `Hash`, `Ord`, `Debug` or any output, so it cannot
  affect ordering or serialisation. Nothing else may use it as a stand-in for
  `==`.

## `JavaRandom`

`java_random.rs` is a 48-bit truncated linear congruential generator with
`next_int`, `next_double` and `set_seed`. It has two users with a fixed-seed
contract: `PolygonShape::split_to_convex` starts its concavity scan at a
pseudo-random corner from seed 99, and the router's ripup resolver draws from
a generator seeded with the ripup cost. Both need the exact same stream on
every run for routing to be reproducible, which is why the crate carries its
own generator rather than depending on `rand`.
`crates/fr-geometry/tests/java_random.rs` pins the stream.

## Tests

`cargo test -p fr-geometry` runs the unit tests plus eight integration
suites: `border_line_index`, `consistency`, `java_random`,
`nearest_and_stairs`, `polygon_geometry`, `polyline`, `rational_point` and
`tail`. None of them needs anything outside the repository.

## Conventions this crate shares with the workspace

**`#![forbid(unsafe_code)]`** sits in the crate root, as it does in every
workspace crate — `fr-geometry`, `fr-board`, `fr-dsn`, `fr-settings`,
`fr-drc`, `fr-router`, `fr-core`, `tests/parity` and the `freerouting`
binary's `main.rs`.
