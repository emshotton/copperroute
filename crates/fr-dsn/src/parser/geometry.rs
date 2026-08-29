//! The DSN geometry scopes: `io/specctra/parser/{Shape,Rectangle,Circle,Polygon,PolygonPath,
//! PolylinePath,Path,Layer,LayerStructure}.java`.
//!
//! Java's `Shape` is an abstract class with five concrete subclasses (`Rectangle`, `Circle`,
//! `Polygon` and the two `Path`s), each holding a `Layer` and a `double[]`; the port keeps one
//! struct per subclass and dispatches through the [`DsnShape`] enum, exactly as
//! `fr_geometry::Shape` does for `geometry.planar.Shape`. `Path` itself is abstract and adds
//! only the two fields its two subclasses share, so it has no Rust struct of its own — its one
//! declared method is `writeScope`, which is abstract there too.
//!
//! Every `FRLogger.warn`/`.error` in these classes is dropped (no `tracing` in `fr-dsn`, plan
//! Global Constraints); Java's `null` returns become `Option::None` and its `IOException`
//! catches become propagated [`DsnError`]s, matching the divergence
//! [`crate::parser::scope_parameter::skip_scope`] already documents.

use fr_geometry::{
    Area, Circle, FloatPoint, IntBox, IntOctagon, IntPoint, Point, PolygonShape, PolylineArea,
    PolylineShapeRef, Shape, Simplex, TileShape,
};

use crate::coordinate_transform::CoordinateTransform;
use crate::error::DsnError;
use crate::format::{IdentifierType, IndentFileWriter, java_double_to_string, java_round_to_int};
use crate::keyword::{Keyword, ScopeKeyword};
use crate::lexer::{DsnScanner, Token};
use crate::parser::dsn_file::read_string_scope;
use crate::parser::scope_parameter::skip_scope;
use std::io::Write;

// ------------------------------------------------------------------------ Layer.java

/// `io/specctra/parser/Layer.java`: one layer of a DSN file's layer structure.
///
/// `no` is `i32`, not an index type: Java's two shared constants ([`DsnLayer::pcb`] and
/// [`DsnLayer::signal`], Layer.java:11,14) both carry `-1`, "this object describes more than one
/// layer" (Layer.java:24-25).
// not ported: a `Default` for `Layer` — Java has no no-argument `Layer` constructor, and a
// derived one would mean `no: 0`, a real layer number (the component side), not an absent one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsnLayer {
    /// `Layer.name`.
    pub name: String,
    /// `Layer.no` — the physical layer number, counting from 0 at the component side, or `-1`
    /// for a `Layer` describing more than one layer.
    pub no: i32,
    /// `Layer.isSignal` — false for a power/ground plane.
    pub is_signal: bool,
    /// `Layer.netNames` — the nets on this layer when it is a power plane.
    pub net_names: Vec<String>,
}

impl DsnLayer {
    /// `Layer(String, int, boolean, Collection<String>)` (Layer.java:27-32).
    #[must_use]
    pub fn with_nets(
        name: impl Into<String>,
        no: i32,
        is_signal: bool,
        net_names: Vec<String>,
    ) -> DsnLayer {
        DsnLayer {
            name: name.into(),
            no,
            is_signal,
            net_names,
        }
    }

    /// `Layer(String, int, boolean)` (Layer.java:40-45) — an empty net list.
    #[must_use]
    pub fn new(name: impl Into<String>, no: i32, is_signal: bool) -> DsnLayer {
        DsnLayer::with_nets(name, no, is_signal, Vec::new())
    }

    /// `Layer.PCB` (Layer.java:11): "all layers of the board".
    ///
    /// A Rust constructor rather than a `static final` — the Java constant is shared by
    /// reference and every `Shape` holds a `Layer`, which the port owns instead of aliasing.
    #[must_use]
    pub fn pcb() -> DsnLayer {
        DsnLayer::new("pcb", -1, false)
    }

    /// `Layer.SIGNAL` (Layer.java:14): "the signal layers".
    #[must_use]
    pub fn signal() -> DsnLayer {
        DsnLayer::new("signal", -1, true)
    }

    // renamed: Layer.writeScope -> `parser::structure::write_layer_scope`. The `(layer <name>
    // (type signal|power) …)` writer belongs to the `structure` scope's writer — it is called
    // only from `Structure.writeLayers` and calls `Rule.writeDefaultRule` — so it lives next to
    // that caller rather than here with `Layer`'s reader (Plan 3 Task 11).
}

// ---------------------------------------------------------------- LayerStructure.java

/// `io/specctra/parser/LayerStructure.java` (the DSN parser's own layer structure, distinct from
/// [`fr_board::LayerStructure`]): the ordered list of [`DsnLayer`]s read from a `structure`
/// scope, before it is turned into a board layer structure.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DsnLayerStructure {
    /// `LayerStructure.layers`.
    pub layers: Vec<DsnLayer>,
}

impl DsnLayerStructure {
    /// `LayerStructure(Collection<Layer>)` (LayerStructure.java:13-19).
    #[must_use]
    pub fn new(layers: Vec<DsnLayer>) -> DsnLayerStructure {
        DsnLayerStructure { layers }
    }

    /// `LayerStructure(board.model.structure.LayerStructure)` (LayerStructure.java:22-28):
    /// numbers the board's layers 0, 1, … in order.
    // renamed: the second `LayerStructure(…)` constructor -> `from_board` (Rust has no
    // overloading).
    #[must_use]
    pub fn from_board(board_layer_structure: &fr_board::LayerStructure) -> DsnLayerStructure {
        DsnLayerStructure {
            layers: board_layer_structure
                .layers
                .iter()
                .enumerate()
                .map(|(i, board_layer)| {
                    DsnLayer::new(
                        board_layer.name.clone(),
                        i32::try_from(i).unwrap_or(i32::MAX),
                        board_layer.is_signal,
                    )
                })
                .collect(),
        }
    }

    /// `LayerStructure.getNo` (LayerStructure.java:31-45): the index of the named layer.
    ///
    /// Java returns `-1` for "not found" and every caller tests `layerIndex < 0`; the port
    /// returns `Option<usize>` instead (plan Task 5 brief: "choose `Option<usize>` and adapt").
    /// The two Electra fall-backs (a name merely *containing* `"Top"`/`"Bottom"`,
    /// LayerStructure.java:37-43) are kept verbatim, including the empty-structure case: Java's
    /// `layers.length - 1` is then `-1`, i.e. the same "not found" its callers test for, so this
    /// returns `None` there.
    // renamed: getNo -> get_no, returning Option<usize> rather than Java's -1 sentinel.
    #[must_use]
    pub fn get_no(&self, name: &str) -> Option<usize> {
        for (i, layer) in self.layers.iter().enumerate() {
            if name == layer.name {
                return Some(i);
            }
        }
        // check for special layers of the Electra autorouter used for the outline
        if name.contains("Top") {
            // Java returns 0 even for an empty structure; its callers then reject it with the
            // `layerIndex >= layers.length` half of their guard (Shape.java:92,329).
            return Some(0);
        }
        if name.contains("Bottom") {
            return self.layers.len().checked_sub(1);
        }
        None
    }

