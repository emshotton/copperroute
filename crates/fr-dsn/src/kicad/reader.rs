use std::cmp::Ordering;

use fr_board::{
    Board, BoardLibrary, BoardRules, ClearanceMatrix, Communication, Components, FixedState,
    ItemClass, ItemIdGenerator, Layer, LayerStructure, NetClassId, Nets, Package, PackagePin,
    Packages, Padstack, PadstackId, Padstacks, Unit, ViaInfo, ViaRule, equals_ignore_case,
    java_to_lower, java_to_upper,
};
use fr_geometry::{
    Area, Circle, FloatPoint, IntBox, IntOctagon, IntPoint, IntVector, Point, PolygonShape,
    PolylineShapeRef, Shape, TileShape, Vector,
};

use crate::coordinate_transform::CoordinateTransform;
use crate::error::{BoardMetadata, BoardReadResult, DsnError};
use crate::format::double::java_format_fixed;
use crate::format::java_round_to_int;
use crate::kicad::dto::{KiCadBoardJson, NetClassJson, PadJson, Point2D, UnitJson};
use crate::parser::network::is_kicad_default_net_class_name;

#[allow(clippy::too_many_lines)]
#[must_use]
pub fn read_board(json: &str, id_generator: Option<ItemIdGenerator>) -> BoardReadResult {
    let id_generator = id_generator.unwrap_or_default();

    let board_json: KiCadBoardJson = if json.chars().all(java_is_whitespace) {
        return parse_error("json_root", "JSON payload is empty or invalid");
    } else {
        match serde_json::from_str::<Option<KiCadBoardJson>>(json) {
            Ok(Some(board_json)) => board_json,
            Ok(None) => return parse_error("json_root", "JSON payload is empty or invalid"),
            Err(error) => {
                return parse_error("json_payload", &format!("Exception occurred: {error}"));
            }
        }
    };

    if let Err(malformed) = board_json.validate() {
        return parse_error(
            malformed.section,
            &format!(
                "the KiCad board JSON file is malformed in its `{}` section: {} {}",
                malformed.section, malformed.object, malformed.problem
            ),
        );
    }

    let user_unit = match board_json.unit {
        Some(UnitJson::MIL) => Unit::Mil,
        Some(UnitJson::UM) => Unit::Um,
        Some(UnitJson::MM) | None => Unit::Mm,
    };

    let mut resolution = java_max(1.0, board_json.resolution) as i32;
    if board_json.resolution == 1.0 && user_unit == Unit::Mm {
        resolution = 10000;
    }

    let scale_factor = f64::from(resolution);

    let Some(json_layers) = board_json.layers.as_ref() else {
        return npe("java.util.List.isEmpty()", "boardJson.layers");
    };
    let layer_count = if json_layers.is_empty() {
        2
    } else {
        json_layers.len()
    };
    let mut board_layers: Vec<Layer> = Vec::with_capacity(layer_count);
    let mut board_layer_names: Vec<String> = Vec::with_capacity(layer_count);
    if json_layers.is_empty() {
        board_layers.push(Layer::new("F.Cu", true));
        board_layers.push(Layer::new("B.Cu", true));
        board_layer_names.push("F.Cu".to_string());
        board_layer_names.push("B.Cu".to_string());
    } else {
        for layer_json in json_layers {
            let is_signal = !layer_json
                .r#type
                .as_deref()
                .is_some_and(|kind| equals_ignore_case("plane", kind));
            let name = layer_json
                .name
                .clone()
                .expect("section 1a refused a null layer name");
            board_layers.push(Layer::new(name.clone(), is_signal));
            board_layer_names.push(name);
        }
    }
    let layer_structure = LayerStructure::new(board_layers);

    let Some(json_net_classes) = board_json.netClasses.as_ref() else {
        return npe("java.util.List.iterator()", "netClasses");
    };
    let additional_net_classes = non_default_net_classes(json_net_classes);

    let clearance_class_count = 2.max(additional_net_classes.len() + 2);
    let mut clearance_class_names: Vec<String> = Vec::with_capacity(clearance_class_count);
    clearance_class_names.push("null".to_string());
    clearance_class_names.push("default".to_string());
    for net_class in &additional_net_classes {
        clearance_class_names.push(
            net_class
                .name
                .clone()
                .expect("section 1a refused a null net-class name"),
        );
    }

    let mut clearance_matrix = ClearanceMatrix::new(
        clearance_class_count,
        &layer_structure,
        &clearance_class_names,
    );
    let default_clearance = java_round_to_int(Unit::scale(0.2, Unit::Mm, user_unit) * scale_factor);
    clearance_matrix.set_default_value(default_clearance);

    let kicad_default_net_class = find_kicad_default_net_class(json_net_classes);
    if let Some(kicad_default) = kicad_default_net_class
        && kicad_default.clearance > 0.0
    {
        let default_cl_val = java_round_to_int(kicad_default.clearance * scale_factor);
        clearance_matrix.set_value_on_all_layers(1, 1, default_cl_val);
    }

    for (i, net_class) in additional_net_classes.iter().enumerate() {
        let cl_no = i + 2;
        let cl_val = java_round_to_int(net_class.clearance * scale_factor);
        clearance_matrix.set_value_on_all_layers(cl_no, cl_no, cl_val);
        clearance_matrix.set_value_on_all_layers(1, cl_no, cl_val);
    }

    let Some(json_clearance_rules) = board_json.clearanceRules.as_ref() else {
        return npe("java.util.List.iterator()", "boardJson.clearanceRules");
    };
    for rule in json_clearance_rules {
        let idx_a = rule
            .classA
            .as_deref()
            .and_then(|name| clearance_matrix.get_no(name));
        let idx_b = rule
            .classB
            .as_deref()
            .and_then(|name| clearance_matrix.get_no(name));
        if let (Some(idx_a), Some(idx_b)) = (idx_a, idx_b) {
            let clearance_val = java_round_to_int(rule.clearance * scale_factor);
            clearance_matrix.set_value_on_all_layers(idx_a, idx_b, clearance_val);
        }
    }

    let mut outline_shapes: Vec<PolylineShapeRef> = Vec::new();
    let bounding_box: IntBox;
    let outline_missing = match board_json.outline.as_ref() {
        None => true,
        Some(outline) => match outline.corners.as_ref() {
            None => return npe("java.util.List.size()", "boardJson.outline.corners"),
            Some(corners) => corners.len() < 3,
        },
    };
    if outline_missing {
        let mut outline = PointOutline::new();
        let mut min_x = f64::MAX;
        let mut max_x = -f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_y = -f64::MAX;

        if let Some(components) = board_json.components.as_ref() {
            for component in components {
                if let Some(position) = component.position.as_ref() {
                    min_x = java_min(min_x, position.x);
                    max_x = java_max(max_x, position.x);
                    min_y = java_min(min_y, position.y);
                    max_y = java_max(max_y, position.y);
                }
                if let Some(pads) = component.pads.as_ref() {
                    for pad in pads {
                        if let Some(position) = pad.position.as_ref() {
                            min_x = java_min(min_x, position.x);
                            max_x = java_max(max_x, position.x);
                            min_y = java_min(min_y, position.y);
                            max_y = java_max(max_y, position.y);
                        }
                    }
                }
            }
        }

        if let Some(vias) = board_json.vias.as_ref() {
            for via in vias {
                if let Some(position) = via.position.as_ref() {
                    min_x = java_min(min_x, position.x);
                    max_x = java_max(max_x, position.x);
                    min_y = java_min(min_y, position.y);
                    max_y = java_max(max_y, position.y);
                }
            }
        }

        if let Some(traces) = board_json.traces.as_ref() {
            for trace in traces {
                if let Some(points) = trace.points.as_ref() {
                    for point in points {
                        min_x = java_min(min_x, point.x);
                        max_x = java_max(max_x, point.x);
                        min_y = java_min(min_y, point.y);
                        max_y = java_max(max_y, point.y);
                    }
                }
            }
        }

        if let Some(zones) = board_json.conductionAreas.as_ref() {
            for zone in zones {
                if let Some(polygon) = zone.polygon.as_ref() {
                    for point in polygon {
                        min_x = java_min(min_x, point.x);
                        max_x = java_max(max_x, point.x);
                        min_y = java_min(min_y, point.y);
                        max_y = java_max(max_y, point.y);
                    }
                }
            }
        }

        let padding = Unit::scale(5.0, Unit::Mm, user_unit);

        if min_x == f64::MAX {
            min_x = 0.0;
            max_x = 1000.0 / scale_factor;
            min_y = -1000.0 / scale_factor;
            max_y = 0.0;
        }

        min_x -= padding;
        max_x += padding;
        min_y -= padding;
        max_y += padding;

        let points = [
            Point::Int(IntPoint::new(
                java_round_to_int(min_x * scale_factor),
                java_round_to_int(-min_y * scale_factor),
            )),
            Point::Int(IntPoint::new(
                java_round_to_int(min_x * scale_factor),
                java_round_to_int(-max_y * scale_factor),
            )),
            Point::Int(IntPoint::new(
                java_round_to_int(max_x * scale_factor),
                java_round_to_int(-max_y * scale_factor),
            )),
            Point::Int(IntPoint::new(
                java_round_to_int(max_x * scale_factor),
                java_round_to_int(-min_y * scale_factor),
            )),
        ];

        for point in &points {
            outline.add_point(point.to_float());
        }
        outline_shapes.push(PolylineShapeRef::Polygon(PolygonShape::from_points(
            &points,
        )));
        bounding_box = outline.bounding_box().offset(1000.0);
    } else {
        let mut outline = PointOutline::new();
        let mut corners: Vec<Point2D> = board_json
            .outline
            .as_ref()
            .expect("outline_missing is true when it is None")
            .corners
            .as_ref()
            .expect("outline_missing returned early when it is None")
            .clone();
        if corners.len() > 2 {
            let mut sum_x = 0.0;
            let mut sum_y = 0.0;
            for point in &corners {
                sum_x += point.x;
                sum_y += point.y;
            }
            let count = corners.len() as f64;
            let cx = sum_x / count;
            let cy = sum_y / count;
            corners.sort_by(|p1, p2| {
                java_double_compare((p1.y - cy).atan2(p1.x - cx), (p2.y - cy).atan2(p2.x - cx))
            });
        }
        let mut points: Vec<Point> = Vec::with_capacity(corners.len());
        for corner in &corners {
            let point = Point::Int(IntPoint::new(
                java_round_to_int(corner.x * scale_factor),
                java_round_to_int(-corner.y * scale_factor),
            ));
            outline.add_point(point.to_float());
            points.push(point);
        }
        outline_shapes.push(PolylineShapeRef::Polygon(PolygonShape::from_points(
            &points,
        )));
        bounding_box = outline.bounding_box().offset(1000.0);
    }

    let outline_clearance_no = board_json.outline.as_ref().map_or(1, |outline| {
        outline_clearance_class(&clearance_matrix, outline.clearance, scale_factor)
    });

    let coordinate_transform = match CoordinateTransform::new(scale_factor, 0.0, 0.0) {
        Ok(coordinate_transform) => coordinate_transform,
        Err(error) => return parse_error("resolution", &error.to_string()),
    };
    let host_cad = board_json
        .hostCad
        .as_deref()
        .filter(|host| !java_is_blank(host))
        .unwrap_or("KiCad")
        .to_string();
    let host_version = board_json
        .hostVersion
        .as_deref()
        .filter(|version| !java_is_blank(version))
        .unwrap_or("v10.0")
        .to_string();
    let communication = Communication::new(
        user_unit,
        resolution,
        id_generator,
        Some(host_cad),
        Some(host_version),
    );

    let board_rules = BoardRules::new(layer_structure.clone(), clearance_matrix);
    let library = BoardLibrary::new(Padstacks::new(layer_structure.clone()), Packages::new());
    let mut board = Board::new(
        outline_shapes,
        outline_clearance_no,
        bounding_box,
        board_rules,
        library,
        Components::new(),
        communication,
    );

    board.rules.create_default_net_class();
    let default_net_class = board.rules.get_default_net_class();
    let mut net_class_index_map: Vec<(String, usize)> = Vec::new();
    if let Some(kicad_default) = kicad_default_net_class {
        apply_kicad_net_class_parameters(
            &mut board.rules,
            default_net_class,
            kicad_default,
            layer_count,
            scale_factor,
            1,
        );
    }

    for (i, net_class) in additional_net_classes.iter().enumerate() {
        let cl_no = i + 2;
        let name = net_class
            .name
            .clone()
            .expect("section 1a refused a null net-class name");
        let board_net_class = board
            .rules
            .net_classes
            .append(name.clone(), &layer_structure, false);
        apply_kicad_net_class_parameters(
            &mut board.rules,
            board_net_class,
            net_class,
            layer_count,
            scale_factor,
            cl_no,
        );
        if let Some(entry) = net_class_index_map
            .iter_mut()
            .find(|(existing, _)| *existing == name)
        {
            entry.1 = cl_no;
        } else {
            net_class_index_map.push((name, cl_no));
        }
    }

    let mut default_via_diameter = 0.8;
    let mut default_via_drill = 0.4;
    if user_unit == Unit::Mil {
        default_via_diameter = 30.0;
        default_via_drill = 15.0;
    } else if user_unit == Unit::Um {
        default_via_diameter = 800.0;
        default_via_drill = 400.0;
    }

    let mut def_via_dia = default_via_diameter;
    let mut def_via_drill = default_via_drill;
    if let Some(kicad_default) = kicad_default_net_class {
        if kicad_default.viaDiameter > 0.0 {
            def_via_dia = kicad_default.viaDiameter;
        }
        if kicad_default.viaDrill > 0.0 {
            def_via_drill = kicad_default.viaDrill;
        }
    }
    let def_radius = def_via_dia * scale_factor / 2.0;
    let def_via_shape = via_shape(def_radius);
    let def_via_shape_arr = vec![Some(def_via_shape); layer_count];
    let default_via_padstack =
        board
            .library
            .padstacks
            .add("defaultVia", def_via_shape_arr, true, false);
    board.library.add_via_padstack(default_via_padstack);

    let default_via_cl_class = board
        .rules
        .net_classes
        .get(default_net_class)
        .default_item_clearance_classes
        .get(ItemClass::Via);
    let default_via_info = ViaInfo::new(
        "defaultVia",
        default_via_padstack,
        default_via_cl_class,
        true,
    );
    board.rules.via_infos.add(default_via_info);

    let mut default_via_rule = ViaRule::new("default");
    default_via_rule.append_via(registered_via_info(&board.rules, "defaultVia"));
    board.rules.via_rules.push(default_via_rule.clone());
    board
        .rules
        .net_classes
        .get_mut(default_net_class)
        .set_via_rule(Some(default_via_rule));

    for (i, net_class) in additional_net_classes.iter().enumerate() {
        let cl_no = i + 2;
        let board_net_class = NetClassId(cl_no - 1);

        let via_dia = if net_class.viaDiameter > 0.0 {
            net_class.viaDiameter
        } else {
            def_via_dia
        };
        let via_drill = if net_class.viaDrill > 0.0 {
            net_class.viaDrill
        } else {
            def_via_drill
        };
        let _ = via_drill;

        let radius = via_dia * scale_factor / 2.0;
        let via_shape_arr = vec![Some(via_shape(radius)); layer_count];

        let net_class_name = net_class
            .name
            .clone()
            .expect("section 1a refused a null net-class name");
        let via_name = format!("via_{net_class_name}");
        let via_padstack =
            board
                .library
                .padstacks
                .add(via_name.clone(), via_shape_arr, true, false);
        board.library.add_via_padstack(via_padstack);

        let via_cl_class = board
            .rules
            .net_classes
            .get(board_net_class)
            .default_item_clearance_classes
            .get(ItemClass::Via);
        let via_info = ViaInfo::new(via_name.clone(), via_padstack, via_cl_class, true);
        board.rules.via_infos.add(via_info);

        let mut via_rule = ViaRule::new(net_class_name);
        via_rule.append_via(registered_via_info(&board.rules, &via_name));
        board.rules.via_rules.push(via_rule.clone());
        board
            .rules
            .net_classes
            .get_mut(board_net_class)
            .set_via_rule(Some(via_rule));
    }

    let Some(json_nets) = board_json.nets.as_ref() else {
        return npe("java.util.List.size()", "boardJson.nets");
    };
    for net_json in json_nets {
        let cl_no = resolve_net_class_index(&net_class_index_map, net_json.className.as_deref());
        let net = board.rules.nets.add(
            net_json
                .name
                .clone()
                .expect("section 1a refused a null net name"),
            1,
            net_json.containsPlane,
            NetClassId(0),
        );
        net.set_class(NetClassId(cl_no - 1));
    }

    let mut referenced_nets = ReferencedNets::new();
    if let Some(components) = board_json.components.as_ref() {
        for component in components {
            if let Some(pads) = component.pads.as_ref() {
                for pad in pads {
                    if let Some(net_name) = pad.netName.as_deref()
                        && !net_name.is_empty()
                    {
                        referenced_nets.add(net_name);
                    }
                }
            }
        }
    }
    if let Some(zones) = board_json.conductionAreas.as_ref() {
        for zone in zones {
            if let Some(net_name) = zone.netName.as_deref()
                && !net_name.is_empty()
            {
                referenced_nets.add(net_name);
            }
        }
    }
    if let Some(traces) = board_json.traces.as_ref() {
        for trace in traces {
            if let Some(net_name) = trace.netName.as_deref()
                && !net_name.is_empty()
            {
                referenced_nets.add(net_name);
            }
        }
    }
    if let Some(vias) = board_json.vias.as_ref() {
        for via in vias {
            if let Some(net_name) = via.netName.as_deref()
                && !net_name.is_empty()
            {
                referenced_nets.add(net_name);
            }
        }
    }

    for net_name in referenced_nets.iteration_order() {
        if java_nets_get(&board.rules.nets, Some(net_name), 1).is_none() {
            let net = board
                .rules
                .nets
                .add(net_name.to_string(), 1, false, NetClassId(0));
            net.set_class(NetClassId(0));
        }
    }

    let mut pad_padstacks: Vec<(Vec<Option<Shape>>, bool, PadstackId)> = Vec::new();

    let Some(json_components) = board_json.components.as_ref() else {
        return npe("java.util.List.iterator()", "boardJson.components");
    };
    for component in json_components {
        let Some(pads) = component.pads.as_ref() else {
            return npe("java.util.List.iterator()", "comp.pads");
        };
        let mut package_pins: Vec<PackagePin> = Vec::new();
        for pad in pads {
            let mut shapes: Vec<Option<Shape>> = vec![None; layer_count];
            let Some(pad_size) = pad.size.as_ref() else {
                return npe_field("x", "pad.size");
            };
            let dx = pad_size.x * scale_factor / 2.0;
            let dy = pad_size.y * scale_factor / 2.0;
            let pad_shape = if pad
                .shape
                .as_deref()
                .is_some_and(|shape| equals_ignore_case("circle", shape))
            {
                let radius = java_min(pad_size.x, pad_size.y) * scale_factor / 2.0;
                Shape::Circle(Circle::new(IntPoint::ZERO, java_round_to_int(radius)))
            } else if pad
                .shape
                .as_deref()
                .is_some_and(|shape| equals_ignore_case("oval", shape))
            {
                let lx = java_round_to_int(-dx);
                let rx = java_round_to_int(dx);
                let ly = java_round_to_int(-dy);
                let uy = java_round_to_int(dy);
                let r = java_round_to_int(java_min(dx, dy));
                let cut = java_round_to_int((2.0 - 2.0_f64.sqrt()) * f64::from(r));
                let octagon = IntOctagon::new(
                    lx,
                    ly,
                    rx,
                    uy,
                    lx - uy + cut,
                    rx - ly - cut,
                    lx + ly + cut,
                    rx + uy - cut,
                );
                Shape::Tile(TileShape::Simplex(octagon.to_simplex()))
            } else {
                Shape::Tile(TileShape::Simplex(
                    IntBox::from_coords(
                        java_round_to_int(-dx),
                        java_round_to_int(-dy),
                        java_round_to_int(dx),
                        java_round_to_int(dy),
                    )
                    .to_simplex(),
                ))
            };

            let mut start_layer = 0usize;
            let mut end_layer = layer_count - 1;
            if let Some(pad_layers) = pad.layers.as_ref()
                && !pad_layers.is_empty()
            {
                let mut lowest_idx = layer_count - 1;
                let mut highest_idx = 0usize;
                for layer_name in pad_layers {
                    let layer_name = layer_name
                        .as_deref()
                        .expect("section 1a refused a null pad-layer element");
                    for (li, board_layer_name) in board_layer_names.iter().enumerate() {
                        if equals_ignore_case(board_layer_name, layer_name) {
                            lowest_idx = lowest_idx.min(li);
                            highest_idx = highest_idx.max(li);
                        }
                    }
                }
                start_layer = lowest_idx;
                end_layer = highest_idx;
            }

            if start_layer <= end_layer {
                for shape in &mut shapes[start_layer..=end_layer] {
                    *shape = Some(pad_shape.clone());
                }
            }

            let is_drillable = pad.drill > 0.0;
            let padstack = match pad_padstacks
                .iter()
                .find(|(existing, drillable, _)| *drillable == is_drillable && *existing == shapes)
            {
                Some((_, _, id)) => *id,
                None => {
                    if shapes.iter().all(Option::is_none) {
                        return no_shape_on_any_layer(
                            "components",
                            &format!(
                                "pad `{}` of component `{}`",
                                pad.name.as_deref().unwrap_or_default(),
                                component.reference.as_deref().unwrap_or_default()
                            ),
                            "its `layers` list names no layer this board has",
                        );
                    }
                    let padstack_name =
                        match get_descriptive_padstack_name(pad, &board_layer_names, layer_count) {
                            Ok(name) => name,
                            Err(message) => {
                                return parse_error(
                                    "json_payload",
                                    &format!("Exception occurred: {message}"),
                                );
                            }
                        };
                    let padstack_name =
                        unique_padstack_name(&board.library.padstacks, padstack_name);
                    let id = board.library.padstacks.add(
                        padstack_name,
                        shapes.clone(),
                        is_drillable,
                        false,
                    );
                    pad_padstacks.push((shapes, is_drillable, id));
                    id
                }
            };
            let Some(offset) = pad.offset.as_ref() else {
                return npe_field("x", "pad.offset");
            };
            let relative_loc = Vector::Int(IntVector::new(
                java_round_to_int(offset.x * scale_factor),
                java_round_to_int(-offset.y * scale_factor),
            ));
            package_pins.push(PackagePin::new(
                pad.name
                    .clone()
                    .expect("section 1a refused a null pad name"),
                padstack,
                relative_loc,
                0.0,
            ));
        }

        let is_front = !component
            .layer
            .as_deref()
            .is_some_and(|layer| equals_ignore_case("B.Cu", layer));
        let base_package_name = match component.footprint.as_deref() {
            Some(footprint) if !footprint.is_empty() => footprint.to_string(),
            _ => "Package".to_string(),
        };

        let mut suffix = 0usize;
        let component_package = loop {
            let test_name = if suffix == 0 {
                base_package_name.clone()
            } else {
                format!("{base_package_name}::{suffix}")
            };
            let existing = board
                .library
                .packages
                .get_by_name(&test_name, is_front)
                .map(|existing| existing.no);
            let names_match = existing.is_some_and(|no| {
                equals_ignore_case(&board.library.packages.get(no).name, &test_name)
            });
            if !names_match {
                break board.library.packages.add(
                    test_name,
                    package_pins.clone(),
                    None,
                    None,
                    None,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    is_front,
                );
            }
            let existing_no = existing.expect("names_match implies a package was found");
            if are_package_pins_identical(board.library.packages.get(existing_no), &package_pins) {
                break existing_no;
            }
            suffix += 1;
        };

        let Some(position) = component.position.as_ref() else {
            return npe_field("x", "comp.position");
        };
        let position = IntPoint::new(
            java_round_to_int(position.x * scale_factor),
            java_round_to_int(-position.y * scale_factor),
        );

        let reference = component
            .reference
            .as_deref()
            .expect("section 1a refused a null component reference");
        let board_comp_id = board
            .components
            .add(
                reference.to_string(),
                Some(Point::Int(position)),
                -component.rotation,
                is_front,
                component_package,
                component_package,
                true,
                component.value.clone(),
            )
            .id;

        for (pad_index, pad) in pads.iter().enumerate() {
            let net_number =
                java_nets_get(&board.rules.nets, pad.netName.as_deref(), 1).unwrap_or(0);
            let net_numbers = if net_number > 0 {
                vec![net_number]
            } else {
                Vec::new()
            };
            board.insert_pin(
                board_comp_id,
                i32::try_from(pad_index).unwrap_or(i32::MAX),
                net_numbers,
                outline_clearance_no,
                FixedState::SystemFixed,
            );
        }
    }

    let Some(json_zones) = board_json.conductionAreas.as_ref() else {
        return npe("java.util.List.iterator()", "boardJson.conductionAreas");
    };
    for zone in json_zones {
        let net_number = java_nets_get(&board.rules.nets, zone.netName.as_deref(), 1).unwrap_or(0);
        let net_numbers = if net_number > 0 {
            vec![net_number]
        } else {
            Vec::new()
        };

        let Some(polygon) = zone.polygon.as_ref() else {
            return npe("java.util.List.size()", "zone.polygon");
        };
        let mut zone_points: Vec<Point> = Vec::with_capacity(polygon.len());
        for corner in polygon {
            zone_points.push(Point::Int(IntPoint::new(
                java_round_to_int(corner.x * scale_factor),
                java_round_to_int(-corner.y * scale_factor),
            )));
        }
        if zone_points.is_empty() {
            return parse_error(
                "json_payload",
                "Exception occurred: Index 0 out of bounds for length 0",
            );
        }
        #[allow(clippy::cast_sign_loss)]
        board.insert_conduction_area(
            Area::Shape(Shape::Polygon(PolygonShape::from_points(&zone_points))),
            zone.layerIndex as usize,
            net_numbers,
            1,
            zone.isObstacle,
            FixedState::UserFixed,
        );
    }

    let Some(json_traces) = board_json.traces.as_ref() else {
        return npe("java.util.List.iterator()", "boardJson.traces");
    };
    for trace in json_traces {
        let net_number = java_nets_get(&board.rules.nets, trace.netName.as_deref(), 1).unwrap_or(0);
        let net_numbers = if net_number > 0 {
            vec![net_number]
        } else {
            Vec::new()
        };
        let trace_half_width = java_round_to_int(trace.width * scale_factor / 2.0);

        let Some(points) = trace.points.as_ref() else {
            return npe("java.util.List.size()", "tr.points");
        };
        let mut trace_points: Vec<Point> = Vec::with_capacity(points.len());
        for point in points {
            trace_points.push(Point::Int(IntPoint::new(
                java_round_to_int(point.x * scale_factor),
                java_round_to_int(-point.y * scale_factor),
            )));
        }
        board.insert_trace_at_points(
            &trace_points,
            trace.layerIndex.max(0) as usize,
            trace_half_width,
            net_numbers,
            1,
            FixedState::UserFixed,
        );
    }

    let Some(json_vias) = board_json.vias.as_ref() else {
        return npe("java.util.List.iterator()", "boardJson.vias");
    };
    for via in json_vias {
        let net_number = java_nets_get(&board.rules.nets, via.netName.as_deref(), 1).unwrap_or(0);
        let net_numbers = if net_number > 0 {
            vec![net_number]
        } else {
            Vec::new()
        };

        let Some(position) = via.position.as_ref() else {
            return npe_field("x", "vj.position");
        };
        let center = IntPoint::new(
            java_round_to_int(position.x * scale_factor),
            java_round_to_int(-position.y * scale_factor),
        );

        let mut shapes: Vec<Option<Shape>> = vec![None; layer_count];
        let radius = via.diameter * scale_factor / 2.0;
        let shape = via_shape(radius);
        let mut layer = via.startLayerIndex;
        while layer <= via.endLayerIndex {
            let Ok(index) = usize::try_from(layer) else {
                return parse_error(
                    "json_payload",
                    &format!(
                        "Exception occurred: Index {layer} out of bounds for length {layer_count}"
                    ),
                );
            };
            if index >= layer_count {
                return parse_error(
                    "json_payload",
                    &format!(
                        "Exception occurred: Index {index} out of bounds for length {layer_count}"
                    ),
                );
            }
            shapes[index] = Some(shape.clone());
            layer += 1;
        }
        let via_padstack_name = format!(
            "Via[{}-{}]_{}:{}_um",
            via.startLayerIndex,
            via.endLayerIndex,
            java_format_fixed(via.diameter * 1000.0, 0),
            java_format_fixed(via.drill * 1000.0, 0)
        );
        if shapes.iter().all(Option::is_none) {
            return no_shape_on_any_layer(
                "vias",
                &format!("via `{via_padstack_name}`"),
                "its layer span is empty",
            );
        }
        let existing_padstack = board
            .library
            .padstacks
            .get_by_name(&via_padstack_name)
            .map(|padstack| PadstackId(padstack.no));
        let via_padstack = match existing_padstack {
            Some(id) => id,
            None => board
                .library
                .padstacks
                .add(via_padstack_name, shapes, true, false),
        };
        let stop = || false;
        if let Err(error) = board.insert_via_checked(
            via_padstack,
            Point::Int(center),
            net_numbers,
            1,
            FixedState::UserFixed,
            true,
            &stop,
        ) {
            return parse_error("json_payload", &format!("Exception occurred: {error}"));
        }
    }

    let metadata = BoardMetadata {
        host_cad: Some("KiCad".to_string()),
        host_version: Some("v10.0".to_string()),
        layer_count,
        unit: user_unit,
        resolution,
        snap_angle: fr_board::AngleRestriction::FortyFiveDegree,
        router_settings: None,
    };

    let mut warnings: Vec<String> = Vec::new();
    if outline_missing {
        warnings.push(
            "Board Outline/Boundary is missing or empty in the JSON file. A supposed board edge \
             around components with 5mm padding was generated."
                .to_string(),
        );
    }
    BoardReadResult::Success {
        board: Some(Box::new(board)),
        metadata: Some(metadata),
        warnings,
        coordinate_transform: Some(coordinate_transform),
    }
}

