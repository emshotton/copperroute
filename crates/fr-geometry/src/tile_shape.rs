use crate::direction::Direction;
use crate::float_line::FloatLine;
use crate::float_point::FloatPoint;
use crate::int_box::IntBox;
use crate::int_direction::IntDirection;
use crate::int_octagon::IntOctagon;
use crate::int_point::IntPoint;
use crate::limits::{JAVA_DOUBLE_MIN_VALUE, java_min};
use crate::line::Line;
use crate::line_segment::LineSegment;
use crate::point::Point;
use crate::polygon::Polygon;
use crate::polyline::{Polyline, PolylineError};
use crate::side::Side;
use crate::simplex::Simplex;
use crate::vector::Vector;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TileShape {
    Box(IntBox),
    Octagon(IntOctagon),
    Simplex(Simplex),
}

impl From<IntBox> for TileShape {
    fn from(value: IntBox) -> Self {
        TileShape::Box(value)
    }
}

impl From<IntOctagon> for TileShape {
    fn from(value: IntOctagon) -> Self {
        TileShape::Octagon(value)
    }
}

impl From<Simplex> for TileShape {
    fn from(value: Simplex) -> Self {
        TileShape::Simplex(value)
    }
}

impl TileShape {
    pub fn get_instance_from_lines(lines: Vec<Line>) -> TileShape {
        Simplex::from_lines(lines).simplify()
    }

    pub fn get_instance_from_points(convex_polygon: &[IntPoint]) -> TileShape {
        Simplex::from_points(convex_polygon).simplify()
    }

    pub fn get_instance_from_line(line: Line) -> TileShape {
        TileShape::Simplex(Simplex::from_lines(vec![line]))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get_instance_octagon(
        lx: i32,
        ly: i32,
        rx: i32,
        uy: i32,
        ulx: i32,
        lrx: i32,
        llx: i32,
        urx: i32,
    ) -> IntOctagon {
        IntOctagon::new(lx, ly, rx, uy, ulx, lrx, llx, urx).normalize()
    }

    pub fn get_instance_from_box(
        lower_left_x: i32,
        lower_left_y: i32,
        upper_right_x: i32,
        upper_right_y: i32,
    ) -> IntOctagon {
        IntBox::from_coords(lower_left_x, lower_left_y, upper_right_x, upper_right_y)
            .to_int_octagon()
    }

    pub fn get_instance_from_point(point: &Point) -> IntBox {
        point.surrounding_box()
    }
}

impl TileShape {
    pub fn border_line_count(&self) -> usize {
        match self {
            TileShape::Box(b) => b.border_line_count(),
            TileShape::Octagon(o) => o.border_line_count(),
            TileShape::Simplex(s) => s.border_line_count(),
        }
    }

    pub fn border_line(&self, no: usize) -> Option<Line> {
        match self {
            TileShape::Box(b) => Some(b.border_line(no)),
            TileShape::Octagon(o) => Some(o.border_line(no)),
            TileShape::Simplex(s) => s.border_line(no),
        }
    }

    pub fn corner(&self, no: usize) -> Point {
        match self {
            TileShape::Box(b) => Point::Int(b.corner(no)),
            TileShape::Octagon(o) => Point::Int(o.corner(no)),
            TileShape::Simplex(s) => s.corner(no),
        }
    }

    pub fn corner_approx(&self, no: usize) -> Option<FloatPoint> {
        match self {
            TileShape::Box(b) => Some(b.corner(no).to_float()),
            TileShape::Octagon(o) => Some(o.corner(no).to_float()),
            TileShape::Simplex(s) => s.corner_approx(no),
        }
    }

    pub fn corner_approx_arr(&self) -> Vec<FloatPoint> {
        match self {
            TileShape::Simplex(s) => s.corner_approx_arr(),
            _ => (0..self.border_line_count())
                .map(|i| self.corner_approx_at(i))
                .collect(),
        }
    }

    pub fn corner_is_bounded(&self, no: usize) -> bool {
        match self {
            TileShape::Box(b) => b.corner_is_bounded(no),
            TileShape::Octagon(o) => o.corner_is_bounded(no),
            TileShape::Simplex(s) => s.corner_is_bounded(no),
        }
    }

    pub fn is_bounded(&self) -> bool {
        match self {
            TileShape::Box(b) => b.is_bounded(),
            TileShape::Octagon(o) => o.is_bounded(),
            TileShape::Simplex(s) => s.is_bounded(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            TileShape::Box(b) => b.is_empty(),
            TileShape::Octagon(o) => o.is_empty(),
            TileShape::Simplex(s) => s.is_empty(),
        }
    }

    pub fn dimension(&self) -> i32 {
        match self {
            TileShape::Box(b) => b.dimension(),
            TileShape::Octagon(o) => o.dimension(),
            TileShape::Simplex(s) => s.dimension(),
        }
    }

    pub fn simplify(&self) -> TileShape {
        match self {
            TileShape::Box(b) => b.simplify(),
            TileShape::Octagon(o) => o.simplify(),
            TileShape::Simplex(s) => s.simplify(),
        }
    }

    pub fn get_id(&self) -> i32 {
        match self {
            TileShape::Box(b) => b.get_id(),
            TileShape::Octagon(o) => o.get_id(),
            TileShape::Simplex(s) => s.get_id(),
        }
    }

    pub fn is_int_box(&self) -> bool {
        match self {
            TileShape::Box(b) => b.is_int_box(),
            TileShape::Octagon(o) => o.is_int_box(),
            TileShape::Simplex(s) => s.is_int_box(),
        }
    }

    pub fn is_int_octagon(&self) -> bool {
        match self {
            TileShape::Box(b) => b.is_int_octagon(),
            TileShape::Octagon(o) => o.is_int_octagon(),
            TileShape::Simplex(s) => s.is_int_octagon(),
        }
    }

    pub fn to_simplex(&self) -> Simplex {
        match self {
            TileShape::Box(b) => b.to_simplex(),
            TileShape::Octagon(o) => o.to_simplex(),
            TileShape::Simplex(s) => s.to_simplex(),
        }
    }

    pub fn border_line_index(&self, line: &Line) -> Option<usize> {
        match self {
            TileShape::Box(b) => b.border_line_index(line),
            TileShape::Octagon(o) => o.border_line_index(line),
            TileShape::Simplex(s) => s.border_line_index(line),
        }
    }

    pub fn bounding_box(&self) -> IntBox {
        match self {
            TileShape::Box(b) => b.bounding_box(),
            TileShape::Octagon(o) => o.bounding_box(),
            TileShape::Simplex(s) => s.bounding_box(),
        }
    }

    pub fn bounding_octagon(&self) -> Option<IntOctagon> {
        match self {
            TileShape::Box(b) => Some(b.bounding_octagon()),
            TileShape::Octagon(o) => Some(o.bounding_octagon()),
            TileShape::Simplex(s) => s.bounding_octagon(),
        }
    }

    pub fn bounding_tile(&self) -> TileShape {
        self.clone()
    }

    pub fn translate_by(&self, vector: &Vector) -> TileShape {
        match self {
            TileShape::Box(b) => TileShape::Box(b.translate_by(vector)),
            TileShape::Octagon(o) => TileShape::Octagon(o.translate_by(vector)),
            TileShape::Simplex(s) => TileShape::Simplex(s.translate_by(vector)),
        }
    }

    pub fn offset(&self, dist: f64) -> TileShape {
        match self {
            TileShape::Box(b) => TileShape::Box(b.offset(dist)),
            TileShape::Octagon(o) => TileShape::Octagon(o.offset(dist)),
            TileShape::Simplex(s) => TileShape::Simplex(s.offset(dist)),
        }
    }

    pub fn enlarge(&self, offset: f64) -> TileShape {
        match self {
            TileShape::Box(b) => TileShape::Octagon(b.enlarge(offset)),
            TileShape::Octagon(o) => TileShape::Octagon(o.enlarge(offset)),
            TileShape::Simplex(s) => TileShape::Simplex(s.enlarge(offset)),
        }
    }

    pub fn max_width(&self) -> f64 {
        match self {
            TileShape::Box(b) => b.max_width(),
            TileShape::Octagon(o) => o.max_width(),
            TileShape::Simplex(s) => s.max_width(),
        }
    }

    pub fn min_width(&self) -> f64 {
        match self {
            TileShape::Box(b) => b.min_width(),
            TileShape::Octagon(o) => o.min_width(),
            TileShape::Simplex(s) => s.min_width(),
        }
    }

    pub fn circumference(&self) -> f64 {
        match self {
            TileShape::Box(b) => b.circumference(),
            _ => {
                if !self.is_bounded() {
                    return i32::MAX as f64;
                }
                let corner_count = self.border_line_count();
                if corner_count == 0 {
                    return 0.0;
                }
                let mut result = 0.0;
                let mut prev_corner = self.corner_approx_at(corner_count - 1);
                for i in 0..corner_count {
                    let current_corner = self.corner_approx_at(i);
                    result += current_corner.distance(&prev_corner);
                    prev_corner = current_corner;
                }
                result
            }
        }
    }

    pub fn centre_of_gravity(&self) -> FloatPoint {
        let corner_count = self.border_line_count();
        let mut x = 0.0;
        let mut y = 0.0;
        for i in 0..corner_count {
            let current_point = self.corner_approx_at(i);
            x += current_point.x;
            y += current_point.y;
        }
        x /= corner_count as f64;
        y /= corner_count as f64;
        FloatPoint::new(x, y)
    }

    fn border_line_at(&self, no: usize) -> Line {
        self.border_line(no)
            .expect("border line index is below border_line_count()")
    }

    fn corner_approx_at(&self, no: usize) -> FloatPoint {
        self.corner_approx(no)
            .expect("corner index is below border_line_count()")
    }
}

impl TileShape {
    pub fn intersection_with_simplify(&self, other: &TileShape) -> TileShape {
        self.intersection(other).simplify()
    }