    /// `LayerStructure.signalLayerCount` (LayerStructure.java:47-55).
    #[must_use]
    pub fn signal_layer_count(&self) -> usize {
        self.layers.iter().filter(|l| l.is_signal).count()
    }

    /// `LayerStructure.containsPlane` (LayerStructure.java:58-68): does the named net have a
    /// power plane on some non-signal layer?
    #[must_use]
    pub fn contains_plane(&self, net_name: &str) -> bool {
        self.layers
            .iter()
            .any(|l| !l.is_signal && l.net_names.iter().any(|n| n == net_name))
    }
}

// ------------------------------------------------------------------- Rectangle.java

/// `io/specctra/parser/Rectangle.java`: `coor` is lower-left x, lower-left y, upper-right x,
/// upper-right y (Rectangle.java:15-18).
#[derive(Debug, Clone, PartialEq)]
pub struct DsnRectangle {
    /// `Shape.layer`.
    pub layer: DsnLayer,
    /// `Rectangle.coor`, Java's `double[4]`.
    pub coor: [f64; 4],
}

impl DsnRectangle {
    /// `Rectangle(Layer, double[])` (Rectangle.java:19-22).
    #[must_use]
    pub fn new(layer: DsnLayer, coor: [f64; 4]) -> DsnRectangle {
        DsnRectangle { layer, coor }
    }

    /// `Rectangle.boundingBox` (Rectangle.java:24-27): a rectangle is its own bounding box.
    ///
    /// Java returns `this` — the *same* object, so a caller that mutated the returned
    /// `Rectangle.layer` (the one non-final field on `Shape`) would mutate the receiver. This
    /// returns a clone; no Java caller relies on the aliasing (`Structure.createBoard`
    /// immediately `union`s the results into fresh `Rectangle`s, Structure.java:1161-1163).
    #[must_use]
    pub fn bounding_box(&self) -> DsnRectangle {
        self.clone()
    }

    /// `Rectangle.union` (Rectangle.java:29-37): the smallest rectangle containing both.
    #[must_use]
    pub fn union(&self, other: &DsnRectangle) -> DsnRectangle {
        DsnRectangle::new(
            self.layer.clone(),
            [
                self.coor[0].min(other.coor[0]),
                self.coor[1].min(other.coor[1]),
                self.coor[2].max(other.coor[2]),
                self.coor[3].max(other.coor[3]),
            ],
        )
    }

    /// `Rectangle.transformToBoardRel` (Rectangle.java:39-56).
    #[must_use]
    pub fn transform_to_board_rel(&self, coordinate_transform: &CoordinateTransform) -> Shape {
        let mut box_coor = [0_i32; 4];
        for (target, source) in box_coor.iter_mut().zip(self.coor) {
            *target = java_round_to_int(coordinate_transform.dsn_to_board(source));
        }
        let result = if box_coor[1] <= box_coor[3] {
            // boxCoor describe lower left and upper right corner
            IntBox::from_coords(box_coor[0], box_coor[1], box_coor[2], box_coor[3])
        } else {
            // boxCoor describe upper left and lower right corner
            IntBox::from_coords(box_coor[0], box_coor[3], box_coor[2], box_coor[1])
        };
        Shape::Tile(TileShape::Box(result))
    }

    /// `Rectangle.transformToBoard` (Rectangle.java:58-69).
    #[must_use]
    pub fn transform_to_board(&self, coordinate_transform: &CoordinateTransform) -> Shape {
        let lower_left = coordinate_transform.dsn_to_board_point(&[
            self.coor[0].min(self.coor[2]),
            self.coor[1].min(self.coor[3]),
        ]);
        let upper_right = coordinate_transform.dsn_to_board_point(&[
            self.coor[0].max(self.coor[2]),
            self.coor[1].max(self.coor[3]),
        ]);
        Shape::Tile(TileShape::Box(IntBox::new(
            lower_left.round(),
            upper_right.round(),
        )))
    }

    /// `Rectangle.writeScope` (Rectangle.java:71-82).
    pub fn write_scope<W: Write>(
        &self,
        file: &mut IndentFileWriter<W>,
        identifier: &IdentifierType,
    ) {
        file.new_line();
        file.write("(rect ");
        identifier.write(&self.layer.name, file);
        for c in self.coor {
            file.write(" ");
            file.write(&java_double_to_string(c));
        }
        file.write(")");
    }

    /// `Rectangle.writeScopeInt` (Rectangle.java:84-95).
    pub fn write_scope_int<W: Write>(
        &self,
        file: &mut IndentFileWriter<W>,
        identifier: &IdentifierType,
    ) {
        file.new_line();
        file.write("(rect ");
        identifier.write(&self.layer.name, file);
        for c in self.coor {
            file.write(" ");
            let current_coor = java_round_to_int(c);
            file.write(&current_coor.to_string());
        }
        file.write(")");
    }
}

// ---------------------------------------------------------------------- Circle.java

/// `io/specctra/parser/Circle.java`: `coor[0]` is the diameter, `coor[1]`/`coor[2]` the centre.
///
/// Java's own comment calls `coor[0]` "the radius" (Circle.java:15-18) but its producers and
/// consumers treat it as a diameter — `transformToBoard`/`transformToBoardRel` halve it
/// (Circle.java:40,48) and `CoordinateTransform.boardToDsn(Shape, Layer)` fills it with `2 *
/// boardToDsn(radius)` (CoordinateTransform.java:99). [`DsnCircle::bounding_box`] is the one
/// place that follows the comment instead of the code, and is wrong because of it.
#[derive(Debug, Clone, PartialEq)]
pub struct DsnCircle {
    /// `Shape.layer`.
    pub layer: DsnLayer,
    /// `Circle.coor`, Java's `double[3]`.
    pub coor: [f64; 3],
}

impl DsnCircle {
    /// `Circle(Layer, double[])` (Circle.java:20-23) and `Circle(Layer, double, double, double)`
    /// (Circle.java:25-31) — one Rust constructor for both, since the second just fills the
    /// array in order.
    #[must_use]
    pub fn new(layer: DsnLayer, coor: [f64; 3]) -> DsnCircle {
        DsnCircle { layer, coor }
    }

    /// `Circle.transformToBoard` (Circle.java:33-42).
    #[must_use]
    pub fn transform_to_board(&self, coordinate_transform: &CoordinateTransform) -> Shape {
        let center = coordinate_transform
            .dsn_to_board_point(&[self.coor[1], self.coor[2]])
            .round();
        let radius = java_round_to_int(coordinate_transform.dsn_to_board(self.coor[0]) / 2.0);
        Shape::Circle(Circle::new(center, radius))
    }

    /// `Circle.transformToBoardRel` (Circle.java:44-54).
    #[must_use]
    pub fn transform_to_board_rel(&self, coordinate_transform: &CoordinateTransform) -> Shape {
        let radius = java_round_to_int(coordinate_transform.dsn_to_board(self.coor[0]) / 2.0);
        let x = java_round_to_int(coordinate_transform.dsn_to_board(self.coor[1]));
        let y = java_round_to_int(coordinate_transform.dsn_to_board(self.coor[2]));
        Shape::Circle(Circle::new(IntPoint::new(x, y), radius))
    }