pub fn import_session(json: &str, board: &mut Board) -> Result<(), DsnError> {
    if json.trim().is_empty() {
        return Err(DsnError::KicadSession(
            "java.lang.IllegalArgumentException: JSON session file payload is empty or invalid"
                .to_string(),
        ));
    }
    let board_json: Option<KiCadBoardJson> = serde_json::from_str(json).map_err(|error| {
        DsnError::KicadSession(format!("com.google.gson.JsonSyntaxException: {error}"))
    })?;
    let Some(board_json) = board_json else {
        return Err(DsnError::KicadSession(
            "java.lang.IllegalArgumentException: JSON session file payload is empty or invalid"
                .to_string(),
        ));
    };

    let user_unit = match board_json.unit {
        Some(UnitJson::MIL) => Unit::Mil,
        Some(UnitJson::UM) => Unit::Um,
        _ => Unit::Mm,
    };

    let mut resolution = java_double_to_int(java_max(1.0, board_json.resolution));
    if board_json.resolution == 1.0 && user_unit == Unit::Mm {
        resolution = 10_000;
    }
    let scale_factor = f64::from(resolution);

    let layer_count = board.get_layer_count();

    if let Some(zones) = board_json.conductionAreas.as_ref() {
        for zone in zones {
            let net_number =
                java_nets_get(&board.rules.nets, zone.netName.as_deref(), 1).unwrap_or(0);
            let net_numbers = if net_number > 0 {
                vec![net_number]
            } else {
                Vec::new()
            };

            let Some(polygon) = zone.polygon.as_ref() else {
                return Err(session_npe("java.util.List.size()", "zone.polygon"));
            };
            let zone_points: Vec<Point> = polygon
                .iter()
                .map(|corner| {
                    Point::Int(IntPoint::new(
                        java_round_to_int(corner.x * scale_factor),
                        java_round_to_int(-corner.y * scale_factor),
                    ))
                })
                .collect();
            if zone_points.is_empty() {
                return Err(DsnError::KicadSession(
                    "java.lang.ArrayIndexOutOfBoundsException: Index 0 out of bounds for length 0"
                        .to_string(),
                ));
            }
            #[allow(clippy::cast_sign_loss)]
            board.insert_conduction_area(
                Area::Shape(Shape::Polygon(PolygonShape::from_points(&zone_points))),
                zone.layerIndex as usize,
                net_numbers,
                1,
                zone.isObstacle,
                FixedState::UserFixed,
            );
        }
    }

    if let Some(traces) = board_json.traces.as_ref() {
        for trace in traces {
            let net_number =
                java_nets_get(&board.rules.nets, trace.netName.as_deref(), 1).unwrap_or(0);
            let net_numbers = if net_number > 0 {
                vec![net_number]
            } else {
                Vec::new()
            };
            let trace_half_width = java_round_to_int(trace.width * scale_factor / 2.0);

            let Some(points) = trace.points.as_ref() else {
                return Err(session_npe("java.util.List.size()", "tr.points"));
            };
            let trace_points: Vec<Point> = points
                .iter()
                .map(|point| {
                    Point::Int(IntPoint::new(
                        java_round_to_int(point.x * scale_factor),
                        java_round_to_int(-point.y * scale_factor),
                    ))
                })
                .collect();
            board.insert_trace_at_points(
                &trace_points,
                trace.layerIndex.max(0) as usize,
                trace_half_width,
                net_numbers,
                1,
                FixedState::UserFixed,
            );
        }
    }

    if let Some(vias) = board_json.vias.as_ref() {
        for via in vias {
            let net_number =
                java_nets_get(&board.rules.nets, via.netName.as_deref(), 1).unwrap_or(0);
            let net_numbers = if net_number > 0 {
                vec![net_number]
            } else {
                Vec::new()
            };

            let Some(position) = via.position.as_ref() else {
                return Err(session_npe_field("x", "vj.position"));
            };
            let center = IntPoint::new(
                java_round_to_int(position.x * scale_factor),
                java_round_to_int(-position.y * scale_factor),
            );

            let mut shapes: Vec<Option<Shape>> = vec![None; layer_count];
            let shape = via_shape(via.diameter * scale_factor / 2.0);
            let mut layer = via.startLayerIndex;
            while layer <= via.endLayerIndex {
                if layer >= 0 && (layer as i64) < layer_count as i64 {
                    shapes[layer as usize] = Some(shape.clone());
                }
                layer = layer.wrapping_add(1);
            }
            let via_padstack_name = format!(
                "Via[{}-{}]_{}:{}_um",
                via.startLayerIndex,
                via.endLayerIndex,
                java_format_fixed(via.diameter * 1000.0, 0),
                java_format_fixed(via.drill * 1000.0, 0)
            );
            if shapes.iter().all(Option::is_none) {
                return Err(DsnError::KicadSession(format!(
                    "the KiCad session JSON file is malformed: via `{via_padstack_name}` has no \
                     shape on any layer, because its layer span is empty"
                )));
            }
            let existing = board
                .library
                .padstacks
                .get_by_name(&via_padstack_name)
                .map(|padstack| PadstackId(padstack.no));
            let via_padstack = match existing {
                Some(id) => id,
                None => board
                    .library
                    .padstacks
                    .add(via_padstack_name, shapes, true, false),
            };
            let stop = || false;
            board
                .insert_via_checked(
                    via_padstack,
                    Point::Int(center),
                    net_numbers,
                    1,
                    FixedState::UserFixed,
                    true,
                    &stop,
                )
                .map_err(|error| {
                    DsnError::KicadSession(format!("java.lang.RuntimeException: {error}"))
                })?;
        }
    }
    Ok(())
}

