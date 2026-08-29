//! Port of `app.freerouting.geometry.planar.PolygonShape`: "Shape described bei a closed polygon
//! of corner points. The corners are ordered in counterclock sense around the border of the
//! shape. The corners are normalised, so that the corner with the lowest y-value comes first. In
//! case of equal y-value the corner with the lowest x-value comes first."
//! (PolygonShape.java:9-14).
//!
//! The three `transient precalculated*` memo fields are dropped, as everywhere else in this port.
//! For `splitToConvex` that is observationally free: Java reseeds its static `Random` with the
//! fixed seed 99 at the top of every non-memoised call (PolygonShape.java:534), so recomputing
//! reproduces the identical division sequence.

use crate::float_point::FloatPoint;
use crate::int_box::IntBox;
use crate::int_octagon::IntOctagon;
use crate::int_point::IntPoint;
use crate::java_random::JavaRandom;
use crate::line::Line;
use crate::point::Point;
use crate::polygon::Polygon;
use crate::polyline::Polyline;
use crate::polyline_shape::PolylineShapeOps;
use crate::side::Side;
use crate::tile_shape::TileShape;
use crate::vector::Vector;

/// The fixed seed of the `Random` that picks the first corner of the concavity search
/// (PolygonShape.java:17).
const SEED: i64 = 99;

/// A shape described by a closed polygon of corner points.
#[derive(Debug, Clone, PartialEq)]
pub struct PolygonShape {
    corners: Vec<Point>,
}

/// The `IntPoint` behind a corner. Java's `PolygonShape` stores abstract `Point`s but feeds them
/// straight into `new Line(Point, Point)`, which warns "only implemented for IntPoints till now"
/// (Line.java:19-27) and then breaks in every arithmetic method. This port panics at the point of
/// use, as [`crate::polyline::Polyline`] does.
fn int_point_of(point: &Point) -> IntPoint {
    match point {
        Point::Int(p) => *p,
        Point::Rational(_) => {
            panic!("PolygonShape: only implemented for IntPoints till now (Line.java:19-27)")
        }
    }
}

impl PolygonShape {
    /// Creates a new instance of `PolygonShape` (PolygonShape.java:27-92).
    ///
    /// The constructor reverses a clockwise polygon, drops a closing duplicate corner and a
    /// collinear first/last corner, and finally rotates the corner list so that the corner with
    /// the lowest y (then lowest x) comes first.
    ///
    /// # Panics
    /// On a corner-less polygon, exactly as Java does: `currentCorners[startCornerNo].toFloat()`
    /// indexes an empty array (PolygonShape.java:71).
    pub fn from_polygon(polygon: &Polygon) -> PolygonShape {
        let current_polygon = if polygon.winding_number_after_closing() < 0 {
            // the corners of the polygon are in clockwise sense
            polygon.revert_corners()
        } else {
            polygon.clone()
        };
        let current_corners = current_polygon.corner_array();
        let mut last_corner_no = current_corners.len() as isize - 1;

        if last_corner_no > 0 && current_corners[0] == current_corners[last_corner_no as usize] {
            // skip last point
            last_corner_no -= 1;
        }

        let mut last_point_collinear = false;

        if last_corner_no >= 2 {
            let l = last_corner_no as usize;
            last_point_collinear = current_corners[l]
                .side_of(&current_corners[l - 1], &current_corners[0])
                == Side::Collinear;
        }
        if last_point_collinear {
            // skip last point
            last_corner_no -= 1;
        }

        let mut first_corner_no: isize = 0;
        let mut first_point_collinear = false;

        if last_corner_no - first_corner_no >= 2 {
            first_point_collinear = current_corners[0].side_of(
                &current_corners[1],
                &current_corners[last_corner_no as usize],
            ) == Side::Collinear;
        }

        if first_point_collinear {
            // skip first point
            first_corner_no += 1;
        }
        // search the point with the lowest y and then with the lowest x
        let mut start_corner_no = first_corner_no;
        let mut start_corner = current_corners[start_corner_no as usize].to_float();
        for i in (start_corner_no + 1)..=last_corner_no {
            let current_corner = current_corners[i as usize].to_float();
            if current_corner.y < start_corner.y
                || current_corner.y == start_corner.y && current_corner.x < start_corner.x
            {
                start_corner_no = i;
                start_corner = current_corner;
            }
        }
        let new_corner_count = last_corner_no - first_corner_no + 1;
        let mut result = Vec::with_capacity(new_corner_count.max(0) as usize);
        for i in start_corner_no..=last_corner_no {
            result.push(current_corners[i as usize].clone());
        }
        for i in first_corner_no..start_corner_no {
            result.push(current_corners[i as usize].clone());
        }
        PolygonShape { corners: result }
    }

    /// Creates a polygon shape from an array of corner points (PolygonShape.java:94-97).
    pub fn from_points(corners: &[Point]) -> PolygonShape {
        PolygonShape::from_polygon(&Polygon::new(corners.to_vec()))
    }

    /// The normalised corners of this shape (Java's `public final Point[] corners`).
    pub fn corners(&self) -> &[Point] {
        &self.corners
    }

