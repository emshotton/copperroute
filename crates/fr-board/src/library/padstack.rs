//! Padstacks: the copper/hole masks placed at pin and via locations.
//!
//! Java: `core/library/Padstack.java`, `core/library/Padstacks.java`.

use std::cmp::Ordering;
use std::fmt;

use fr_geometry::{Direction, IntDirection, Shape, ShapeOps, TileShape};

use crate::ids::PadstackId;
use crate::rules::{PadstackLookup, compare_to_ignore_case, equals_ignore_case};
use crate::structure::LayerStructure;

/// Port of `Padstack` (`core/library/Padstack.java`): describes padstack masks for pins or vias
/// located at the origin, one shape per board layer (Java's `ConvexShape[] shapes`, `null` on a
/// layer with no copper).
///
/// Java's `shapes` field is typed `ConvexShape[]` (`IntBox`/`IntOctagon`/`Simplex`/`Circle`
/// only); this port widens it to `Vec<Option<Shape>>` per the task brief, which also admits
/// [`Shape::Polygon`]. A `PolygonShape` can never actually appear here — nothing in this crate
/// constructs one — so [`shape_max_width`] panics on that variant rather than modelling a case
/// Java's type system rules out entirely.
///
/// not ported: `Padstack.padstackList` (Padstack.java:33) — a back-pointer to the owning
/// [`Padstacks`], read only by `Padstack.printInfo` (Padstack.java:198-213,209) to name the
/// board layer; `ItemInfoPrinter.Printable` is GUI-only and dropped (see `rules/mod.rs`'s note).
///
/// not ported: `Padstack.cachedDrillRadius` (Padstack.java:44) — a pure memoization of
/// [`Padstack::drill_radius`] over immutable state (`name` and `shapes` never change after
/// construction); the port recomputes on every call, which is observably identical and avoids
/// interior mutability for a performance-only field.
///
/// not ported: `Padstack.printInfo` (Padstack.java:197-213) — `ItemInfoPrinter.Printable`, GUI
/// only.
#[derive(Debug, Clone, PartialEq)]
pub struct Padstack {
    /// `Padstack.name` (Padstack.java:18).
    pub name: String,
    /// `Padstack.id` (Padstack.java:19), renamed per the task brief; starts at 1
    /// (`Padstacks.add`, Padstacks.java:57).
    pub no: usize,
    /// `Padstack.attachAllowed` (Padstack.java:22): whether vias of the own net may overlap
    /// with this padstack.
    pub attach_allowed: bool,
    /// `Padstack.placedAbsolute` (Padstack.java:28): if false, the layers of the padstack are
    /// mirrored when placed on the back side.
    pub placed_absolute: bool,
    /// `Padstack.shapes` (Padstack.java:30), one entry per board layer, `None` where Java has a
    /// `null` shape.
    shapes: Vec<Option<Shape>>,
    /// `Padstack.holeOnly` (Padstack.java:41): true for padstacks whose copper-layer shapes were
    /// synthesized from the drill radius because the source padstack had no copper at all.
    pub hole_only: bool,
}

impl Padstack {
    /// Port of the package-private `Padstack(String, int, ConvexShape[], boolean, boolean,
    /// Padstacks)` constructor (Padstack.java:47-60), minus the `Padstacks` back-pointer. Only
    /// [`Padstacks::add`] calls this, matching Java's package-private visibility.
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

    /// Port of `Padstack.compareTo` (Padstack.java:63-66): compares by name, case-insensitively.
    /// Not an `Ord` impl for the same reason as `ViaInfo::compare_to`: two padstacks with the
    /// same name but different shapes compare `Equal` here while `PartialEq` is structural.
    pub fn compare_to(&self, other: &Padstack) -> Ordering {
        compare_to_ignore_case(&self.name, &other.name)
    }

    /// Port of `Padstack.getShape` (Padstack.java:127-134): the shape on `layer`, or `None` when
    /// `layer` is out of range or that layer has no shape. Java's out-of-range branch also logs
    /// a warning (dropped, `fr-board` must not depend on `tracing`).
    pub fn get_shape(&self, layer: i32) -> Option<&Shape> {
        if layer < 0 || layer as usize >= self.shapes.len() {
            return None;
        }
        self.shapes[layer as usize].as_ref()
    }

    /// Port of `Padstack.fromLayer` (Padstack.java:137-143): the first layer with a non-`None`
    /// shape, or `shapes.len()` (out of range) if every shape is `None`.
    pub fn from_layer(&self) -> i32 {
        let mut result = 0usize;
        while result < self.shapes.len() && self.shapes[result].is_none() {
            result += 1;
        }
        result as i32
    }

    /// Port of `Padstack.toLayer` (Padstack.java:146-152): the last layer with a non-`None`
    /// shape, or `-1` if every shape is `None`.
    pub fn to_layer(&self) -> i32 {
        let mut result = self.shapes.len() as i32 - 1;
        while result >= 0 && self.shapes[result as usize].is_none() {
            result -= 1;
        }
        result
    }

