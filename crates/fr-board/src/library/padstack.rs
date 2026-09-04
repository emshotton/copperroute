use std::cmp::Ordering;
use std::fmt;

use fr_geometry::{Direction, IntDirection, Shape, ShapeOps, TileShape};

use crate::ids::PadstackId;
use crate::rules::{PadstackLookup, compare_to_ignore_case, equals_ignore_case};
use crate::structure::LayerStructure;

#[derive(Debug, Clone, PartialEq)]
pub struct Padstack {
        pub name: String,
            pub no: usize,
            pub attach_allowed: bool,
            pub placed_absolute: bool,
            shapes: Vec<Option<Shape>>,
            pub hole_only: bool,
}

impl Padstack {
                pub(crate) fn new(
        name: String,
        no: usize,
        shapes: Vec<Option<Shape>>,
        attach_allowed: bool,
        placed_absolute: bool,
    ) -> Padstack {
        Padstack {
            name,
            no,
            attach_allowed,
            placed_absolute,
            shapes,
            hole_only: false,
        }
    }

                pub fn compare_to(&self, other: &Padstack) -> Ordering {
        compare_to_ignore_case(&self.name, &other.name)
    }

                pub fn get_shape(&self, layer: i32) -> Option<&Shape> {
        if layer < 0 || layer as usize >= self.shapes.len() {
            return None;
        }
        self.shapes[layer as usize].as_ref()
    }

            pub fn from_layer(&self) -> i32 {
        let mut result = 0usize;
        while result < self.shapes.len() && self.shapes[result].is_none() {
            result += 1;
        }
        result as i32
    }

            pub fn to_layer(&self) -> i32 {
        let mut result = self.shapes.len() as i32 - 1;
        while result >= 0 && self.shapes[result as usize].is_none() {
            result -= 1;
        }
        result
    }

            pub fn board_layer_count(&self) -> usize {
        self.shapes.len()
    }

                            pub fn drill_radius(&self) -> f64 {
        if let Some(colon_index) = self.name.find(':') {
            let underscore_index = self.name[colon_index..]
                .find('_')
                .map(|offset| offset + colon_index);
            let drill_str: &str = match underscore_index {
                Some(u) if u > colon_index => &self.name[colon_index + 1..u],
                _ => &self.name[colon_index + 1..],
            };
            let filtered_drill = strip_non_digits(drill_str);
            if let Ok(drill_dia) = filtered_drill.parse::<f64>() {
                if let Some(last_underscore) = self.name[..=colon_index].rfind('_') {
                    let outer_str = &self.name[last_underscore + 1..colon_index];
                    let filtered_outer = strip_non_digits(outer_str);
                    if let Ok(outer_dia) = filtered_outer.parse::<f64>()
                        && outer_dia > 0.0
                    {
                        let actual_outer_radius = self.smallest_radius();
                        if actual_outer_radius > 0.0 {
                            return actual_outer_radius * (drill_dia / outer_dia);
                        }
                    }
                }
            }
        }
        self.smallest_radius() * 0.45
    }

                fn smallest_radius(&self) -> f64 {
        let mut min_radius = f64::MAX;
        for shape in self.shapes.iter().flatten() {
            let bounding_box = shape.bounding_box();
            let radius = (bounding_box.width() as f64).min(bounding_box.height() as f64) / 2.0;
            if radius < min_radius {
                min_radius = radius;
            }
        }
        if min_radius == f64::MAX {
            0.0
        } else {
            min_radius
        }
    }

                    pub fn get_trace_exit_directions(&self, layer: i32, factor: f64) -> Vec<Direction> {
        let mut result = Vec::new();
        if layer < 0 || layer as usize >= self.shapes.len() {
            return result;
        }
        let Some(current_shape) = self.shapes[layer as usize].as_ref() else {
            return result;
        };
        let is_box_or_octagon = matches!(
            current_shape,
            Shape::Tile(TileShape::Box(_)) | Shape::Tile(TileShape::Octagon(_))
        );
        if !is_box_or_octagon {
            return result;
        }
        let current_box = current_shape.bounding_box();
        let width = current_box.width() as f64;
        let height = current_box.height() as f64;
        let all_dirs = width.max(height) < factor * width.min(height);

        if all_dirs || width >= height {
            result.push(Direction::Int(IntDirection::RIGHT));
            result.push(Direction::Int(IntDirection::LEFT));
        }
        if all_dirs || width <= height {
            result.push(Direction::Int(IntDirection::UP));
            result.push(Direction::Int(IntDirection::DOWN));
        }
        result
    }
}