    /// Returns the `no`-th corner (PolygonShape.java:99-106).
    ///
    /// # Panics
    /// For an out-of-range index. Java warns and returns `null`, which every caller then
    /// dereferences.
    pub fn corner(&self, no: usize) -> Point {
        assert!(
            no < self.corners.len(),
            "PolygonShape.corner: no out of range"
        );
        self.corners[no].clone()
    }

    /// Returns the number of border lines of the shape (PolygonShape.java:108-111).
    pub fn border_line_count(&self) -> usize {
        self.corners.len()
    }

    /// A polygon shape is bounded at every corner (PolygonShape.java:113-116).
    pub fn corner_is_bounded(&self, _no: usize) -> bool {
        true
    }

    /// Checks if this shape and `shape` have a nonempty intersection (PolygonShape.java:118-121;
    /// Java double-dispatches through `shape.intersects(this)`).
    pub fn intersects(&self, shape: &crate::shape::Shape) -> bool {
        shape.intersects_polygon(self)
    }

    /// Checks if this shape and `circle` have a nonempty intersection
    /// (PolygonShape.java:123-132).
    pub fn intersects_circle(&self, circle: &crate::circle::Circle) -> bool {
        self.convex_pieces()
            .iter()
            .any(|piece| circle.intersects_tile(piece))
    }

    /// Checks if this shape and `simplex` have a nonempty intersection
    /// (PolygonShape.java:134-143).
    pub fn intersects_simplex(&self, simplex: &crate::simplex::Simplex) -> bool {
        self.convex_pieces()
            .iter()
            .any(|piece| piece.intersects_simplex(simplex))
    }

    /// Checks if this shape and `oct` have a nonempty intersection (PolygonShape.java:145-154).
    pub fn intersects_octagon(&self, oct: &IntOctagon) -> bool {
        self.convex_pieces()
            .iter()
            .any(|piece| piece.intersects_octagon(oct))
    }

    /// Checks if this shape and `b` have a nonempty intersection (PolygonShape.java:156-165).
    pub fn intersects_box(&self, b: &IntBox) -> bool {
        self.convex_pieces()
            .iter()
            .any(|piece| piece.intersects_box(b))
    }

    /// `intersects(IntBox|IntOctagon|Simplex)` dispatched over the [`TileShape`] enum, for
    /// `TileShape.intersects(Shape)`.
    pub fn intersects_tile_shape(&self, tile: &TileShape) -> bool {
        match tile {
            TileShape::Box(b) => self.intersects_box(b),
            TileShape::Octagon(o) => self.intersects_octagon(o),
            TileShape::Simplex(s) => self.intersects_simplex(s),
        }
    }

    /// Java stub: warns "PolygonShape.cutout not yet implemented" and returns `null`
    /// (PolygonShape.java:167-171).
    pub fn cutout(&self, _polyline: &Polyline) -> Option<Vec<Polyline>> {
        None
    }

    /// Java stub: returns `this` for a zero offset, otherwise warns "PolygonShape.enlarge not yet
    /// implemented" and returns `null` (PolygonShape.java:173-180).
    pub fn enlarge(&self, offset: f64) -> Option<PolygonShape> {
        if offset == 0.0 {
            return Some(self.clone());
        }
        None
    }

    /// Java stub: warns "PolygonShape.border_distance not yet implemented" and returns 0
    /// (PolygonShape.java:182-186).
    pub fn border_distance(&self, _point: &FloatPoint) -> f64 {
        0.0
    }

    /// `borderDistance(centreOfGravity())`, hence always 0 while `borderDistance` is a stub
    /// (PolygonShape.java:188-191).
    pub fn smallest_radius(&self) -> f64 {
        self.border_distance(&PolylineShapeOps::centre_of_gravity(self))
    }

    /// Returns true if `point` is contained in this shape, but not inside a hole
    /// (PolygonShape.java:193-202).
    pub fn contains_float(&self, point: &FloatPoint) -> bool {
        self.convex_pieces()
            .iter()
            .any(|piece| piece.contains_float(point))
    }

    /// Returns true if `point` is inside or on the border of this shape
    /// (PolygonShape.java:204-207).
    pub fn contains(&self, point: &Point) -> bool {
        !self.is_outside(point)
    }

    /// Returns true if `point` is contained in this shape, but not on the border
    /// (PolygonShape.java:209-215).
    ///
    /// Because `containsOnBorder` is a Java stub that always answers `false`, this is exactly
    /// `contains`.
    pub fn contains_inside(&self, point: &Point) -> bool {
        if self.contains_on_border(point) {
            return false;
        }
        !self.is_outside(point)
    }

    /// Returns true if `point` is not contained in the inside or the boundary of the shape
    /// (PolygonShape.java:217-226).
    pub fn is_outside(&self, point: &Point) -> bool {
        !self
            .convex_pieces()
            .iter()
            .any(|piece| !piece.is_outside(point))
    }

    /// Java stub: the body is a commented-out warning followed by `return false`
    /// (PolygonShape.java:228-232). Ported as-is.
    pub fn contains_on_border(&self, _point: &Point) -> bool {
        // FRLogger.warn("PolygonShape.contains_on_edge not yet implemented");
        false
    }