    pub fn intersection(&self, other: &TileShape) -> TileShape {
        match (self, other) {
            (TileShape::Box(a), TileShape::Box(b)) => TileShape::Box(b.intersection(a)),
            (TileShape::Box(a), TileShape::Octagon(b)) => TileShape::Octagon(b.intersection_box(a)),
            (TileShape::Box(a), TileShape::Simplex(b)) => TileShape::Simplex(b.intersection_box(a)),
            (TileShape::Octagon(a), TileShape::Box(b)) => {
                TileShape::Octagon(b.intersection_octagon(a))
            }
            (TileShape::Octagon(a), TileShape::Octagon(b)) => TileShape::Octagon(b.intersection(a)),
            (TileShape::Octagon(a), TileShape::Simplex(b)) => {
                TileShape::Simplex(b.intersection_octagon(a))
            }
            (TileShape::Simplex(a), TileShape::Box(b)) => TileShape::Simplex(a.intersection_box(b)),
            (TileShape::Simplex(a), TileShape::Octagon(b)) => {
                TileShape::Simplex(a.intersection_octagon(b))
            }
            (TileShape::Simplex(a), TileShape::Simplex(b)) => TileShape::Simplex(b.intersection(a)),
        }
    }

    pub fn intersects(&self, other: &TileShape) -> bool {
        match self {
            TileShape::Box(b) => other.intersects_box(b),
            TileShape::Octagon(o) => other.intersects_octagon(o),
            TileShape::Simplex(s) => other.intersects_simplex(s),
        }
    }

    pub fn intersects_box(&self, other: &IntBox) -> bool {
        match self {
            TileShape::Box(b) => b.intersects(other),
            TileShape::Octagon(o) => o.intersects_box(other),
            TileShape::Simplex(s) => s.intersects_box(other),
        }
    }

    pub fn intersects_octagon(&self, other: &IntOctagon) -> bool {
        match self {
            TileShape::Box(b) => b.intersects_octagon(other),
            TileShape::Octagon(o) => o.intersects_octagon(other),
            TileShape::Simplex(s) => s.intersects_octagon(other),
        }
    }

    pub fn intersects_simplex(&self, other: &Simplex) -> bool {
        match self {
            TileShape::Box(b) => b.intersects_simplex(other),
            TileShape::Octagon(o) => o.intersects_simplex(other),
            TileShape::Simplex(s) => s.intersects(other),
        }
    }

    pub fn area(&self) -> f64 {
        match self {
            TileShape::Box(b) => b.area(),
            TileShape::Octagon(o) => o.area(),
            TileShape::Simplex(_) => {
                if !self.is_bounded() {
                    return f64::MAX;
                }
                if self.dimension() < 2 {
                    return 0.0;
                }
                let corner_count = self.border_line_count();
                debug_assert!(corner_count >= 3);
                let mut result = 0.0;
                let mut prev_corner = self.corner_approx_at(corner_count - 2);
                let mut current_corner = self.corner_approx_at(corner_count - 1);
                for i in 0..corner_count {
                    let next_corner = self.corner_approx_at(i);
                    result += current_corner.x * (next_corner.y - prev_corner.y);
                    prev_corner = current_corner;
                    current_corner = next_corner;
                }
                0.5 * result.abs()
            }
        }
    }

    pub fn is_outside(&self, point: &Point) -> bool {
        let line_count = self.border_line_count();
        if line_count == 0 {
            return true;
        }
        for i in 0..line_count {
            if self.border_line_at(i).side_of(point) == Side::OnTheLeft {
                return true;
            }
        }
        false
    }

    pub fn contains(&self, point: &Point) -> bool {
        !self.is_outside(point)
    }

    pub fn contains_float(&self, point: &FloatPoint) -> bool {
        match self {
            TileShape::Octagon(o) => o.contains_float(point),
            _ => self.contains_float_tol(point, 0.0),
        }
    }

    pub fn contains_float_tol(&self, point: &FloatPoint, tolerance: f64) -> bool {
        let line_count = self.border_line_count();
        if line_count == 0 {
            return false;
        }
        for i in 0..line_count {
            if self.border_line_at(i).side_of_float(point, tolerance) != Side::OnTheRight {
                return false;
            }
        }
        true
    }

    pub fn contains_tile(&self, other: &TileShape) -> bool {
        for i in 0..other.border_line_count() {
            if !self.contains(&other.corner(i)) {
                return false;
            }
        }
        true
    }

    pub fn contains_inside(&self, point: &Point) -> bool {
        let line_count = self.border_line_count();
        if line_count == 0 {
            return false;
        }
        for i in 0..line_count {
            if self.border_line_at(i).side_of(point) != Side::OnTheRight {
                return false;
            }
        }
        true
    }

    pub fn side_of_border(&self, point: &FloatPoint, tolerance: f64) -> Side {
        let line_count = self.border_line_count();
        if line_count == 0 {
            return Side::Collinear;
        }
        let mut result = Side::OnTheRight;
        for i in 0..line_count {
            let current_side = self.border_line_at(i).side_of_float(point, tolerance);
            if current_side == Side::OnTheLeft {
                return Side::OnTheLeft;
            } else if current_side == Side::Collinear {
                result = current_side;
            }
        }
        result
    }

    pub fn contains_on_border_line_no(&self, point: &Point) -> Option<usize> {
        let line_count = self.border_line_count();
        if line_count == 0 {
            return None;
        }
        let mut containing_line_no = None;
        for i in 0..line_count {
            let side_of = self.border_line_at(i).side_of(point);
            if side_of == Side::OnTheLeft {
                return None;
            }
            if side_of == Side::Collinear {
                containing_line_no = Some(i);
            }
        }
        containing_line_no
    }

    pub fn contains_on_border(&self, point: &Point) -> bool {
        self.contains_on_border_line_no(point).is_some()
    }

