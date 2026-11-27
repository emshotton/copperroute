use copper_board::Item;
use copper_dsn::error::BoardReadResult;
use copper_geometry::{FloatPoint, Point, ShapeOps};
use serde_json::json;

fn board(pad: serde_json::Value) -> copper_board::Board {
    let input = json!({
        "layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
        "resolution":10000,
        "components":[{"reference":"J1", "position":{"x":10,"y":10}, "rotation":-90,
          "layer":"F.Cu", "pads":[pad]}],
        "outline":{"ordered":true, "clearance":0.5, "corners":[{"x":0,"y":0},{"x":20,"y":0},{"x":20,"y":20},{"x":0,"y":20}],
          "cutouts":[[{"x":2,"y":2},{"x":3,"y":2},{"x":3,"y":3},{"x":2,"y":3}]]}
    });
    match copper_dsn::kicad::read_board(&input.to_string(), None) {
        BoardReadResult::Success {
            board: Some(board), ..
        } => *board,
        result => panic!("{result:?}"),
    }
}

#[test]
fn offset_copper_rotates_without_moving_the_hole_anchor() {
    let board = board(json!({"name":"1", "shape":"rect", "size":{"x":1.3,"y":0.8},
        "drill":0.6,"shapeOffset":{"x":0.25,"y":0}, "layers":["F.Cu","B.Cu"]}));
    let ctx = board.ctx();
    let pin = board
        .get_items()
        .find_map(|i| if let Item::Pin(p) = i { Some(p) } else { None })
        .unwrap();
    assert_eq!(
        pin.get_center(&ctx).to_float(),
        FloatPoint::new(100000.0, -100000.0)
    );
    let shape = pin.get_shape_on_layer(0, &ctx).unwrap();
    assert!(shape.contains_inside(&Point::from(copper_geometry::IntPoint::new(100000, -92000))));
    assert!(
        !shape.contains_inside(&Point::from(copper_geometry::IntPoint::new(
            100000, -108000
        )))
    );
    assert_eq!(pin.get_padstack(&ctx).unwrap().drill_diameter, Some(6000.0));
    assert_eq!(
        board
            .get_items()
            .filter(|i| matches!(i, Item::ObstacleArea(_)))
            .count(),
        2
    );
    for item in board
        .get_items()
        .filter(|i| matches!(i, Item::ObstacleArea(_)))
    {
        assert!(
            board
                .rules
                .clearance_matrix
                .get_value(item.header().clearance_class(), 1, 0, false)
                >= 5000
        );
    }
}

#[test]
fn a_custom_polygon_keeps_its_clipped_corner() {
    let board = board(
        json!({"name":"1", "shape":"custom", "size":{"x":1,"y":1},"drill":0.6,
      "copperPolygon":[{"x":-1,"y":-1},{"x":0,"y":-1},{"x":1,"y":0},{"x":1,"y":1},{"x":-1,"y":1}],
      "layers":["F.Cu","B.Cu"]}),
    );
    let ctx = board.ctx();
    let pin = board
        .get_items()
        .find_map(|i| if let Item::Pin(p) = i { Some(p) } else { None })
        .unwrap();
    let shape = pin.get_shape_on_layer(0, &ctx).unwrap();
    assert!(
        shape.contains_inside(&Point::from(copper_geometry::IntPoint::new(
            100000, -100000
        )))
    );
    assert!(!shape.contains_inside(&Point::from(copper_geometry::IntPoint::new(92000, -92000))));
}

