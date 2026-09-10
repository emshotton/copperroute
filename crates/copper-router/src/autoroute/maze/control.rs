use super::corridor::Corridor;
use copper_board::ids::{ItemId, NetClassId};
use copper_board::rules::{PadstackLookup, ViaRule};
use copper_board::structure::Unit;
use copper_board::{Board, Item};
use copper_geometry::{FloatPoint, Point};
use copper_settings::{ExpansionCostFactor, RouterSettings};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViaPricing {
    ByPadstackRadius,
    PerMillimetre,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViaMask {
    pub from_layer: i32,
    pub to_layer: i32,
    pub attach_smd_allowed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AutorouteControl {
    corridor: Option<Corridor>,
    pub trace_costs: Vec<ExpansionCostFactor>,
    pub bend_costs: Vec<f64>,
    pub with_neckdown: bool,
    pub layer_active: Vec<bool>,
    pub layer_count: usize,
    pub trace_half_width: Vec<i32>,
    pub compensated_trace_half_width: Vec<i32>,
    pub via_radii: Vec<f64>,
    pub add_via_costs: Vec<Vec<i32>>,
    pub trace_clearance_class_index: usize,
    pub vias_allowed: bool,
    legacy_smd_attachment_relaxation: bool,
    pub min_normal_via_cost: f64,
    pub ripup_allowed: bool,
    pub ripup_costs: i32,
    pub ripup_pass_no: i32,
    pub is_fanout: bool,
    pub fanout_start_pin_name: Option<String>,
    pub fanout_start_pin_center: Option<Point>,
    pub fanout_start_pin_layer: i32,
    pub remove_unconnected_vias: bool,
    pub via_rule: Option<ViaRule>,
    pub net_number: i32,
    pub via_clearance_class: usize,
    pub via_infos: Vec<ViaMask>,
    pub via_lower_bound: usize,
    pub via_upper_bound: usize,
    pub max_via_radius: f64,
    pub tidy_region_width: i32,
    pub pull_tight_accuracy: i32,
    pub max_shove_trace_recursion_depth: i32,
    pub max_shove_via_recursion_depth: i32,
    pub max_spring_over_recursion_depth: i32,
    pub min_cheap_via_cost: f64,

    pub fanout_max_escape_length: f64,
    pub fanout_min_escape_length: f64,

    pub start_ripup_costs: i32,

    pub smd_via_relaxation: bool,
    pub units_per_mm: f64,
    pub trace_cost_per_mm: f64,
    pub smd_via_cost_factor: f64,
    pub via_pricing: ViaPricing,
}

pub fn board_units_per_mm(board: &Board) -> f64 {
    let resolution = board.communication.resolution.max(1);
    f64::from(resolution) / Unit::scale(1.0, board.communication.unit, Unit::Mm)
}

impl AutorouteControl {
    pub(super) fn movement_cost(&self, from: &FloatPoint, to: &FloatPoint, layer: usize) -> f64 {
        let distance = from.weighted_distance(
            to,
            self.trace_costs[layer].horizontal,
            self.trace_costs[layer].vertical,
        );
        let Some(corridor) = &self.corridor else {
            return distance;
        };
        distance * (1.0 + 0.25 * corridor.outside_fraction(from, to))
    }

    pub fn new(
        board: &Board,
        net_no: i32,
        settings: &RouterSettings,
        via_costs: i32,
        trace_costs: &[ExpansionCostFactor],
    ) -> AutorouteControl {
        AutorouteControl::priced(
            board,
            net_no,
            settings,
            via_costs,
            trace_costs,
            ViaPricing::ByPadstackRadius,
        )
    }

    pub fn priced(
        board: &Board,
        net_no: i32,
        settings: &RouterSettings,
        via_costs: i32,
        trace_costs: &[ExpansionCostFactor],
        via_pricing: ViaPricing,
    ) -> AutorouteControl {
        let mut control = AutorouteControl::private(board, settings, trace_costs);
        control.via_pricing = via_pricing;
        control.init_net(net_no, board, via_costs);
        if corridor_guidance_enabled() {
            control.corridor = Corridor::for_net(board, net_no);
        }
        control
    }

    pub fn from_settings(
        board: &Board,
        net_no: i32,
        settings: &RouterSettings,
    ) -> AutorouteControl {
        let trace_costs = settings.get_trace_costs();
        AutorouteControl::new(
            board,
            net_no,
            settings,
            settings.get_via_costs(),
            &trace_costs,
        )
    }

    fn private(
        board: &Board,
        settings: &RouterSettings,
        trace_costs: &[ExpansionCostFactor],
    ) -> AutorouteControl {
        let layer_count = board.get_layer_count();
        let units_per_mm = board_units_per_mm(board);
        let trace_cost_per_mm = settings
            .scoring
            .as_ref()
            .and_then(|s| s.default_preferred_direction_trace_cost)
            .unwrap_or(1.0);
        let mut bend_costs = Vec::with_capacity(layer_count);
        for i in 0..layer_count {
            bend_costs.push(settings.get_bend_cost(i) * units_per_mm * trace_cost_per_mm);
        }

        let mut layer_active = Vec::with_capacity(layer_count);
        for i in 0..layer_count {
            let active_setting = settings.get_layer_active(i);
            let layer = &board.layer_structure().layers[i];
            layer_active.push(if !layer.is_signal && active_setting {
                false
            } else {
                active_setting
            });
        }

        AutorouteControl {
            corridor: None,
            trace_costs: trace_costs.to_vec(),
            bend_costs,
            with_neckdown: settings.get_automatic_neckdown(),
            layer_active,
            layer_count,
            trace_half_width: vec![0; layer_count],
            compensated_trace_half_width: vec![0; layer_count],
            via_radii: vec![0.0; layer_count],
            add_via_costs: vec![vec![0; layer_count]; layer_count],
            trace_clearance_class_index: 0,
            vias_allowed: settings.get_vias_allowed(),
            legacy_smd_attachment_relaxation: false,
            min_normal_via_cost: 0.0,
            ripup_allowed: false,
            ripup_costs: 1000,
            ripup_pass_no: 1,
            is_fanout: false,
            fanout_start_pin_name: None,
            fanout_start_pin_center: None,
            fanout_start_pin_layer: -1,
            remove_unconnected_vias: true,
            via_rule: None,
            net_number: 0,
            via_clearance_class: 0,
            via_infos: Vec::new(),
            via_lower_bound: 0,
            via_upper_bound: layer_count,
            max_via_radius: 0.0,
            tidy_region_width: i32::MAX,
            pull_tight_accuracy: 500,
            max_shove_trace_recursion_depth: 20,
            max_shove_via_recursion_depth: 5,
            max_spring_over_recursion_depth: 5,
            min_cheap_via_cost: 0.0,
            fanout_max_escape_length: settings
                .fanout
                .as_ref()
                .and_then(|f| f.max_escape_length_mm)
                .map_or(3000.0, |mm| mm * 1000.0),
            fanout_min_escape_length: settings
                .fanout
                .as_ref()
                .and_then(|f| f.min_escape_length_mm)
                .map_or(500.0, |mm| mm * 1000.0),
            start_ripup_costs: settings.get_start_ripup_costs(),
            smd_via_relaxation: settings.get_smd_via_relaxation(),
            units_per_mm,
            trace_cost_per_mm,
            smd_via_cost_factor: settings.get_smd_via_cost_factor(),
            via_pricing: ViaPricing::ByPadstackRadius,
        }
    }

    pub fn is_pure_smd_net(board: &Board, net_number: i32) -> bool {
        let net_items = board.get_connectable_items(net_number);
        if net_items.is_empty() {
            return false;
        }
        let ctx = board.ctx();
        net_items
            .into_iter()
            .all(|id: ItemId| match board.get_item(id) {
                Some(item @ Item::Pin(_)) => item.first_layer(&ctx) == item.last_layer(&ctx),
                _ => false,
            })
    }

    fn init_net(&mut self, net_number: i32, board: &Board, via_costs: i32) {
        self.net_number = net_number;
        let current_net_class = match board.rules.nets.get(net_number) {
            Some(net) => {
                let class = net.get_net_class();
                let net_class = board.rules.net_classes.get(class);
                self.trace_clearance_class_index = net_class.get_trace_clearance_class();
                self.via_rule = net_class.get_via_rule().cloned();
                Some(class)
            }
            None => {
                self.trace_clearance_class_index = 1;
                assert!(
                    !board.rules.via_rules.is_empty(),
                    "AutorouteControl.initNet: board.rules.viaRules.firstElement() — Java's \
                     Vector.firstElement() throws NoSuchElementException on an empty vector \
                     (AutorouteControl.java:214)"
                );
                self.via_rule = Some(board.rules.via_rules[0].clone());
                None
            }
        };
        for i in 0..self.layer_count {
            self.trace_half_width[i] = match current_net_class {
                Some(_) => board.rules.get_trace_half_width(net_number, i),
                None if board.rules.net_classes.count() == 0 => 0,
                None => board
                    .rules
                    .net_classes
                    .get(NetClassId(0))
                    .get_trace_half_width(i),
            };
            self.compensated_trace_half_width[i] = self.trace_half_width[i]
                + board
                    .rules
                    .clearance_matrix
                    .clearance_compensation_value(self.trace_clearance_class_index, i);
            if let Some(class) = current_net_class
                && !board
                    .rules
                    .net_classes
                    .get(class)
                    .is_active_routing_layer(i)
            {
                self.layer_active[i] = false;
            }
        }
        self.rebuild_via_info(board, via_costs, net_number);
    }

    pub fn attach_smd_allowed(&self) -> bool {
        self.legacy_smd_attachment_relaxation || self.via_infos.iter().any(|v| v.attach_smd_allowed)
    }

    pub fn rebuild_via_info(&mut self, board: &Board, via_costs: i32, net_number: i32) {
        let via_rule = self
            .via_rule
            .clone()
            .expect("AutorouteControl.rebuildViaInfo: viaRule is null — Java NPEs at :235");
        let via_rule = &via_rule;

        self.via_clearance_class = if via_rule.via_count() > 0 {
            via_rule.get_via(0).get_clearance_class_index()
        } else {
            1
        };
        self.via_infos = Vec::with_capacity(via_rule.via_count());
        for i in 0..via_rule.via_count() {
            let current_via = via_rule.get_via(i);
            let padstack = current_via.get_padstack();
            let from_layer = board.library.padstacks.padstack_from_layer(padstack);
            let to_layer = board.library.padstacks.padstack_to_layer(padstack);
            for j in from_layer..=to_layer {
                let current_radius = board
                    .library
                    .padstacks
                    .padstack_shape_max_width(padstack, j)
                    .map_or(0.0, |width| 0.5 * width);
                let slot = &mut self.via_radii[j as usize];
                *slot = (*slot).max(current_radius);
            }
            self.via_infos.push(ViaMask {
                from_layer,
                to_layer,
                attach_smd_allowed: current_via.attach_smd_allowed(),
            });
        }

        let relaxed_smd_net =
            self.smd_via_relaxation && AutorouteControl::is_pure_smd_net(board, net_number);
        self.legacy_smd_attachment_relaxation =
            !board.rules.strict_smd_via_attachment && self.layer_count > 1 && relaxed_smd_net;

        for j in 0..self.layer_count {
            self.via_radii[j] = (self.via_radii[j]).max(f64::from(self.trace_half_width[j]));
            self.max_via_radius = (self.max_via_radius).max(self.via_radii[j]);
        }
        let mut via_cost_factor = match self.via_pricing {
            ViaPricing::ByPadstackRadius => (self.max_via_radius).max(1.0),
            ViaPricing::PerMillimetre => self.units_per_mm * self.trace_cost_per_mm,
        };
        if relaxed_smd_net {
            via_cost_factor *= self.smd_via_cost_factor;
        }
        self.min_normal_via_cost = f64::from(via_costs) * via_cost_factor;
        self.min_cheap_via_cost = 0.8 * self.min_normal_via_cost;
    }
}

pub(crate) fn corridor_guidance_enabled() -> bool {
    static GUIDANCE: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        std::env::var_os("COPPERROUTE_CORRIDOR_GUIDANCE").is_none_or(|value| value != "0")
    });
    *GUIDANCE
}

#[cfg(test)]
mod corridor_default_tests {
    use super::corridor_guidance_enabled;

    #[test]
    fn corridor_guidance_defaults_on_and_can_be_disabled() {
        if let Ok(expected) = std::env::var("COPPERROUTE_TEST_CORRIDOR_EXPECTED") {
            assert_eq!(corridor_guidance_enabled(), expected == "1");
            return;
        }
        for (setting, expected) in [(None, "1"), (Some("0"), "0"), (Some("1"), "1")] {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            child.args(["--exact", "autoroute::maze::control::corridor_default_tests::corridor_guidance_defaults_on_and_can_be_disabled", "--nocapture"])
                .env("COPPERROUTE_TEST_CORRIDOR_EXPECTED", expected)
                .env_remove("COPPERROUTE_CORRIDOR_GUIDANCE");
            if let Some(value) = setting {
                child.env("COPPERROUTE_CORRIDOR_GUIDANCE", value);
            }
            assert!(child.status().unwrap().success(), "setting {setting:?}");
        }
    }
}