    /// `Circle.boundingBox` (Circle.java:56-64) — **twice** the box the circle actually needs.
    //
    // Java bug: Circle.boundingBox treats `coor[0]` as a radius (`coor[1] ± coor[0]`) while
    // every other user of the field treats it as a diameter — `Circle.transformToBoard` halves
    // it (`dsnToBoard(coor[0]) / 2`, Circle.java:40), `transformToBoardRel` halves it
    // (:48), and `CoordinateTransform.boardToDsn(Shape, Layer)` fills it with `2 *
    // boardToDsn(radius)` (CoordinateTransform.java:99). So this box is 2x too wide and 2x too
    // tall. Reachable: `Structure.createBoard` unions the outline shapes' `boundingBox()` and
    // derives the board size and DSN scale factor from the result (Structure.java:1161-1167), so
    // a circular board outline is sized from a doubled box. Reproduced verbatim — see
    // docs/java-quirks.md.
    #[must_use]
    pub fn bounding_box(&self) -> DsnRectangle {
        DsnRectangle::new(
            self.layer.clone(),
            [
                self.coor[1] - self.coor[0],
                self.coor[2] - self.coor[0],
                self.coor[1] + self.coor[0],
                self.coor[2] + self.coor[0],
            ],
        )
    }

    /// `Circle.writeScope` (Circle.java:66-76).
    pub fn write_scope<W: Write>(
        &self,
        file: &mut IndentFileWriter<W>,
        identifier: &IdentifierType,
    ) {
        file.new_line();
        file.write("(circle ");
        identifier.write(&self.layer.name, file);
        for c in self.coor {
            file.write(" ");
            file.write(&java_double_to_string(c));
        }
        file.write(")");
    }

    /// `Circle.writeScopeInt` (Circle.java:78-90).
    pub fn write_scope_int<W: Write>(
        &self,
        file: &mut IndentFileWriter<W>,
        identifier: &IdentifierType,
    ) {
        file.new_line();
        file.write("(circle ");
        identifier.write(&self.layer.name, file);
        for c in self.coor {
            file.write(" ");
            let current_coor = java_round_to_int(c);
            file.write(&current_coor.to_string());
        }
        file.write(")");
    }
}

// --------------------------------------------------------------------- Polygon.java

/// `io/specctra/parser/Polygon.java`: `coor` is `x0, y0, x1, y1, …` (Polygon.java:16-19).
#[derive(Debug, Clone, PartialEq)]
pub struct DsnPolygon {
    /// `Shape.layer`.
    pub layer: DsnLayer,
    /// `Polygon.coor`.
    pub coor: Vec<f64>,
}

impl DsnPolygon {
    /// `Polygon(Layer, double[])` (Polygon.java:20-23).
    #[must_use]
    pub fn new(layer: DsnLayer, coor: Vec<f64>) -> DsnPolygon {
        DsnPolygon { layer, coor }
    }

    /// `Polygon.transformToBoard` (Polygon.java:25-36).
    #[must_use]
    pub fn transform_to_board(&self, coordinate_transform: &CoordinateTransform) -> Shape {
        let corners: Vec<Point> = (0..self.coor.len() / 2)
            .map(|i| {
                Point::Int(
                    coordinate_transform
                        .dsn_to_board_point(&[self.coor[2 * i], self.coor[2 * i + 1]])
                        .round(),
                )
            })
            .collect();
        Shape::Polygon(PolygonShape::from_points(&corners))
    }

    /// `Polygon.transformToBoardRel` (Polygon.java:38-51).
    #[must_use]
    pub fn transform_to_board_rel(&self, coordinate_transform: &CoordinateTransform) -> Shape {
        if self.coor.len() < 2 {
            return Shape::Tile(TileShape::Simplex(Simplex::EMPTY));
        }
        let corners: Vec<Point> = (0..self.coor.len() / 2)
            .map(|i| {
                let current_x =
                    java_round_to_int(coordinate_transform.dsn_to_board(self.coor[2 * i]));
                let current_y =
                    java_round_to_int(coordinate_transform.dsn_to_board(self.coor[2 * i + 1]));
                Point::Int(IntPoint::new(current_x, current_y))
            })
            .collect();
        Shape::Polygon(PolygonShape::from_points(&corners))
    }

    /// `Polygon.boundingBox` (Polygon.java:53-72). The seed values are `Integer.MAX_VALUE` /
    /// `Integer.MIN_VALUE` widened to `double`, exactly as Java writes them, so an empty `coor`
    /// yields that same inverted box rather than an error.
    #[must_use]
    pub fn bounding_box(&self) -> DsnRectangle {
        let mut bounds = [
            f64::from(i32::MAX),
            f64::from(i32::MAX),
            f64::from(i32::MIN),
            f64::from(i32::MIN),
        ];
        for (i, c) in self.coor.iter().enumerate() {
            if i % 2 == 0 {
                // x coordinate
                bounds[0] = bounds[0].min(*c);
                bounds[2] = bounds[2].max(*c);
            } else {
                // y coordinate
                bounds[1] = bounds[1].min(*c);
                bounds[3] = bounds[3].max(*c);
            }
        }
        DsnRectangle::new(self.layer.clone(), bounds)
    }

    /// `Polygon.writeScope` (Polygon.java:74-90). The `0` after the layer name is the aperture
    /// width, always written as a literal zero (`String.valueOf(0)`, Polygon.java:81).
    pub fn write_scope<W: Write>(
        &self,
        file: &mut IndentFileWriter<W>,
        identifier: &IdentifierType,
    ) {
        file.start_scope_nl();
        file.write("polygon ");
        identifier.write(&self.layer.name, file);
        file.write(" ");
        file.write("0");
        let corner_count = self.coor.len() / 2;
        for i in 0..corner_count {
            file.new_line();
            file.write(&java_double_to_string(self.coor[2 * i]));
            file.write(" ");
            file.write(&java_double_to_string(self.coor[2 * i + 1]));
        }
        file.end_scope();
    }

    /// `Polygon.writeScopeInt` (Polygon.java:92-110).
    pub fn write_scope_int<W: Write>(
        &self,
        file: &mut IndentFileWriter<W>,
        identifier: &IdentifierType,
    ) {
        file.start_scope_nl();
        file.write("polygon ");
        identifier.write(&self.layer.name, file);
        file.write(" ");
        file.write("0");
        let corner_count = self.coor.len() / 2;
        for i in 0..corner_count {
            file.new_line();
            let mut current_coor = java_round_to_int(self.coor[2 * i]);
            file.write(&current_coor.to_string());
            file.write(" ");
            current_coor = java_round_to_int(self.coor[2 * i + 1]);
            file.write(&current_coor.to_string());
        }
        file.end_scope();
    }
}

// ----------------------------------------------------------------- PolygonPath.java

