
use crate::direction::Direction;
use crate::float_point::FloatPoint;
use crate::int_box::IntBox;
use crate::int_octagon::IntOctagon;
use crate::int_point::IntPoint;
use crate::int_vector::IntVector;
use crate::limits::{java_max, java_min};
use crate::line::Line;
use crate::line_segment::LineSegment;
use crate::point::Point;
use crate::polygon::Polygon;
use crate::side::Side;
use crate::tile_shape::TileShape;
use crate::vector::Vector;

const USE_BOUNDING_OCTAGON_FOR_OFFSET_SHAPES: bool = true;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PolylineError {
            NormalizationIndexUnderflow,
}

impl std::fmt::Display for PolylineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolylineError::NormalizationIndexUnderflow => f.write_str(
                "Polyline normalisation: removeOverlaps ran out of lines \
                 (Java throws ArrayIndexOutOfBoundsException: Index -1, Polyline.java:148)",
            ),
        }
    }
}

impl std::error::Error for PolylineError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Polyline {
    lines: Vec<Line>,
}


impl Polyline {
                                        pub fn from_polygon(polygon: &Polygon) -> Polyline {
        let points = polygon.corner_array();
        if points.len() < 2 {
            // Java: FRLogger.warn("Polyline: must contain at least 2 different points")
            return Polyline { lines: Vec::new() };
        }
        let pts: Vec<IntPoint> = points.iter().map(int_point_of).collect();
        let mut lines: Vec<Line> = Vec::with_capacity(pts.len() + 1);
        lines.push(Line::new(pts[0], pts[0]));
        for i in 1..pts.len() {
            lines.push(Line::new(pts[i - 1], pts[i]));
        }

        let dir = Direction::between(&points[0], &points[1])
            .expect("Polygon has no two equal consecutive corners");
        lines[0] = line_from_direction(pts[0], &dir.turn_45_degree(2));

        let last = pts.len() - 1;
        let dir = Direction::between(&points[last], &points[last - 1])
            .expect("Polygon has no two equal consecutive corners");
        lines.push(line_from_direction(pts[last], &dir.turn_45_degree(2)));
        Polyline { lines }
    }

        pub fn from_points(points: &[Point]) -> Polyline {
        Polyline::from_polygon(&Polygon::new(points.to_vec()))
    }

                        pub fn from_two_points(from_corner: &Point, to_corner: &Point) -> Polyline {
        if from_corner == to_corner {
            return Polyline { lines: Vec::new() };
        }
        let from = int_point_of(from_corner);
        let to = int_point_of(to_corner);
        let dir = Direction::between(from_corner, to_corner).expect("the corners differ");
        let l0 = line_from_direction(from, &dir.turn_45_degree(2));
        let l1 = Line::new(from, to);
        let dir = Direction::between(from_corner, to_corner).expect("the corners differ");
        let l2 = line_from_direction(to, &dir.turn_45_degree(2));
        Polyline {
            lines: vec![l0, l1, l2],
        }
    }

                                                        pub fn from_lines(input_lines: Vec<Line>) -> Result<Polyline, PolylineError> {
        Ok(Polyline::build(input_lines)?.0)
    }

                                                                                                            pub fn from_lines_in_place(input_lines: &mut Vec<Line>) -> Result<Polyline, PolylineError> {
        let (polyline, writes_through) = Polyline::build(input_lines.clone())?;
        if writes_through {
            input_lines.clone_from(&polyline.lines);
        }
        Ok(polyline)
    }

                        fn build(input_lines: Vec<Line>) -> Result<(Polyline, bool), PolylineError> {
        let input_len = input_lines.len();
        let filtered_lines = remove_consecutive_parallel_lines(input_lines);
        let mut filtered_lines = remove_overlaps(filtered_lines)?;
        if filtered_lines.len() < 3 {
            return Ok((Polyline { lines: Vec::new() }, false));
        }
        let writes_through = filtered_lines.len() == input_len;

        for i in 1..filtered_lines.len() - 1 {
            let corner = filtered_lines[i].intersection_approx(&filtered_lines[i + 1]);
            let side_of_line = filtered_lines[i - 1].side_of_float_exact(&corner);
            if side_of_line != Side::Collinear {
                let d0 = filtered_lines[i - 1].direction();
                let d1 = filtered_lines[i].direction();
                let side1 = d0.side_of(&d1);
                if side1 != side_of_line {
                    filtered_lines[i] = filtered_lines[i].opposite();
                }
            }
        }
        Ok((
            Polyline {
                lines: filtered_lines,
            },
            writes_through,
        ))
    }
}

fn int_point_of(point: &Point) -> IntPoint {
    match point {
        Point::Int(p) => *p,
        Point::Rational(_) => panic!(
            "Polyline: only implemented for IntPoints till now (Polyline.java:42, Line.java:19-27)"
        ),
    }
}

fn line_from_direction(a: IntPoint, dir: &Direction) -> Line {
    Line::from_direction_any(a, dir)
        .expect("a Direction derived from IntPoints is always an IntDirection")
}