    /// Port of `Padstack.boardLayerCount` (Padstack.java:154-157): the layer count of the board
    /// this padstack belongs to.
    pub fn board_layer_count(&self) -> usize {
        self.shapes.len()
    }

    // renamed: Padstack.getDrillRadius -> drill_radius (Padstack.java:68-112, Rust accessor
    // naming convention drops the `get_` prefix).
    /// Port of `Padstack.getDrillRadius` (Padstack.java:68-112): the drill radius in board
    /// units, parsed from a KiCad-style `outerDia:drillDia_...mm` name fragment when present,
    /// else `smallest_radius() * 0.45`.
    ///
    /// Java's leading `if (name != null)` guard (Padstack.java:77) is dropped: `name` is a
    /// non-nullable `String` in the port, so the guard is always true.
    pub fn drill_radius(&self) -> f64 {
        if let Some(colon_index) = self.name.find(':') {
            // Java: `name.indexOf('_', colonIndex)` searches forward from `colonIndex`
            // (inclusive); the char at `colonIndex` is ':', so a match is always > colonIndex.
            let underscore_index = self.name[colon_index..]
                .find('_')
                .map(|offset| offset + colon_index);
            let drill_str: &str = match underscore_index {
                Some(u) if u > colon_index => &self.name[colon_index + 1..u],
                _ => &self.name[colon_index + 1..],
            };
            let filtered_drill = strip_non_digits(drill_str);
            if let Ok(drill_dia) = filtered_drill.parse::<f64>() {
                // Java: `name.lastIndexOf('_', colonIndex)` searches backward from `colonIndex`
                // (inclusive).
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

    /// Port of `Padstack.getSmallestRadius` (Padstack.java:114-125): the smallest half-width
    /// (over both box dimensions) of any non-`None` shape's bounding box, or `0.0` if every
    /// shape is `None`.
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

    /// Port of `Padstack.getTraceExitDirections` (Padstack.java:164-195): the allowed trace exit
    /// directions on `layer`. If the pad's shorter side is smaller than `factor` times its
    /// longer side, connection to the long side is also allowed. Empty when `layer` is out of
    /// range, has no shape, or the shape is not an `IntBox`/`IntOctagon`.
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
    // renamed: Padstack.toString -> Display::fmt (Padstack.java:159-162 returns `name`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// Strips every character that is not an ASCII digit or `.`, matching Java's
/// `str.replaceAll("[^0-9.]", "")`.
fn strip_non_digits(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect()
}

/// The maximum width of a padstack shape (Java's `ConvexShape.maxWidth`, only implemented by
/// `TileShape` and `Circle`). A `Shape::Polygon` can never occur in a padstack's shape list (see
/// the [`Padstack`] doc comment), so that arm panics rather than returning a placeholder value
/// Java's type system never has to produce.
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

/// Port of `Padstacks` (`core/library/Padstacks.java`): a library of padstacks for pins or vias.
#[derive(Debug, Clone, PartialEq)]
pub struct Padstacks {
    /// `Padstacks.boardLayerStructure` (Padstacks.java:13): the layer structure shared by every
    /// padstack in this library.
    pub board_layer_structure: LayerStructure,
    /// `Padstacks.padstacks` (Padstacks.java:16, a `Vector`).
    list: Vec<Padstack>,
}

impl Padstacks {
    /// Port of the `Padstacks(LayerStructure)` constructor (Padstacks.java:19-22).
    pub fn new(board_layer_structure: LayerStructure) -> Padstacks {
        Padstacks {
            board_layer_structure,
            list: Vec::new(),
        }
    }

    /// Port of `Padstacks.get(String)` (Padstacks.java:24-32): the padstack named `name`
    /// (case-insensitive), or `None`.
    pub fn get_by_name(&self, name: &str) -> Option<&Padstack> {
        self.list.iter().find(|p| equals_ignore_case(&p.name, name))
    }

    /// Port of `Padstacks.get(int)` (Padstacks.java:34-46): the padstack with this id (ids start
    /// at 1), or `None` when out of range. Java's out-of-range branch and its
    /// inconsistent-id check are both `FRLogger.warn` only (dropped).
    pub fn get(&self, id: PadstackId) -> Option<&Padstack> {
        if id.0 == 0 || id.0 > self.list.len() {
            return None;
        }
        Some(&self.list[id.0 - 1])
    }

    /// Port of `Padstacks.count` (Padstacks.java:48-51).
    pub fn count(&self) -> usize {
        self.list.len()
    }

    /// Port of `Padstacks.add(String, ConvexShape[], boolean, boolean)` (Padstacks.java:53-60):
    /// appends a new padstack, returning its freshly assigned id.
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

    /// Port of `Padstacks.add(ConvexShape[])` (Padstacks.java:62-69): appends a new padstack with
    /// an internally generated name (`"padstack#" + no`), `drillAllowed = false`,
    /// `placedAbsolute = false`.
    pub fn add_unnamed(&mut self, shapes: Vec<Option<Shape>>) -> PadstackId {
        let new_name = format!("padstack#{}", self.list.len() + 1);
        self.add(new_name, shapes, false, false)
    }

    /// Port of `Padstacks.add(ConvexShape, int, int)` (Padstacks.java:71-83): appends a new
    /// padstack whose shape is `shape` from `from_layer` to `to_layer` (clamped to the board's
    /// layer range) and `None` elsewhere.
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
    /// An empty padstack library over a layer-less board. Java has no no-arg `Padstacks`
    /// constructor; this exists only so [`crate::library::BoardLibrary::default`] can build a
    /// `Padstacks` for Java's no-arg `BoardLibrary()` constructor (BoardLibrary.java:34), whose
    /// `padstacks` field is `null` until a caller assigns a real one.
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
        // A padstack present only on layers 0 and 1 of a 4-layer board.
        // Padstack.fromLayer (Padstack.java:137-143) / toLayer (Padstack.java:146-152).
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
        // Padstack.java:137-143 / 146-152: an all-null shapes array walks off both ends.
        let mut padstacks = Padstacks::new(layer_structure(4));
        let id = padstacks.add_unnamed(vec![None, None, None, None]);
        let p = padstacks.get(id).unwrap();
        assert_eq!(p.from_layer(), 4); // shapes.length
        assert_eq!(p.to_layer(), -1);
    }

    #[test]
    fn add_layer_range_clamps_to_board_bounds() {
        // Padstacks.java:75-83: firstLayer = max(fromLayer, 0), lastLayer = min(toLayer,
        // layers.length - 1).
        let mut padstacks = Padstacks::new(layer_structure(2));
        let id = padstacks.add_layer_range(box_shape(0, 0, 10, 10), -5, 99);
        let p = padstacks.get(id).unwrap();
        assert_eq!(p.from_layer(), 0);
        assert_eq!(p.to_layer(), 1);
    }

    #[test]
    fn padstacks_get_by_id_is_one_based_and_bounds_checked() {
        // Padstacks.java:34-46: `1 <= padstackId <= count`, else None.
        let mut padstacks = Padstacks::new(layer_structure(2));
        let id = padstacks.add_unnamed(vec![Some(box_shape(0, 0, 1, 1)), None]);
        assert_eq!(id, PadstackId(1));
        assert!(padstacks.get(PadstackId(0)).is_none());
        assert!(padstacks.get(PadstackId(2)).is_none());
        assert_eq!(padstacks.get(PadstackId(1)).unwrap().no, 1);
    }

    #[test]
    fn padstacks_get_by_name_is_case_insensitive() {
        // Padstacks.java:24-32: `equalsIgnoreCase`.
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
        // Padstacks.java:66-69: "padstack#" + (size + 1).
        let mut padstacks = Padstacks::new(layer_structure(1));
        let a = padstacks.add_unnamed(vec![Some(box_shape(0, 0, 1, 1))]);
        let b = padstacks.add_unnamed(vec![Some(box_shape(0, 0, 1, 1))]);
        assert_eq!(padstacks.get(a).unwrap().name, "padstack#1");
        assert_eq!(padstacks.get(b).unwrap().name, "padstack#2");
    }

    #[test]
    fn padstack_compare_to_is_case_insensitive() {
        // Padstack.java:63-66.
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
        // Padstack.java:76-77 (no ':'), then 109-111.
        let mut padstacks = Padstacks::new(layer_structure(1));
        // 20 x 10 box: smallest radius = min(20,10)/2 = 5.0; drill radius = 5.0 * 0.45 = 2.25.
        let id = padstacks.add_unnamed(vec![Some(box_shape(0, 0, 20, 10))]);
        let p = padstacks.get(id).unwrap();
        assert_eq!(p.drill_radius(), 2.25);
    }

    #[test]
    fn drill_radius_parses_outer_and_drill_diameter_from_the_name() {
        // Padstack.java:78-104: "..._<outer>:<drill>_..." style name.
        // Outer box has smallest radius 10.0 (20x30 box -> min/2 = 10.0); name encodes
        // outer=2.0, drill=1.0, so drillRadius = 10.0 * (1.0/2.0) = 5.0.
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
        // Padstack.java:90-91: `lastIndexOf('_', colonIndex) < 0` skips the outer-diameter
        // branch entirely, falling through to smallestRadius() * 0.45.
        let mut padstacks = Padstacks::new(layer_structure(1));
        let id = padstacks.add(
            "Round2.0:1.0mm",
            vec![Some(box_shape(0, 0, 20, 10))],
            true,
            false,
        );
        let p = padstacks.get(id).unwrap();
        // smallest_radius = min(20, 10) / 2.0 = 5.0.
        assert_eq!(p.drill_radius(), 5.0 * 0.45);
    }

    #[test]
    fn get_trace_exit_directions_matches_java_for_a_wide_pad() {
        // Padstack.java:168-195: a wide box (width > height, and not within `factor`) allows
        // only RIGHT/LEFT.
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
        // Padstack.java:182-184: a near-square pad (within `factor`) allows all four.
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
        // Padstack.java:177-179: a Circle is neither IntBox nor IntOctagon.
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

    /// Stand-in board-layer-count-independent [`PadstackLookup`] exercise: confirms `Padstacks`
    /// itself implements the trait the rules layer depends on (rules/mod.rs `PadstackLookup`).
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