/// `io/specctra/parser/PolygonPath.java`, a `Path` (Path.java) whose coordinates are a sequence
/// of corners.
#[derive(Debug, Clone, PartialEq)]
pub struct DsnPolygonPath {
    /// `Shape.layer`.
    pub layer: DsnLayer,
    /// `Path.width` (Path.java:10).
    pub width: f64,
    /// `Path.coordinateArr` (Path.java:11).
    pub coordinate_arr: Vec<f64>,
}

impl DsnPolygonPath {
    /// `PolygonPath(Layer, double, double[])` (PolygonPath.java:16-18), through
    /// `Path(Layer, double, double[])` (Path.java:14-18).
    #[must_use]
    pub fn new(layer: DsnLayer, width: f64, coordinate_arr: Vec<f64>) -> DsnPolygonPath {
        DsnPolygonPath {
            layer,
            width,
            coordinate_arr,
        }
    }

    /// `PolygonPath.writeScope` (PolygonPath.java:20-36).
    pub fn write_scope<W: Write>(
        &self,
        file: &mut IndentFileWriter<W>,
        identifier: &IdentifierType,
    ) {
        file.start_scope_nl();
        file.write("path ");
        identifier.write(&self.layer.name, file);
        file.write(" ");
        file.write(&java_double_to_string(self.width));
        let corner_count = self.coordinate_arr.len() / 2;
        for i in 0..corner_count {
            file.new_line();
            file.write(&java_double_to_string(self.coordinate_arr[2 * i]));
            file.write(" ");
            file.write(&java_double_to_string(self.coordinate_arr[2 * i + 1]));
        }
        file.end_scope();
    }

    /// `PolygonPath.writeScopeInt` (PolygonPath.java:38-56). Note the width is **not** rounded
    /// here — Java writes `String.valueOf(this.width)` in both writers (PolygonPath.java:27,45).
    pub fn write_scope_int<W: Write>(
        &self,
        file: &mut IndentFileWriter<W>,
        identifier: &IdentifierType,
    ) {
        file.start_scope_nl();
        file.write("path ");
        identifier.write(&self.layer.name, file);
        file.write(" ");
        file.write(&java_double_to_string(self.width));
        let corner_count = self.coordinate_arr.len() / 2;
        for i in 0..corner_count {
            file.new_line();
            let mut current_coor = java_round_to_int(self.coordinate_arr[2 * i]);
            file.write(&current_coor.to_string());
            file.write(" ");
            current_coor = java_round_to_int(self.coordinate_arr[2 * i + 1]);
            file.write(&current_coor.to_string());
        }
        file.end_scope();
    }

    /// `PolygonPath.transformToBoard` (PolygonPath.java:58-82).
    #[must_use]
    pub fn transform_to_board(&self, coordinate_transform: &CoordinateTransform) -> Shape {
        let corners: Vec<FloatPoint> = (0..self.coordinate_arr.len() / 2)
            .map(|i| {
                coordinate_transform.dsn_to_board_point(&[
                    self.coordinate_arr[2 * i],
                    self.coordinate_arr[2 * i + 1],
                ])
            })
            .collect();
        self.enlarged_polygon(coordinate_transform, &corners)
    }

    /// `PolygonPath.transformToBoardRel` (PolygonPath.java:84-108) — identical to
    /// `transformToBoard` but for `dsnToBoardRel` on each corner.
    #[must_use]
    pub fn transform_to_board_rel(&self, coordinate_transform: &CoordinateTransform) -> Shape {
        let corners: Vec<FloatPoint> = (0..self.coordinate_arr.len() / 2)
            .map(|i| {
                coordinate_transform
                    .dsn_to_board_rel(&[self.coordinate_arr[2 * i], self.coordinate_arr[2 * i + 1]])
            })
            .collect();
        self.enlarged_polygon(coordinate_transform, &corners)
    }

    /// The shared tail of `transformToBoard`/`transformToBoardRel`
    /// (PolygonPath.java:68-81 == :94-107, character for character).
    fn enlarged_polygon(
        &self,
        coordinate_transform: &CoordinateTransform,
        corners: &[FloatPoint],
    ) -> Shape {
        let offset = coordinate_transform.dsn_to_board(self.width) / 2.0;
        if corners.len() <= 2 {
            let bounding_oct = bounding_octagon(corners);
            return Shape::Tile(TileShape::Octagon(bounding_oct.enlarge(offset)));
        }
        let rounded_corner_arr: Vec<Point> =
            corners.iter().map(|c| Point::Int(c.round())).collect();
        let result = PolygonShape::from_points(&rounded_corner_arr);
        if offset > 0.0 {
            return Shape::Tile(result.bounding_tile().enlarge(offset));
        }
        Shape::Polygon(result)
    }

    /// `PolygonPath.boundingBox` (PolygonPath.java:110-130).
    #[must_use]
    pub fn bounding_box(&self) -> DsnRectangle {
        let offset = self.width / 2.0;
        let mut bounds = [
            f64::from(i32::MAX),
            f64::from(i32::MAX),
            f64::from(i32::MIN),
            f64::from(i32::MIN),
        ];
        for (i, c) in self.coordinate_arr.iter().enumerate() {
            if i % 2 == 0 {
                // x coordinate
                bounds[0] = bounds[0].min(*c - offset);
                // Java bug: PolygonPath.boundingBox adds the offset *outside* the `Math.max`
                // on the x axis (PolygonPath.java:122, `Math.max(bounds[2], arr[i]) + offset`)
                // but inside it on the y axis (:126), so the upper x bound grows by `offset`
                // once per x coordinate instead of once in total. See docs/java-quirks.md.
                bounds[2] = bounds[2].max(*c) + offset;
            } else {
                // y coordinate
                bounds[1] = bounds[1].min(*c - offset);
                bounds[3] = bounds[3].max(*c + offset);
            }
        }
        DsnRectangle::new(self.layer.clone(), bounds)
    }
}

// ---------------------------------------------------------------- PolylinePath.java

/// `io/specctra/parser/PolylinePath.java`, a `Path` (Path.java) "defined by a sequence of lines
/// instead of a sequence of corners" (PolylinePath.java:10): `coordinate_arr` holds four numbers
/// per line.
#[derive(Debug, Clone, PartialEq)]
pub struct DsnPolylinePath {
    /// `Shape.layer`.
    pub layer: DsnLayer,
    /// `Path.width` (Path.java:10).
    pub width: f64,
    /// `Path.coordinateArr` (Path.java:11).
    pub coordinate_arr: Vec<f64>,
}

impl DsnPolylinePath {
    /// `PolylinePath(Layer, double, double[])` (PolylinePath.java:14-16).
    #[must_use]
    pub fn new(layer: DsnLayer, width: f64, coordinate_arr: Vec<f64>) -> DsnPolylinePath {
        DsnPolylinePath {
            layer,
            width,
            coordinate_arr,
        }
    }

