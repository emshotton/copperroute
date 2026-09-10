use copper_dsn::{BoardReadResult, DsnReadOptions};
mod corridor {
    include!("/Users/em/Development/freerouting/copperroute-corridor-coherent/crates/copper-router/src/autoroute/maze/corridor.rs");
    impl Corridor {
        pub fn contains(&self, p: copper_geometry::FloatPoint) -> bool {
            let u=p.x*self.axis.x+p.y*self.axis.y;
            let v=p.y*self.axis.x-p.x*self.axis.y;
            u>=self.along[0]-1e-6 && u<=self.along[1]+1e-6 && v>=self.across[0]-1e-6 && v<=self.across[1]+1e-6
        }
    }
}
fn main() {
    for path in std::env::args().skip(1) {
        let bytes=std::fs::read(&path).unwrap();
        let BoardReadResult::Success{board:Some(board),..}=copper_dsn::read_board(&bytes[..],None,None,&DsnReadOptions::default()) else {panic!("read {path}")};
        let mut nets=std::collections::BTreeSet::new();
        for id in board.get_pins(){nets.extend(board.get_item(id).unwrap().net_nos().iter().copied());}
        println!("BOARD {path}");
        for net in nets {
            let Some(band)=corridor::Corridor::for_net(&board,net) else{continue};
            let mut pins=Vec::new();
            for id in board.get_pins(){
                let item=board.get_item(id).unwrap();
                if !item.net_nos().contains(&net){continue}
                let Some(p)=board.drill_center(id) else{continue};
                let name=&board.components.get(item.component_id()).name;
                pins.push((name.clone(),band.contains(p.to_float()),p));
            }
            println!("NET {} {:?} band={:?} terminals={:?}",net,board.rules.nets.get(net).map(|n|&n.name),band,pins);
        }
    }
}