    pub fn contains_approx(&self, other: &TileShape) -> bool {
        for current_corner in other.corner_approx_arr() {
            if !self.contains_float(&current_corner) {
                return false;
            }
        }
        true
    }

    pub fn distance(&self, point: &FloatPoint) -> f64 {
        match self {
            TileShape::Box(b) => b.distance(point),
            _ => self
                .nearest_point_approx(point)
                .expect("TileShape.distance: no nearest point on a shape without border lines")
                .distance(point),
        }
    }

    pub fn border_distance(&self, point: &FloatPoint) -> f64 {
        self.nearest_border_point_approx(point)
            .expect(
                "TileShape.borderDistance: no nearest border point on a shape without border lines",
            )
            .distance(point)
    }

    pub fn smallest_radius(&self) -> f64 {
        self.border_distance(&self.centre_of_gravity())
    }

    pub fn nearest_point(&self, from_point: &Point) -> Option<Point> {
        if !self.is_outside(from_point) {
            return Some(from_point.clone());
        }
        self.nearest_border_point(from_point)
    }

    pub fn nearest_point_approx(&self, from_point: &FloatPoint) -> Option<FloatPoint> {
        if self.contains_float(from_point) {
            return Some(*from_point);
        }
        self.nearest_border_point_approx(from_point)
    }

    pub fn nearest_border_point(&self, from_point: &Point) -> Option<Point> {
        let line_count = self.border_line_count();
        if line_count == 0 {
            return None;
        }
        let from_point_f = from_point.to_float();
        if line_count == 1 {
            return Some(self.border_line_at(0).perpendicular_projection(from_point));
        }
        let mut min_dist = f64::MAX;
        let mut min_dist_ind = 0;

        for i in 0..line_count {
            let current_corner_f = self.corner_approx_at(i);
            let current_distance = current_corner_f.distance_square(&from_point_f);
            if current_distance < min_dist {
                min_dist = current_distance;
                min_dist_ind = i;
            }
        }

        let mut nearest_point = self.corner(min_dist_ind);

        let mut prev_ind = line_count - 2;
        let mut current_ind = line_count - 1;

        for next_ind in 0..line_count {
            let projection = self
                .border_line_at(current_ind)
                .perpendicular_projection(from_point);
            if (!self.corner_is_bounded(current_ind)
                || self.border_line_at(prev_ind).side_of(&projection) == Side::OnTheRight)
                && (!self.corner_is_bounded(next_ind)
                    || self.border_line_at(next_ind).side_of(&projection) == Side::OnTheRight)
            {
                let projection_f = projection.to_float();
                let current_distance = projection_f.distance_square(&from_point_f);
                if current_distance < min_dist {
                    min_dist = current_distance;
                    nearest_point = projection;
                }
            }
            prev_ind = current_ind;
            current_ind = next_ind;
        }
        Some(nearest_point)
    }

    pub fn nearest_border_point_approx(&self, from_point: &FloatPoint) -> Option<FloatPoint> {
        self.nearest_border_points_approx(from_point, 1)
            .first()
            .copied()
    }

    pub fn nearest_border_points_approx(
        &self,
        from_point: &FloatPoint,
        count: usize,
    ) -> Vec<FloatPoint> {
        if count == 0 {
            return Vec::new();
        }
        let line_count = self.border_line_count();
        if line_count == 0 {
            return Vec::new();
        }
        if line_count == 1 {
            return vec![from_point.projection_approx(&self.border_line_at(0))];
        }
        if self.dimension() == 0 {
            return vec![self.corner_approx_at(0)];
        }
        let result_count = count.min(line_count);
        let mut nearest_points: Vec<Option<FloatPoint>> = vec![None; result_count];
        let mut min_dists = vec![f64::MAX; result_count];

        for i in 0..line_count {
            if self.corner_is_bounded(i) {
                let current_corner = self.corner_approx_at(i);
                let current_distance = current_corner.distance_square(from_point);
                insert_sorted(
                    &mut min_dists,
                    &mut nearest_points,
                    current_distance,
                    current_corner,
                );
            }
        }

        let mut prev_ind = line_count - 2;
        let mut current_ind = line_count - 1;

        for next_ind in 0..line_count {
            let projection = from_point.projection_approx(&self.border_line_at(current_ind));
            if (!self.corner_is_bounded(current_ind)
                || self
                    .border_line_at(prev_ind)
                    .side_of_float_exact(&projection)
                    == Side::OnTheRight)
                && (!self.corner_is_bounded(next_ind)
                    || self
                        .border_line_at(next_ind)
                        .side_of_float_exact(&projection)
                        == Side::OnTheRight)
            {
                let current_distance = projection.distance_square(from_point);
                insert_sorted(
                    &mut min_dists,
                    &mut nearest_points,
                    current_distance,
                    projection,
                );
            }
            prev_ind = current_ind;
            current_ind = next_ind;
        }
        nearest_points.into_iter().flatten().collect()
    }

    pub fn index_of_nearest_corner(&self, from_point: &Point) -> usize {
        let from_point_f = from_point.to_float();
        let mut result = 0;
        let corner_count = self.border_line_count();
        let mut min_dist = JAVA_DOUBLE_MIN_VALUE;
        for i in 0..corner_count {
            let current_distance = self.corner_approx_at(i).distance(&from_point_f);
            if current_distance < min_dist {
                min_dist = current_distance;
                result = i;
            }
        }
        result
    }

    pub fn diagonal_corner_segment(&self) -> Option<FloatLine> {
        if self.is_empty() {
            return None;
        }
        let first_corner = self.corner_approx_at(0);
        let last_corner = self.corner_approx_at(self.border_line_count() / 2);
        Some(FloatLine::new(first_corner, last_corner))
    }

    pub fn nearest_relative_outside_locations(
        &self,
        shape: &TileShape,
        count: usize,
    ) -> Vec<FloatPoint> {
        let line_count = self.border_line_count();
        if count == 0 || line_count < 3 || !self.intersects(shape) {
            return Vec::new();
        }

        let result_count = count.min(line_count);

        let mut translate_coors: Vec<Option<FloatPoint>> = vec![None; result_count];
        let mut min_dists = vec![f64::MAX; result_count];

        let mut current_ind = line_count - 1;

        let other_line_count = shape.border_line_count();

        for next_ind in 0..line_count {
            let mut current_max_dist = 0.0;
            let mut current_translate_coor = FloatPoint::ZERO;
            for corner_index in 0..other_line_count {
                let current_corner = shape.corner_approx_at(corner_index);
                if self
                    .border_line_at(current_ind)
                    .side_of_float_exact(&current_corner)
                    == Side::OnTheRight
                {
                    let projection =
                        current_corner.projection_approx(&self.border_line_at(current_ind));
                    let current_distance = projection.distance_square(&current_corner);
                    if current_distance > current_max_dist {
                        current_max_dist = current_distance;
                        current_translate_coor = projection.subtract(&current_corner);
                    }
                }
            }

            insert_sorted(
                &mut min_dists,
                &mut translate_coors,
                current_max_dist,
                current_translate_coor,
            );
            current_ind = next_ind;
        }
        translate_coors.into_iter().flatten().collect()
    }

    pub fn shrink(&self, offset: f64) -> TileShape {
        let result = self.offset(-offset);
        if result.is_empty() {
            let centre_box = self.centre_of_gravity().bounding_box();
            return self.intersection(&TileShape::Box(centre_box));
        }
        result
    }

    pub fn length(&self) -> f64 {
        if !self.is_bounded() {
            return i32::MAX as f64;
        }
        let dimension = self.dimension();
        if dimension <= 0 {
            return 0.0;
        }
        if dimension == 1 {
            return self.circumference() / 2.0;
        }
        let mut max_distance = -1.0;
        let mut max_distance2 = -1.0;
        let gravity_point = self.centre_of_gravity();
        for i in 0..self.border_line_count() {
            let current_distance = self.border_line_at(i).signed_distance(&gravity_point).abs();
            if current_distance > max_distance {
                max_distance2 = max_distance;
                max_distance = current_distance;
            } else if current_distance > max_distance2 {
                max_distance2 = current_distance;
            }
        }
        max_distance + max_distance2
    }