    /// Java stub: warns "PolygonShape.distance not yet implemented" and returns 0
    /// (PolygonShape.java:234-238).
    pub fn distance(&self, _point: &FloatPoint) -> f64 {
        0.0
    }

    /// Returns the affine translation of the shape by `vector` (PolygonShape.java:240-250).
    pub fn translate_by(&self, vector: &Vector) -> PolygonShape {
        if *vector == Vector::ZERO {
            return self.clone();
        }
        let new_corners: Vec<Point> = self
            .corners
            .iter()
            .map(|corner| corner.translate_by(vector))
            .collect();
        PolygonShape::from_points(&new_corners)
    }

    /// Returns the bounding `RegularTileShape` with the fixed directions `dirs`
    /// (PolygonShape.java:252-255).
    pub fn bounding_shape(
        &self,
        dirs: crate::bounding_directions::ShapeBoundingDirections,
    ) -> crate::regular_tile_shape::RegularTileShape {
        dirs.bounds_polygon(self)
    }

    /// The smallest surrounding box of this shape (PolygonShape.java:257-276).
    pub fn bounding_box(&self) -> IntBox {
        let mut llx = i32::MAX as f64;
        let mut lly = i32::MAX as f64;
        let mut urx = i32::MIN as f64;
        let mut ury = i32::MIN as f64;
        for corner in &self.corners {
            let current = corner.to_float();
            llx = crate::limits::java_min(llx, current.x);
            lly = crate::limits::java_min(lly, current.y);
            urx = crate::limits::java_max(urx, current.x);
            ury = crate::limits::java_max(ury, current.y);
        }
        let lower_left = IntPoint::new(llx.floor() as i32, lly.floor() as i32);
        let upper_right = IntPoint::new(urx.ceil() as i32, ury.ceil() as i32);
        IntBox::new(lower_left, upper_right)
    }

    /// The smallest surrounding octagon of this shape (PolygonShape.java:278-316).
    pub fn bounding_octagon(&self) -> IntOctagon {
        let mut lx = i32::MAX as f64;
        let mut ly = i32::MAX as f64;
        let mut rx = i32::MIN as f64;
        let mut uy = i32::MIN as f64;
        let mut ulx = i32::MAX as f64;
        let mut lrx = i32::MIN as f64;
        let mut llx = i32::MAX as f64;
        let mut urx = i32::MIN as f64;
        for corner in &self.corners {
            let current = corner.to_float();
            lx = crate::limits::java_min(lx, current.x);
            ly = crate::limits::java_min(ly, current.y);
            rx = crate::limits::java_max(rx, current.x);
            uy = crate::limits::java_max(uy, current.y);

            let mut tmp = current.x - current.y;
            ulx = crate::limits::java_min(ulx, tmp);
            lrx = crate::limits::java_max(lrx, tmp);

            tmp = current.x + current.y;
            llx = crate::limits::java_min(llx, tmp);
            urx = crate::limits::java_max(urx, tmp);
        }
        IntOctagon::new(
            lx.floor() as i32,
            ly.floor() as i32,
            rx.ceil() as i32,
            uy.ceil() as i32,
            ulx.floor() as i32,
            lrx.ceil() as i32,
            llx.floor() as i32,
            urx.ceil() as i32,
        )
    }

    /// Checks whether every line segment between 2 points of the shape is contained completely in
    /// the shape (PolygonShape.java:318-363).
    pub fn is_convex(&self) -> bool {
        let corners = &self.corners;
        let len = corners.len();
        if len <= 2 {
            return true;
        }
        let mut prev_point = corners[len - 1].clone();
        let mut current_point = corners[0].clone();
        let mut next_point = corners[1].clone();

        for ind in 0..len {
            if next_point.side_of(&prev_point, &current_point) == Side::OnTheRight {
                return false;
            }
            prev_point = current_point;
            current_point = next_point;
            next_point = if ind == len - 2 {
                corners[0].clone()
            } else if ind == len - 1 {
                corners[1].clone()
            } else {
                corners[ind + 2].clone()
            };
        }
        // check, if the sum of the interior angles is at most 2 * pi

        let first_line = Line::new(int_point_of(&corners[len - 1]), int_point_of(&corners[0]));
        let mut current_line = Line::new(int_point_of(&corners[0]), int_point_of(&corners[1]));
        let first_direction = first_line.direction();
        let mut current_direction = current_line.direction();
        let mut last_det = first_direction.determinant(&current_direction);

        for corner in corners.iter().take(len).skip(2) {
            current_line = Line::new(current_line.b, int_point_of(corner));
            current_direction = current_line.direction();
            let current_det = first_direction.determinant(&current_direction);
            if last_det <= 0 && current_det > 0 {
                return false;
            }
            last_det = current_det;
        }

        true
    }

