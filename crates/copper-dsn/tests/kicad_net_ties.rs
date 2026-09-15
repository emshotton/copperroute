use copper_board::{Board, Item};
use copper_dsn::error::BoardReadResult;
use copper_dsn::kicad::pcb::{default_net_class, read_pcb};
use copper_dsn::kicad::read_board_json;

const TIE_BOARD: &str = r#"(kicad_pcb (version 20241229)
  (layers (0 "F.Cu" signal) (2 "B.Cu" signal))
  (net 0 "")
  (net 1 "GND")
  (net 2 "AGND")
  (gr_line (start 0 0) (end 20 0) (layer "Edge.Cuts") (stroke (width 0.05) (type solid)))
  (gr_line (start 20 0) (end 20 20) (layer "Edge.Cuts") (stroke (width 0.05) (type solid)))
  (gr_line (start 20 20) (end 0 20) (layer "Edge.Cuts") (stroke (width 0.05) (type solid)))
  (gr_line (start 0 20) (end 0 0) (layer "Edge.Cuts") (stroke (width 0.05) (type solid)))
  (footprint "NT" (layer "F.Cu") (at 10 10) (net_tie_pad_groups "1,2")
    (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND"))
    (pad "2" smd rect (at 1 0) (size 1 1) (layers "F.Cu") (net 2 "AGND")))
)"#;

fn board_of(text: &str) -> Board {
    let imported = read_pcb(text, "tie", &default_net_class()).expect("the board imports");
    match read_board_json(imported.board, None) {
        BoardReadResult::Success { board: Some(b), .. } => *b,
        other => panic!("expected a loaded board, got {other:?}"),
    }
}

fn pins(board: &Board) -> Vec<copper_board::ItemId> {
    let mut ids: Vec<_> = board
        .items
        .iter()
        .filter(|(_, item)| matches!(item, Item::Pin(_)))
        .map(|(id, _)| *id)
        .collect();
    ids.sort();
    ids
}

#[test]
fn a_tie_boards_pads_are_registered_against_each_others_nets() {
    let board = board_of(TIE_BOARD);
    let ties = &board.rules.net_ties;
    assert!(!ties.is_empty());
    let ids = pins(&board);
    assert_eq!(ids.len(), 2);
    let nets: Vec<&[i32]> = ids.iter().map(|id| ties.nets_of(*id)).collect();
    assert_eq!(nets, vec![&[2][..], &[1][..]]);
    assert!(ties.may_short(ids[0], &[1], ids[1], &[2]));
}

#[test]
fn a_board_without_ties_registers_nothing() {
    let plain = TIE_BOARD.replace(r#" (net_tie_pad_groups "1,2")"#, "");
    let board = board_of(&plain);
    assert!(board.rules.net_ties.is_empty());
}

const TIE_BOARD_WITH_COPPER: &str = r#"(kicad_pcb (version 20241229)
  (layers (0 "F.Cu" signal) (2 "B.Cu" signal))
  (net 0 "")
  (net 1 "GND")
  (net 2 "AGND")
  (gr_line (start 0 0) (end 20 0) (layer "Edge.Cuts") (stroke (width 0.05) (type solid)))
  (gr_line (start 20 0) (end 20 20) (layer "Edge.Cuts") (stroke (width 0.05) (type solid)))
  (gr_line (start 20 20) (end 0 20) (layer "Edge.Cuts") (stroke (width 0.05) (type solid)))
  (gr_line (start 0 20) (end 0 0) (layer "Edge.Cuts") (stroke (width 0.05) (type solid)))
  (footprint "NT" (layer "F.Cu") (at 10 10) (net_tie_pad_groups "1,2")
    (fp_rect (start 0 0) (end 1 0.1) (layer "F.Cu") (stroke (width 0.05) (type solid)))
    (fp_rect (start 5 5) (end 5.5 5.1) (layer "F.Cu") (stroke (width 0.05) (type solid)))
    (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND"))
    (pad "2" smd rect (at 1 0) (size 1 1) (layers "F.Cu") (net 2 "AGND")))
)"#;

fn conduction_nets(board: &Board) -> Vec<Vec<i32>> {
    let mut areas: Vec<(copper_board::ItemId, Vec<i32>)> = board
        .items
        .iter()
        .filter(|(_, item)| matches!(item, Item::ConductionArea(_)))
        .map(|(id, item)| {
            let mut nets = item.net_nos().to_vec();
            nets.sort();
            (*id, nets)
        })
        .collect();
    areas.sort_by_key(|(id, _)| *id);
    areas.into_iter().map(|(_, nets)| nets).collect()
}

#[test]
fn tie_copper_takes_the_nets_of_the_pads_it_overlaps() {
    let board = board_of(TIE_BOARD_WITH_COPPER);
    assert_eq!(conduction_nets(&board), vec![vec![1, 2], Vec::new()]);
}

#[test]
fn tie_copper_is_transparent_to_its_own_nets_and_opaque_to_others() {
    let board = board_of(TIE_BOARD_WITH_COPPER);
    let area = board
        .items
        .values()
        .find(|item| matches!(item, Item::ConductionArea(_)) && item.net_count() == 2)
        .expect("the joining copper");
    assert!(!area.is_trace_obstacle(1));
    assert!(!area.is_trace_obstacle(2));
    assert!(area.is_trace_obstacle(9));
}