    pub fn touching_sides(&self, other: &TileShape) -> Option<[usize; 2]> {
        let mut side_no2 = 0;
        let mut dir2: Option<IntDirection> = None;
        for i in 0..other.border_line_count() {
            let current_direction = other.border_line_at(i).direction();
            if current_direction.compare_to(&IntDirection::LEFT) != std::cmp::Ordering::Less {
                side_no2 = i;
                dir2 = Some(current_direction.opposite());
                break;
            }
        }
        let mut dir2 = dir2?;
        let mut side_no1 = 0;
        let mut dir1 = self.border_line_at(0).direction();
        let max_ind = self.border_line_count() + other.border_line_count();

        for _ in 0..max_ind {
            let compare = dir2.compare_to(&dir1);
            if compare == std::cmp::Ordering::Equal
                && self
                    .border_line_at(side_no1)
                    .is_equal_or_opposite(&other.border_line_at(side_no2))
            {
                return Some([side_no1, side_no2]);
            }
            if compare != std::cmp::Ordering::Less {
                side_no1 = (side_no1 + 1) % self.border_line_count();
                dir1 = self.border_line_at(side_no1).direction();
            } else {
                side_no2 = (side_no2 + 1) % other.border_line_count();
                dir2 = other.border_line_at(side_no2).direction().opposite();
            }
        }
        None
    }

    pub fn distance_to_the_left(&self, line: &Line) -> f64 {
        let mut result = i32::MAX as f64;
        for i in 0..self.border_line_count() {
            let current_corner = self.corner_approx_at(i);
            let mut line_side = line.side_of_float(&current_corner, 1.0);
            if line_side == Side::Collinear {
                line_side = line.side_of(&self.corner(i));
            }
            if line_side == Side::OnTheRight {
                result = -1.0;
                break;
            }
            result = java_min(result, line.signed_distance(&current_corner));
        }
        result
    }

    pub fn side_of_line(&self, line: &Line) -> Side {
        let mut on_the_left = false;
        let mut on_the_right = false;
        for i in 0..self.border_line_count() {
            let current_side = line.side_of(&self.corner(i));
            if current_side == Side::OnTheLeft {
                on_the_right = true;
            } else if current_side == Side::OnTheRight {
                on_the_left = true;
            }
            if on_the_left && on_the_right {
                return Side::Collinear;
            }
        }
        if on_the_left {
            Side::OnTheLeft
        } else {
            Side::OnTheRight
        }
    }

    pub fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> TileShape {
        if let TileShape::Box(b) = self {
            return TileShape::Box(b.turn_90_degree(factor, pole));
        }
        let new_lines: Vec<Line> = (0..self.border_line_count())
            .map(|i| self.border_line_at(i).turn_90_degree(factor, pole))
            .collect();
        TileShape::get_instance_from_lines(new_lines)
    }

    pub fn rotate_approx(&self, angle: f64, pole: &FloatPoint) -> TileShape {
        if angle == 0.0 {
            return self.clone();
        }
        let new_corners: Vec<Point> = (0..self.border_line_count())
            .map(|i| Point::Int(self.corner_approx_at(i).rotate(angle, pole).round()))
            .collect();
        let corner_polygon = Polygon::new(new_corners);
        let polygon_corners = corner_polygon.corner_array();
        match polygon_corners.len() {
            0 => TileShape::Simplex(Simplex::EMPTY),
            1 => TileShape::Box(TileShape::get_instance_from_point(&polygon_corners[0])),
            2 => TileShape::Simplex(Simplex::EMPTY),
            _ => {
                let int_corners: Vec<IntPoint> = polygon_corners
                    .iter()
                    .map(|p| match p {
                        Point::Int(p) => *p,
                        Point::Rational(_) => unreachable!(
                            "the corners were built with FloatPoint::round, so they are IntPoints"
                        ),
                    })
                    .collect();
                TileShape::get_instance_from_points(&int_corners)
            }
        }
    }

    pub fn mirror_vertical(&self, pole: &IntPoint) -> TileShape {
        let new_lines: Vec<Line> = (0..self.border_line_count())
            .map(|i| self.border_line_at(i).mirror_vertical(pole))
            .collect();
        TileShape::get_instance_from_lines(new_lines)
    }

    pub fn mirror_horizontal(&self, pole: &IntPoint) -> TileShape {
        let new_lines: Vec<Line> = (0..self.border_line_count())
            .map(|i| self.border_line_at(i).mirror_horizontal(pole))
            .collect();
        TileShape::get_instance_from_lines(new_lines)
    }

    pub fn intersecting_border_line_no(
        &self,
        point: &Point,
        direction: &Direction,
    ) -> Option<usize> {
        if !self.contains(point) {
            return None;
        }
        let Point::Int(int_point) = point else {
            return None;
        };
        let from_point = point.to_float();
        let intersection_line = Line::from_direction_any(*int_point, direction)?;
        let second_line_point = intersection_line.b.to_float();
        let mut result = None;
        let mut min_distance = f32::MAX as f64;
        for i in 0..self.border_line_count() {
            let current_border_line = self.border_line_at(i);
            let current_intersection = current_border_line.intersection_approx(&intersection_line);
            if current_intersection.x >= i32::MAX as f64 {
                continue;
            }
            let current_distance = current_intersection.distance_square(&from_point);
            if current_distance < min_distance {
                let direction_ok = current_border_line.side_of_float_exact(&second_line_point)
                    == Side::OnTheLeft
                    || second_line_point.distance_square(&current_intersection) < current_distance;
                if direction_ok {
                    result = Some(i);
                    min_distance = current_distance;
                }
            }
        }
        result
    }

    pub fn split_to_convex(&self) -> Vec<TileShape> {
        vec![self.clone()]
    }

    pub fn divide_into_sections(&self, max_section_width: f64) -> Vec<TileShape> {
        if let TileShape::Box(b) = self {
            return b
                .divide_into_sections(max_section_width)
                .into_iter()
                .map(TileShape::Box)
                .collect();
        }
        if self.is_empty() {
            return vec![self.clone()];
        }
        let section_boxes = self.bounding_box().divide_into_sections(max_section_width);
        let mut section_list = Vec::new();
        for section_box in section_boxes {
            let current_section = self.intersection_with_simplify(&TileShape::Box(section_box));
            if current_section.dimension() == 2 {
                section_list.push(current_section);
            }
        }
        section_list
    }

    pub fn is_intersected_interior_by(&self, line_segment: &LineSegment) -> bool {
        self.is_intersected_interior_by_points(
            &line_segment.start_point(),
            &line_segment.end_point(),
            &line_segment.get_line(),
        )
    }

