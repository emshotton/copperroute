use copper_board::items::Item;
use copper_board::{Board, ItemId};
use copper_drc::DesignRulesChecker;

pub fn build_unrouted_report(board: &mut Board) -> String {
    let airlines = {
        let mut drc = DesignRulesChecker::new(board);
        drc.calculate_all_incompletes();
        drc.get_all_airlines()
    };

    if airlines.is_empty() {
        return "  (no unrouted connections found)".to_string();
    }

    let mut by_net: Vec<(String, Vec<String>)> = Vec::new();
    for airline in &airlines {
        let net_name = board
            .rules
            .nets
            .get(airline.net_number)
            .map(|net| net.name.clone())
            .unwrap_or_else(|| "(unknown net)".to_string());
        let from_desc = describe_item(board, airline.from_item);
        let to_desc = describe_item(board, airline.to_item);
        let line = format!("    - {from_desc}  ->  {to_desc}");
        match by_net.iter_mut().find(|(name, _)| *name == net_name) {
            Some((_, lines)) => lines.push(line),
            None => by_net.push((net_name, vec![line])),
        }
    }

    let mut result = String::new();
    for (net_name, lines) in &by_net {
        let count = lines.len();
        result.push_str("  Net '");
        result.push_str(net_name);
        result.push_str("' (");
        result.push_str(&count.to_string());
        result.push_str(" unrouted connection");
        if count != 1 {
            result.push('s');
        }
        result.push_str("):\n");
        for line in lines {
            result.push_str(line);
            result.push('\n');
        }
    }
    result.trim_end().to_string()
}

pub(crate) fn describe_item(board: &Board, item: ItemId) -> String {
    if let Some(Item::Pin(pin)) = board.get_item(item) {
        let component_id = pin.hdr.get_component_id();
        if component_id >= 1 && (component_id as usize) <= board.components.count() {
            let component = board.components.get(component_id);
            let package = board.library.packages.get(component.get_package());
            if let Some(package_pin) = package.get_pin(pin.get_pin_index()) {
                return format!("{}-{}", component.name, package_pin.name);
            }
            return format!("{} (pin #{})", component.name, pin.get_pin_index());
        }
    }
    board
        .get_item(item)
        .map(|i| i.to_string())
        .unwrap_or_else(|| "(unknown)".to_string())
}