fn session_npe(invoked: &str, receiver: &str) -> DsnError {
    DsnError::KicadSession(format!(
        "java.lang.NullPointerException: Cannot invoke \"{invoked}\" because \"{receiver}\" is null"
    ))
}

fn session_npe_field(field: &str, receiver: &str) -> DsnError {
    DsnError::KicadSession(format!(
        "java.lang.NullPointerException: Cannot read field \"{field}\" because \"{receiver}\" is null"
    ))
}

#[allow(clippy::cast_possible_truncation)]
fn java_double_to_int(value: f64) -> i32 {
    value as i32
}

fn find_kicad_default_net_class(net_classes: &[NetClassJson]) -> Option<&NetClassJson> {
    net_classes.iter().find(|net_class| {
        net_class
            .name
            .as_deref()
            .is_some_and(is_kicad_default_net_class_name)
    })
}

fn non_default_net_classes(net_classes: &[NetClassJson]) -> Vec<&NetClassJson> {
    net_classes
        .iter()
        .filter(|net_class| {
            !net_class
                .name
                .as_deref()
                .is_some_and(is_kicad_default_net_class_name)
        })
        .collect()
}

fn apply_kicad_net_class_parameters(
    rules: &mut BoardRules,
    target: NetClassId,
    source: &NetClassJson,
    layer_count: usize,
    scale_factor: f64,
    clearance_class_index: usize,
) {
    if source.traceWidth > 0.0 {
        let trace_half_width = java_round_to_int(source.traceWidth * scale_factor / 2.0);
        for layer in 0..layer_count {
            rules
                .net_classes
                .get_mut(target)
                .set_trace_half_width(layer, trace_half_width);
        }
    }
    if source.clearance > 0.0 {
        rules
            .net_classes
            .get_mut(target)
            .set_trace_clearance_class(clearance_class_index);
    }
}

