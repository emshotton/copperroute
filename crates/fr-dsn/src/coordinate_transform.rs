//! `io/CoordinateTransform.java` — board-coordinate ⇄ external-coordinate (DSN/KiCad JSON)
//! conversion.
//!
//! **Not** `board/state/CoordinateTransform.java`, which is a different, view-only class (screen
//! ⇄ board coordinates for the GUI) and is out of scope for this port.

use fr_geometry::{Circle, FloatPoint, IntBox, Line, Shape, ShapeOps, TileShape, Vector};

use crate::error::DsnError;
use crate::parser::geometry::{DsnCircle, DsnLayer, DsnPolygon, DsnRectangle, DsnShape};

/// "Computes transformations between board coordinates and external coordinates, such as
/// Specctra DSN or KiCad JSON coordinates" (CoordinateTransform.java:16-18).
///
/// Java's `implements Serializable` is `not ported: Serializable` — the port has no Java object
/// serialization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoordinateTransform {
    /// `CoordinateTransform.scaleFactor` (CoordinateTransform.java:22).
    scale_factor: f64,
    /// `CoordinateTransform.baseX` (CoordinateTransform.java:23).
    base_x: f64,
    /// `CoordinateTransform.baseY` (CoordinateTransform.java:24).
    base_y: f64,
}

impl CoordinateTransform {
    /// `CoordinateTransform(double, double, double)` (CoordinateTransform.java:27-31).
    ///
    // Java bug: (#89) the constructor accepts any `double` — a `0`, an infinity, a `NaN` — and
    // says nothing. `Structure.createBoard` builds `new CoordinateTransform(0, 0, 0)` for real
    // (quirk #94), and from then on every coordinate the DSN/SES writers emit is
    // `Infinity`/`NaN` while every coordinate the reader transforms collapses to `0`, with the
    // whole read still reported as `Success`. JVM-verified.
    //
    // fixed: T4 (#89) — a scale factor that is not finite and non-zero is refused here, loudly.
    // Every arithmetic method on this type divides or multiplies by it, so this is the one place
    // that can make the silent-`Infinity` state unrepresentable rather than merely unlikely; #94
    // (the `f64` scale-factor loop) is what stops `createBoard` reaching for it in the first
    // place, and the two land in the same commit.
    ///
    /// # Errors
    ///
    /// [`DsnError::InvalidScaleFactor`] when `scale_factor` is zero, infinite or `NaN`.
    pub fn new(
        scale_factor: f64,
        base_x: f64,
        base_y: f64,
    ) -> Result<CoordinateTransform, DsnError> {
        if !scale_factor.is_finite() || scale_factor == 0.0 {
            return Err(DsnError::InvalidScaleFactor { scale_factor });
        }
        Ok(CoordinateTransform {
            scale_factor,
            base_x,
            base_y,
        })
    }

    /// `CoordinateTransform.scaleFactor` (CoordinateTransform.java:22).
    ///
    /// **Not a Java accessor** — the three fields are `private final` with no getters, and Java's
    /// own probes read them by reflection. The port exposes them because a `CoordinateTransform`
    /// now *leaves* the reader on [`crate::BoardReadResult`] (plan-3 controller ruling A), so a
    /// caller that has one has no other way to see what it scales by. Read-only: there is no
    /// setter, exactly as in Java.
    #[must_use]
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    /// `CoordinateTransform.baseX` (CoordinateTransform.java:23) — see [`Self::scale_factor`].
    #[must_use]
    pub fn base_x(&self) -> f64 {
        self.base_x
    }

    /// `CoordinateTransform.baseY` (CoordinateTransform.java:24) — see [`Self::scale_factor`].
    #[must_use]
    pub fn base_y(&self) -> f64 {
        self.base_y
    }

    /// `boardToDsn(double)` (CoordinateTransform.java:34-36): "scales a value from the board to
    /// the external coordinate system".
    //
    // Java bug: (#89) CoordinateTransform.boardToDsn with a `scaleFactor` of 0 yields
    // `±Infinity` (or `NaN` for a value of 0) rather than throwing, because `/` on `double`s is
    // IEEE division. // fixed: T4 (#89) — not here, but at the constructor: a `CoordinateTransform`
    // whose scale factor is zero or non-finite cannot be built any more, so this division has
    // nothing left to divide by.
    #[must_use]
    pub fn board_to_dsn(&self, value: f64) -> f64 {
        value / self.scale_factor
    }

    /// `boardToDsn(FloatPoint)` (CoordinateTransform.java:39-44).
    // renamed: boardToDsn(FloatPoint) -> board_to_dsn_point (Rust has no overloading).
    #[must_use]
    pub fn board_to_dsn_point(&self, point: &FloatPoint) -> [f64; 2] {
        [
            self.board_to_dsn(point.x) + self.base_x,
            self.board_to_dsn(point.y) + self.base_y,
        ]
    }