#[test]
fn ordered_concave_outlines_keep_their_boundary_edges() {
    let corners = [
        (0, 0),
        (10, 0),
        (10, 2),
        (2, 2),
        (2, 8),
        (10, 8),
        (10, 10),
        (0, 10),
    ];
    let input = json!({"layers":[{"name":"F.Cu"},{"name":"B.Cu"}], "resolution":10000,
        "outline":{"ordered":true,"corners":corners.iter().map(|(x,y)|json!({"x":x,"y":y})).collect::<Vec<_>>()}});
    let BoardReadResult::Success {
        board: Some(board), ..
    } = copper_dsn::kicad::read_board(&input.to_string(), None)
    else {
        panic!("load failed")
    };
    let Item::BoardOutline(outline) = board.get_item(board.get_outline().unwrap()).unwrap() else {
        panic!("missing outline")
    };
    let ps = outline.get_shape(0).unwrap().as_ops().bounded_corners();
    assert_eq!(ps.len(), corners.len());
    for (i, p) in ps.iter().enumerate() {
        let q = ps[(i + 1) % ps.len()].to_float();
        let p = p.to_float();
        assert!(corners.iter().enumerate().any(|(j, (x, y))| {
            let a = FloatPoint::new(f64::from(*x) * 10000.0, -f64::from(*y) * 10000.0);
            let (x, y) = corners[(j + 1) % corners.len()];
            let b = FloatPoint::new(f64::from(x) * 10000.0, -f64::from(y) * 10000.0);
            (p == a && q == b) || (p == b && q == a)
        }));
    }
}

#[test]
fn an_offset_nonplated_pad_can_have_copper_outside_its_hole() {
    let board = board(
        json!({"name":"1", "shape":"circle", "size":{"x":0.4,"y":0.4},
        "drill":0.5,"nonPlated":true,"shapeOffset":{"x":0.1,"y":0}, "layers":["F.Cu","B.Cu"]}),
    );
    let ctx = board.ctx();
    let pin = board
        .get_items()
        .find_map(|i| if let Item::Pin(p) = i { Some(p) } else { None })
        .unwrap();
    assert!(!pin.get_padstack(&ctx).unwrap().hole_only);
}

fn unrotated_board(pad: serde_json::Value) -> copper_board::Board {
    let input = json!({
        "layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
        "resolution":10000,
        "components":[{"reference":"J1", "position":{"x":10,"y":10}, "rotation":0,
          "layer":"F.Cu", "pads":[pad]}],
        "outline":{"ordered":true, "clearance":0.5, "corners":[{"x":0,"y":0},{"x":20,"y":0},{"x":20,"y":20},{"x":0,"y":20}]}
    });
    match copper_dsn::kicad::read_board(&input.to_string(), None) {
        BoardReadResult::Success {
            board: Some(board), ..
        } => *board,
        result => panic!("{result:?}"),
    }
}

fn l_shaped_pad_board() -> copper_board::Board {
    unrotated_board(
        json!({"name":"1", "shape":"custom", "size":{"x":1,"y":1},"drill":0.4,
      "copperPolygon":[{"x":-0.5,"y":0.5},{"x":-0.5,"y":-0.5},{"x":0.5,"y":-0.5},{"x":0.5,"y":0.5}],
      "extraCopperPolygons":[[{"x":-0.5,"y":-0.5},{"x":4,"y":-0.5},{"x":4,"y":0.5},{"x":0.5,"y":0.5},
        {"x":0.5,"y":4},{"x":-0.5,"y":4}]],
      "layers":["F.Cu","B.Cu"]}),
    )
}

#[test]
fn a_concave_custom_pad_becomes_several_convex_shapes_per_layer() {
    let board = l_shaped_pad_board();
    let ctx = board.ctx();
    let (id, pin) = board
        .get_items()
        .find_map(|i| {
            if let Item::Pin(p) = i {
                Some((i.id(), p))
            } else {
                None
            }
        })
        .unwrap();
    let pieces_per_layer = pin.shapes_on_layer(0, &ctx).len();
    assert!(
        pieces_per_layer >= 3,
        "the anchor and at least two pieces of the L"
    );
    assert_eq!(pin.shapes_on_layer(1, &ctx).len(), pieces_per_layer);
    assert_eq!(pin.tile_shape_count(&ctx), 2 * pieces_per_layer);
    let layers: Vec<usize> = (0..pin.tile_shape_count(&ctx))
        .map(|i| pin.shape_layer(i, &ctx))
        .collect();
    assert_eq!(layers.iter().filter(|&&l| l == 0).count(), pieces_per_layer);
    assert_eq!(layers.iter().filter(|&&l| l == 1).count(), pieces_per_layer);
    assert_eq!(
        pin.get_center(&ctx).to_float(),
        FloatPoint::new(100000.0, -100000.0),
        "the anchor keeps the pin centre"
    );

    let probe = |x: i32, y: i32| {
        let tile = copper_geometry::TileShape::Box(copper_geometry::IntBox::from_coords(
            x - 10,
            y - 10,
            x + 10,
            y + 10,
        ));
        board
            .overlapping_objects(&tile, Some(0))
            .into_iter()
            .any(|object| object == copper_board::TreeObject::Item(id))
    };
    assert!(probe(135000, -100000), "the horizontal arm is copper");
    assert!(probe(100000, -135000), "the vertical arm is copper");
    assert!(
        !probe(125000, -125000),
        "the notch between the arms is free"
    );
}