fn resolve_net_class_index(map: &[(String, usize)], class_name: Option<&str>) -> usize {
    if class_name.is_some_and(is_kicad_default_net_class_name) {
        return 1;
    }
    if let Some(name) = class_name
        && let Some((_, value)) = map.iter().find(|(key, _)| key == name)
    {
        return *value;
    }
    if let Some(name) = class_name
        && let Some((_, value)) = map.iter().find(|(key, _)| equals_ignore_case(key, name))
    {
        return *value;
    }
    1
}

struct PointOutline {
    points: Vec<FloatPoint>,
}

impl PointOutline {
    fn new() -> PointOutline {
        PointOutline { points: Vec::new() }
    }

    fn add_point(&mut self, point: FloatPoint) {
        self.points.push(point);
    }

    fn bounding_box(&self) -> IntBox {
        if self.points.is_empty() {
            return IntBox::EMPTY;
        }
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = -f64::MAX;
        let mut max_y = -f64::MAX;
        for point in &self.points {
            min_x = java_min(min_x, point.x);
            min_y = java_min(min_y, point.y);
            max_x = java_max(max_x, point.x);
            max_y = java_max(max_y, point.y);
        }
        IntBox::from_coords(
            java_round_to_int(min_x),
            java_round_to_int(min_y),
            java_round_to_int(max_x),
            java_round_to_int(max_y),
        )
    }
}