    /// `boardToDsn(FloatPoint[])` (CoordinateTransform.java:47-54): `x0, y0, x1, y1, …`.
    // renamed: boardToDsn(FloatPoint[]) -> board_to_dsn_points.
    #[must_use]
    pub fn board_to_dsn_points(&self, points: &[FloatPoint]) -> Vec<f64> {
        let mut result = Vec::with_capacity(2 * points.len());
        for point in points {
            result.push(self.board_to_dsn(point.x) + self.base_x);
            result.push(self.board_to_dsn(point.y) + self.base_y);
        }
        result
    }

    /// `boardToDsn(Line[])` (CoordinateTransform.java:57-68): four numbers per line, the two
    /// defining points of each.
    // renamed: boardToDsn(Line[]) -> board_to_dsn_lines.
    #[must_use]
    pub fn board_to_dsn_lines(&self, lines: &[Line]) -> Vec<f64> {
        let mut result = Vec::with_capacity(4 * lines.len());
        for line in lines {
            let a = FloatPoint::from_int(&line.a);
            let b = FloatPoint::from_int(&line.b);
            result.push(self.board_to_dsn(a.x) + self.base_x);
            result.push(self.board_to_dsn(a.y) + self.base_y);
            result.push(self.board_to_dsn(b.x) + self.base_x);
            result.push(self.board_to_dsn(b.y) + self.base_y);
        }
        result
    }

    /// `boardToDsn(Vector)` (CoordinateTransform.java:71-77): a vector carries no origin, so the
    /// base offsets do **not** apply.
    // renamed: boardToDsn(Vector) -> board_to_dsn_vector.
    #[must_use]
    pub fn board_to_dsn_vector(&self, vector: &Vector) -> [f64; 2] {
        let value = vector.to_float();
        [self.board_to_dsn(value.x), self.board_to_dsn(value.y)]
    }

    /// `boardToDsn(IntBox)` (CoordinateTransform.java:80-87): lower-left x/y, upper-right x/y.
    // renamed: boardToDsn(IntBox) -> board_to_dsn_box.
    #[must_use]
    pub fn board_to_dsn_box(&self, b: &IntBox) -> [f64; 4] {
        [
            f64::from(b.ll.x) / self.scale_factor + self.base_x,
            f64::from(b.ll.y) / self.scale_factor + self.base_y,
            f64::from(b.ur.x) / self.scale_factor + self.base_x,
            f64::from(b.ur.y) / self.scale_factor + self.base_y,
        ]
    }

    /// `boardToDsn(geometry.planar.Shape, Layer)` (CoordinateTransform.java:90-107).
    ///
    /// `None` is Java's `null` — the `else` branch that warns
    /// "CoordinateTransform.board_to_dsn not yet implemented for boardShape"
    /// (CoordinateTransform.java:103-104). Java's chain tests `IntBox` first, then any other
    /// `PolylineShape` (the two remaining tiles and `PolygonShape`), then `Circle`; nothing else
    /// implements `geometry.planar.Shape`, so the `else` is in fact unreachable — the `Option`
    /// is the faithful port of a `null` its callers test for, not a totalization.
    // renamed: boardToDsn(Shape, Layer) -> board_to_dsn_shape.
    #[must_use]
    pub fn board_to_dsn_shape(&self, board_shape: &Shape, layer: DsnLayer) -> Option<DsnShape> {
        match board_shape {
            Shape::Tile(TileShape::Box(b)) => Some(DsnShape::Rect(DsnRectangle::new(
                layer,
                self.board_to_dsn_box(b),
            ))),
            Shape::Tile(_) | Shape::Polygon(_) => {
                let corners = board_shape.corner_approx_arr();
                let coordinates = self.board_to_dsn_points(&corners);
                Some(DsnShape::Polygon(DsnPolygon::new(layer, coordinates)))
            }
            Shape::Circle(board_circle) => {
                Some(DsnShape::Circle(self.circle(layer, board_circle, false)))
            }
        }
    }

    /// `boardToDsnRel(FloatPoint)` (CoordinateTransform.java:110-115): relative (vector)
    /// coordinates, i.e. without the base offsets.
    // renamed: boardToDsnRel(FloatPoint) -> board_to_dsn_rel_point.
    #[must_use]
    pub fn board_to_dsn_rel_point(&self, point: &FloatPoint) -> [f64; 2] {
        [self.board_to_dsn(point.x), self.board_to_dsn(point.y)]
    }

    /// `boardToDsnRel(FloatPoint[])` (CoordinateTransform.java:118-125).
    // renamed: boardToDsnRel(FloatPoint[]) -> board_to_dsn_rel_points.
    #[must_use]
    pub fn board_to_dsn_rel_points(&self, points: &[FloatPoint]) -> Vec<f64> {
        let mut result = Vec::with_capacity(2 * points.len());
        for point in points {
            result.push(self.board_to_dsn(point.x));
            result.push(self.board_to_dsn(point.y));
        }
        result
    }