    /// `PolylinePath.writeScope` (PolylinePath.java:18-35). Every one of the four numbers on a
    /// line is followed by a space, including the last — so each line ends with a trailing
    /// space, exactly as Java writes it (PolylinePath.java:31).
    pub fn write_scope<W: Write>(
        &self,
        file: &mut IndentFileWriter<W>,
        identifier: &IdentifierType,
    ) {
        file.start_scope_nl();
        file.write("polyline_path ");
        identifier.write(&self.layer.name, file);
        file.write(" ");
        file.write(&java_double_to_string(self.width));
        let line_count = self.coordinate_arr.len() / 4;
        for i in 0..line_count {
            file.new_line();
            for j in 0..4 {
                file.write(&java_double_to_string(self.coordinate_arr[4 * i + j]));
                file.write(" ");
            }
        }
        file.end_scope();
    }

    /// `PolylinePath.writeScopeInt` (PolylinePath.java:37-54).
    pub fn write_scope_int<W: Write>(
        &self,
        file: &mut IndentFileWriter<W>,
        identifier: &IdentifierType,
    ) {
        file.start_scope_nl();
        file.write("polyline_path ");
        identifier.write(&self.layer.name, file);
        file.write(" ");
        file.write(&java_double_to_string(self.width));
        let line_count = self.coordinate_arr.len() / 4;
        for i in 0..line_count {
            file.new_line();
            for j in 0..4 {
                let current_coor = java_round_to_int(self.coordinate_arr[4 * i + j]);
                file.write(&current_coor.to_string());
                file.write(" ");
            }
        }
        file.end_scope();
    }

    /// `PolylinePath.transformToBoardRel` (PolylinePath.java:56-60): Java warns
    /// "PolylinePath.transform_to_board_rel not implemented" and returns `null`.
    #[must_use]
    pub fn transform_to_board_rel(
        &self,
        _coordinate_transform: &CoordinateTransform,
    ) -> Option<Shape> {
        None
    }

    /// `PolylinePath.transformToBoard` (PolylinePath.java:62-66): Java warns
    /// "PolylinePath.transform_to_board not implemented" and returns `null`.
    #[must_use]
    pub fn transform_to_board(&self, _coordinate_transform: &CoordinateTransform) -> Option<Shape> {
        None
    }

    /// `PolylinePath.boundingBox` (PolylinePath.java:68-72): Java warns
    /// "PolylinePath.boundingBox not implemented" and returns `null`.
    #[must_use]
    pub fn bounding_box(&self) -> Option<DsnRectangle> {
        None
    }
}

// ----------------------------------------------------------------------- Shape.java

/// `io/specctra/parser/Shape.java`'s five concrete subclasses, as one enum.
#[derive(Debug, Clone, PartialEq)]
pub enum DsnShape {
    /// `Rectangle.java`.
    Rect(DsnRectangle),
    /// `Circle.java`.
    Circle(DsnCircle),
    /// `Polygon.java`.
    Polygon(DsnPolygon),
    /// `PolygonPath.java` — written as a `path` scope.
    Path(DsnPolygonPath),
    /// `PolylinePath.java`.
    PolylinePath(DsnPolylinePath),
}

impl DsnShape {
    /// `Shape.layer` (Shape.java:23), a field on the abstract base class.
    #[must_use]
    pub fn layer(&self) -> &DsnLayer {
        match self {
            DsnShape::Rect(s) => &s.layer,
            DsnShape::Circle(s) => &s.layer,
            DsnShape::Polygon(s) => &s.layer,
            DsnShape::Path(s) => &s.layer,
            DsnShape::PolylinePath(s) => &s.layer,
        }
    }

    /// `Shape.layer` as an assignable field — Java's is `public` and non-final (Shape.java:23),
    /// and the `structure`/`wiring` readers do reassign it.
    pub fn layer_mut(&mut self) -> &mut DsnLayer {
        match self {
            DsnShape::Rect(s) => &mut s.layer,
            DsnShape::Circle(s) => &mut s.layer,
            DsnShape::Polygon(s) => &mut s.layer,
            DsnShape::Path(s) => &mut s.layer,
            DsnShape::PolylinePath(s) => &mut s.layer,
        }
    }

    /// `Shape.writeScope` (Shape.java:596-598), dispatched to the subclass.
    pub fn write_scope<W: Write>(
        &self,
        file: &mut IndentFileWriter<W>,
        identifier: &IdentifierType,
    ) {
        match self {
            DsnShape::Rect(s) => s.write_scope(file, identifier),
            DsnShape::Circle(s) => s.write_scope(file, identifier),
            DsnShape::Polygon(s) => s.write_scope(file, identifier),
            DsnShape::Path(s) => s.write_scope(file, identifier),
            DsnShape::PolylinePath(s) => s.write_scope(file, identifier),
        }
    }

    /// `Shape.writeScopeInt` (Shape.java:600-605): the session-file writer, where every
    /// coordinate must be an integer.
    pub fn write_scope_int<W: Write>(
        &self,
        file: &mut IndentFileWriter<W>,
        identifier: &IdentifierType,
    ) {
        match self {
            DsnShape::Rect(s) => s.write_scope_int(file, identifier),
            DsnShape::Circle(s) => s.write_scope_int(file, identifier),
            DsnShape::Polygon(s) => s.write_scope_int(file, identifier),
            DsnShape::Path(s) => s.write_scope_int(file, identifier),
            DsnShape::PolylinePath(s) => s.write_scope_int(file, identifier),
        }
    }

    /// `Shape.writeHoleScope` (Shape.java:607-613): wraps this shape in a `window` scope.
    pub fn write_hole_scope<W: Write>(
        &self,
        file: &mut IndentFileWriter<W>,
        identifier: &IdentifierType,
    ) {
        file.start_scope_nl();
        file.write("window");
        self.write_scope(file, identifier);
        file.end_scope();
    }

    /// `Shape.transformToBoard` (Shape.java:615-617). `None` only for a `PolylinePath`, where
    /// Java warns and returns `null`.
    #[must_use]
    pub fn transform_to_board(&self, coordinate_transform: &CoordinateTransform) -> Option<Shape> {
        match self {
            DsnShape::Rect(s) => Some(s.transform_to_board(coordinate_transform)),
            DsnShape::Circle(s) => Some(s.transform_to_board(coordinate_transform)),
            DsnShape::Polygon(s) => Some(s.transform_to_board(coordinate_transform)),
            DsnShape::Path(s) => Some(s.transform_to_board(coordinate_transform)),
            DsnShape::PolylinePath(s) => s.transform_to_board(coordinate_transform),
        }
    }

    /// `Shape.transformToBoardRel` (Shape.java:622-627).
    #[must_use]
    pub fn transform_to_board_rel(
        &self,
        coordinate_transform: &CoordinateTransform,
    ) -> Option<Shape> {
        match self {
            DsnShape::Rect(s) => Some(s.transform_to_board_rel(coordinate_transform)),
            DsnShape::Circle(s) => Some(s.transform_to_board_rel(coordinate_transform)),
            DsnShape::Polygon(s) => Some(s.transform_to_board_rel(coordinate_transform)),
            DsnShape::Path(s) => Some(s.transform_to_board_rel(coordinate_transform)),
            DsnShape::PolylinePath(s) => s.transform_to_board_rel(coordinate_transform),
        }
    }