    /// Returns the convex hull of this polygon shape (PolygonShape.java:365-394).
    pub fn convex_hull(&self) -> PolygonShape {
        let corners = &self.corners;
        let len = corners.len();
        if len <= 2 {
            return self.clone();
        }
        let mut prev_point = corners[len - 1].clone();
        let mut current_point = corners[0].clone();
        for ind in 0..len {
            let next_point = if ind == len - 1 {
                corners[0].clone()
            } else {
                corners[ind + 1].clone()
            };
            if next_point.side_of(&prev_point, &current_point) != Side::OnTheLeft {
                // skip currentPoint;
                let mut new_corners = corners.clone();
                new_corners.remove(ind);
                let result = PolygonShape::from_points(&new_corners);
                return result.convex_hull();
            }
            prev_point = current_point;
            current_point = next_point;
        }
        self.clone()
    }

    /// Returns a bounding `TileShape` of this shape (PolygonShape.java:396-406).
    pub fn bounding_tile(&self) -> TileShape {
        let hull = self.convex_hull();
        let len = hull.corners.len();
        let mut bounding_lines: Vec<Line> = Vec::with_capacity(len);
        for i in 0..len - 1 {
            bounding_lines.push(Line::new(
                int_point_of(&hull.corners[i]),
                int_point_of(&hull.corners[i + 1]),
            ));
        }
        bounding_lines.push(Line::new(
            int_point_of(&hull.corners[len - 1]),
            int_point_of(&hull.corners[0]),
        ));
        TileShape::get_instance_from_lines(bounding_lines)
    }

    /// The content of the area of the shape (PolygonShape.java:408-428).
    ///
    // Java bug: the guard is `if (dimension() <= 2) return 0;`, but `PolygonShape.dimension()`
    // returns at most 2 (PolygonShape.java:430-442), so the guard is always true and the shoelace
    // sum below is dead code — `area()` always answers 0. Reproduced verbatim; the shoelace body
    // is kept so that a post-parity fix is a one-character change (`<= 2` → `< 2`).
    pub fn area(&self) -> f64 {
        if self.dimension() <= 2 {
            return 0.0;
        }
        // calculate half of the absolute value of
        // x0 (y1 - yn-1) + x1 (y2 - y0) + x2 (y3 - y1) + ...+ xn-1( y0 - yn-2)
        // where xi, yi are the coordinates of the i-th corner of this polygon.
        let corners = &self.corners;
        let mut result = 0.0;
        let mut prev_corner = corners[corners.len() - 2].to_float();
        let mut current_corner = corners[corners.len() - 1].to_float();
        for corner in corners {
            let next_corner = corner.to_float();
            result += current_corner.x * (next_corner.y - prev_corner.y);
            prev_corner = current_corner;
            current_corner = next_corner;
        }
        0.5 * result.abs()
    }

    /// 2 for a polygon, 1 for a segment, 0 for a point and -1 for the empty shape
    /// (PolygonShape.java:430-442).
    pub fn dimension(&self) -> i32 {
        match self.corners.len() {
            0 => -1,
            1 => 0,
            2 => 1,
            _ => 2,
        }
    }

    /// A polygon shape is always bounded (PolygonShape.java:444-447).
    pub fn is_bounded(&self) -> bool {
        true
    }

    /// Returns true if this shape has no corners (PolygonShape.java:449-452).
    pub fn is_empty(&self) -> bool {
        self.corners.is_empty()
    }

    /// Returns the `no`-th border line of this shape (PolygonShape.java:454-467). `None` where
    /// Java warns and returns `null`, i.e. for an out-of-range index.
    pub fn border_line(&self, no: usize) -> Option<Line> {
        if no >= self.corners.len() {
            // Java: FRLogger.warn("PolygonShape.borderLine: no out of range")
            return None;
        }
        let next_corner = if no == self.corners.len() - 1 {
            &self.corners[0]
        } else {
            &self.corners[no + 1]
        };
        Some(Line::new(
            int_point_of(&self.corners[no]),
            int_point_of(next_corner),
        ))
    }

    /// An approximation of the nearest point of the shape to `from_point`
    /// (PolygonShape.java:469-483). `None` where Java returns the `null` it starts from, i.e.
    /// when the shape has no convex pieces.
    ///
    /// # Panics
    /// When the convex split fails, or when a convex piece has no border lines — both are
    /// NullPointerExceptions in Java.
    pub fn nearest_point_approx(&self, from_point: &FloatPoint) -> Option<FloatPoint> {
        let mut min_dist = f64::MAX;
        let mut result = None;
        for piece in self.convex_pieces() {
            // Java reads `currentNearestPoint.distanceSquare(...)` without a null check
            // (PolygonShape.java:475-476), so a line-less convex piece is a NullPointerException.
            let current_nearest_point = piece.nearest_point_approx(from_point).expect(
                "TileShape.nearestPointApprox returned null for a convex piece with no border lines",
            );
            let current_distance = current_nearest_point.distance_square(from_point);
            if current_distance < min_dist {
                min_dist = current_distance;
                result = Some(current_nearest_point);
            }
        }
        result
    }

    /// Turns this shape by `factor` times 90 degree around `pole` (PolygonShape.java:485-492).
    pub fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> PolygonShape {
        let pole = Point::Int(*pole);
        let new_corners: Vec<Point> = self
            .corners
            .iter()
            .map(|corner| corner.turn_90_degree(factor, &pole))
            .collect();
        PolygonShape::from_points(&new_corners)
    }