    pub fn is_intersected_interior_by_points(
        &self,
        start_point: &Point,
        end_point: &Point,
        line: &Line,
    ) -> bool {
        let float_start_point = start_point.to_float();
        let float_end_point = end_point.to_float();

        let line_count = self.border_line_count();
        let mut border_line_side_of_start_point_arr: Vec<Side> = Vec::with_capacity(line_count);
        let mut border_line_side_of_end_point_arr: Vec<Side> = Vec::with_capacity(line_count);
        for i in 0..line_count {
            let current_border_line = self.border_line_at(i);
            let mut border_line_side_of_start_point =
                current_border_line.side_of_float(&float_start_point, 1.0);
            if border_line_side_of_start_point == Side::Collinear {
                border_line_side_of_start_point = current_border_line.side_of(start_point);
            }
            let mut border_line_side_of_end_point =
                current_border_line.side_of_float(&float_end_point, 1.0);
            if border_line_side_of_end_point == Side::Collinear {
                border_line_side_of_end_point = current_border_line.side_of(end_point);
            }
            if border_line_side_of_start_point != Side::OnTheRight
                && border_line_side_of_end_point != Side::OnTheRight
            {
                return false;
            }
            border_line_side_of_start_point_arr.push(border_line_side_of_start_point);
            border_line_side_of_end_point_arr.push(border_line_side_of_end_point);
        }
        let start_point_is_inside = border_line_side_of_start_point_arr
            .iter()
            .all(|s| *s == Side::OnTheRight);
        if start_point_is_inside {
            return true;
        }
        let end_point_is_inside = border_line_side_of_end_point_arr
            .iter()
            .all(|s| *s == Side::OnTheRight);
        if end_point_is_inside {
            return true;
        }
        let segment_line = line;
        for i in 0..line_count {
            let border_line_side_of_start_point = border_line_side_of_start_point_arr[i];
            let border_line_side_of_end_point = border_line_side_of_end_point_arr[i];
            if border_line_side_of_start_point != border_line_side_of_end_point {
                if border_line_side_of_start_point == Side::Collinear
                    && border_line_side_of_end_point == Side::OnTheLeft
                    || border_line_side_of_end_point == Side::Collinear
                        && border_line_side_of_start_point == Side::OnTheLeft
                {
                    continue;
                }
                let mut prev_corner_side =
                    segment_line.side_of_float(&self.corner_approx_at(i), 1.0);
                if prev_corner_side == Side::Collinear {
                    prev_corner_side = segment_line.side_of(&self.corner(i));
                }
                let next_corner_index = if i == line_count - 1 { 0 } else { i + 1 };
                let mut next_corner_side =
                    segment_line.side_of_float(&self.corner_approx_at(next_corner_index), 1.0);
                if next_corner_side == Side::Collinear {
                    next_corner_side = segment_line.side_of(&self.corner(next_corner_index));
                }
                if prev_corner_side == Side::OnTheLeft && next_corner_side == Side::OnTheRight
                    || prev_corner_side == Side::OnTheRight && next_corner_side == Side::OnTheLeft
                {
                    return true;
                }
            }
        }
        false
    }

    pub fn cutout(&self, shape: &TileShape) -> Option<Vec<TileShape>> {
        let pieces = shape.cutout_from(self)?;
        if matches!(self, TileShape::Box(_)) {
            return Some(pieces.iter().map(|p| p.simplify()).collect());
        }
        Some(pieces)
    }

    pub fn cutout_from(&self, outer: &TileShape) -> Option<Vec<TileShape>> {
        match (self, outer) {
            (TileShape::Box(a), TileShape::Box(d)) => {
                Some(a.cutout_from(d).into_iter().map(TileShape::Box).collect())
            }
            (TileShape::Box(a), TileShape::Octagon(d)) => Some(
                a.cutout_from_octagon(d)
                    .into_iter()
                    .map(TileShape::Octagon)
                    .collect(),
            ),
            (TileShape::Box(a), TileShape::Simplex(d)) => Some(
                a.cutout_from_simplex(d)?
                    .into_iter()
                    .map(TileShape::Simplex)
                    .collect(),
            ),
            (TileShape::Octagon(a), TileShape::Box(d)) => Some(
                a.cutout_from_box(d)
                    .into_iter()
                    .map(TileShape::Octagon)
                    .collect(),
            ),
            (TileShape::Octagon(a), TileShape::Octagon(d)) => Some(
                a.cutout_from_octagon(d)
                    .into_iter()
                    .map(TileShape::Octagon)
                    .collect(),
            ),
            (TileShape::Octagon(a), TileShape::Simplex(d)) => Some(
                a.cutout_from_simplex(d)?
                    .into_iter()
                    .map(TileShape::Simplex)
                    .collect(),
            ),
            (TileShape::Simplex(a), TileShape::Box(d)) => Some(
                a.cutout_from_box(d)?
                    .into_iter()
                    .map(TileShape::Simplex)
                    .collect(),
            ),
            (TileShape::Simplex(a), TileShape::Octagon(d)) => Some(
                a.cutout_from_octagon(d)?
                    .into_iter()
                    .map(TileShape::Simplex)
                    .collect(),
            ),
            (TileShape::Simplex(a), TileShape::Simplex(d)) => Some(
                a.cutout_from(d)?
                    .into_iter()
                    .map(TileShape::Simplex)
                    .collect(),
            ),
        }
    }

    pub fn cutout_polyline(&self, polyline: &Polyline) -> Result<Vec<Polyline>, PolylineError> {
        let intersection_no = self.entrance_points(polyline);
        let first_corner = polyline.first_corner();
        let first_corner_is_inside = match &first_corner {
            Some(corner) => self.contains_inside(corner),
            None => false,
        };
        if intersection_no.is_empty() {
            if first_corner_is_inside {
                return Ok(Vec::new());
            }
            return Ok(vec![polyline.clone()]);
        }
        let mut pieces: Vec<Polyline> = Vec::new();
        let mut current_intersection_no = 0usize;
        let mut current_intersection_tuple = intersection_no[current_intersection_no];
        let first_intersection = polyline.lines()[current_intersection_tuple[0]]
            .intersection(&self.border_line_at(current_intersection_tuple[1]));
        if !first_corner_is_inside {
            if first_corner.as_ref() != Some(&first_intersection) {
                let current_polyline_intersection_no = current_intersection_tuple[0];
                let mut current_lines: Vec<Line> =
                    polyline.lines()[..current_polyline_intersection_no + 1].to_vec();
                current_lines.push(self.border_line_at(current_intersection_tuple[1]));
                let current_piece = Polyline::from_lines(current_lines)?;
                if !current_piece.is_empty() {
                    pieces.push(current_piece);
                }
            }
            current_intersection_no += 1;
        }
        while current_intersection_no + 1 < intersection_no.len() {
            current_intersection_tuple = intersection_no[current_intersection_no];
            let next_intersection_tuple = intersection_no[current_intersection_no + 1];
            let current_intersection_no_of_polyline = current_intersection_tuple[0];
            let next_intersection_no_of_polyline = next_intersection_tuple[0];
            let mut insert_piece = false;
            for i in current_intersection_no_of_polyline + 1..next_intersection_no_of_polyline {
                if polyline
                    .corner(i)
                    .is_some_and(|corner| self.is_outside(&corner))
                {
                    insert_piece = true;
                    break;
                }
            }

            if insert_piece {
                let mut current_lines: Vec<Line> = Vec::with_capacity(
                    next_intersection_no_of_polyline - current_intersection_no_of_polyline + 3,
                );
                current_lines.push(self.border_line_at(current_intersection_tuple[1]));
                current_lines.extend_from_slice(
                    &polyline.lines()
                        [current_intersection_no_of_polyline..=next_intersection_no_of_polyline],
                );
                current_lines.push(self.border_line_at(next_intersection_tuple[1]));
                let current_piece = Polyline::from_lines(current_lines)?;
                if !current_piece.is_empty() {
                    pieces.push(current_piece);
                }
            }
            current_intersection_no += 2;
        }
        if current_intersection_no < intersection_no.len() {
            current_intersection_tuple = intersection_no[current_intersection_no];
            let current_polyline_intersection_no = current_intersection_tuple[0];
            let mut current_lines: Vec<Line> =
                Vec::with_capacity(polyline.lines().len() - current_polyline_intersection_no + 1);
            current_lines.push(self.border_line_at(current_intersection_tuple[1]));
            current_lines.extend_from_slice(&polyline.lines()[current_polyline_intersection_no..]);
            let current_piece = Polyline::from_lines(current_lines)?;
            if !current_piece.is_empty() {
                pieces.push(current_piece);
            }
        }
        Ok(pieces)
    }