impl fmt::Display for Padstack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

fn strip_non_digits(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect()
}

fn shape_max_width(shape: &Shape) -> f64 {
    match shape {
        Shape::Tile(tile) => tile.max_width(),
        Shape::Circle(circle) => circle.max_width(),
        Shape::Polygon(_) => panic!(
            "Padstack shapes are Java's ConvexShape[] (IntBox/IntOctagon/Simplex/Circle only); \
             a PolygonShape can never appear here"
        ),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Padstacks {
            pub board_layer_structure: LayerStructure,
        list: Vec<Padstack>,
}

impl Padstacks {
        pub fn new(board_layer_structure: LayerStructure) -> Padstacks {
        Padstacks {
            board_layer_structure,
            list: Vec::new(),
        }
    }

            pub fn get_by_name(&self, name: &str) -> Option<&Padstack> {
        self.list.iter().find(|p| equals_ignore_case(&p.name, name))
    }

                        pub fn get(&self, id: PadstackId) -> Option<&Padstack> {
        if id.0 == 0 || id.0 > self.list.len() {
            return None;
        }
        let result = &self.list[id.0 - 1];
        debug_assert_eq!(
            result.no, id.0,
            "Padstacks.get: inconsistent padstack ID (Padstacks.java:41-43)"
        );
        Some(result)
    }

        pub fn count(&self) -> usize {
        self.list.len()
    }

            pub fn add(
        &mut self,
        name: impl Into<String>,
        shapes: Vec<Option<Shape>>,
        drill_allowed: bool,
        placed_absolute: bool,
    ) -> PadstackId {
        let no = self.list.len() + 1;
        self.list.push(Padstack::new(
            name.into(),
            no,
            shapes,
            drill_allowed,
            placed_absolute,
        ));
        PadstackId(no)
    }

                pub fn add_unnamed(&mut self, shapes: Vec<Option<Shape>>) -> PadstackId {
        let new_name = format!("padstack#{}", self.list.len() + 1);
        self.add(new_name, shapes, false, false)
    }

                pub fn add_layer_range(&mut self, shape: Shape, from_layer: i32, to_layer: i32) -> PadstackId {
        let layer_count = self.board_layer_structure.layers.len();
        let mut shapes = vec![None; layer_count];
        let first_layer = from_layer.max(0);
        let last_layer = to_layer.min(layer_count as i32 - 1);
        if first_layer <= last_layer {
            for i in first_layer..=last_layer {
                shapes[i as usize] = Some(shape.clone());
            }
        }
        self.add_unnamed(shapes)
    }
}

impl Default for Padstacks {
                    fn default() -> Self {
        Padstacks::new(LayerStructure::new(Vec::new()))
    }
}

impl PadstackLookup for Padstacks {
    fn padstack_from_layer(&self, padstack: PadstackId) -> i32 {
        self.get(padstack)
            .expect("PadstackLookup: PadstackId must be valid")
            .from_layer()
    }

    fn padstack_to_layer(&self, padstack: PadstackId) -> i32 {
        self.get(padstack)
            .expect("PadstackLookup: PadstackId must be valid")
            .to_layer()
    }

    fn padstack_shape_max_width(&self, padstack: PadstackId, layer: i32) -> Option<f64> {
        self.get(padstack)
            .expect("PadstackLookup: PadstackId must be valid")
            .get_shape(layer)
            .map(shape_max_width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fr_geometry::IntBox;

    fn layer_structure(count: usize) -> LayerStructure {
        use crate::structure::Layer;
        LayerStructure::new(
            (0..count)
                .map(|i| Layer::new(format!("L{i}"), true))
                .collect(),
        )
    }

    fn box_shape(x0: i32, y0: i32, x1: i32, y1: i32) -> Shape {
        Shape::Tile(TileShape::Box(IntBox::from_coords(x0, y0, x1, y1)))
    }

    #[test]
    fn two_layer_padstack_from_layer_and_to_layer() {
        let mut padstacks = Padstacks::new(layer_structure(4));
        let id = padstacks.add_layer_range(box_shape(0, 0, 10, 10), 0, 1);
        let p = padstacks.get(id).unwrap();
        assert_eq!(p.from_layer(), 0);
        assert_eq!(p.to_layer(), 1);
        assert_eq!(p.board_layer_count(), 4);
        assert!(p.get_shape(0).is_some());
        assert!(p.get_shape(1).is_some());
        assert!(p.get_shape(2).is_none());
        assert!(p.get_shape(3).is_none());
    }

    #[test]
    fn padstack_with_no_shapes_reports_out_of_range_from_and_to_layer() {
        let mut padstacks = Padstacks::new(layer_structure(4));
        let id = padstacks.add_unnamed(vec![None, None, None, None]);
        let p = padstacks.get(id).unwrap();
        assert_eq!(p.from_layer(), 4); 
        assert_eq!(p.to_layer(), -1);
    }

    #[test]
    fn add_layer_range_clamps_to_board_bounds() {
        let mut padstacks = Padstacks::new(layer_structure(2));
        let id = padstacks.add_layer_range(box_shape(0, 0, 10, 10), -5, 99);
        let p = padstacks.get(id).unwrap();
        assert_eq!(p.from_layer(), 0);
        assert_eq!(p.to_layer(), 1);
    }

    #[test]
    fn padstacks_get_by_id_is_one_based_and_bounds_checked() {
        let mut padstacks = Padstacks::new(layer_structure(2));
        let id = padstacks.add_unnamed(vec![Some(box_shape(0, 0, 1, 1)), None]);
        assert_eq!(id, PadstackId(1));
        assert!(padstacks.get(PadstackId(0)).is_none());
        assert!(padstacks.get(PadstackId(2)).is_none());
        assert_eq!(padstacks.get(PadstackId(1)).unwrap().no, 1);
    }

    #[test]
    fn padstacks_get_by_name_is_case_insensitive() {
        let mut padstacks = Padstacks::new(layer_structure(2));
        padstacks.add(
            "Round_1mm",
            vec![Some(box_shape(0, 0, 1, 1)), None],
            true,
            false,
        );
        assert!(padstacks.get_by_name("round_1mm").is_some());
        assert!(padstacks.get_by_name("ROUND_1MM").is_some());
        assert!(padstacks.get_by_name("missing").is_none());
    }

    #[test]
    fn add_unnamed_generates_sequential_names() {
        let mut padstacks = Padstacks::new(layer_structure(1));
        let a = padstacks.add_unnamed(vec![Some(box_shape(0, 0, 1, 1))]);
        let b = padstacks.add_unnamed(vec![Some(box_shape(0, 0, 1, 1))]);
        assert_eq!(padstacks.get(a).unwrap().name, "padstack#1");
        assert_eq!(padstacks.get(b).unwrap().name, "padstack#2");
    }

    #[test]
    fn padstack_compare_to_is_case_insensitive() {
        let mut padstacks = Padstacks::new(layer_structure(1));
        padstacks.add("Alpha", vec![None], false, false);
        padstacks.add("beta", vec![None], false, false);
        let a = padstacks.get(PadstackId(1)).unwrap();
        let b = padstacks.get(PadstackId(2)).unwrap();
        assert_eq!(a.compare_to(b), Ordering::Less);
        assert_eq!(a.to_string(), "Alpha");
    }

    #[test]
    fn drill_radius_falls_back_to_smallest_radius_times_045_without_a_colon() {
        let mut padstacks = Padstacks::new(layer_structure(1));
        let id = padstacks.add_unnamed(vec![Some(box_shape(0, 0, 20, 10))]);
        let p = padstacks.get(id).unwrap();
        assert_eq!(p.drill_radius(), 2.25);
    }

    #[test]
    fn drill_radius_parses_outer_and_drill_diameter_from_the_name() {
        let mut padstacks = Padstacks::new(layer_structure(1));
        let id = padstacks.add(
            "Round_2.0:1.0_mm",
            vec![Some(box_shape(0, 0, 20, 30))],
            true,
            false,
        );
        let p = padstacks.get(id).unwrap();
        assert_eq!(p.drill_radius(), 5.0);
    }

    #[test]
    fn drill_radius_falls_back_when_no_underscore_before_the_colon() {
        let mut padstacks = Padstacks::new(layer_structure(1));
        let id = padstacks.add(
            "Round2.0:1.0mm",
            vec![Some(box_shape(0, 0, 20, 10))],
            true,
            false,
        );
        let p = padstacks.get(id).unwrap();
        assert_eq!(p.drill_radius(), 5.0 * 0.45);
    }

    #[test]
    fn get_trace_exit_directions_matches_java_for_a_wide_pad() {
        let mut padstacks = Padstacks::new(layer_structure(1));
        let id = padstacks.add_unnamed(vec![Some(box_shape(0, 0, 100, 10))]);
        let p = padstacks.get(id).unwrap();
        let dirs = p.get_trace_exit_directions(0, 1.0);
        assert_eq!(
            dirs,
            vec![
                Direction::Int(IntDirection::RIGHT),
                Direction::Int(IntDirection::LEFT),
            ]
        );
    }

    #[test]
    fn get_trace_exit_directions_allows_all_when_within_factor() {
        let mut padstacks = Padstacks::new(layer_structure(1));
        let id = padstacks.add_unnamed(vec![Some(box_shape(0, 0, 10, 12))]);
        let p = padstacks.get(id).unwrap();
        let dirs = p.get_trace_exit_directions(0, 2.0);
        assert_eq!(
            dirs,
            vec![
                Direction::Int(IntDirection::RIGHT),
                Direction::Int(IntDirection::LEFT),
                Direction::Int(IntDirection::UP),
                Direction::Int(IntDirection::DOWN),
            ]
        );
    }

    #[test]
    fn get_trace_exit_directions_empty_for_non_box_octagon_shapes() {
        use fr_geometry::{Circle, IntPoint};
        let mut padstacks = Padstacks::new(layer_structure(1));
        let id = padstacks.add_unnamed(vec![Some(Shape::Circle(Circle::new(
            IntPoint::new(0, 0),
            5,
        )))]);
        let p = padstacks.get(id).unwrap();
        assert!(p.get_trace_exit_directions(0, 1.0).is_empty());
    }

    #[test]
    fn get_trace_exit_directions_empty_out_of_range_or_no_shape() {
        let mut padstacks = Padstacks::new(layer_structure(2));
        let id = padstacks.add_unnamed(vec![Some(box_shape(0, 0, 1, 1)), None]);
        let p = padstacks.get(id).unwrap();
        assert!(p.get_trace_exit_directions(-1, 1.0).is_empty());
        assert!(p.get_trace_exit_directions(5, 1.0).is_empty());
        assert!(p.get_trace_exit_directions(1, 1.0).is_empty());
    }

            #[test]
    fn padstacks_implements_padstack_lookup() {
        let mut padstacks = Padstacks::new(layer_structure(4));
        let id = padstacks.add_layer_range(box_shape(0, 0, 20, 10), 0, 1);
        assert_eq!(PadstackLookup::padstack_from_layer(&padstacks, id), 0);
        assert_eq!(PadstackLookup::padstack_to_layer(&padstacks, id), 1);
        assert_eq!(
            PadstackLookup::padstack_shape_max_width(&padstacks, id, 0),
            Some(20.0)
        );
        assert_eq!(
            PadstackLookup::padstack_shape_max_width(&padstacks, id, 2),
            None
        );
    }
}