fn get_descriptive_padstack_name(
    pad: &PadJson,
    board_layer_names: &[String],
    layer_count: usize,
) -> Result<String, String> {
    let mut shape_str = "Round";
    let mut fall_through: String;
    if let Some(shape) = pad.shape.as_deref() {
        if equals_ignore_case("circle", shape) || equals_ignore_case("round", shape) {
            shape_str = "Round";
        } else if equals_ignore_case("rect", shape) || equals_ignore_case("rectangle", shape) {
            shape_str = "Rect";
        } else if equals_ignore_case("oval", shape) {
            shape_str = "Oval";
        } else {
            let mut chars = shape.chars();
            let Some(first) = chars.next() else {
                return Err("Range [0, 1) out of bounds for length 0".to_string());
            };
            fall_through = String::new();
            fall_through.push(java_to_upper(first));
            for c in chars {
                fall_through.push(java_to_lower(c));
            }
            shape_str = &fall_through;
        }
    }

    let mut layer_type = "A";
    if let Some(pad_layers) = pad.layers.as_ref()
        && pad_layers.len() == 1
    {
        let layer_name = pad_layers[0]
            .as_deref()
            .expect("section 1a refused a null pad-layer element");
        if equals_ignore_case(&board_layer_names[0], layer_name) {
            layer_type = "T";
        } else if equals_ignore_case(&board_layer_names[layer_count - 1], layer_name) {
            layer_type = "B";
        }
    }

    let size = pad
        .size
        .as_ref()
        .expect("KiCadJsonReader.java:509 dereferenced pad.size before calling this");
    Ok(format!(
        "{shape_str}[{layer_type}]Pad_{}x{}_um",
        java_format_fixed(size.x * 1000.0, 0),
        java_format_fixed(size.y * 1000.0, 0)
    ))
}