    pub fn entrance_points(&self, polyline: &Polyline) -> Vec<[usize; 2]> {
        let mut result: Vec<[usize; 2]> = Vec::new();
        let mut prev_intersection_line_no: Option<usize> = None;
        let mut prev_intersection_edge_no: Option<usize> = None;
        for line_index in 1..polyline.lines().len().saturating_sub(1) {
            let Some(current_line_seg) = LineSegment::from_polyline(polyline, line_index) else {
                continue;
            };
            for edge_index in current_line_seg.border_intersections(self) {
                if Some(line_index) != prev_intersection_line_no
                    || Some(edge_index) != prev_intersection_edge_no
                {
                    result.push([line_index, edge_index]);
                    prev_intersection_line_no = Some(line_index);
                    prev_intersection_edge_no = Some(edge_index);
                }
            }
        }
        result
    }
    pub fn intersects_circle(&self, other: &crate::circle::Circle) -> bool {
        other.intersects_tile(self)
    }
}

fn insert_sorted(
    min_dists: &mut [f64],
    values: &mut [Option<FloatPoint>],
    current_distance: f64,
    current_value: FloatPoint,
) {
    let result_count = min_dists.len();
    for j in 0..result_count {
        if current_distance < min_dists[j] {
            for k in (j + 1)..result_count {
                min_dists[k] = min_dists[k - 1];
                values[k] = values[k - 1];
            }
            min_dists[j] = current_distance;
            values[j] = Some(current_value);
            break;
        }
    }
}

impl IntBox {
    pub fn simplify(&self) -> TileShape {
        TileShape::Box(*self)
    }

    pub fn bounding_tile(&self) -> IntBox {
        *self
    }
}

impl IntOctagon {
    pub fn simplify(&self) -> TileShape {
        if self.is_int_box() {
            return TileShape::Box(self.bounding_box());
        }
        TileShape::Octagon(*self)
    }

    pub fn bounding_tile(&self) -> IntOctagon {
        *self
    }
}

impl Simplex {
    pub fn simplify(&self) -> TileShape {
        if self.is_empty() {
            TileShape::Simplex(Simplex::EMPTY)
        } else if self.is_int_box() {
            TileShape::Box(self.bounding_box())
        } else if self.is_int_octagon() {
            match self.to_int_octagon() {
                Some(oct) => TileShape::Octagon(oct),
                None => TileShape::Simplex(self.clone()),
            }
        } else {
            TileShape::Simplex(self.clone())
        }
    }
}

impl Line {
    pub fn is_on_the_left(&self, tile: &TileShape) -> bool {
        for i in 0..tile.border_line_count() {
            if self.side_of(&tile.corner(i)) == Side::OnTheRight {
                return false;
            }
        }
        true
    }