fn remove_consecutive_parallel_lines(lines: Vec<Line>) -> Vec<Line> {
    if lines.len() < 3 {
        return lines;
    }
    let mut tmp_arr: Vec<Line> = Vec::with_capacity(lines.len());
    tmp_arr.push(lines[0]);
    for line in lines.iter().skip(1) {
        if !tmp_arr[tmp_arr.len() - 1].is_parallel(line) {
            tmp_arr.push(*line);
        }
    }
    let new_length = tmp_arr.len();
    if new_length == lines.len() {
        return lines;
    }
    if new_length < 3 {
        return Vec::new();
    }
    tmp_arr
}

fn remove_overlaps(lines: Vec<Line>) -> Result<Vec<Line>, PolylineError> {
    if lines.len() < 4 {
        return Ok(lines);
    }
    let mut new_length: usize = 0;
    let mut tmp_arr: Vec<Line> = vec![Line::new(IntPoint::ZERO, IntPoint::ZERO); lines.len()];
    tmp_arr[0] = lines[0];
    if !lines[0].is_equal_or_opposite(&lines[2]) {
        new_length += 1;
    }
    tmp_arr[new_length] = lines[1];
    new_length += 1;
    for i in 2..lines.len() - 2 {
        if new_length >= 1 && tmp_arr[new_length - 1].is_equal_or_opposite(&lines[i + 1]) {
            new_length -= 1;
        } else {
            tmp_arr[new_length] = lines[i];
            new_length += 1;
        }
    }
    tmp_arr[new_length] = lines[lines.len() - 2];
    new_length += 1;
    if new_length >= 2 && !lines[lines.len() - 1].is_equal_or_opposite(&tmp_arr[new_length - 2]) {
        tmp_arr[new_length] = lines[lines.len() - 1];
        new_length += 1;
    }
    if new_length == lines.len() {
        return Ok(lines);
    }
    if new_length < 3 {
        return Ok(Vec::new());
    }
    tmp_arr.truncate(new_length);
    Ok(tmp_arr)
}


impl Polyline {
        pub fn lines(&self) -> &[Line] {
        &self.lines
    }

                pub fn corner_count(&self) -> usize {
        self.lines.len().saturating_sub(1)
    }

        pub fn is_empty(&self) -> bool {
        self.lines.len() < 3
    }

            pub fn is_point(&self) -> bool {
        if self.lines.len() < 3 {
            return true;
        }
        let first_corner = self.corner_at(0);
        for i in 1..self.lines.len() - 1 {
            if self.corner_at(i) != first_corner {
                return false;
            }
        }
        true
    }

        pub fn is_orthogonal(&self) -> bool {
        self.lines.iter().all(Line::is_orthogonal)
    }

        pub fn is_multiple_of_45_degree(&self) -> bool {
        self.lines.iter().all(Line::is_multiple_of_45_degree)
    }

        pub fn first_corner(&self) -> Option<Point> {
        self.corner(0)
    }

            pub fn last_corner(&self) -> Option<Point> {
        if self.lines.len() < 2 {
            return None;
        }
        self.corner(self.lines.len() - 2)
    }

        pub fn corners(&self) -> Vec<Point> {
        if self.lines.len() < 2 {
            return Vec::new();
        }
        (0..self.lines.len() - 1)
            .map(|i| self.corner_at(i))
            .collect()
    }

            pub fn corner_approx_arr(&self) -> Vec<FloatPoint> {
        if self.lines.len() < 2 {
            return Vec::new();
        }
        (0..self.lines.len() - 1)
            .map(|i| self.corner_approx_at(i))
            .collect()
    }

                        pub fn corner_approx(&self, no: usize) -> Option<FloatPoint> {
        if self.lines.len() < 2 {
            return None;
        }
        Some(self.corner_approx_at(no))
    }

            pub fn corner(&self, no: usize) -> Option<Point> {
        if self.lines.len() < 2 {
            return None;
        }
        Some(self.corner_at(no))
    }

            fn corner_at(&self, no: usize) -> Point {
        let no = self.clamp_corner_index(no as i64);
        self.lines[no].intersection(&self.lines[no + 1])
    }

        fn corner_approx_at(&self, no: usize) -> FloatPoint {
        let no = self.clamp_corner_index(no as i64);
        self.lines[no].intersection_approx(&self.lines[no + 1])
    }

                fn clamp_corner_index(&self, corner_index: i64) -> usize {
        debug_assert!(self.lines.len() >= 2);
        if corner_index < 0 {
            0
        } else if corner_index >= self.lines.len() as i64 - 1 {
            self.lines.len() - 2
        } else {
            corner_index as usize
        }
    }
}


impl Polyline {
                pub fn reverse(&self) -> Result<Polyline, PolylineError> {
        let reversed: Vec<Line> = self.lines.iter().rev().map(Line::opposite).collect();
        Polyline::from_lines(reversed)
    }

            pub fn length_approx_between(&self, from_corner: usize, to_corner: usize) -> f64 {
        if self.lines.len() < 2 {
            return 0.0;
        }
        let to_corner = to_corner.min(self.lines.len() - 2);
        let mut result = 0.0;
        let mut i = from_corner;
        while i < to_corner {
            result += self
                .corner_approx_at(i + 1)
                .distance(&self.corner_approx_at(i));
            i += 1;
        }
        result
    }

        pub fn length_approx(&self) -> f64 {
        if self.lines.len() < 2 {
            return 0.0;
        }
        self.length_approx_between(0, self.lines.len() - 2)
    }
}