    /// Rotates this shape around `pole` by `angle` (PolygonShape.java:494-504).
    pub fn rotate_approx(&self, angle: f64, pole: &FloatPoint) -> PolygonShape {
        if angle == 0.0 {
            return self.clone();
        }
        let new_corners: Vec<Point> = self
            .corners
            .iter()
            .map(|corner| Point::Int(corner.to_float().rotate(angle, pole).round()))
            .collect();
        PolygonShape::from_points(&new_corners)
    }

    /// Mirrors this shape at the vertical line through `pole` (PolygonShape.java:506-513).
    pub fn mirror_vertical(&self, pole: &IntPoint) -> PolygonShape {
        let pole = Point::Int(*pole);
        let new_corners: Vec<Point> = self
            .corners
            .iter()
            .map(|corner| corner.mirror_vertical(&pole))
            .collect();
        PolygonShape::from_points(&new_corners)
    }

    /// Mirrors this shape at the horizontal line through `pole` (PolygonShape.java:515-522).
    pub fn mirror_horizontal(&self, pole: &IntPoint) -> PolygonShape {
        let pole = Point::Int(*pole);
        let new_corners: Vec<Point> = self
            .corners
            .iter()
            .map(|corner| corner.mirror_horizontal(&pole))
            .collect();
        PolygonShape::from_points(&new_corners)
    }

    /// Splits this polygon shape into convex pieces (PolygonShape.java:524-548).
    ///
    /// "The result is not exact, because rounded intersections of lines are used in the result
    /// pieces. It can be made exact, if Polylines are returned instead of Polygons, so that no
    /// intersection points are needed in the result."
    ///
    /// `None` where Java returns `null`: the split failed, maybe the polygon has
    /// self-intersections.
    pub fn split_to_convex(&self) -> Option<Vec<TileShape>> {
        // use a fixed seed to get reproducible result
        let mut random_generator = JavaRandom::new(SEED);
        let convex_pieces = self.split_to_convex_recu(&mut random_generator)?;
        Some(
            convex_pieces
                .iter()
                .map(|current_piece| {
                    let pts: Vec<IntPoint> =
                        current_piece.corners.iter().map(int_point_of).collect();
                    TileShape::get_instance_from_points(&pts)
                })
                .collect(),
        )
    }

    /// The convex pieces of this shape, for the methods that dereference `splitToConvex()`
    /// without a null check.
    ///
    /// # Panics
    /// When the split fails, exactly where Java raises a `NullPointerException` on
    /// `convexPieces.length` (PolygonShape.java:126, 137, 148, 159, 196, 220, 474).
    fn convex_pieces(&self) -> Vec<TileShape> {
        self.split_to_convex()
            .expect("PolygonShape.splitToConvex failed: the polygon may have selfintersections")
    }

    /// Private recursive part of `split_to_convex`. Returns a collection of polygon shape pieces
    /// (PolygonShape.java:550-633).
    fn split_to_convex_recu(&self, random_generator: &mut JavaRandom) -> Option<Vec<PolygonShape>> {
        let corners = &self.corners;
        let len = corners.len();
        // start with a hashed corner and search the first concave corner
        let mut start_corner_no = random_generator.next_int(len as i32) as usize;
        let mut current_corner = corners[start_corner_no].clone();
        let mut prev_corner = if start_corner_no != 0 {
            corners[start_corner_no - 1].clone()
        } else {
            corners[len - 1].clone()
        };

        // search for the next concave corner from here
        let mut concave_corner_no: Option<usize> = None;
        for _ in 0..len {
            let next_corner = if start_corner_no < len - 1 {
                corners[start_corner_no + 1].clone()
            } else {
                corners[0].clone()
            };
            if next_corner.side_of(&prev_corner, &current_corner) == Side::OnTheRight {
                // concave corner found
                concave_corner_no = Some(start_corner_no);
                break;
            }
            prev_corner = current_corner;
            current_corner = next_corner;
            start_corner_no = (start_corner_no + 1) % len;
        }
        let mut result: Vec<PolygonShape> = Vec::new();
        let Some(concave_corner_no) = concave_corner_no else {
            // no concave corner found, this shape is already convex
            result.push(self.clone());
            return Some(result);
        };
        let d = DivisionPoint::new(corners, concave_corner_no);
        // projection not found, maybe polygon has selfintersections
        let projection = d.projection?;

        // construct the result pieces from polygon and the division point
        let mut corner_count = d.corner_no_after_projection as isize - concave_corner_no as isize;

        if corner_count < 0 {
            corner_count += len as isize;
        }
        corner_count += 1;
        let mut first_arr: Vec<Point> = Vec::with_capacity(corner_count as usize);
        let mut corner_ind = concave_corner_no;

        for _ in 0..corner_count - 1 {
            first_arr.push(corners[corner_ind].clone());
            corner_ind = (corner_ind + 1) % len;
        }
        first_arr.push(Point::Int(projection.round()));
        let mut corner_count = concave_corner_no as isize - d.corner_no_after_projection as isize;
        if corner_count < 0 {
            corner_count += len as isize;
        }
        corner_count += 2;
        let mut last_arr: Vec<Point> = Vec::with_capacity(corner_count as usize);
        last_arr.push(Point::Int(projection.round()));
        corner_ind = d.corner_no_after_projection;
        for _ in 1..corner_count {
            last_arr.push(corners[corner_ind].clone());
            corner_ind = (corner_ind + 1) % len;
        }
        let last_piece = PolygonShape::from_points(&last_arr);
        let first_piece = PolygonShape::from_points(&first_arr);
        let c1 = first_piece.split_to_convex_recu(random_generator)?;
        let c2 = last_piece.split_to_convex_recu(random_generator)?;
        result.extend(c1);
        result.extend(c2);
        Some(result)
    }
}