#[test]
fn concave_pad_copper_survives_a_dsn_round_trip() {
    let input = json!({
        "layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
        "resolution":10000,
        "components":[{"reference":"J1", "position":{"x":10,"y":10}, "rotation":0,
          "layer":"F.Cu", "pads":[{"name":"1", "shape":"custom", "size":{"x":1,"y":1},"drill":0.4,
            "copperPolygon":[{"x":-0.5,"y":0.5},{"x":-0.5,"y":-0.5},{"x":0.5,"y":-0.5},{"x":0.5,"y":0.5}],
            "extraCopperPolygons":[[{"x":-0.5,"y":-0.5},{"x":4,"y":-0.5},{"x":4,"y":0.5},{"x":0.5,"y":0.5},
              {"x":0.5,"y":4},{"x":-0.5,"y":4}]],
            "layers":["F.Cu","B.Cu"]}]}],
        "outline":{"ordered":true, "clearance":0.5, "corners":[{"x":0,"y":0},{"x":20,"y":0},{"x":20,"y":20},{"x":0,"y":20}]}
    });
    let BoardReadResult::Success {
        board: Some(original),
        coordinate_transform: Some(ct),
        ..
    } = copper_dsn::kicad::read_board(&input.to_string(), None)
    else {
        panic!("load failed")
    };
    let mut out: Vec<u8> = Vec::new();
    copper_dsn::dsn_writer::write(&original, &ct, &mut out, "roundtrip", false).expect("write");
    let options = copper_dsn::parser::scope_parameter::DsnReadOptions::default();
    let reloaded = match copper_dsn::read_board(out.as_slice(), None, Some("roundtrip"), &options) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.expect("re-read must produce a board")
        }
        result => panic!("{result:?}"),
    };
    let pieces = |board: &copper_board::Board| {
        let ctx = board.ctx();
        let pin = board
            .get_items()
            .find_map(|i| if let Item::Pin(p) = i { Some(p) } else { None })
            .unwrap();
        let mut boxes: Vec<_> = pin
            .shapes_on_layer(0, &ctx)
            .iter()
            .map(|shape| shape.bounding_box())
            .collect();
        boxes.sort_by_key(|b| (b.ll.x, b.ll.y, b.ur.x, b.ur.y));
        boxes
    };
    assert!(pieces(&original).len() >= 3);
    assert_eq!(pieces(&original), pieces(&reloaded));
}

#[test]
fn a_back_side_concave_pad_keeps_its_pieces_on_the_mirrored_layer() {
    let input = json!({
        "layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
        "resolution":10000,
        "components":[{"reference":"J1", "position":{"x":10,"y":10}, "rotation":0,
          "layer":"B.Cu", "pads":[{"name":"1", "shape":"custom", "size":{"x":1,"y":1},"drill":0,
            "copperPolygon":[{"x":-0.5,"y":0.5},{"x":-0.5,"y":-0.5},{"x":0.5,"y":-0.5},{"x":0.5,"y":0.5}],
            "extraCopperPolygons":[[{"x":-0.5,"y":-0.5},{"x":4,"y":-0.5},{"x":4,"y":0.5},{"x":0.5,"y":0.5},
              {"x":0.5,"y":4},{"x":-0.5,"y":4}]],
            "layers":["B.Cu"]}]}],
        "outline":{"ordered":true, "clearance":0.5, "corners":[{"x":0,"y":0},{"x":20,"y":0},{"x":20,"y":20},{"x":0,"y":20}]}
    });
    let BoardReadResult::Success {
        board: Some(board), ..
    } = copper_dsn::kicad::read_board(&input.to_string(), None)
    else {
        panic!("load failed")
    };
    let ctx = board.ctx();
    let pin = board
        .get_items()
        .find_map(|i| if let Item::Pin(p) = i { Some(p) } else { None })
        .unwrap();
    let copper_layer = pin.first_layer(&ctx);
    assert_eq!(copper_layer, pin.last_layer(&ctx));
    let pieces = pin.shapes_on_layer(copper_layer, &ctx);
    assert!(pieces.len() >= 3);
    assert_eq!(pin.tile_shape_count(&ctx), pieces.len());
    assert!((0..pin.tile_shape_count(&ctx)).all(|i| pin.shape_layer(i, &ctx) == copper_layer));
    let span = pieces
        .iter()
        .map(|shape| shape.bounding_box())
        .reduce(|a, b| a.union(&b))
        .unwrap();
    assert_eq!(span.width(), 45000, "the L spans 4.5 mm across");
    assert_eq!(span.height(), 45000, "the L spans 4.5 mm down");
}