fn are_package_pins_identical(pkg1: &Package, p2: &[PackagePin]) -> bool {
    if pkg1.pin_count() != p2.len() {
        return false;
    }
    for (i, pin2) in p2.iter().enumerate() {
        let pin1 = pkg1
            .get_pin(i32::try_from(i).unwrap_or(i32::MAX))
            .expect("i < pin_count(), checked above");
        if pin1.name != pin2.name {
            return false;
        }
        if pin1.padstack_no != pin2.padstack_no {
            return false;
        }
        let loc1 = pin1.relative_location.to_float();
        let loc2 = pin2.relative_location.to_float();
        if (loc1.x - loc2.x).abs() > 0.001 || (loc1.y - loc2.y).abs() > 0.001 {
            return false;
        }
        if (pin1.rotation_in_degree - pin2.rotation_in_degree).abs() > 0.001 {
            return false;
        }
    }
    true
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct JavaNpe {
    invoked: &'static str,
    receiver: &'static str,
}

struct ReferencedNets {
    keys: Vec<String>,
}

impl ReferencedNets {
    fn new() -> ReferencedNets {
        ReferencedNets { keys: Vec::new() }
    }

    fn add(&mut self, key: &str) {
        if !self.keys.iter().any(|existing| existing == key) {
            self.keys.push(key.to_string());
        }
    }

    fn iteration_order(&self) -> &[String] {
        &self.keys
    }
}

fn unique_padstack_name(padstacks: &Padstacks, name: String) -> String {
    if padstacks.get_by_name(&name).is_none() {
        return name;
    }
    let mut suffix = 2usize;
    loop {
        let candidate = format!("{name}#{suffix}");
        if padstacks.get_by_name(&candidate).is_none() {
            return candidate;
        }
        suffix += 1;
    }
}

fn outline_clearance_class(matrix: &ClearanceMatrix, clearance: f64, scale_factor: f64) -> usize {
    if clearance <= 0.0 {
        return 1;
    }
    let value = java_round_to_int(clearance * scale_factor);
    (1..matrix.get_class_count())
        .find(|&class| matrix.get_value(class, class, 0, false) == value)
        .unwrap_or(1)
}

fn no_shape_on_any_layer(section: &str, object: &str, because: &str) -> BoardReadResult {
    parse_error(
        section,
        &format!(
            "the KiCad board JSON file is malformed in its `{section}` section: {object} has no \
             shape on any layer, because {because}"
        ),
    )
}

fn java_nets_get(nets: &Nets, name: Option<&str>, subnet_number: i32) -> Option<i32> {
    for current_net in nets.iter() {
        if name.is_some_and(|name| equals_ignore_case(&current_net.name, name))
            && current_net.subnet_number == subnet_number
        {
            return Some(current_net.net_number);
        }
    }
    None
}

#[allow(dead_code)]
fn java_drill_item_tile_shape_count(padstack: &Padstack) -> i32 {
    padstack.to_layer() - padstack.from_layer() + 1
}

fn parse_error(location: &str, detail: &str) -> BoardReadResult {
    BoardReadResult::ParseError {
        location: location.to_string(),
        detail: detail.to_string(),
    }
}

fn npe(invoked: &str, receiver: &str) -> BoardReadResult {
    parse_error(
        "json_payload",
        &format!("Exception occurred: {}", npe_message(invoked, receiver)),
    )
}

fn npe_message(invoked: &str, receiver: &str) -> String {
    format!("Cannot invoke \"{invoked}\" because \"{receiver}\" is null")
}

fn npe_field(field: &str, receiver: &str) -> BoardReadResult {
    parse_error(
        "json_payload",
        &format!(
            "Exception occurred: Cannot read field \"{field}\" because \"{receiver}\" is null"
        ),
    )
}

fn via_shape(radius: f64) -> Shape {
    let lower = java_round_to_int(-radius);
    let upper = java_round_to_int(radius);
    Shape::Tile(fr_geometry::TileShape::Simplex(
        IntBox::from_coords(lower, lower, upper, upper).to_simplex(),
    ))
}

fn registered_via_info(rules: &BoardRules, name: &str) -> ViaInfo {
    rules
        .via_infos
        .get_by_name(name)
        .cloned()
        .expect("ViaInfos::add was called with this name immediately above")
}

fn java_min(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && b.is_sign_negative() {
        return b;
    }
    if a <= b { a } else { b }
}

fn java_max(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && a.is_sign_negative() {
        return b;
    }
    if a >= b { a } else { b }
}

fn java_double_compare(a: f64, b: f64) -> Ordering {
    if a < b {
        Ordering::Less
    } else if a > b {
        Ordering::Greater
    } else {
        let bits = |x: f64| {
            if x.is_nan() {
                0x7ff8_0000_0000_0000_u64 as i64
            } else {
                x.to_bits() as i64
            }
        };
        bits(a).cmp(&bits(b))
    }
}

fn java_is_blank(text: &str) -> bool {
    text.chars().all(java_is_whitespace)
}

fn java_is_whitespace(c: char) -> bool {
    matches!(c, '\u{1c}'..='\u{1f}')
        || (c.is_whitespace() && !matches!(c, '\u{a0}' | '\u{85}' | '\u{2007}' | '\u{202f}'))
}

#[allow(dead_code)]
fn java_string_hash(text: &str) -> i32 {
    let mut hash: i32 = 0;
    for unit in text.encode_utf16() {
        hash = hash.wrapping_mul(31).wrapping_add(i32::from(unit));
    }
    hash
}

#[allow(dead_code)]
fn java_hash_iteration_order(keys: &[String]) -> Vec<usize> {
    let mut capacity = 16_usize;
    let mut threshold = 12_usize;
    for size in 1..=keys.len() {
        if size > threshold {
            capacity *= 2;
            threshold *= 2;
        }
    }
    let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); capacity];
    for (index, key) in keys.iter().enumerate() {
        let hash = java_string_hash(key) as u32;
        let spread = hash ^ (hash >> 16);
        buckets[(capacity - 1) & (spread as usize)].push(index);
    }
    buckets.into_iter().flatten().collect()
}