/// At a concave corner of the closed polygon, a minimal axis parallel division line is
/// constructed, to divide the closed polygon into two (PolygonShape.java:635-776).
struct DivisionPoint {
    corner_no_after_projection: usize,
    projection: Option<FloatPoint>,
}

impl DivisionPoint {
    fn new(corners: &[Point], concave_corner_no: usize) -> DivisionPoint {
        let len = corners.len();
        let concave_corner = corners[concave_corner_no].to_float();
        let before_concave_corner = if concave_corner_no != 0 {
            corners[concave_corner_no - 1].to_float()
        } else {
            corners[len - 1].to_float()
        };

        let after_concave_corner = if concave_corner_no == len - 1 {
            corners[0].to_float()
        } else {
            corners[concave_corner_no + 1].to_float()
        };

        let search_right =
            before_concave_corner.y > concave_corner.y || concave_corner.y > after_concave_corner.y;

        let search_left =
            before_concave_corner.y < concave_corner.y || concave_corner.y < after_concave_corner.y;

        let search_up =
            before_concave_corner.x < concave_corner.x || concave_corner.x < after_concave_corner.x;

        let search_down =
            before_concave_corner.x > concave_corner.x || concave_corner.x > after_concave_corner.x;

        let mut min_projection_dist = i32::MAX as f64;
        let mut min_projection: Option<FloatPoint> = None;
        let mut corner_no_after_min_projection = 0usize;

        let mut corner_no_after_curr_projection = (concave_corner_no + 2) % len;

        let mut corner_before_curr_projection = if corner_no_after_curr_projection != 0 {
            corners[corner_no_after_curr_projection - 1].clone()
        } else {
            corners[len - 1].clone()
        };
        let mut corner_before_projection_approx = corner_before_curr_projection.to_float();

        // Java: `int loopEnd = corners.length - 2;` — negative for a 1-corner polygon, where the
        // loop simply does not run.
        let loop_end = len.saturating_sub(2);

        for _ in 0..loop_end {
            let corner_after_curr_projection = corners[corner_no_after_curr_projection].clone();
            let corner_after_projection_approx = corner_after_curr_projection.to_float();
            if corner_before_projection_approx.y != corner_after_projection_approx.y {
                // try a horizontal division
                let (min_y, max_y) =
                    if corner_after_projection_approx.y > corner_before_projection_approx.y {
                        (
                            corner_before_projection_approx.y,
                            corner_after_projection_approx.y,
                        )
                    } else {
                        (
                            corner_after_projection_approx.y,
                            corner_before_projection_approx.y,
                        )
                    };

                if concave_corner.y >= min_y && concave_corner.y <= max_y {
                    let current_line = Line::new(
                        int_point_of(&corner_before_curr_projection),
                        int_point_of(&corner_after_curr_projection),
                    );
                    let xintersection = current_line.function_in_y_value_approx(concave_corner.y);
                    let current_distance = (xintersection - concave_corner.x).abs();
                    // Make sure, that the new shape will not be concave at the projection point.
                    // That might happen, if the boundary curve runs back in itself.
                    let projection_ok = current_distance < min_projection_dist
                        && (search_right
                            && xintersection > concave_corner.x
                            && concave_corner.y <= corner_after_projection_approx.y
                            || search_left
                                && xintersection < concave_corner.x
                                && concave_corner.y >= corner_after_projection_approx.y);
                    if projection_ok {
                        min_projection_dist = current_distance;
                        corner_no_after_min_projection = corner_no_after_curr_projection;
                        min_projection = Some(FloatPoint::new(xintersection, concave_corner.y));
                    }
                }
            }

            if corner_before_projection_approx.x != corner_after_projection_approx.x {
                // try a vertical division
                let (min_x, max_x) =
                    if corner_after_projection_approx.x > corner_before_projection_approx.x {
                        (
                            corner_before_projection_approx.x,
                            corner_after_projection_approx.x,
                        )
                    } else {
                        (
                            corner_after_projection_approx.x,
                            corner_before_projection_approx.x,
                        )
                    };
                if concave_corner.x >= min_x && concave_corner.x <= max_x {
                    let current_line = Line::new(
                        int_point_of(&corner_before_curr_projection),
                        int_point_of(&corner_after_curr_projection),
                    );
                    let yintersection = current_line.function_value_approx(concave_corner.x);
                    let current_distance = (yintersection - concave_corner.y).abs();
                    // make sure, that the new shape will be convex at the projection point
                    let projection_ok = current_distance < min_projection_dist
                        && (search_up
                            && yintersection > concave_corner.y
                            && concave_corner.x >= corner_after_projection_approx.x
                            || search_down
                                && yintersection < concave_corner.y
                                && concave_corner.x <= corner_after_projection_approx.x);

                    if projection_ok {
                        min_projection_dist = current_distance;
                        corner_no_after_min_projection = corner_no_after_curr_projection;
                        min_projection = Some(FloatPoint::new(concave_corner.x, yintersection));
                    }
                }
            }
            corner_before_curr_projection = corner_after_curr_projection;
            corner_before_projection_approx = corner_after_projection_approx;
            if corner_no_after_curr_projection == len - 1 {
                corner_no_after_curr_projection = 0;
            } else {
                corner_no_after_curr_projection += 1;
            }
        }
        // Java: FRLogger.warn("PolygonShape.DivisionPoint: projection not found") when
        // `minProjectionDist == Integer.MAX_VALUE`; a pure diagnostic, dropped here.

        DivisionPoint {
            corner_no_after_projection: corner_no_after_min_projection,
            projection: min_projection,
        }
    }
}

