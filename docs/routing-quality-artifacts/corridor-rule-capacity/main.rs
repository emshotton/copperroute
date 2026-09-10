mod corridor;
use copperroute::ops::load::{load,BoardSource,LoadRequest};
fn main(){
 let args:Vec<_>=std::env::args().collect();
 let mut request=LoadRequest::for_board(BoardSource::Path(args[1].clone().into()));
 request.kicad_project=Some(args[2].clone().into());
 let loaded=load(&request).unwrap();let board=loaded.board;
 let net=board.rules.nets.get_by_name(&args[3])[0].net_number;
 println!("band={:?}",corridor::Corridor::for_net(&board,net));
}