impl Polyline {
                pub fn offset_shapes(&self, half_width: i32) -> Vec<TileShape> {
        if self.lines.is_empty() {
            return Vec::new();
        }
        self.offset_shapes_between(half_width, 0, self.lines.len() - 1)
    }

                pub fn offset_shapes_between(
        &self,
        half_width: i32,
        from_no: usize,
        to_no: usize,
    ) -> Vec<TileShape> {
        if self.lines.is_empty() {
            return Vec::new();
        }
        let to_no = to_no.min(self.lines.len() - 1);
        let shape_count = (to_no as i64 - from_no as i64 - 1).max(0) as usize;
        let mut shapes: Vec<TileShape> = Vec::with_capacity(shape_count);
        if shape_count == 0 {
            return shapes;
        }
        let mut prev_dir = self.lines[from_no].direction().get_vector();
        let mut current_direction = self.lines[from_no + 1].direction().get_vector();
        for i in from_no + 1..to_no {
            let next_dir = self.lines[i + 1].direction().get_vector();

            let mut offset_lines: [Line; 4] = [Line::new(IntPoint::ZERO, IntPoint::ZERO); 4];

            offset_lines[0] = self.lines[i].translate(-half_width as f64);

            let next_dir_from_curr_dir = next_dir.side_of(&current_direction);
            if next_dir_from_curr_dir == Side::OnTheLeft {
                offset_lines[1] = self.lines[i + 1].translate(-half_width as f64);
            } else {
                offset_lines[1] = self.lines[i + 1].opposite().translate(-half_width as f64);
            }

            offset_lines[2] = self.lines[i].opposite().translate(-half_width as f64);

            let current_dir_from_prev_dir = current_direction.side_of(&prev_dir);
            if current_dir_from_prev_dir == Side::OnTheLeft {
                offset_lines[3] = self.lines[i - 1].translate(-half_width as f64);
            } else {
                offset_lines[3] = self.lines[i - 1].opposite().translate(-half_width as f64);
            }
            let mut corner_to_check: Option<FloatPoint> = None;
            let mut current_line = offset_lines[1];
            let mut check_line = if next_dir_from_curr_dir == Side::OnTheLeft {
                offset_lines[2]
            } else {
                offset_lines[0]
            };
            let mut check_distance_corner = self.corner_approx_at(i);
            let check_dist_square = 2.0 * half_width as f64 * half_width as f64;
            let mut cut_dog_ear_lines: Vec<Line> = Vec::new();
            let mut tmp_curr_dir = next_dir;
            let mut direction_changed = false;
            for j in i + 2..self.lines.len() - 1 {
                if self
                    .corner_approx_at(j - 1)
                    .distance_square(&check_distance_corner)
                    > check_dist_square
                {
                    break;
                }
                if !direction_changed {
                    corner_to_check = Some(current_line.intersection_approx(&check_line));
                }
                let tmp_next_dir = self.lines[j].direction().get_vector();
                let tmp_next_dir_from_tmp_curr_dir = tmp_next_dir.side_of(&tmp_curr_dir);
                direction_changed = tmp_next_dir_from_tmp_curr_dir != next_dir_from_curr_dir;
                if !direction_changed {
                    let next_border_line = if tmp_next_dir_from_tmp_curr_dir == Side::OnTheLeft {
                        self.lines[j].translate(-half_width as f64)
                    } else {
                        self.lines[j].opposite().translate(-half_width as f64)
                    };

                    let corner = corner_to_check.expect("assigned on the first pass");
                    if next_border_line.side_of_float_exact(&corner) == Side::OnTheLeft
                        && next_border_line.side_of(&self.corner_at(i)) == Side::OnTheRight
                        && next_border_line.side_of(&self.corner_at(i - 1)) == Side::OnTheRight
                    {
                        cut_dog_ear_lines.push(next_border_line);
                    }
                    tmp_curr_dir = tmp_next_dir;
                    current_line = next_border_line;
                }
            }
            check_distance_corner = self.corner_approx_at(i - 1);
            check_line = if current_dir_from_prev_dir == Side::OnTheLeft {
                offset_lines[2]
            } else {
                offset_lines[0]
            };
            current_line = offset_lines[3];
            tmp_curr_dir = prev_dir;
            direction_changed = false;
            let mut j = i as i64 - 2;
            while j >= 1 {
                let ju = j as usize;
                if self
                    .corner_approx_at(ju)
                    .distance_square(&check_distance_corner)
                    > check_dist_square
                {
                    break;
                }
                if !direction_changed {
                    corner_to_check = Some(current_line.intersection_approx(&check_line));
                }
                let tmp_prev_dir = self.lines[ju].direction().get_vector();
                let tmp_curr_dir_from_tmp_prev_dir = tmp_curr_dir.side_of(&tmp_prev_dir);
                direction_changed = tmp_curr_dir_from_tmp_prev_dir != current_dir_from_prev_dir;
                if !direction_changed {
                    let prev_border_line = if tmp_curr_dir.side_of(&tmp_prev_dir) == Side::OnTheLeft
                    {
                        self.lines[ju].translate(-half_width as f64)
                    } else {
                        self.lines[ju].opposite().translate(-half_width as f64)
                    };
                    let corner = corner_to_check.expect("assigned on the first pass");
                    if prev_border_line.side_of_float_exact(&corner) == Side::OnTheLeft
                        && prev_border_line.side_of(&self.corner_at(i)) == Side::OnTheRight
                        && prev_border_line.side_of(&self.corner_at(i - 1)) == Side::OnTheRight
                    {
                        cut_dog_ear_lines.push(prev_border_line);
                    }
                    tmp_curr_dir = tmp_prev_dir;
                    current_line = prev_border_line;
                }
                j -= 1;
            }
            let mut s1 = TileShape::get_instance_from_lines(offset_lines.to_vec());
            if !cut_dog_ear_lines.is_empty() {
                s1 = s1.intersection(&TileShape::get_instance_from_lines(cut_dog_ear_lines));
            }
            let bounding_shape = if USE_BOUNDING_OCTAGON_FOR_OFFSET_SHAPES {
                let surr_oct = self.bounding_octagon_between(i - 1, i);
                TileShape::Octagon(surr_oct.offset(half_width as f64))
            } else {
                let surr_box = self.bounding_box_between(i - 1, i);
                let offset_box = surr_box.offset(half_width as f64);
                TileShape::Simplex(offset_box.to_simplex())
            };
            shapes.push(bounding_shape.intersection_with_simplify(&s1));

            prev_dir = current_direction;
            current_direction = next_dir;
        }
        shapes
    }