impl PolylineShapeOps for PolygonShape {
    fn corner_is_bounded(&self, no: usize) -> bool {
        PolygonShape::corner_is_bounded(self, no)
    }
    fn border_line_count(&self) -> usize {
        PolygonShape::border_line_count(self)
    }
    fn corner(&self, no: usize) -> Point {
        PolygonShape::corner(self, no)
    }
    fn border_line(&self, no: usize) -> Option<Line> {
        PolygonShape::border_line(self, no)
    }
    fn is_bounded(&self) -> bool {
        PolygonShape::is_bounded(self)
    }
    fn is_empty(&self) -> bool {
        PolygonShape::is_empty(self)
    }
    fn dimension(&self) -> i32 {
        PolygonShape::dimension(self)
    }
    fn bounding_box(&self) -> IntBox {
        PolygonShape::bounding_box(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::float_point::FloatPoint;
    use crate::int_box::IntBox;
    use crate::int_point::IntPoint;
    use crate::point::Point;
    use crate::polyline_shape::PolylineShapeOps;

    fn pts(v: &[(i32, i32)]) -> Vec<Point> {
        v.iter()
            .map(|&(x, y)| Point::Int(IntPoint::new(x, y)))
            .collect()
    }
    fn square() -> PolygonShape {
        PolygonShape::from_points(&pts(&[(0, 0), (10, 0), (10, 10), (0, 10)]))
    }
    fn l_shape() -> PolygonShape {
        PolygonShape::from_points(&pts(&[
            (0, 0),
            (20, 0),
            (20, 10),
            (10, 10),
            (10, 20),
            (0, 20),
        ]))
    }

    #[test]
    fn convexity_area_and_split() {
        assert!(square().is_convex());
        // Java bug (PolygonShape.java:411): `dimension() <= 2` is always true, so `area()` is
        // always 0 — the shoelace sum below it is unreachable. Verified against the Java class.
        assert_eq!(square().area(), 0.0);
        assert_eq!(square().split_to_convex().unwrap().len(), 1);
        assert!(!l_shape().is_convex());
        assert_eq!(l_shape().area(), 0.0);
        let parts = l_shape().split_to_convex().unwrap();
        assert_eq!(parts.len(), 2);
        // The convex pieces are ordinary TileShapes, so their `area()` is the real one.
        assert!((parts.iter().map(|t| t.area()).sum::<f64>() - 300.0).abs() < 1e-9);
        assert_eq!(l_shape().convex_hull().area(), 0.0);
        assert_eq!(
            l_shape().convex_hull().corners().to_vec(),
            pts(&[(0, 0), (20, 0), (20, 10), (10, 20), (0, 20)])
        );
        assert_eq!(l_shape().bounding_box(), IntBox::from_coords(0, 0, 20, 20));
    }

    #[test]
    fn orientation_is_normalised_to_counterclockwise() {
        let cw = PolygonShape::from_points(&pts(&[(0, 0), (0, 10), (10, 10), (10, 0)]));
        // Java's constructor reverses clockwise input; both must then be equal corner sequences.
        let a = square().corner_approx_arr();
        let b = cw.corner_approx_arr();
        assert_eq!(a.len(), b.len());
        assert!(a.iter().all(|p| b.contains(p)));
        assert_eq!(cw.corners().to_vec(), square().corners().to_vec());
        assert!(cw.is_convex());
    }

    #[test]
    fn containment() {
        assert!(l_shape().contains(&Point::Int(IntPoint::new(5, 15))));
        assert!(!l_shape().contains(&Point::Int(IntPoint::new(15, 15))));
        assert!(l_shape().contains_float(&FloatPoint::new(15.0, 5.0)));
        assert!(l_shape().is_outside(&Point::Int(IntPoint::new(25, 5))));
        // Pinned against Java: the two convex pieces of the L touch the box only in the corner
        // (10, 20) resp. (20, 10), and `IntBox.intersects` is exclusive there.
        assert!(!l_shape().intersects_box(&IntBox::from_coords(15, 15, 30, 30)));
        assert!(square().intersects_box(&IntBox::from_coords(5, 5, 30, 30)));
    }

    #[test]
    fn polyline_shape_ops() {
        let s = square();
        assert_eq!(s.border_line_count(), 4);
        assert_eq!(PolylineShapeOps::circumference(&s), 40.0);
        assert_eq!(
            PolylineShapeOps::centre_of_gravity(&s),
            FloatPoint::new(5.0, 5.0)
        );
        assert_eq!(s.equals_corner(&Point::Int(IntPoint::new(10, 10))), Some(2));
        assert_eq!(s.next_no(3), 0);
        assert_eq!(s.prev_no(0), 3);
    }

    #[test]
    fn java_random_reproduces_the_jdk_sequences() {
        // `new Random(99).nextInt(bound)` for the first 12 draws, taken from a JDK 23 run.
        let expected: &[(i32, &[i32])] = &[
            (2, &[1, 0, 0, 1, 1, 0, 1, 1, 1, 0, 0, 1]),
            (3, &[1, 2, 0, 0, 0, 2, 1, 2, 2, 2, 1, 0]),
            (4, &[2, 1, 1, 2, 3, 1, 3, 2, 3, 0, 1, 2]),
            (5, &[2, 3, 4, 1, 0, 2, 4, 1, 0, 4, 3, 4]),
            (6, &[1, 2, 3, 3, 0, 2, 4, 2, 2, 5, 4, 3]),
            (7, &[4, 6, 4, 0, 1, 1, 5, 1, 0, 6, 0, 3]),
            (8, &[5, 3, 2, 5, 6, 3, 6, 4, 6, 0, 2, 5]),
            (16, &[11, 6, 5, 11, 13, 6, 13, 9, 12, 0, 4, 11]),
            (100, &[87, 58, 29, 11, 0, 62, 74, 6, 20, 99, 68, 39]),
        ];
        for (bound, seq) in expected {
            let mut rng = JavaRandom::new(SEED);
            let got: Vec<i32> = (0..seq.len()).map(|_| rng.next_int(*bound)).collect();
            assert_eq!(&got, seq, "bound {bound}");
        }
    }

    #[test]
    fn stubs_match_the_java_stubs() {
        let s = square();
        assert_eq!(s.border_distance(&FloatPoint::new(100.0, 100.0)), 0.0);
        assert_eq!(s.smallest_radius(), 0.0);
        assert_eq!(s.distance(&FloatPoint::new(100.0, 100.0)), 0.0);
        assert!(!s.contains_on_border(&Point::Int(IntPoint::new(0, 0))));
        assert_eq!(s.enlarge(0.0), Some(s.clone()));
        assert_eq!(s.enlarge(1.0), None);
    }

    #[test]
    fn transformations_round_trip() {
        let s = square();
        assert_eq!(
            s.turn_90_degree(4, &IntPoint::new(0, 0)).corners().to_vec(),
            s.corners().to_vec()
        );
        assert_eq!(
            s.mirror_vertical(&IntPoint::new(0, 0))
                .mirror_vertical(&IntPoint::new(0, 0))
                .corners()
                .to_vec(),
            s.corners().to_vec()
        );
        assert_eq!(
            s.mirror_horizontal(&IntPoint::new(0, 0))
                .mirror_horizontal(&IntPoint::new(0, 0))
                .corners()
                .to_vec(),
            s.corners().to_vec()
        );
        assert_eq!(s.rotate_approx(0.0, &FloatPoint::new(0.0, 0.0)), s);
        assert_eq!(s.translate_by(&Vector::ZERO), s);
        assert_eq!(
            s.translate_by(&Vector::Int(crate::int_vector::IntVector::new(5, 5)))
                .bounding_box(),
            IntBox::from_coords(5, 5, 15, 15)
        );
    }

    #[test]
    fn border_lines_and_bounding_shapes() {
        let s = square();
        assert_eq!(s.border_line(0), Some(Line::from_coords(0, 0, 10, 0)));
        assert_eq!(s.border_line(3), Some(Line::from_coords(0, 10, 0, 0)));
        assert_eq!(s.border_line(4), None);
        assert_eq!(
            s.bounding_octagon(),
            IntOctagon::new(0, 0, 10, 10, -10, 10, 0, 20)
        );
        assert!(s.bounding_tile().contains(&Point::Int(IntPoint::new(5, 5))));
        assert_eq!(
            s.nearest_point_approx(&FloatPoint::new(20.0, 5.0)),
            Some(FloatPoint::new(10.0, 5.0))
        );
        assert_eq!(s.dimension(), 2);
        assert!(s.is_bounded());
        assert!(!s.is_empty());
        assert_eq!(
            s.cutout(&crate::polyline::Polyline::from_two_points(
                &Point::Int(IntPoint::new(-5, 5)),
                &Point::Int(IntPoint::new(15, 5))
            )),
            None
        );
    }
}
