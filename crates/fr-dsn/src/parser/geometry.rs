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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsnLayer {
    pub name: String,
    pub no: i32,
    pub is_signal: bool,
    pub net_names: Vec<String>,
}

impl DsnLayer {
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

    #[must_use]
    pub fn new(name: impl Into<String>, no: i32, is_signal: bool) -> DsnLayer {
        DsnLayer::with_nets(name, no, is_signal, Vec::new())
    }

    #[must_use]
    pub fn pcb() -> DsnLayer {
        DsnLayer::new("pcb", -1, false)
    }

    #[must_use]
    pub fn signal() -> DsnLayer {
        DsnLayer::new("signal", -1, true)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DsnLayerStructure {
    pub layers: Vec<DsnLayer>,
}

impl DsnLayerStructure {
    #[must_use]
    pub fn new(layers: Vec<DsnLayer>) -> DsnLayerStructure {
        DsnLayerStructure { layers }
    }

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

    #[must_use]
    pub fn get_no(&self, name: &str) -> Option<usize> {
        for (i, layer) in self.layers.iter().enumerate() {
            if name == layer.name {
                return Some(i);
            }
        }
        if name.contains("Top") {
            return Some(0);
        }
        if name.contains("Bottom") {
            return self.layers.len().checked_sub(1);
        }
        None
    }

    #[must_use]
    pub fn signal_layer_count(&self) -> usize {
        self.layers.iter().filter(|l| l.is_signal).count()
    }

    #[must_use]
    pub fn contains_plane(&self, net_name: &str) -> bool {
        self.layers
            .iter()
            .any(|l| !l.is_signal && l.net_names.iter().any(|n| n == net_name))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DsnRectangle {
    pub layer: DsnLayer,
    pub coor: [f64; 4],
}

impl DsnRectangle {
    #[must_use]
    pub fn new(layer: DsnLayer, coor: [f64; 4]) -> DsnRectangle {
        DsnRectangle { layer, coor }
    }

    #[must_use]
    pub fn bounding_box(&self) -> DsnRectangle {
        self.clone()
    }

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

    #[must_use]
    pub fn transform_to_board_rel(&self, coordinate_transform: &CoordinateTransform) -> Shape {
        let mut box_coor = [0_i32; 4];
        for (target, source) in box_coor.iter_mut().zip(self.coor) {
            *target = java_round_to_int(coordinate_transform.dsn_to_board(source));
        }
        let result = if box_coor[1] <= box_coor[3] {
            IntBox::from_coords(box_coor[0], box_coor[1], box_coor[2], box_coor[3])
        } else {
            IntBox::from_coords(box_coor[0], box_coor[3], box_coor[2], box_coor[1])
        };
        Shape::Tile(TileShape::Box(result))
    }

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

#[derive(Debug, Clone, PartialEq)]
pub struct DsnCircle {
    pub layer: DsnLayer,
    pub coor: [f64; 3],
}

impl DsnCircle {
    #[must_use]
    pub fn new(layer: DsnLayer, coor: [f64; 3]) -> DsnCircle {
        DsnCircle { layer, coor }
    }

    #[must_use]
    pub fn transform_to_board(&self, coordinate_transform: &CoordinateTransform) -> Shape {
        let center = coordinate_transform
            .dsn_to_board_point(&[self.coor[1], self.coor[2]])
            .round();
        let radius = java_round_to_int(coordinate_transform.dsn_to_board(self.coor[0]) / 2.0);
        Shape::Circle(Circle::new(center, radius))
    }

    #[must_use]
    pub fn transform_to_board_rel(&self, coordinate_transform: &CoordinateTransform) -> Shape {
        let radius = java_round_to_int(coordinate_transform.dsn_to_board(self.coor[0]) / 2.0);
        let x = java_round_to_int(coordinate_transform.dsn_to_board(self.coor[1]));
        let y = java_round_to_int(coordinate_transform.dsn_to_board(self.coor[2]));
        Shape::Circle(Circle::new(IntPoint::new(x, y), radius))
    }

    #[must_use]
    pub fn bounding_box(&self) -> DsnRectangle {
        let radius = self.coor[0] / 2.0;
        DsnRectangle::new(
            self.layer.clone(),
            [
                self.coor[1] - radius,
                self.coor[2] - radius,
                self.coor[1] + radius,
                self.coor[2] + radius,
            ],
        )
    }

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

#[derive(Debug, Clone, PartialEq)]
pub struct DsnPolygon {
    pub layer: DsnLayer,
    pub coor: Vec<f64>,
}

impl DsnPolygon {
    #[must_use]
    pub fn new(layer: DsnLayer, coor: Vec<f64>) -> DsnPolygon {
        DsnPolygon { layer, coor }
    }

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
                bounds[0] = bounds[0].min(*c);
                bounds[2] = bounds[2].max(*c);
            } else {
                bounds[1] = bounds[1].min(*c);
                bounds[3] = bounds[3].max(*c);
            }
        }
        DsnRectangle::new(self.layer.clone(), bounds)
    }

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

#[derive(Debug, Clone, PartialEq)]
pub struct DsnPolygonPath {
    pub layer: DsnLayer,
    pub width: f64,
    pub coordinate_arr: Vec<f64>,
}

impl DsnPolygonPath {
    #[must_use]
    pub fn new(layer: DsnLayer, width: f64, coordinate_arr: Vec<f64>) -> DsnPolygonPath {
        DsnPolygonPath {
            layer,
            width,
            coordinate_arr,
        }
    }

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
                bounds[0] = bounds[0].min(*c - offset);
                bounds[2] = bounds[2].max(*c) + offset;
            } else {
                bounds[1] = bounds[1].min(*c - offset);
                bounds[3] = bounds[3].max(*c + offset);
            }
        }
        DsnRectangle::new(self.layer.clone(), bounds)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DsnPolylinePath {
    pub layer: DsnLayer,
    pub width: f64,
    pub coordinate_arr: Vec<f64>,
}

impl DsnPolylinePath {
    #[must_use]
    pub fn new(layer: DsnLayer, width: f64, coordinate_arr: Vec<f64>) -> DsnPolylinePath {
        DsnPolylinePath {
            layer,
            width,
            coordinate_arr,
        }
    }

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

    #[must_use]
    pub fn transform_to_board_rel(
        &self,
        _coordinate_transform: &CoordinateTransform,
    ) -> Option<Shape> {
        None
    }

    #[must_use]
    pub fn transform_to_board(&self, _coordinate_transform: &CoordinateTransform) -> Option<Shape> {
        None
    }

    #[must_use]
    pub fn bounding_box(&self) -> Option<DsnRectangle> {
        None
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DsnShape {
    Rect(DsnRectangle),
    Circle(DsnCircle),
    Polygon(DsnPolygon),
    Path(DsnPolygonPath),
    PolylinePath(DsnPolylinePath),
}

impl DsnShape {
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

    pub fn layer_mut(&mut self) -> &mut DsnLayer {
        match self {
            DsnShape::Rect(s) => &mut s.layer,
            DsnShape::Circle(s) => &mut s.layer,
            DsnShape::Polygon(s) => &mut s.layer,
            DsnShape::Path(s) => &mut s.layer,
            DsnShape::PolylinePath(s) => &mut s.layer,
        }
    }

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

#[derive(Debug, Clone, PartialEq)]
pub struct ReadAreaScopeResult {
    pub shape_list: Vec<Option<DsnShape>>,
    pub clearance_class_name: Option<String>,
    pub area_name: Option<String>,
}

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

pub fn read_scope(
    scanner: &mut DsnScanner,
    layer_structure: Option<&DsnLayerStructure>,
) -> Result<Option<DsnShape>, DsnError> {
    let mut next_token = scanner.next_token()?;
    if next_token == Some(Token::Open) {
        next_token = scanner.next_token()?;
    }
    read_scope_from_keyword(scanner, next_token, layer_structure)
}

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
            let _ = skip_scope(scanner)?;
            Ok(None)
        }
    }
}

fn read_tokens_to_close(
    scanner: &mut DsnScanner,
    skip_unknown_scopes: bool,
) -> Result<Option<Vec<Token>>, DsnError> {
    let mut result = Vec::new();
    loop {
        let mut next_token = scanner.next_token()?;
        if skip_unknown_scopes && next_token == Some(Token::Open) {
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

fn token_as_number(token: &Token) -> Option<f64> {
    match token {
        Token::Int(i) => Some(*i as f64),
        Token::Float(f) => Some(*f),
        _ => None,
    }
}

fn tokens_as_numbers(tokens: &[Token]) -> Option<Vec<f64>> {
    tokens.iter().map(token_as_number).collect()
}

pub fn read_polyline_path_scope(
    scanner: &mut DsnScanner,
    layer_structure: Option<&DsnLayerStructure>,
) -> Result<Option<DsnPolylinePath>, DsnError> {
    let layer_name = scanner.next_string();
    let layer = get_layer(layer_structure, &layer_name);

    let Some(corner_list) = read_tokens_to_close(scanner, false)? else {
        return Ok(None);
    };
    if corner_list.len() < 5 {
        return Ok(None);
    }
    let Some(numbers) = tokens_as_numbers(&corner_list) else {
        return Ok(None);
    };
    let Some(layer) = layer else {
        return Ok(None);
    };
    let width = numbers[0];
    let corners = numbers[1..].to_vec();
    Ok(Some(DsnPolylinePath::new(layer, width, corners)))
}

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
            return Ok(None);
        };
        if token == Token::Close {
            break;
        }
        if prev_token == Some(Token::Open) {
            if token == Token::Kw(Keyword::Window) && !skip_window_scopes {
                let hole_shape = read_scope(scanner, layer_structure)?;
                shape_list.push(hole_shape);
                next_token = scanner.next_token()?;
                if next_token != Some(Token::Close) {
                    return Ok(None);
                }
            } else if token == Token::Kw(Keyword::ClearanceClass) {
                clearance_class_name = Some(read_string_scope(scanner)?);
            } else {
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
    for coordinate in &mut rect_coor {
        match scanner.next_token()? {
            Some(Token::Int(i)) => *coordinate = i as f64,
            Some(Token::Float(f)) => *coordinate = f,
            _ => return Ok(None),
        }
    }
    if scanner.next_token()? != Some(Token::Close) {
        return Ok(None);
    }
    let Some(rect_layer) = rect_layer else {
        return Ok(None);
    };
    Ok(Some(DsnRectangle::new(rect_layer, rect_coor)))
}

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

    let _ = scanner.next_token()?;

    let Some(coor_list) = read_tokens_to_close(scanner, true)? else {
        return Ok(None);
    };
    if !layer_ok {
        return Ok(None);
    }
    let Some(coor_arr) = tokens_as_numbers(&coor_list) else {
        return Ok(None);
    };
    let Some(polygon_layer) = polygon_layer else {
        return Ok(None);
    };
    Ok(Some(DsnPolygon::new(polygon_layer, coor_arr)))
}

pub fn read_circle_scope(
    scanner: &mut DsnScanner,
    layer_structure: Option<&DsnLayerStructure>,
) -> Result<Option<DsnCircle>, DsnError> {
    let layer_name = scanner.next_string();
    let circle_layer = get_layer(layer_structure, &layer_name);

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

pub fn read_polygon_path_scope(
    scanner: &mut DsnScanner,
    layer_structure: Option<&DsnLayerStructure>,
) -> Result<Option<DsnPolygonPath>, DsnError> {
    let layer_name = scanner.next_string();
    let layer = get_layer(layer_structure, &layer_name);

    let Some(corner_list) = read_tokens_to_close(scanner, true)? else {
        return Ok(None);
    };
    if corner_list.len() < 5 {
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

#[must_use]
pub fn transform_area_to_board(
    area: &[Option<DsnShape>],
    coordinate_transform: &CoordinateTransform,
) -> Option<Area> {
    transform_area(area, coordinate_transform, false)
}

#[must_use]
pub fn transform_area_to_board_rel(
    area: &[Option<DsnShape>],
    coordinate_transform: &CoordinateTransform,
) -> Option<Area> {
    transform_area(area, coordinate_transform, true)
}

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
        return None;
    }
    let hole_count = area.len() - 1;
    let boundary_shape = transform(area[0].as_ref()?)?;
    if hole_count == 0 {
        return Some(Area::Shape(boundary_shape));
    }
    let border = to_polyline_shape(&boundary_shape)?;
    let mut holes = Vec::with_capacity(hole_count);
    for hole in &area[1..] {
        let hole_shape = transform(hole.as_ref()?)?;
        holes.push(to_polyline_shape(&hole_shape)?);
    }
    Some(Area::Polyline(PolylineArea::new(border, holes)))
}

fn to_polyline_shape(shape: &Shape) -> Option<PolylineShapeRef> {
    match shape {
        Shape::Tile(t) => Some(PolylineShapeRef::Tile(t.clone())),
        Shape::Polygon(p) => Some(PolylineShapeRef::Polygon(p.clone())),
        Shape::Circle(_) => None,
    }
}

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
        let transform = CoordinateTransform::new(1.0, 0.0, 0.0).expect("a valid scale");
        assert!(path.transform_to_board(&transform).is_none());
        assert!(path.transform_to_board_rel(&transform).is_none());
        assert!(path.bounding_box().is_none());
    }
}