                pub fn offset_shape(&self, half_width: i32, no: usize) -> Option<TileShape> {
        if no + 3 > self.lines.len() {
            // Java: FRLogger.warn("Polyline.offsetShape: no out of range")
            return None;
        }
        let result = self.offset_shapes_between(half_width, no, no + 2);
        result.into_iter().next()
    }

                        pub fn offset_box(&self, half_width: i32, no: usize) -> Option<IntBox> {
        let current_line_segment = LineSegment::from_polyline(self, no + 1)?;
        Some(
            current_line_segment
                .bounding_box()
                .offset(half_width as f64),
        )
    }
}


impl Polyline {
                    pub fn translate_by(&self, vector: &Vector) -> Result<Polyline, PolylineError> {
        if *vector == Vector::ZERO {
            return Ok(self.clone());
        }
        let v: IntVector = match vector {
            Vector::Int(v) => *v,
            Vector::Rational(_) => {
                panic!("Polyline::translate_by: only implemented for Vector::Int")
            }
        };
        Polyline::from_lines(self.lines.iter().map(|l| l.translate_by(&v)).collect())
    }

            pub fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> Result<Polyline, PolylineError> {
        Polyline::from_lines(
            self.lines
                .iter()
                .map(|l| l.turn_90_degree(factor, pole))
                .collect(),
        )
    }

        pub fn rotate_approx(&self, angle: f64, pole: &FloatPoint) -> Polyline {
        if angle == 0.0 {
            return self.clone();
        }
        let new_corners: Vec<Point> = (0..self.corner_count())
            .map(|i| Point::Int(self.corner_approx_at(i).rotate(angle, pole).round()))
            .collect();
        Polyline::from_points(&new_corners)
    }

        pub fn mirror_vertical(&self, pole: &IntPoint) -> Result<Polyline, PolylineError> {
        Polyline::from_lines(self.lines.iter().map(|l| l.mirror_vertical(pole)).collect())
    }

        pub fn mirror_horizontal(&self, pole: &IntPoint) -> Result<Polyline, PolylineError> {
        Polyline::from_lines(
            self.lines
                .iter()
                .map(|l| l.mirror_horizontal(pole))
                .collect(),
        )
    }
}


impl Polyline {
            pub fn bounding_box_between(&self, from_corner_no: usize, to_corner_no: usize) -> IntBox {
        let mut llx = i32::MAX as f64;
        let mut lly = llx;
        let mut urx = i32::MIN as f64;
        let mut ury = urx;
        if self.lines.len() >= 2 {
            let to_corner_no = to_corner_no.min(self.lines.len() - 2);
            let mut i = from_corner_no;
            while i <= to_corner_no {
                let current_corner = self.corner_approx_at(i);
                llx = java_min(llx, current_corner.x);
                lly = java_min(lly, current_corner.y);
                urx = java_max(urx, current_corner.x);
                ury = java_max(ury, current_corner.y);
                i += 1;
            }
        }
        let lower_left = IntPoint::new(llx.floor() as i32, lly.floor() as i32);
        let upper_right = IntPoint::new(urx.ceil() as i32, ury.ceil() as i32);
        IntBox::new(lower_left, upper_right)
    }

            pub fn bounding_box(&self) -> IntBox {
        self.bounding_box_between(0, self.corner_count().saturating_sub(1))
    }

