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
    assert!(!shape.contains_inside(&Point::from(copper_geometry::IntPoint::new(100000, -108000))));
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
    assert!(shape.contains_inside(&Point::from(copper_geometry::IntPoint::new(100000, -100000))));
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