#[allow(dead_code)]
struct JavaStringSet {
    keys: Vec<String>,
}

#[allow(dead_code)]
impl JavaStringSet {
    fn new() -> JavaStringSet {
        JavaStringSet { keys: Vec::new() }
    }

    fn add(&mut self, key: &str) {
        if !self.keys.iter().any(|existing| existing == key) {
            self.keys.push(key.to_string());
        }
    }

    fn iteration_order(&self) -> Vec<&str> {
        java_hash_iteration_order(&self.keys)
            .into_iter()
            .map(|index| self.keys[index].as_str())
            .collect()
    }
}

#[allow(dead_code)]
struct JavaStringMap {
    entries: Vec<(String, usize)>,
}

#[allow(dead_code)]
impl JavaStringMap {
    fn new() -> JavaStringMap {
        JavaStringMap {
            entries: Vec::new(),
        }
    }

    fn put(&mut self, key: String, value: usize) {
        if let Some(entry) = self.entries.iter_mut().find(|(name, _)| *name == key) {
            entry.1 = value;
        } else {
            self.entries.push((key, value));
        }
    }

    fn get(&self, key: &str) -> Option<usize> {
        self.entries
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| *value)
    }

    fn entry_set(&self) -> Vec<(&str, usize)> {
        let keys: Vec<String> = self.entries.iter().map(|(name, _)| name.clone()).collect();
        java_hash_iteration_order(&keys)
            .into_iter()
            .map(|index| {
                let (name, value) = &self.entries[index];
                (name.as_str(), *value)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_is_blank_matches_character_is_whitespace() {
        assert!(java_is_blank(""));
        assert!(java_is_blank("  \t\r\n\u{b}\u{c}"));
        assert!(java_is_blank("\u{1c}\u{1d}\u{1e}\u{1f}"));
        assert!(!java_is_blank("\u{a0}"));
        assert!(!java_is_blank("\u{85}"));
        assert!(!java_is_blank("\u{2007}"));
        assert!(!java_is_blank("\u{202f}"));
        assert!(!java_is_blank("KiCad"));
    }

    #[test]
    fn java_string_hash_matches_the_jvm() {
        assert_eq!(java_string_hash(""), 0);
        assert_eq!(java_string_hash("GND"), 70717);
        assert_eq!(java_string_hash("default"), 1_544_803_905);
        assert_eq!(java_string_hash("kicad_default"), -800_824_662);
        assert_eq!(java_string_hash("\u{1F600}"), 1_772_899);
    }

    #[test]
    fn java_double_compare_is_a_total_order() {
        assert_eq!(java_double_compare(1.0, 2.0), Ordering::Less);
        assert_eq!(java_double_compare(2.0, 1.0), Ordering::Greater);
        assert_eq!(java_double_compare(1.0, 1.0), Ordering::Equal);
        assert_eq!(java_double_compare(-0.0, 0.0), Ordering::Less);
        assert_eq!(
            java_double_compare(f64::NAN, f64::INFINITY),
            Ordering::Greater
        );
        assert_eq!(java_double_compare(f64::NAN, f64::NAN), Ordering::Equal);
    }

    #[test]
    fn java_min_and_max_propagate_nan_where_rusts_do_not() {
        assert!(java_min(f64::NAN, 1.0).is_nan());
        assert!(java_max(f64::NAN, 1.0).is_nan());
        assert!(java_min(1.0, f64::NAN).is_nan());
        assert!(java_max(1.0, f64::NAN).is_nan());
        assert_eq!(f64::min(f64::NAN, 1.0), 1.0);
        assert_eq!(f64::max(1.0, f64::NAN), 1.0);
    }

    #[test]
    fn java_min_and_max_order_signed_zero() {
        assert!(java_min(-0.0, 0.0).is_sign_negative());
        assert!(java_min(0.0, -0.0).is_sign_negative());
        assert!(java_max(-0.0, 0.0).is_sign_positive());
        assert!(java_max(0.0, -0.0).is_sign_positive());
        assert_eq!(java_min(1.0, 2.0), 1.0);
        assert_eq!(java_max(1.0, 2.0), 2.0);
        assert_eq!(java_min(2.0, 1.0), 1.0);
        assert_eq!(java_max(2.0, 1.0), 2.0);
    }

    #[test]
    fn the_via_shape_rounds_each_corner_separately() {
        let Shape::Tile(fr_geometry::TileShape::Simplex(simplex)) = via_shape(0.5) else {
            panic!("IntBox::to_simplex answers a Simplex");
        };
        let expected = IntBox::from_coords(0, 0, 1, 1).to_simplex();
        assert_eq!(simplex, expected, "round(-0.5) is 0, not -1");
        let Shape::Tile(fr_geometry::TileShape::Simplex(simplex)) = via_shape(4000.0) else {
            panic!("IntBox::to_simplex answers a Simplex");
        };
        assert_eq!(
            simplex,
            IntBox::from_coords(-4000, -4000, 4000, 4000).to_simplex()
        );
    }

    #[test]
    fn the_referenced_net_set_dedups_on_exact_equality_and_keeps_insertion_order() {
        let mut set = ReferencedNets::new();
        for name in ["GND", "gnd", "GND", "VCC"] {
            set.add(name);
        }
        assert_eq!(set.iteration_order(), ["GND", "gnd", "VCC"]);
    }
}