            pub fn bounding_octagon_between(
        &self,
        from_corner_no: usize,
        to_corner_no: usize,
    ) -> IntOctagon {
        let mut lx = i32::MAX as f64;
        let mut ly = i32::MAX as f64;
        let mut rx = i32::MIN as f64;
        let mut uy = i32::MIN as f64;
        let mut ulx = i32::MAX as f64;
        let mut lrx = i32::MIN as f64;
        let mut llx = i32::MAX as f64;
        let mut urx = i32::MIN as f64;
        if self.lines.len() >= 2 {
            let to_corner_no = to_corner_no.min(self.lines.len() - 2);
            let mut i = from_corner_no;
            while i <= to_corner_no {
                let current = self.corner_approx_at(i);
                lx = java_min(lx, current.x);
                ly = java_min(ly, current.y);
                rx = java_max(rx, current.x);
                uy = java_max(uy, current.y);
                let mut tmp = current.x - current.y;
                ulx = java_min(ulx, tmp);
                lrx = java_max(lrx, tmp);
                tmp = current.x + current.y;
                llx = java_min(llx, tmp);
                urx = java_max(urx, tmp);
                i += 1;
            }
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

                pub fn nearest_point_approx(&self, from_point: &FloatPoint) -> Option<FloatPoint> {
        let mut min_distance = f64::MAX;
        let mut nearest_point: Option<FloatPoint> = None;
        let corners = self.corner_approx_arr();
        for corner in &corners {
            let current_distance = corner.distance(from_point);
            if current_distance < min_distance {
                min_distance = current_distance;
                nearest_point = Some(*corner);
            }
        }
        const CTOLERANCE: f64 = 1.0;
        for i in 1..self.lines.len().saturating_sub(1) {
            let projection = from_point.projection_approx(&self.lines[i]);
            let current_distance = projection.distance(from_point);
            if current_distance < min_distance {
                let segment_length = corners[i].distance(&corners[i - 1]);
                if projection.distance(&corners[i]) + projection.distance(&corners[i - 1])
                    < segment_length + CTOLERANCE
                {
                    min_distance = current_distance;
                    nearest_point = Some(projection);
                }
            }
        }
        nearest_point
    }

                        pub fn distance(&self, from_point: &FloatPoint) -> f64 {
        match self.nearest_point_approx(from_point) {
            Some(p) => from_point.distance(&p),
            None => f64::MAX,
        }
    }
}


impl Polyline {
                                    pub fn combine(&self, other: &Polyline) -> Result<Polyline, PolylineError> {
        if self.lines.len() < 3 || other.lines.len() < 3 {
            return Ok(self.clone());
        }
        let (combine_at_start, combine_other_at_start) =
            if self.first_corner() == other.first_corner() {
                (true, true)
            } else if self.first_corner() == other.last_corner() {
                (true, false)
            } else if self.last_corner() == other.first_corner() {
                (false, true)
            } else if self.last_corner() == other.last_corner() {
                (false, false)
            } else {
                return Ok(self.clone()); 
            };
        let mut new_lines: Vec<Line> = Vec::with_capacity(self.lines.len() + other.lines.len() - 2);
        if combine_at_start {
            if combine_other_at_start {
                for i in 0..other.lines.len() - 1 {
                    new_lines.push(other.lines[other.lines.len() - i - 1].opposite());
                }
            } else {
                new_lines.extend_from_slice(&other.lines[..other.lines.len() - 1]);
            }
            new_lines.extend_from_slice(&self.lines[1..]);
        } else {
            new_lines.extend_from_slice(&self.lines[..self.lines.len() - 1]);
            if combine_other_at_start {
                new_lines.extend_from_slice(&other.lines[1..]);
            } else {
                for i in 1..other.lines.len() {
                    new_lines.push(other.lines[other.lines.len() - i - 1].opposite());
                }
            }
        }
        Polyline::from_lines(new_lines)
    }

                            pub fn split(
        &self,
        line_index: usize,
        end_line: &Line,
    ) -> Result<Option<[Polyline; 2]>, PolylineError> {
        if line_index < 1 || line_index + 2 > self.lines.len() {
            // Java: FRLogger.warn("Polyline.split: lineIndex out of range")
            return Ok(None);
        }
        if self.lines[line_index].is_parallel(end_line) {
            return Ok(None);
        }
        let new_end_corner = self.lines[line_index].intersection(end_line);
        if (line_index == 1 && Some(&new_end_corner) == self.first_corner().as_ref())
            || (line_index + 2 >= self.lines.len()
                && Some(&new_end_corner) == self.last_corner().as_ref())
        {
            return Ok(None);
        }
        let mut first_piece: Vec<Line>;
        if self.corner_at(line_index - 1) == new_end_corner {
            first_piece = self.lines[..line_index + 1].to_vec();
        } else {
            first_piece = Vec::with_capacity(line_index + 2);
            first_piece.extend_from_slice(&self.lines[..line_index + 1]);
            first_piece.push(*end_line);
        }
        let mut second_piece: Vec<Line>;
        if self.corner_at(line_index) == new_end_corner {
            second_piece = self.lines[line_index..].to_vec();
        } else {
            second_piece = Vec::with_capacity(self.lines.len() - line_index + 1);
            second_piece.push(*end_line);
            second_piece.extend_from_slice(&self.lines[line_index..]);
        }
        let result = [
            Polyline::from_lines(std::mem::take(&mut first_piece))?,
            Polyline::from_lines(std::mem::take(&mut second_piece))?,
        ];
        if result[0].is_point() || result[1].is_point() {
            return Ok(None);
        }
        Ok(Some(result))
    }

            pub fn skip_lines(&self, from_no: usize, to_no: usize) -> Result<Polyline, PolylineError> {
        self.skip_lines_i64(from_no as i64, to_no as i64)
    }

            fn skip_lines_i64(&self, from_no: i64, to_no: i64) -> Result<Polyline, PolylineError> {
        if from_no < 0 || to_no > self.lines.len() as i64 - 1 || from_no > to_no {
            return Ok(self.clone());
        }
        let (from_no, to_no) = (from_no as usize, to_no as usize);
        let mut new_lines: Vec<Line> = Vec::with_capacity(self.lines.len() - (to_no - from_no + 1));
        new_lines.extend_from_slice(&self.lines[..from_no]);
        new_lines.extend_from_slice(&self.lines[to_no + 1..]);
        Polyline::from_lines(new_lines)
    }

        pub fn contains(&self, point: &Point) -> bool {
        for i in 1..self.lines.len().saturating_sub(1) {
            if let Some(current_segment) = LineSegment::from_polyline(self, i)
                && current_segment.contains(point)
            {
                return true;
            }
        }
        false
    }
}


impl Polyline {
                                                pub fn projection_line(&self, point: &Point) -> Option<LineSegment> {
        let from_point = point.to_float();
        let mut min_distance = f64::MAX;
        let mut result_line: Option<Line> = None;
        let mut nearest_line: Option<Line> = None;
        for i in 1..self.lines.len().saturating_sub(1) {
            let projection = from_point.projection_approx(&self.lines[i]);
            let current_distance = projection.distance(&from_point);
            if current_distance < min_distance {
                let Some(direction_towards_line) = self.lines[i].perpendicular_direction(point)
                else {
                    continue;
                };
                let Some(current_result_line) =
                    Line::from_direction_any(int_point_of(point), &direction_towards_line)
                else {
                    continue;
                };
                let prev_corner = self.corner_at(i - 1);
                let next_corner = self.corner_at(i);
                let prev_corner_side = current_result_line.side_of(&prev_corner);
                let next_corner_side = current_result_line.side_of(&next_corner);
                if prev_corner_side == next_corner_side && prev_corner_side != Side::Collinear {
                    continue;
                }
                nearest_line = Some(self.lines[i]);
                min_distance = current_distance;
                result_line = Some(current_result_line);
            }
        }
        let nearest_line = nearest_line?;
        let start_line = Line::from_direction(int_point_of(point), &nearest_line.direction());
        Some(LineSegment::new(
            start_line,
            result_line.expect("set together with nearest_line"),
            nearest_line,
        ))
    }

                pub fn shorten(
        &self,
        new_line_count: usize,
        last_segment_length: f64,
    ) -> Result<Polyline, PolylineError> {
        let last_corner = self.corner_approx_at_i64(new_line_count as i64 - 2);
        let prev_last_corner = self.corner_approx_at_i64(new_line_count as i64 - 3);
        let new_last_corner = prev_last_corner
            .change_length(&last_corner, last_segment_length)
            .round();
        if self.corner_at_i64(self.corner_count() as i64 - 2) == Point::Int(new_last_corner) {
            return self.skip_lines_i64(new_line_count as i64 - 1, new_line_count as i64 - 1);
        }
        let mut new_lines: Vec<Line> = Vec::with_capacity(new_line_count);
        new_lines.extend_from_slice(&self.lines[..new_line_count - 2]);
        let mut first_line_point = self.lines[new_line_count - 2].a;
        if first_line_point == new_last_corner {
            first_line_point = self.lines[new_line_count - 2].b;
        }
        let new_prev_last_line = Line::new(first_line_point, new_last_corner);
        new_lines.push(new_prev_last_line);
        new_lines.push(Line::from_direction(
            new_last_corner,
            &new_prev_last_line.direction().turn_45_degree(6),
        ));
        Polyline::from_lines(new_lines)
    }

            fn corner_approx_at_i64(&self, corner_index: i64) -> FloatPoint {
        let no = self.clamp_corner_index(corner_index);
        self.lines[no].intersection_approx(&self.lines[no + 1])
    }

        fn corner_at_i64(&self, corner_index: i64) -> Point {
        let no = self.clamp_corner_index(corner_index);
        self.lines[no].intersection(&self.lines[no + 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::float_point::FloatPoint;
    use crate::int_box::IntBox;
    use crate::int_point::IntPoint;
    use crate::line::Line;
    use crate::point::Point;

    fn pts(v: &[(i32, i32)]) -> Vec<Point> {
        v.iter()
            .map(|&(x, y)| Point::Int(IntPoint::new(x, y)))
            .collect()
    }
    fn l_shape() -> Polyline {
        Polyline::from_points(&pts(&[(0, 0), (10, 0), (10, 10)]))
    }

    #[test]
    fn corners_roundtrip_and_length() {
        let p = l_shape();
        assert_eq!(p.corner_count(), 3);
        assert_eq!(p.lines().len(), 4); 
        assert_eq!(p.corners(), pts(&[(0, 0), (10, 0), (10, 10)]));
        assert_eq!(p.first_corner().unwrap(), Point::Int(IntPoint::new(0, 0)));
        assert_eq!(p.last_corner().unwrap(), Point::Int(IntPoint::new(10, 10)));
        assert_eq!(p.length_approx(), 20.0);
        assert!(p.is_orthogonal());
        assert!(p.is_multiple_of_45_degree());
        assert!(!p.is_point());
        assert_eq!(
            p.reverse().unwrap().first_corner().unwrap(),
            Point::Int(IntPoint::new(10, 10))
        );
        assert_eq!(p.bounding_box(), IntBox::from_coords(0, 0, 10, 10));
    }

    #[test]
    fn the_closing_lines_are_perpendicular_to_the_end_segments() {
        let p = l_shape();
        assert_eq!(p.lines()[0], Line::from_coords(0, 0, 0, 1));
        assert_eq!(p.lines()[1], Line::from_coords(0, 0, 10, 0));
        assert_eq!(p.lines()[2], Line::from_coords(10, 0, 10, 10));
        assert_eq!(p.lines()[3], Line::from_coords(10, 10, 11, 10));
        let two = Polyline::from_two_points(
            &Point::Int(IntPoint::new(0, 0)),
            &Point::Int(IntPoint::new(10, 0)),
        );
        assert_eq!(two.lines().len(), 3);
        assert_eq!(two.lines()[2], Line::from_coords(10, 0, 10, 1));
        assert_eq!(
            Polyline::from_points(&pts(&[(0, 0), (10, 0)])).lines()[2],
            Line::from_coords(10, 0, 10, -1)
        );
        assert!(
            Polyline::from_two_points(
                &Point::Int(IntPoint::new(1, 1)),
                &Point::Int(IntPoint::new(1, 1))
            )
            .is_empty()
        );
    }

    #[test]
    fn collinear_middle_corner_is_dropped() {
        let p = Polyline::from_points(&pts(&[(0, 0), (5, 0), (10, 0)]));
        assert_eq!(p.corner_count(), 2);
        let q = Polyline::from_points(&pts(&[(0, 0), (0, 0), (10, 0)]));
        assert_eq!(q.corner_count(), 2);
    }

    #[test]
    fn degenerate_polylines_are_empty() {
        let p = Polyline::from_points(&pts(&[(0, 0)]));
        assert!(p.is_empty());
        assert!(p.is_point());
        assert_eq!(p.corner_count(), 0);
        assert_eq!(p.corners(), Vec::<Point>::new());
        assert_eq!(p.corner(0), None);
        assert_eq!(p.first_corner(), None);
        assert_eq!(p.last_corner(), None);
        assert_eq!(p.corner_approx(0), None);
        assert_eq!(p.length_approx(), 0.0);
        assert_eq!(p.offset_shapes(3).len(), 0);
        assert_eq!(p.distance(&FloatPoint::new(0.0, 0.0)), f64::MAX);
    }

    #[test]
    fn offset_shapes_cover_segments() {
        let p = l_shape();
        let shapes = p.offset_shapes(2);
        assert_eq!(shapes.len(), 2);
        assert!(shapes[0].contains(&Point::Int(IntPoint::new(5, 1))));
        assert!(shapes[0].contains(&Point::Int(IntPoint::new(0, 0))));
        assert!(!shapes[0].contains(&Point::Int(IntPoint::new(5, 4))));
        assert_eq!(
            p.offset_box(2, 0).unwrap(),
            IntBox::from_coords(-2, -2, 12, 2)
        );
        assert_eq!(
            p.offset_box(2, 1).unwrap(),
            IntBox::from_coords(8, -2, 12, 12)
        );
        assert_eq!(
            shapes[0],
            TileShape::Octagon(IntOctagon::new(-2, -2, 12, 2, -3, 13, -3, 13))
        );
        assert_eq!(
            shapes[1],
            TileShape::Octagon(IntOctagon::new(8, -2, 12, 12, -3, 13, 7, 23))
        );
        assert_eq!(p.offset_shape(2, 0).unwrap(), shapes[0]);
        assert_eq!(p.offset_shape(2, 1).unwrap(), shapes[1]);
        assert_eq!(p.offset_shape(2, 2), None);
    }

    #[test]
    fn combine_and_split() {
        let a = Polyline::from_points(&pts(&[(0, 0), (10, 0)]));
        let b = Polyline::from_points(&pts(&[(10, 0), (10, 10)]));
        let c = a.combine(&b).unwrap();
        assert_eq!(c.corners(), pts(&[(0, 0), (10, 0), (10, 10)]));
        let d = Polyline::from_points(&pts(&[(50, 50), (60, 50)]));
        assert_eq!(a.combine(&d), Ok(a));
        let parts = l_shape()
            .split(1, &Line::from_coords(5, 0, 5, 1))
            .unwrap()
            .expect("splits");
        assert_eq!(parts[0].corners(), pts(&[(0, 0), (5, 0)]));
        assert_eq!(parts[1].corners(), pts(&[(5, 0), (10, 0), (10, 10)]));
        assert_eq!(l_shape().split(1, &Line::from_coords(5, 0, 6, 0)), Ok(None));
        assert_eq!(l_shape().split(1, &Line::from_coords(0, 0, 0, 1)), Ok(None));
        assert_eq!(l_shape().split(0, &Line::from_coords(5, 0, 5, 1)), Ok(None));
    }

    #[test]
    fn nearest_point_distance_contains() {
        let p = l_shape();
        assert_eq!(
            p.nearest_point_approx(&FloatPoint::new(5.0, 3.0)).unwrap(),
            FloatPoint::new(5.0, 0.0)
        );
        assert_eq!(p.distance(&FloatPoint::new(5.0, 3.0)), 3.0);
        assert!(p.contains(&Point::Int(IntPoint::new(10, 4))));
        assert!(!p.contains(&Point::Int(IntPoint::new(4, 4))));
        let proj = p
            .projection_line(&Point::Int(IntPoint::new(12, 4)))
            .unwrap();
        assert_eq!(proj.start_point(), Point::Int(IntPoint::new(12, 4)));
        assert_eq!(proj.end_point(), Point::Int(IntPoint::new(10, 4)));
    }

    #[test]
    fn transformations() {
        let p = l_shape();
        assert_eq!(
            p.translate_by(&crate::vector::Vector::new(1, 1))
                .unwrap()
                .bounding_box(),
            IntBox::from_coords(1, 1, 11, 11)
        );
        assert_eq!(
            p.turn_90_degree(1, &IntPoint::new(0, 0))
                .unwrap()
                .bounding_box(),
            IntBox::from_coords(-10, 0, 0, 10)
        );
        assert_eq!(
            p.mirror_vertical(&IntPoint::new(0, 0))
                .unwrap()
                .bounding_box(),
            IntBox::from_coords(-10, 0, 0, 10)
        );
        assert_eq!(
            p.mirror_horizontal(&IntPoint::new(0, 0))
                .unwrap()
                .bounding_box(),
            IntBox::from_coords(0, -10, 10, 0)
        );
        assert_eq!(p.skip_lines(0, 0).unwrap().corner_count(), 2);
        assert_eq!(p.skip_lines(0, 4), Ok(p.clone()));
        assert_eq!(p.skip_lines(2, 1), Ok(p.clone()));
        assert_eq!(p.translate_by(&Vector::ZERO), Ok(p.clone()));
        assert_eq!(p.rotate_approx(0.0, &FloatPoint::new(0.0, 0.0)), p);
    }

    #[test]
    fn bounding_octagon_between_matches_java() {
        assert_eq!(
            l_shape().bounding_octagon_between(0, 2),
            IntOctagon::new(0, 0, 10, 10, 0, 10, 0, 20)
        );
    }

    #[test]
    fn shorten_reduces_lines() {
        let p = Polyline::from_points(&pts(&[(0, 0), (10, 0), (10, 10), (20, 10)]));
        let s = p.shorten(3, 5.0).unwrap();
        assert_eq!(s.lines().len(), 3);
        assert!(s.length_approx() < p.length_approx());
        assert_eq!(s.corners(), pts(&[(0, 0), (5, 0)]));
        assert_eq!(s.length_approx(), 5.0);
    }

    #[test]
    fn from_lines_skips_parallel_and_overlapping_lines() {
        let p = Polyline::from_lines(vec![
            Line::from_coords(0, 0, 0, 1),
            Line::from_coords(0, 0, 10, 0),
            Line::from_coords(3, 0, 13, 0), 
            Line::from_coords(10, 0, 10, 10),
            Line::from_coords(10, 10, 11, 10),
        ])
        .unwrap();
        assert_eq!(p.lines().len(), 4);
        assert_eq!(p.corners(), pts(&[(0, 0), (10, 0), (10, 10)]));
        assert!(
            Polyline::from_lines(vec![
                Line::from_coords(0, 0, 1, 0),
                Line::from_coords(2, 0, 3, 0),
                Line::from_coords(4, 0, 5, 0),
            ])
            .unwrap()
            .is_empty()
        );
        let degenerate = Polyline::from_lines(vec![
            Line::from_coords(0, 0, 1, 0),
            Line::from_coords(0, 0, 0, 1),
            Line::from_coords(0, 0, 1, 0),
            Line::from_coords(0, 0, 0, 1),
            Line::from_coords(0, 0, 1, 0),
            Line::from_coords(0, 0, 0, 1),
        ])
        .expect("the tmpArr[-1] read is guarded");
        assert!(degenerate.is_empty());
        assert_eq!(degenerate.lines().len(), 0);
    }

                            #[test]
    fn from_lines_in_place_writes_the_normalised_lines_back_to_the_caller() {
        let base = Polyline::from_points(&pts(&[(0, 0), (10000, 0)]));
        let mut arr = vec![base.lines()[0], base.lines()[1].opposite(), base.lines()[2]];
        let handed_in = arr.clone();

        let polyline = Polyline::from_lines_in_place(&mut arr).expect("normalises");

        assert_eq!(polyline.lines().len(), 3);
        assert_eq!(arr, polyline.lines());
        for (caller, built) in arr.iter().zip(polyline.lines()) {
            assert!(caller.is_same_object(built));
        }
        assert!(!arr[1].is_same_object(&handed_in[1]));
        assert_ne!(arr[1], handed_in[1]);
        assert_eq!(arr[1], base.lines()[1]);
        assert!(arr[0].is_same_object(&handed_in[0]));
        assert!(arr[2].is_same_object(&handed_in[2]));

        let mut same_input = handed_in.clone();
        assert_eq!(
            Polyline::from_lines(same_input.clone()).expect("normalises"),
            polyline
        );
        same_input.clone_from(&handed_in);
        assert!(same_input[1].is_same_object(&handed_in[1]));
    }

                #[test]
    fn from_lines_in_place_leaves_the_caller_alone_when_a_line_is_skipped() {
        let mut arr = vec![
            Line::from_coords(0, 0, 0, 1),
            Line::from_coords(0, 0, 10000, 0),
            Line::from_coords(0, 100, 10000, 100),
        ];
        let handed_in = arr.clone();
        let polyline = Polyline::from_lines_in_place(&mut arr).expect("no underflow");
        assert!(polyline.lines().is_empty());
        for (after, before) in arr.iter().zip(&handed_in) {
            assert!(after.is_same_object(before));
        }
    }
}
