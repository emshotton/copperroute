use copper_dsn::{BoardReadResult, DsnReadOptions};
fn main() {
    let bytes = std::fs::read("/tmp/quality-saiboard-boundary/unrouted.dsn").unwrap();
    let BoardReadResult::Success { board: Some(mut board), coordinate_transform: Some(transform), .. } = copper_dsn::read_board(&bytes[..], None, None, &DsnReadOptions::default()) else { panic!("load") };
    for id in board.get_pins() {
        let Some(copper_board::Item::Pin(pin)) = board.get_item(id) else { continue };
        let name = &board.components.get(pin.hdr.get_component_id()).name;
        if !["Q3", "Q5", "U2", "U18"].contains(&name.as_str()) {continue}
        let ctx = board.ctx();
        println!("{} {} center={:?} class={} rule-to-trace={}",name,pin.name(&ctx).unwrap(),transform.board_to_dsn_point(&pin.get_center(&ctx).to_float()),pin.hdr.clearance_class(),board.rules.clearance_matrix.get_value(pin.hdr.clearance_class(),1,0,false));
        println!("shape={:?}",board.item_tile_shape(id,0));
    }
}