    /// `Shape.boundingBox` (Shape.java:619-620).
    #[must_use]
    pub fn bounding_box(&self) -> Option<DsnRectangle> {
        match self {
            DsnShape::Rect(s) => Some(s.bounding_box()),
            DsnShape::Circle(s) => Some(s.bounding_box()),
            DsnShape::Polygon(s) => Some(s.bounding_box()),
            DsnShape::Path(s) => Some(s.bounding_box()),
            DsnShape::PolylinePath(s) => s.bounding_box(),
        }
    }
}

/// `Shape.ReadAreaScopeResult` (Shape.java:629-645): the result of [`read_area_scope`]. The
/// first entry of `shape_list` is the area's border; the rest are its holes (windows).
#[derive(Debug, Clone, PartialEq)]
pub struct ReadAreaScopeResult {
    /// `ReadAreaScopeResult.shapeList`. Elements are `Option` because Java's `readAreaScope`
    /// adds an unchecked `null` for a `window` whose shape it could not read
    /// (Shape.java:222-223) — see [`transform_area_to_board`].
    pub shape_list: Vec<Option<DsnShape>>,
    /// `ReadAreaScopeResult.clearanceClassName`; `null` in Java when absent.
    pub clearance_class_name: Option<String>,
    /// `ReadAreaScopeResult.areaName`; `null` in Java when absent, and "may be generated later
    /// on" (Shape.java:637), hence not read-only.
    pub area_name: Option<String>,
}

/// `Shape.getLayer` (Shape.java:78-104): looks a layer name up in the layer structure, with
/// `pcb` and `signal` handled as the two shared multi-layer constants.
fn get_layer(layer_structure: Option<&DsnLayerStructure>, layer_name: &str) -> Option<DsnLayer> {
    if layer_name == ScopeKeyword::Pcb.name() {
        return Some(DsnLayer::pcb());
    }
    if layer_name == Keyword::Signal.name() {
        return Some(DsnLayer::signal());
    }
    let layer_structure = layer_structure?;
    let layer_index = layer_structure.get_no(layer_name)?;
    if layer_index >= layer_structure.layers.len() {
        return None;
    }
    Some(layer_structure.layers[layer_index].clone())
}

/// `Shape.readScope` (Shape.java:33-45): reads a shape scope, over-reading a leading `(`.
///
/// `layer_structure` is `None` for Java's `layerStructure == null`, where "only `Layer.PCB` and
/// `Layer.Signal` are expected, no individual layers" (Shape.java:30-31).
pub fn read_scope(
    scanner: &mut DsnScanner,
    layer_structure: Option<&DsnLayerStructure>,
) -> Result<Option<DsnShape>, DsnError> {
    let mut next_token = scanner.next_token()?;
    if next_token == Some(Token::Open) {
        // overread the open bracket
        next_token = scanner.next_token()?;
    }
    read_scope_from_keyword(scanner, next_token, layer_structure)
}

/// `Shape.readScopeFromKeyword` (Shape.java:50-69): the shape keyword has already been scanned.
pub fn read_scope_from_keyword(
    scanner: &mut DsnScanner,
    keyword: Option<Token>,
    layer_structure: Option<&DsnLayerStructure>,
) -> Result<Option<DsnShape>, DsnError> {
    match keyword {
        Some(Token::Kw(Keyword::Rectangle)) => {
            Ok(read_rectangle_scope(scanner, layer_structure)?.map(DsnShape::Rect))
        }
        Some(Token::Kw(Keyword::Polygon)) => {
            Ok(read_polygon_scope(scanner, layer_structure)?.map(DsnShape::Polygon))
        }
        Some(Token::Kw(Keyword::Circle)) => {
            Ok(read_circle_scope(scanner, layer_structure)?.map(DsnShape::Circle))
        }
        Some(Token::Kw(Keyword::PolygonPath)) => {
            Ok(read_polygon_path_scope(scanner, layer_structure)?.map(DsnShape::Path))
        }
        Some(Token::Kw(Keyword::PolylinePath)) => {
            Ok(read_polyline_path_scope(scanner, layer_structure)?.map(DsnShape::PolylinePath))
        }
        _ => {
            // Java discards `skipScope`'s boolean (Shape.java:67).
            let _ = skip_scope(scanner)?;
            Ok(None)
        }
    }
}

/// Collects every token up to (and consuming) the closing bracket, as the three list-shaped
/// shape readers below all do — Java accumulates into a `LinkedList<Object>` and only converts
/// the entries to numbers afterwards, so a non-numeric entry is diagnosed *after* the whole
/// scope has been consumed, not in the middle of it.
///
// totalized: Shape.readPolygonPathScope / Shape.readPolylinePathScope loop forever at
// end-of-file (Shape.java:456-467, :116-122) — `nextToken()` returns `null`, which is neither
// `CLOSED_BRACKET` nor a terminator, so `null` is appended to `cornerList` for as long as memory
// lasts. `readPolygonScope` is the one of the three that checks (Shape.java:350-356). The port
// answers end-of-file with `None`, i.e. the same "could not read this shape" the callers already
// handle, for all three.
fn read_tokens_to_close(
    scanner: &mut DsnScanner,
    skip_unknown_scopes: bool,
) -> Result<Option<Vec<Token>>, DsnError> {
    let mut result = Vec::new();
    loop {
        let mut next_token = scanner.next_token()?;
        if skip_unknown_scopes && next_token == Some(Token::Open) {
            // unknown scope
            let _ = skip_scope(scanner)?;
            next_token = scanner.next_token()?;
        }
        match next_token {
            Some(Token::Close) => return Ok(Some(result)),
            Some(token) => result.push(token),
            None => return Ok(None),
        }
    }
}

/// Java's `nextObject instanceof Double ? … : nextObject instanceof Integer ? … : warn+null`
/// (Shape.java:133-141 and its five siblings).
fn token_as_number(token: &Token) -> Option<f64> {
    match token {
        Token::Int(i) => Some(*i as f64),
        Token::Float(f) => Some(*f),
        _ => None,
    }
}

/// Converts a whole collected token list to `double`s, `None` as soon as one entry is not a
/// number.
fn tokens_as_numbers(tokens: &[Token]) -> Option<Vec<f64>> {
    tokens.iter().map(token_as_number).collect()
}

/// `Shape.readPolylinePathScope` (Shape.java:107-162).
pub fn read_polyline_path_scope(
    scanner: &mut DsnScanner,
    layer_structure: Option<&DsnLayerStructure>,
) -> Result<Option<DsnPolylinePath>, DsnError> {
    let layer_name = scanner.next_string();
    let layer = get_layer(layer_structure, &layer_name);

    // read the width and the corners of the path
    let Some(corner_list) = read_tokens_to_close(scanner, false)? else {
        return Ok(None);
    };
    if corner_list.len() < 5 {
        return Ok(None);
    }
    let Some(numbers) = tokens_as_numbers(&corner_list) else {
        return Ok(None);
    };
    // totalized: Shape.readPolylinePathScope builds a `PolylinePath` with a **null** layer when
    // the layer name is unknown — it is the one of the five readers that never re-checks
    // `getLayer`'s result (contrast Shape.java:479-481), and the null then NPEs in
    // `PolylinePath.writeScope`. The port returns `None` instead, the value every caller of a
    // shape reader already handles.
    let Some(layer) = layer else {
        return Ok(None);
    };
    let width = numbers[0];
    let corners = numbers[1..].to_vec();
    Ok(Some(DsnPolylinePath::new(layer, width, corners)))
}