#[test]
fn a_lone_concave_dsn_pad_polygon_keeps_its_real_outline() {
    let input = json!({
        "layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
        "resolution":10000,
        "components":[{"reference":"J1", "position":{"x":10,"y":10}, "rotation":0,
          "layer":"F.Cu", "pads":[{"name":"1", "shape":"custom", "size":{"x":1,"y":1},"drill":0,
            "copperPolygon":[{"x":-0.5,"y":-0.5},{"x":4,"y":-0.5},{"x":4,"y":0.5},{"x":0.5,"y":0.5},
              {"x":0.5,"y":4},{"x":-0.5,"y":4}],
            "layers":["F.Cu"]}]}],
        "outline":{"ordered":true, "clearance":0.5, "corners":[{"x":0,"y":0},{"x":20,"y":0},{"x":20,"y":20},{"x":0,"y":20}]}
    });
    let BoardReadResult::Success {
        board: Some(original),
        coordinate_transform: Some(ct),
        ..
    } = copper_dsn::kicad::read_board(&input.to_string(), None)
    else {
        panic!("load failed")
    };
    let mut out: Vec<u8> = Vec::new();
    copper_dsn::dsn_writer::write(&original, &ct, &mut out, "lone", false).expect("write");
    let written = String::from_utf8(out).expect("UTF-8");
    let start = written.find("(padstack ").expect("a padstack");
    let shapes_start = start + written[start..].find("(shape").expect("a shape");
    let shapes_end = start
        + written[start..]
            .find("\n    )")
            .expect("the padstack closes");
    let lone = format!(
        "{}(shape (polygon F.Cu 0 -500 500 4000 500 4000 -500 500 -500 500 -4000 -500 -4000)){}",
        &written[..shapes_start],
        &written[shapes_end..]
    );
    let options = copper_dsn::parser::scope_parameter::DsnReadOptions::default();
    let reloaded = match copper_dsn::read_board(lone.as_bytes(), None, Some("lone"), &options) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.expect("re-read must produce a board")
        }
        result => panic!("{result:?}\n{lone}"),
    };
    let ctx = reloaded.ctx();
    let (id, pin) = reloaded
        .get_items()
        .find_map(|i| {
            if let Item::Pin(p) = i {
                Some((i.id(), p))
            } else {
                None
            }
        })
        .unwrap();
    assert!(pin.shapes_on_layer(0, &ctx).len() >= 2, "{lone}");
    let probe = |x: i32, y: i32| {
        let tile = copper_geometry::TileShape::Box(copper_geometry::IntBox::from_coords(
            x - 10,
            y - 10,
            x + 10,
            y + 10,
        ));
        reloaded
            .overlapping_objects(&tile, Some(0))
            .into_iter()
            .any(|object| object == copper_board::TreeObject::Item(id))
    };
    let center = pin.get_center(&ctx).to_float();
    let (cx, cy) = (center.x as i32, center.y as i32);
    assert!(probe(cx + 35000, cy), "the horizontal arm is copper");
    assert!(!probe(cx + 25000, cy - 25000), "the notch is free");
}
