#![allow(dead_code)]

use fr_board::prelude::*;
use fr_geometry::{
    Circle, IntBox, IntPoint, IntVector, Point, Polyline, PolylineShapeRef, Shape, TileShape,
};

pub const BOUNDING_BOX: IntBox = IntBox {
    ll: IntPoint {
        x: -100_000,
        y: -100_000,
    },
    ur: IntPoint {
        x: 100_000,
        y: 100_000,
    },
};

pub fn two_layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("F.Cu", true), Layer::new("B.Cu", true)])
}

pub struct PadSpec {
    pub name: &'static str,
    pub half: i32,
    pub offset: IntVector,
    pub through_hole: bool,
}

pub struct SyntheticBoard {
    pub board: Board,
    pub via_padstack: PadstackId,
    pub microvia_padstack: PadstackId,
}

impl SyntheticBoard {
    pub fn new(pads: &[PadSpec], net_count: usize, clearance: i32) -> SyntheticBoard {
        let ls = two_layers();
        let mut padstacks = Padstacks::new(ls.clone());
        let mut pins = Vec::new();
        for pad in pads {
            let shape = Shape::Tile(TileShape::Box(IntBox::from_coords(
                -pad.half, -pad.half, pad.half, pad.half,
            )));
            let shapes = if pad.through_hole {
                vec![Some(shape.clone()), Some(shape)]
            } else {
                vec![Some(shape), None]
            };
            let padstack = padstacks.add(pad.name, shapes, pad.through_hole, false);
            pins.push(PackagePin::new(pad.name, padstack, pad.offset.into(), 0.0));
        }
        let via_shape = Shape::Circle(Circle::new(IntPoint::new(0, 0), 3000));
        let via_padstack = padstacks.add(
            "Via[0-1]_600:300_um",
            vec![Some(via_shape.clone()), Some(via_shape)],
            true,
            false,
        );
        let micro_shape = Shape::Circle(Circle::new(IntPoint::new(0, 0), 1500));
        let microvia_padstack = padstacks.add(
            "Via[0-1]_300:100_um",
            vec![Some(micro_shape.clone()), Some(micro_shape)],
            true,
            false,
        );
        let mut packages = Packages::new();
        let package = packages.add(
            "pkg",
            pins,
            None,
            None,
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            true,
        );
        let mut components = Components::new();
        components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, package);

        let matrix = ClearanceMatrix::get_default_instance(&ls, clearance);
        let mut rules = BoardRules::new(ls, matrix);
        rules.create_default_net_class();
        let default_class = rules.get_default_net_class();
        let mut board = Board::new(
            Vec::new(),
            1,
            BOUNDING_BOX,
            rules,
            BoardLibrary::new(padstacks, packages),
            components,
            Communication::default(),
        );
        for i in 0..net_count {
            board
                .rules
                .nets
                .add(format!("N{}", i + 1), 1, false, default_class);
        }
        board.insert_outline(
            vec![PolylineShapeRef::Tile(TileShape::Box(IntBox::from_coords(
                -50_000, -50_000, 50_000, 50_000,
            )))],
            1,
        );
        SyntheticBoard {
            board,
            via_padstack,
            microvia_padstack,
        }
    }

    pub fn pin(&mut self, pin_index: i32, net: i32) -> ItemId {
        self.board
            .insert_pin(1, pin_index, vec![net], 1, FixedState::Unfixed)
    }

    pub fn trace(
        &mut self,
        points: &[(i32, i32)],
        layer: usize,
        half_width: i32,
        net: i32,
    ) -> ItemId {
        let points: Vec<Point> = points.iter().map(|(x, y)| Point::new(*x, *y)).collect();
        self.board
            .insert_trace_without_cleaning(
                Polyline::from_points(&points),
                layer,
                half_width,
                vec![net],
                1,
                FixedState::Unfixed,
            )
            .expect("a synthetic trace has at least two distinct corners")
    }

    pub fn via(&mut self, x: i32, y: i32, net: i32) -> ItemId {
        self.board
            .insert_via(
                self.via_padstack,
                Point::new(x, y),
                vec![net],
                1,
                FixedState::Unfixed,
                true,
            )
            .expect("a synthetic via inserts")
    }

    pub fn microvia(&mut self, x: i32, y: i32, net: i32) -> ItemId {
        self.board
            .insert_via(
                self.microvia_padstack,
                Point::new(x, y),
                vec![net],
                1,
                FixedState::Unfixed,
                true,
            )
            .expect("a synthetic microvia inserts")
    }
}