/// `Shape.readAreaScope` (Shape.java:169-251): a shape that may contain holes. The first entry
/// of the result's `shape_list` is the border; the rest are the `window` scopes.
pub fn read_area_scope(
    scanner: &mut DsnScanner,
    layer_structure: Option<&DsnLayerStructure>,
    skip_window_scopes: bool,
) -> Result<Option<ReadAreaScopeResult>, DsnError> {
    let mut shape_list: Vec<Option<DsnShape>> = Vec::new();
    let mut clearance_class_name: Option<String> = None;
    let mut area_name: Option<String> = None;
    let mut result_ok = true;

    let mut next_token = scanner.next_token()?;
    if let Some(Token::Str(current_name)) = &next_token {
        let current_name = current_name.clone();
        scanner.set_scope_identifier(&current_name);
        if !current_name.is_empty() {
            area_name = Some(current_name);
        }
    }
    let current_shape = read_scope(scanner, layer_structure)?;
    if current_shape.is_none() {
        result_ok = false;
    }
    shape_list.push(current_shape);

    next_token = None;
    loop {
        let prev_token = next_token;
        next_token = scanner.next_token()?;
        let Some(token) = next_token.clone() else {
            // Java: "unexpected end of file", returning null.
            return Ok(None);
        };
        if token == Token::Close {
            // end of scope
            break;
        }
        if prev_token == Some(Token::Open) {
            // a new scope is expected
            if token == Token::Kw(Keyword::Window) && !skip_window_scopes {
                let hole_shape = read_scope(scanner, layer_structure)?;
                shape_list.push(hole_shape);
                // overread closing bracket
                next_token = scanner.next_token()?;
                if next_token != Some(Token::Close) {
                    return Ok(None);
                }
            } else if token == Token::Kw(Keyword::ClearanceClass) {
                clearance_class_name = Some(read_string_scope(scanner)?);
            } else {
                // skip unknown scope
                let _ = skip_scope(scanner)?;
            }
        }
    }
    if !result_ok {
        return Ok(None);
    }
    Ok(Some(ReadAreaScopeResult {
        shape_list,
        clearance_class_name,
        area_name,
    }))
}

/// `Shape.readRectangleScope` (Shape.java:257-298).
pub fn read_rectangle_scope(
    scanner: &mut DsnScanner,
    layer_structure: Option<&DsnLayerStructure>,
) -> Result<Option<DsnRectangle>, DsnError> {
    let layer_name = scanner.next_string();
    let mut rect_layer = get_layer(layer_structure, &layer_name);
    if rect_layer.is_none() {
        rect_layer = get_layer(layer_structure, Keyword::Signal.name());
    }

    let mut rect_coor = [0.0_f64; 4];
    // fill the rectangle
    for coordinate in &mut rect_coor {
        match scanner.next_token()? {
            Some(Token::Int(i)) => *coordinate = i as f64,
            Some(Token::Float(f)) => *coordinate = f,
            _ => return Ok(None),
        }
    }
    // overread the closing bracket
    if scanner.next_token()? != Some(Token::Close) {
        return Ok(None);
    }
    let Some(rect_layer) = rect_layer else {
        return Ok(None);
    };
    Ok(Some(DsnRectangle::new(rect_layer, rect_coor)))
}

/// `Shape.readPolygonScope` (Shape.java:304-391).
///
/// Unlike the other four readers this one takes the layer from a **token**, not from
/// `nextString()`: `pcb` and `signal` arrive as keywords, anything else as a string
/// (Shape.java:308-340).
pub fn read_polygon_scope(
    scanner: &mut DsnScanner,
    layer_structure: Option<&DsnLayerStructure>,
) -> Result<Option<DsnPolygon>, DsnError> {
    let mut polygon_layer: Option<DsnLayer> = None;
    let mut layer_ok = true;
    let next_token = scanner.next_token()?;
    if next_token == Some(Token::Kw(Keyword::PcbScope)) {
        polygon_layer = Some(DsnLayer::pcb());
    } else if next_token == Some(Token::Kw(Keyword::Signal)) {
        polygon_layer = Some(DsnLayer::signal());
    } else {
        let Some(layer_structure) = layer_structure else {
            return Ok(None);
        };
        let Some(Token::Str(layer_name)) = next_token else {
            return Ok(None);
        };
        match layer_structure.get_no(&layer_name) {
            Some(layer_index) if layer_index < layer_structure.layers.len() => {
                polygon_layer = Some(layer_structure.layers[layer_index].clone());
            }
            _ => layer_ok = false,
        }
    }

    // overread the aperture width
    let _ = scanner.next_token()?;

    // read the coordinates of the polygon
    let Some(coor_list) = read_tokens_to_close(scanner, true)? else {
        return Ok(None);
    };
    if !layer_ok {
        return Ok(None);
    }
    let Some(coor_arr) = tokens_as_numbers(&coor_list) else {
        return Ok(None);
    };
    // totalized: `polygonLayer` is still `null` here whenever `layerOk` stayed true but no branch
    // assigned it — unreachable in practice, since the only path that leaves it null also clears
    // `layerOk`. The port's `Option` makes the impossible state a `None` return rather than a
    // deferred NPE.
    let Some(polygon_layer) = polygon_layer else {
        return Ok(None);
    };
    Ok(Some(DsnPolygon::new(polygon_layer, coor_arr)))
}

/// `Shape.readCircleScope` (Shape.java:394-444). More than three numbers before the closing
/// bracket is an error (Shape.java:417-423).
pub fn read_circle_scope(
    scanner: &mut DsnScanner,
    layer_structure: Option<&DsnLayerStructure>,
) -> Result<Option<DsnCircle>, DsnError> {
    let layer_name = scanner.next_string();
    let circle_layer = get_layer(layer_structure, &layer_name);

    // fill the coordinates
    let mut circle_coor = [0.0_f64; 3];
    let mut current_index = 0_usize;
    loop {
        let next_token = scanner.next_token()?;
        if next_token == Some(Token::Close) {
            break;
        }
        if current_index > 2 {
            return Ok(None);
        }
        match next_token {
            Some(Token::Int(i)) => circle_coor[current_index] = i as f64,
            Some(Token::Float(f)) => circle_coor[current_index] = f,
            _ => return Ok(None),
        }
        current_index += 1;
    }

    let Some(circle_layer) = circle_layer else {
        return Ok(None);
    };
    Ok(Some(DsnCircle::new(circle_layer, circle_coor)))
}