    /// `boardToDsnRel(IntBox)` (CoordinateTransform.java:128-135).
    // renamed: boardToDsnRel(IntBox) -> board_to_dsn_rel_box.
    #[must_use]
    pub fn board_to_dsn_rel_box(&self, b: &IntBox) -> [f64; 4] {
        [
            f64::from(b.ll.x) / self.scale_factor,
            f64::from(b.ll.y) / self.scale_factor,
            f64::from(b.ur.x) / self.scale_factor,
            f64::from(b.ur.y) / self.scale_factor,
        ]
    }

    /// `boardToDsnRel(geometry.planar.Shape, Layer)` (CoordinateTransform.java:138-155).
    ///
    /// Note the circle branch: Java scales the **diameter** with `boardToDsn`, not
    /// `boardToDsnRel` (CoordinateTransform.java:147 == :99, both plain `boardToDsn`), and only
    /// the centre with the relative transform.
    // renamed: boardToDsnRel(Shape, Layer) -> board_to_dsn_rel_shape.
    #[must_use]
    pub fn board_to_dsn_rel_shape(&self, board_shape: &Shape, layer: DsnLayer) -> Option<DsnShape> {
        match board_shape {
            Shape::Tile(TileShape::Box(b)) => Some(DsnShape::Rect(DsnRectangle::new(
                layer,
                self.board_to_dsn_rel_box(b),
            ))),
            Shape::Tile(_) | Shape::Polygon(_) => {
                let corners = board_shape.corner_approx_arr();
                let coordinates = self.board_to_dsn_rel_points(&corners);
                Some(DsnShape::Polygon(DsnPolygon::new(layer, coordinates)))
            }
            Shape::Circle(board_circle) => {
                Some(DsnShape::Circle(self.circle(layer, board_circle, true)))
            }
        }
    }

    /// The circle branch shared by `boardToDsn(Shape, Layer)` (CoordinateTransform.java:98-101)
    /// and `boardToDsnRel(Shape, Layer)` (:146-149).
    fn circle(&self, layer: DsnLayer, board_circle: &Circle, relative: bool) -> DsnCircle {
        let diameter = 2.0 * self.board_to_dsn(f64::from(board_circle.radius));
        let center = FloatPoint::from_int(&board_circle.center);
        let center_coordinates = if relative {
            self.board_to_dsn_rel_point(&center)
        } else {
            self.board_to_dsn_point(&center)
        };
        DsnCircle::new(
            layer,
            [diameter, center_coordinates[0], center_coordinates[1]],
        )
    }

    /// `dsnToBoard(double)` (CoordinateTransform.java:158-160).
    #[must_use]
    pub fn dsn_to_board(&self, value: f64) -> f64 {
        value * self.scale_factor
    }

    /// `dsnToBoard(double[])` (CoordinateTransform.java:163-167).
    // renamed: dsnToBoard(double[]) -> dsn_to_board_point.
    #[must_use]
    pub fn dsn_to_board_point(&self, tuple: &[f64; 2]) -> FloatPoint {
        FloatPoint::new(
            self.dsn_to_board(tuple[0] - self.base_x),
            self.dsn_to_board(tuple[1] - self.base_y),
        )
    }

    /// `dsnToBoardRel(double[])` (CoordinateTransform.java:170-174).
    #[must_use]
    pub fn dsn_to_board_rel(&self, tuple: &[f64; 2]) -> FloatPoint {
        FloatPoint::new(self.dsn_to_board(tuple[0]), self.dsn_to_board(tuple[1]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fr_geometry::IntPoint;

    #[test]
    fn board_to_dsn_vector_ignores_the_base_offsets() {
        let transform = CoordinateTransform::new(10.0, 100.0, 200.0).expect("a valid scale");
        let vector = Vector::Int(fr_geometry::IntVector::new(100, 200));
        assert_eq!(transform.board_to_dsn_vector(&vector), [10.0, 20.0]);
    }

    #[test]
    fn board_to_dsn_lines_writes_four_numbers_per_line() {
        let transform = CoordinateTransform::new(10.0, 0.0, 0.0).expect("a valid scale");
        let line = Line::new(IntPoint::new(0, 10), IntPoint::new(20, 30));
        assert_eq!(
            transform.board_to_dsn_lines(std::slice::from_ref(&line)),
            vec![0.0, 1.0, 2.0, 3.0]
        );
    }

    #[test]
    fn board_to_dsn_shape_of_a_circle_doubles_the_radius_into_a_diameter() {
        let transform = CoordinateTransform::new(10.0, 1.0, 2.0).expect("a valid scale");
        let shape = Shape::Circle(Circle::new(IntPoint::new(100, 200), 50));
        let Some(DsnShape::Circle(circle)) =
            transform.board_to_dsn_shape(&shape, DsnLayer::signal())
        else {
            panic!("a Circle becomes a Circle");
        };
        assert_eq!(circle.coor, [10.0, 11.0, 22.0]);

        // The relative variant offsets neither the centre nor the diameter.
        let Some(DsnShape::Circle(rel)) =
            transform.board_to_dsn_rel_shape(&shape, DsnLayer::signal())
        else {
            panic!("a Circle becomes a Circle");
        };
        assert_eq!(rel.coor, [10.0, 10.0, 20.0]);
    }
}