    pub fn is_on_the_right(&self, tile: &TileShape) -> bool {
        for i in 0..tile.border_line_count() {
            if self.side_of(&tile.corner(i)) == Side::OnTheLeft {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::float_point::FloatPoint;
    use crate::int_box::IntBox;
    use crate::int_point::IntPoint;
    use crate::point::Point;
    use crate::simplex::Simplex;

    fn bx() -> TileShape {
        TileShape::Box(IntBox::from_coords(0, 0, 10, 10))
    }

    fn tri() -> TileShape {
        TileShape::Simplex(Simplex::from_points(&[
            IntPoint::new(0, 0),
            IntPoint::new(10, 0),
            IntPoint::new(0, 10),
        ]))
    }

    #[test]
    fn containment_family() {
        let b = bx();
        assert!(b.contains(&Point::Int(IntPoint::new(5, 5))));
        assert!(b.contains(&Point::Int(IntPoint::new(0, 5))));
        assert!(!b.contains_inside(&Point::Int(IntPoint::new(0, 5))));
        assert!(b.contains_on_border(&Point::Int(IntPoint::new(0, 5))));
        assert_eq!(
            b.contains_on_border_line_no(&Point::Int(IntPoint::new(5, 5))),
            None
        );
        assert!(b.is_outside(&Point::Int(IntPoint::new(11, 5))));
        assert!(b.contains_float(&FloatPoint::new(9.9, 0.1)));
        assert!(b.contains_tile(&TileShape::Box(IntBox::from_coords(2, 2, 3, 3))));
        assert!(!b.contains_tile(&TileShape::Box(IntBox::from_coords(2, 2, 30, 3))));
    }

    #[test]
    fn area_and_intersection_across_variants() {
        assert_eq!(bx().area(), 100.0);
        assert!((tri().area() - 50.0).abs() < 1e-9);
        let i = bx().intersection(&tri());
        assert!((i.area() - 50.0).abs() < 1e-9);
        let oct = TileShape::Octagon(IntBox::from_coords(5, 5, 20, 20).to_int_octagon());
        let j = bx().intersection(&oct);
        assert_eq!(j.area(), 25.0);
        assert!(bx().intersects(&oct));
        assert!(!tri().intersects(&TileShape::Box(IntBox::from_coords(8, 8, 9, 9))));
        let s = TileShape::Simplex(IntBox::from_coords(0, 0, 10, 10).to_simplex());
        assert!(matches!(s.simplify(), TileShape::Box(_)));
        assert!(matches!(
            bx().intersection_with_simplify(&oct),
            TileShape::Box(_)
        ));
    }

    #[test]
    fn distances_and_nearest() {
        let b = bx();
        assert_eq!(b.distance(&FloatPoint::new(13.0, 14.0)), 5.0);
        assert_eq!(b.distance(&FloatPoint::new(5.0, 5.0)), 0.0);
        assert_eq!(b.border_distance(&FloatPoint::new(5.0, 5.0)), 5.0);
        assert_eq!(b.smallest_radius(), 5.0);
        assert_eq!(
            b.nearest_point(&Point::Int(IntPoint::new(-4, 5))),
            Some(Point::Int(IntPoint::new(0, 5)))
        );
        assert_eq!(
            b.nearest_border_point(&Point::Int(IntPoint::new(1, 5))),
            Some(Point::Int(IntPoint::new(0, 5)))
        );
        assert_eq!(b.max_width(), 10.0);
    }

    #[test]
    fn touching_sides_and_side_of_line() {
        let a = bx();
        let b = TileShape::Box(IntBox::from_coords(10, 0, 20, 10));
        let ts = a.touching_sides(&b).expect("boxes share an edge");
        assert_eq!(a.border_line(ts[0]).unwrap().a.x, 10);
        assert_eq!(b.border_line(ts[1]).unwrap().a.x, 10);
        assert!(
            a.touching_sides(&TileShape::Box(IntBox::from_coords(30, 0, 40, 10)))
                .is_none()
        );
        let far = crate::line::Line::from_coords(50, 0, 50, 1);
        assert_ne!(a.side_of_line(&far), crate::Side::Collinear);
        assert!(far.is_on_the_right(&a) || far.is_on_the_left(&a));
    }

    #[test]
    fn transformations() {
        let t = tri().turn_90_degree(1, &IntPoint::new(0, 0));
        assert_eq!(t.bounding_box(), IntBox::from_coords(-10, 0, 0, 10));
        let m = tri().mirror_vertical(&IntPoint::new(0, 0));
        assert_eq!(m.bounding_box(), IntBox::from_coords(-10, 0, 0, 10));
        let s = bx().shrink(2.0);
        assert_eq!(s.bounding_box(), IntBox::from_coords(2, 2, 8, 8));
        let parts = bx().divide_into_sections(4.0);
        assert!(parts.len() >= 4);
        assert!((parts.iter().map(|p| p.area()).sum::<f64>() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn divide_into_sections_drops_degenerate_pieces() {
        let parts = tri().divide_into_sections(6.0);
        assert_eq!(parts.len(), 3);
        assert!((parts.iter().map(|p| p.area()).sum::<f64>() - 50.0).abs() < 1e-9);
    }

    #[test]
    fn cutout_from_tile() {
        let outer = TileShape::Box(IntBox::from_coords(0, 0, 20, 20));
        let pieces = tri()
            .cutout_from(&outer)
            .expect("the triangle is 2-dimensional");
        let total: f64 = pieces.iter().map(|p| p.area()).sum();
        assert!((total - (400.0 - 50.0)).abs() < 1e-6);
    }
    #[test]
    fn static_factories() {
        let square = TileShape::get_instance_from_points(&[
            IntPoint::new(0, 0),
            IntPoint::new(10, 0),
            IntPoint::new(10, 10),
            IntPoint::new(0, 10),
        ]);
        assert_eq!(square, bx());
        assert_eq!(
            TileShape::get_instance_from_box(0, 0, 10, 10),
            IntBox::from_coords(0, 0, 10, 10).to_int_octagon()
        );
        assert_eq!(
            TileShape::get_instance_octagon(0, 0, 10, 10, -10, 10, 0, 20),
            IntOctagon::new(0, 0, 10, 10, -10, 10, 0, 20).normalize()
        );
        assert_eq!(
            TileShape::get_instance_from_point(&Point::Int(IntPoint::new(3, 5))),
            IntPoint::new(3, 5).surrounding_box()
        );
        let half_plane = TileShape::get_instance_from_line(Line::from_coords(0, 0, 0, 1));
        assert_eq!(half_plane.border_line_count(), 1);
        assert!(!half_plane.is_bounded());
        assert_eq!(half_plane.area(), f64::MAX);
    }

    #[test]
    fn representation_dispatch() {
        let oct = TileShape::Octagon(IntBox::from_coords(0, 0, 10, 10).to_int_octagon());
        assert_eq!(bx().border_line_count(), 4);
        assert_eq!(oct.border_line_count(), 8);
        assert_eq!(tri().border_line_count(), 3);
        assert!(bx().is_int_box() && bx().is_int_octagon());
        assert!(oct.is_int_box());
        assert!(!tri().is_int_box());
        assert!(tri().is_int_octagon());
        assert_eq!(oct.simplify(), bx());
        assert_eq!(
            bx().to_simplex(),
            IntBox::from_coords(0, 0, 10, 10).to_simplex()
        );
        assert_eq!(bx().bounding_tile(), bx());
        assert_eq!(bx().get_id(), IntBox::from_coords(0, 0, 10, 10).get_id());
        assert_eq!(
            bx().bounding_octagon(),
            Some(IntBox::from_coords(0, 0, 10, 10).to_int_octagon())
        );
        assert_eq!(bx().dimension(), 2);
        assert_eq!(tri().dimension(), 2);
        assert!(bx().corner_is_bounded(0) && tri().corner_is_bounded(0));
        assert_eq!(bx().corner(2), Point::Int(IntPoint::new(10, 10)));
        assert_eq!(bx().corner_approx_arr().len(), 4);
        let tri_line = tri().border_line(0).unwrap();
        assert_eq!(tri().border_line_index(&tri_line), Some(0));
        assert_eq!(bx().border_line_index(&bx().border_line(0).unwrap()), None);
        assert_eq!(oct.border_line_index(&oct.border_line(0).unwrap()), None);
    }

    #[test]
    fn offsets_translation_and_widths() {
        let v = Vector::Int(crate::int_vector::IntVector::new(3, 4));
        assert_eq!(
            bx().translate_by(&v),
            TileShape::Box(IntBox::from_coords(3, 4, 13, 14))
        );
        assert_eq!(
            bx().offset(2.0),
            TileShape::Box(IntBox::from_coords(-2, -2, 12, 12))
        );
        assert!(matches!(bx().enlarge(2.0), TileShape::Octagon(_)));
        assert_eq!(bx().min_width(), 10.0);
        assert_eq!(bx().circumference(), 40.0);
        assert!((tri().circumference() - (20.0 + 200.0_f64.sqrt())).abs() < 1e-9);
        assert_eq!(bx().centre_of_gravity(), FloatPoint::new(5.0, 5.0));
        assert_eq!(bx().length(), 10.0);
        assert_eq!(
            bx().diagonal_corner_segment(),
            Some(crate::float_line::FloatLine::new(
                FloatPoint::new(0.0, 0.0),
                FloatPoint::new(10.0, 10.0)
            ))
        );
        assert_eq!(bx().split_to_convex(), vec![bx()]);
        let m = tri().mirror_horizontal(&IntPoint::new(0, 0));
        assert_eq!(m.bounding_box(), IntBox::from_coords(0, -10, 10, 0));
    }

    #[test]
    fn index_of_nearest_corner_only_moves_off_zero_at_distance_zero() {
        assert_eq!(
            bx().index_of_nearest_corner(&Point::Int(IntPoint::new(9, 9))),
            0
        );
        assert_eq!(
            bx().index_of_nearest_corner(&Point::Int(IntPoint::new(10, 10))),
            2
        );
    }

    #[test]
    fn side_of_border_and_contains_approx() {
        let b = bx();
        assert_eq!(
            b.side_of_border(&FloatPoint::new(5.0, 5.0), 0.0),
            crate::Side::OnTheRight
        );
        assert_eq!(
            b.side_of_border(&FloatPoint::new(0.0, 5.0), 0.0),
            crate::Side::Collinear
        );
        assert_eq!(
            b.side_of_border(&FloatPoint::new(20.0, 5.0), 0.0),
            crate::Side::OnTheLeft
        );
        assert!(b.contains_approx(&TileShape::Box(IntBox::from_coords(2, 2, 3, 3))));
        assert!(!b.contains_approx(&b));
        assert!(!b.contains_float(&FloatPoint::new(0.0, 5.0)));
        assert!(!b.contains_float_tol(&FloatPoint::new(0.0, 5.0), 0.0));
        let oct = TileShape::Octagon(IntBox::from_coords(0, 0, 10, 10).to_int_octagon());
        assert!(oct.contains_float(&FloatPoint::new(0.0, 5.0)));
        assert!(!oct.contains_float_tol(&FloatPoint::new(0.0, 5.0), 0.0));
    }

    #[test]
    fn distance_to_the_left_and_ray_intersection() {
        let b = bx();
        assert_eq!(
            b.distance_to_the_left(&Line::from_coords(-5, 0, -5, 1)),
            5.0
        );
        assert_eq!(
            b.distance_to_the_left(&Line::from_coords(15, 0, 15, 1)),
            -1.0
        );
        assert_eq!(
            b.intersecting_border_line_no(
                &Point::Int(IntPoint::new(5, 5)),
                &Direction::Int(IntDirection::RIGHT)
            ),
            Some(1)
        );
        assert_eq!(
            b.intersecting_border_line_no(
                &Point::Int(IntPoint::new(50, 5)),
                &Direction::Int(IntDirection::RIGHT)
            ),
            None
        );
    }

    #[test]
    fn distance_to_the_left_propagates_nan_for_degenerate_line() {
        let degenerate = Line::from_coords(5, 5, 5, 5);
        assert!(
            degenerate
                .signed_distance(&FloatPoint::new(0.0, 0.0))
                .is_nan()
        );
        assert!(bx().distance_to_the_left(&degenerate).is_nan());
        assert!(tri().distance_to_the_left(&degenerate).is_nan());
    }

    #[test]
    fn is_intersected_interior_by_points_crossings() {
        let b = bx();
        let start = Point::Int(IntPoint::new(-5, 5));
        let end = Point::Int(IntPoint::new(15, 5));
        let line = Line::from_coords(-5, 5, 15, 5);
        assert!(b.is_intersected_interior_by_points(&start, &end, &line));
        let start = Point::Int(IntPoint::new(-5, 20));
        let end = Point::Int(IntPoint::new(15, 20));
        let line = Line::from_coords(-5, 20, 15, 20);
        assert!(!b.is_intersected_interior_by_points(&start, &end, &line));
        let start = Point::Int(IntPoint::new(-5, 0));
        let end = Point::Int(IntPoint::new(15, 0));
        let line = Line::from_coords(-5, 0, 15, 0);
        assert!(!b.is_intersected_interior_by_points(&start, &end, &line));
    }

    #[test]
    fn nearest_border_points_and_relative_outside_locations() {
        let b = bx();
        assert_eq!(
            b.nearest_border_points_approx(&FloatPoint::new(5.0, 5.0), 2),
            vec![FloatPoint::new(0.0, 5.0), FloatPoint::new(5.0, 0.0)]
        );
        assert!(
            b.nearest_border_points_approx(&FloatPoint::new(5.0, 5.0), 0)
                .is_empty()
        );
        let half_plane = TileShape::get_instance_from_line(Line::from_coords(0, 0, 0, 1));
        assert_eq!(
            half_plane.nearest_border_point_approx(&FloatPoint::new(4.0, 7.0)),
            Some(FloatPoint::new(0.0, 7.0))
        );
        assert_eq!(
            half_plane.nearest_border_point(&Point::Int(IntPoint::new(4, 7))),
            Some(Point::Int(IntPoint::new(0, 7)))
        );
        assert_eq!(
            b.nearest_relative_outside_locations(
                &TileShape::Box(IntBox::from_coords(8, 8, 12, 12)),
                1
            ),
            vec![FloatPoint::new(2.0, 0.0)]
        );
        assert!(
            b.nearest_relative_outside_locations(
                &TileShape::Box(IntBox::from_coords(80, 80, 120, 120)),
                1
            )
            .is_empty()
        );
    }

    #[test]
    fn cutout_simplifies_only_for_a_box_receiver() {
        let outer = TileShape::Box(IntBox::from_coords(0, 0, 20, 20));
        let pieces = outer.cutout(&tri()).expect("the triangle is 2-dimensional");
        let total: f64 = pieces.iter().map(|p| p.area()).sum();
        assert!((total - 350.0).abs() < 1e-6);
        let inner = TileShape::Box(IntBox::from_coords(5, 5, 15, 15));
        let pieces = outer.cutout(&inner).expect("boxes always cut out");
        assert!(pieces.iter().all(|p| matches!(p, TileShape::Box(_))));
        let total: f64 = pieces.iter().map(|p| p.area()).sum();
        assert!((total - 300.0).abs() < 1e-9);
        let degenerate = TileShape::Simplex(Simplex::from_points(&[
            IntPoint::new(0, 0),
            IntPoint::new(10, 0),
        ]));
        assert_eq!(degenerate.cutout_from(&outer), None);
    }

    #[test]
    fn empty_shape_behaviour() {
        let e = TileShape::Simplex(Simplex::EMPTY);
        assert!(e.is_empty());
        assert_eq!(e.border_line_count(), 0);
        assert_eq!(e.border_line(0), None);
        assert_eq!(e.corner_approx(0), None);
        assert!(e.is_outside(&Point::Int(IntPoint::new(0, 0))));
        assert!(!e.contains(&Point::Int(IntPoint::new(0, 0))));
        assert!(!e.contains_inside(&Point::Int(IntPoint::new(0, 0))));
        assert!(!e.contains_float(&FloatPoint::ZERO));
        assert_eq!(
            e.side_of_border(&FloatPoint::ZERO, 0.0),
            crate::Side::Collinear
        );
        assert_eq!(
            e.contains_on_border_line_no(&Point::Int(IntPoint::ZERO)),
            None
        );
        assert_eq!(e.nearest_point(&Point::Int(IntPoint::ZERO)), None);
        assert_eq!(e.nearest_border_point(&Point::Int(IntPoint::ZERO)), None);
        assert_eq!(e.nearest_border_point_approx(&FloatPoint::ZERO), None);
        assert!(
            e.nearest_border_points_approx(&FloatPoint::ZERO, 3)
                .is_empty()
        );
        assert_eq!(e.diagonal_corner_segment(), None);
        assert_eq!(e.area(), 0.0);
        assert_eq!(e.length(), 0.0);
        assert_eq!(e.circumference(), 0.0);
        assert_eq!(e.divide_into_sections(4.0), vec![e.clone()]);
        // `#[should_panic]` tests below.
    }

    #[test]
    #[should_panic(expected = "no nearest point on a shape without border lines")]
    fn distance_on_a_border_line_free_shape_throws_like_java() {
        TileShape::Simplex(Simplex::EMPTY).distance(&FloatPoint::ZERO);
    }

    #[test]
    #[should_panic(expected = "no nearest border point on a shape without border lines")]
    fn border_distance_on_a_border_line_free_shape_throws_like_java() {
        TileShape::Simplex(Simplex::EMPTY).border_distance(&FloatPoint::ZERO);
    }

    #[test]
    #[should_panic(expected = "no nearest border point on a shape without border lines")]
    fn smallest_radius_on_a_border_line_free_shape_throws_like_java() {
        TileShape::Simplex(Simplex::EMPTY).smallest_radius();
    }

    #[test]
    fn rotate_approx_across_representations() {
        let b = IntBox::from_coords(0, 0, 10, 10);
        let shape = TileShape::Box(b);
        assert_eq!(shape.rotate_approx(0.0, &FloatPoint::ZERO), shape);
        assert_eq!(
            shape.rotate_approx(std::f64::consts::FRAC_PI_2, &FloatPoint::ZERO),
            TileShape::Box(IntBox::from_coords(-10, 0, 0, 10))
        );
        let r45 = shape.rotate_approx(std::f64::consts::FRAC_PI_4, &FloatPoint::ZERO);
        assert!(matches!(r45, TileShape::Octagon(_)));
        let corners: Vec<Point> = (0..r45.border_line_count())
            .map(|i| r45.corner(i))
            .collect();
        assert_eq!(
            corners,
            vec![
                Point::Int(IntPoint::new(0, 0)),
                Point::Int(IntPoint::new(0, 0)),
                Point::Int(IntPoint::new(7, 7)),
                Point::Int(IntPoint::new(7, 7)),
                Point::Int(IntPoint::new(0, 14)),
                Point::Int(IntPoint::new(0, 14)),
                Point::Int(IntPoint::new(-7, 7)),
                Point::Int(IntPoint::new(-7, 7)),
            ]
        );
    }

    #[test]
    fn entrance_points_and_cutout_of_a_polyline() {
        let shape = TileShape::Box(IntBox::from_coords(0, 0, 10, 10));
        let cross = Polyline::from_two_points(
            &Point::Int(IntPoint::new(-5, 5)),
            &Point::Int(IntPoint::new(15, 5)),
        );
        assert_eq!(shape.entrance_points(&cross), vec![[1, 3], [1, 1]]);
        let pieces = shape.cutout_polyline(&cross).unwrap();
        assert_eq!(pieces.len(), 2);
        assert_eq!(
            pieces[0].corners(),
            vec![
                Point::Int(IntPoint::new(-5, 5)),
                Point::Int(IntPoint::new(0, 5))
            ]
        );
        assert_eq!(
            pieces[1].corners(),
            vec![
                Point::Int(IntPoint::new(10, 5)),
                Point::Int(IntPoint::new(15, 5))
            ]
        );
        let inside = Polyline::from_two_points(
            &Point::Int(IntPoint::new(2, 2)),
            &Point::Int(IntPoint::new(8, 8)),
        );
        assert_eq!(shape.cutout_polyline(&inside).unwrap().len(), 0);
        let outside = Polyline::from_two_points(
            &Point::Int(IntPoint::new(20, 20)),
            &Point::Int(IntPoint::new(30, 30)),
        );
        assert_eq!(shape.cutout_polyline(&outside), Ok(vec![outside]));
        let empty = Polyline::from_points(&[Point::Int(IntPoint::new(4, 4))]);
        assert!(empty.is_empty());
        assert_eq!(shape.cutout_polyline(&empty), Ok(vec![empty.clone()]));
        assert_eq!(
            TileShape::Simplex(Simplex::EMPTY).cutout_polyline(&empty),
            Ok(vec![empty])
        );
    }
}