/// `Shape.readPolygonPathScope` (Shape.java:447-516).
pub fn read_polygon_path_scope(
    scanner: &mut DsnScanner,
    layer_structure: Option<&DsnLayerStructure>,
) -> Result<Option<DsnPolygonPath>, DsnError> {
    let layer_name = scanner.next_string();
    let layer = get_layer(layer_structure, &layer_name);

    // read the width and the corners of the path
    let Some(corner_list) = read_tokens_to_close(scanner, true)? else {
        return Ok(None);
    };
    // cornerList contains width + coordinate pairs
    if corner_list.len() < 5 {
        // Single-point paths are not valid traces, skip them
        return Ok(None);
    }
    let Some(layer) = layer else {
        return Ok(None);
    };
    let Some(numbers) = tokens_as_numbers(&corner_list) else {
        return Ok(None);
    };
    let width = numbers[0];
    let coordinate_arr = numbers[1..].to_vec();
    Ok(Some(DsnPolygonPath::new(layer, width, coordinate_arr)))
}

/// `Shape.transformAreaToBoard` (Shape.java:518-555): the first shape is the border, the rest
/// are holes.
///
// totalized: transformAreaToBoard — Java dereferences a `null` border or hole here and throws a
// `NullPointerException` (`readAreaScope` can put a `null` hole in the list, Shape.java:222-223,
// without clearing `resultOk`). The port returns `None`, the same value Java's three other
// failure paths in this method already return, so the caller takes the branch it already has for
// "could not read this area" instead of the process dying mid-read.
#[must_use]
pub fn transform_area_to_board(
    area: &[Option<DsnShape>],
    coordinate_transform: &CoordinateTransform,
) -> Option<Area> {
    transform_area(area, coordinate_transform, false)
}

/// `Shape.transformAreaToBoardRel` (Shape.java:557-594) — identical but for
/// `transformToBoardRel` on each shape.
// totalized: transformAreaToBoardRel — see `transform_area_to_board`.
#[must_use]
pub fn transform_area_to_board_rel(
    area: &[Option<DsnShape>],
    coordinate_transform: &CoordinateTransform,
) -> Option<Area> {
    transform_area(area, coordinate_transform, true)
}

/// The shared body of the two `transformArea…` methods, which differ only in which of
/// `transformToBoard`/`transformToBoardRel` they call (Shape.java:522-554 == :561-593).
fn transform_area(
    area: &[Option<DsnShape>],
    coordinate_transform: &CoordinateTransform,
    relative: bool,
) -> Option<Area> {
    let transform = |shape: &DsnShape| {
        if relative {
            shape.transform_to_board_rel(coordinate_transform)
        } else {
            shape.transform_to_board(coordinate_transform)
        }
    };

    if area.is_empty() {
        // Java: `holeCount <= -1`, i.e. an empty collection.
        return None;
    }
    let hole_count = area.len() - 1;
    let boundary_shape = transform(area[0].as_ref()?)?;
    if hole_count == 0 {
        return Some(Area::Shape(boundary_shape));
    }
    // Area with holes
    let border = to_polyline_shape(&boundary_shape)?;
    let mut holes = Vec::with_capacity(hole_count);
    for hole in &area[1..] {
        let hole_shape = transform(hole.as_ref()?)?;
        holes.push(to_polyline_shape(&hole_shape)?);
    }
    Some(Area::Polyline(PolylineArea::new(border, holes)))
}

/// Java's `shape instanceof PolylineShape` narrowing — `TileShape` and `PolygonShape` implement
/// `PolylineShape`, `Circle` does not.
fn to_polyline_shape(shape: &Shape) -> Option<PolylineShapeRef> {
    match shape {
        Shape::Tile(t) => Some(PolylineShapeRef::Tile(t.clone())),
        Shape::Polygon(p) => Some(PolylineShapeRef::Polygon(p.clone())),
        Shape::Circle(_) => None,
    }
}

/// `geometry.planar.FloatPoint.boundingOctagon(FloatPoint[])` (FloatPoint.java:37-66), which
/// `fr-geometry` records as `not ported:` — reproduced here because
/// [`DsnPolygonPath::transform_to_board`] is its only caller in the ported subset
/// (PolygonPath.java:70,96).
fn bounding_octagon(points: &[FloatPoint]) -> IntOctagon {
    let mut min_x = f64::from(i32::MAX);
    let mut min_y = f64::from(i32::MAX);
    let mut max_x = f64::from(i32::MIN);
    let mut max_y = f64::from(i32::MIN);
    let mut min_upper_left_diagonal_x = f64::from(i32::MAX);
    let mut max_lower_right_diagonal_x = f64::from(i32::MIN);
    let mut min_lower_left_diagonal_x = f64::from(i32::MAX);
    let mut max_upper_right_diagonal_x = f64::from(i32::MIN);
    for current in points {
        min_x = min_x.min(current.x);
        min_y = min_y.min(current.y);
        max_x = max_x.max(current.x);
        max_y = max_y.max(current.y);
        let mut tmp = current.x - current.y;
        min_upper_left_diagonal_x = min_upper_left_diagonal_x.min(tmp);
        max_lower_right_diagonal_x = max_lower_right_diagonal_x.max(tmp);
        tmp = current.x + current.y;
        min_lower_left_diagonal_x = min_lower_left_diagonal_x.min(tmp);
        max_upper_right_diagonal_x = max_upper_right_diagonal_x.max(tmp);
    }
    IntOctagon::new(
        min_x.floor() as i32,
        min_y.floor() as i32,
        max_x.ceil() as i32,
        max_y.ceil() as i32,
        min_upper_left_diagonal_x.floor() as i32,
        max_lower_right_diagonal_x.ceil() as i32,
        min_lower_left_diagonal_x.floor() as i32,
        max_upper_right_diagonal_x.ceil() as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_no_prefers_an_exact_name_over_the_electra_fallback() {
        let layers = DsnLayerStructure::new(vec![
            DsnLayer::new("A", 0, true),
            DsnLayer::new("Bottom", 1, true),
            DsnLayer::new("C", 2, true),
        ]);
        assert_eq!(layers.get_no("Bottom"), Some(1));
    }

    #[test]
    fn rectangle_union_takes_the_layer_of_the_receiver() {
        let a = DsnRectangle::new(DsnLayer::pcb(), [0.0, 0.0, 1.0, 1.0]);
        let b = DsnRectangle::new(DsnLayer::signal(), [-1.0, -1.0, 0.5, 3.0]);
        let u = a.union(&b);
        assert_eq!(u.coor, [-1.0, -1.0, 1.0, 3.0]);
        assert_eq!(u.layer.name, "pcb");
    }

    #[test]
    fn polygon_bounding_box_of_an_empty_coordinate_array_is_javas_inverted_seed() {
        let polygon = DsnPolygon::new(DsnLayer::pcb(), Vec::new());
        assert_eq!(
            polygon.bounding_box().coor,
            [
                f64::from(i32::MAX),
                f64::from(i32::MAX),
                f64::from(i32::MIN),
                f64::from(i32::MIN)
            ]
        );
    }

    #[test]
    fn polyline_path_transforms_are_javas_unimplemented_nulls() {
        let path = DsnPolylinePath::new(DsnLayer::pcb(), 1.0, vec![0.0, 0.0, 1.0, 1.0]);
        let transform = CoordinateTransform::new(1.0, 0.0, 0.0);
        assert!(path.transform_to_board(&transform).is_none());
        assert!(path.transform_to_board_rel(&transform).is_none());
        assert!(path.bounding_box().is_none());
    }
}
