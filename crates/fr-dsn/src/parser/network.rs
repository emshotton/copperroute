use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::Write;

use fr_board::{
    Board, BoardRules, FixedState, Item, ItemClass, ItemId, Keepout, NetClass, NetClassId,
    PadstackId, PartPin, ViaInfo, ViaInfoId, ViaRule,
};
use fr_geometry::{Area, Point, ShapeOps, Vector};

use crate::coordinate_transform::CoordinateTransform;
use crate::error::DsnError;
use crate::format::{IdentifierType, IndentFileWriter, java_double_to_string, java_round_to_int};
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::dsn_file::{
    CLASS_CLEARANCE_SEPARATOR, read_on_off_scope, read_string_list_scope, read_string_scope,
};
use crate::parser::geometry::DsnLayerStructure;
use crate::parser::library::strip_dot_digits;
use crate::parser::part_library::{DsnLogicalPart, DsnLogicalPartMapping, java_string_cmp};
use crate::parser::placement::ComponentLocation;
use crate::parser::scope_parameter::{ReadScopeParameter, WriteScopeParameter, skip_scope};
use crate::parser::structure::{contains_wire_clearance_pair, read_via_padstacks};

#[derive(Debug, Clone, PartialEq)]
pub enum DsnRule {
    Width(f64),
    Clearance(DsnClearanceRule),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DsnClearanceRule {
    pub value: f64,
    pub clearance_class_pairs: Vec<String>,
}

pub fn read_rule_scope(scanner: &mut DsnScanner) -> Result<Option<Vec<DsnRule>>, DsnError> {
    let mut result = Vec::new();
    let mut prev_was_open = false;
    loop {
        let Some(current_token) = scanner.next_token()? else {
            return Ok(None);
        };
        if current_token == Token::Close {
            break;
        }
        let is_open = current_token == Token::Open;
        if prev_was_open {
            let current_rule = match current_token {
                Token::Kw(Keyword::Width) => read_width_rule(scanner)?,
                Token::Kw(Keyword::Clearance) => read_clearance_rule(scanner)?,
                _ => {
                    let _ = skip_scope(scanner)?;
                    None
                }
            };
            if let Some(rule) = current_rule {
                result.push(rule);
            }
        }
        prev_was_open = is_open;
    }
    Ok(Some(result))
}

pub fn read_width_rule(scanner: &mut DsnScanner) -> Result<Option<DsnRule>, DsnError> {
    let value = scanner.next_double();
    if !scanner.next_closing_bracket()? {
        return Ok(None);
    }
    Ok(value.map(DsnRule::Width))
}

pub fn read_clearance_rule(scanner: &mut DsnScanner) -> Result<Option<DsnRule>, DsnError> {
    let Some(value) = scanner.next_double() else {
        return Ok(None);
    };
    let mut class_pairs: Vec<String> = Vec::new();
    let next_token = scanner.next_token()?;
    if next_token != Some(Token::Close) {
        if next_token != Some(Token::Open) {
            return Ok(None);
        }
        if scanner.next_token()? != Some(Token::Kw(Keyword::Type)) {
            return Ok(None);
        }
        class_pairs.extend(scanner.next_string_list_sep(CLASS_CLEARANCE_SEPARATOR));
        if !scanner.next_closing_bracket()? {
            return Ok(None);
        }
        if !scanner.next_closing_bracket()? {
            return Ok(None);
        }
    }
    Ok(Some(DsnRule::Clearance(DsnClearanceRule {
        value,
        clearance_class_pairs: class_pairs,
    })))
}

#[derive(Debug, Clone, PartialEq)]
pub struct DsnLayerRule {
    pub layer_names: Vec<String>,
    pub rules: Vec<DsnRule>,
}

pub fn read_layer_rule_scope(scanner: &mut DsnScanner) -> Result<Option<DsnLayerRule>, DsnError> {
    let mut layer_names: Vec<String> = Vec::new();
    let mut rule_list: Vec<DsnRule> = Vec::new();
    loop {
        scanner.yybegin(LexicalState::LayerName);
        match scanner.next_token()? {
            Some(Token::Open) => break,
            Some(Token::Str(name)) => layer_names.push(name),
            _ => return Ok(None),
        }
    }
    loop {
        match scanner.next_token()? {
            Some(Token::Close) => break,
            Some(Token::Kw(Keyword::Rule)) => {
                rule_list.extend(read_rule_scope(scanner)?.unwrap_or_default());
            }
            _ => return Ok(None),
        }
    }
    Ok(Some(DsnLayerRule {
        layer_names,
        rules: rule_list,
    }))
}

#[derive(Debug, Clone, PartialEq)]
pub struct DsnCircuit {
    pub max_length: f64,
    pub min_length: f64,
    pub use_via: Vec<String>,
    pub use_layer: Vec<String>,
}

pub fn read_circuit_scope(scanner: &mut DsnScanner) -> Result<Option<DsnCircuit>, DsnError> {
    let mut min_trace_length = 0.0;
    let mut max_trace_length = 0.0;
    let mut use_via: Vec<String> = Vec::new();
    let mut use_layer: Vec<String> = Vec::new();
    let mut prev_was_open = false;
    loop {
        let Some(next_token) = scanner.next_token()? else {
            return Ok(None);
        };
        if next_token == Token::Close {
            break;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            match next_token {
                Token::Kw(Keyword::Length) => {
                    if let Some(length_rule) = read_length_scope(scanner)? {
                        max_trace_length = length_rule[0];
                        min_trace_length = length_rule[1];
                    }
                }
                Token::Kw(Keyword::UseVia) => {
                    use_via.extend(read_via_padstacks(scanner)?.unwrap_or_default());
                }
                Token::Kw(Keyword::UseLayer) => {
                    use_layer.extend(read_string_list_scope(scanner)?.unwrap_or_default());
                }
                _ => {
                    let _ = skip_scope(scanner)?;
                }
            }
        }
        prev_was_open = is_open;
    }
    Ok(Some(DsnCircuit {
        max_length: max_trace_length,
        min_length: min_trace_length,
        use_via,
        use_layer,
    }))
}

fn read_length_scope(scanner: &mut DsnScanner) -> Result<Option<[f64; 2]>, DsnError> {
    let mut length_arr = [0.0_f64; 2];
    for slot in &mut length_arr {
        #[allow(clippy::cast_precision_loss)]
        match scanner.next_token()? {
            Some(Token::Float(value)) => *slot = value,
            Some(Token::Int(value)) => *slot = value as f64,
            _ => return Ok(None),
        }
    }
    let mut prev_was_open = false;
    loop {
        let Some(next_token) = scanner.next_token()? else {
            return Ok(None);
        };
        if next_token == Token::Close {
            break;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            let _ = skip_scope(scanner)?;
        }
        prev_was_open = is_open;
    }
    Ok(Some(length_arr))
}

#[derive(Debug, Clone, PartialEq)]
pub struct DsnNetClass {
    pub name: String,
    pub trace_clearance_class: Option<String>,
    pub net_list: Vec<String>,
    pub rules: Vec<DsnRule>,
    pub layer_rules: Vec<DsnLayerRule>,
    pub use_via: Vec<String>,
    pub use_layer: Vec<String>,
    pub via_rule: Option<String>,
    pub shove_fixed: bool,
    pub pull_tight: bool,
    pub min_trace_length: f64,
    pub max_trace_length: f64,
}

pub fn read_net_class_scope(scanner: &mut DsnScanner) -> Result<Option<DsnNetClass>, DsnError> {
    scanner.yybegin(LexicalState::Name);
    let class_name = scanner.next_string();

    let net_list = scanner.next_string_list();

    let mut rules: Vec<DsnRule> = Vec::new();
    let mut layer_rules: Vec<DsnLayerRule> = Vec::new();
    let mut use_via: Vec<String> = Vec::new();
    let mut use_layer: Vec<String> = Vec::new();
    let mut via_rule: Option<String> = None;
    let mut trace_clearance_class: Option<String> = None;
    let mut pull_tight = true;
    let mut shove_fixed = false;
    let mut min_trace_length = 0.0;
    let mut max_trace_length = 0.0;

    let mut prev_was_open = scanner.next_token()? == Some(Token::Open);
    loop {
        let Some(next_token) = scanner.next_token()? else {
            return Ok(None);
        };
        if next_token == Token::Close {
            break;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            match &next_token {
                Token::Kw(Keyword::Rule) => {
                    rules.extend(read_rule_scope(scanner)?.unwrap_or_default());
                }
                Token::Kw(Keyword::LayerRule) => {
                    if let Some(layer_rule) = read_layer_rule_scope(scanner)? {
                        layer_rules.push(layer_rule);
                    }
                }
                Token::Kw(Keyword::ViaRule) => via_rule = Some(read_string_scope(scanner)?),
                Token::Kw(Keyword::Circuit) => {
                    if let Some(current_rule) = read_circuit_scope(scanner)? {
                        max_trace_length = current_rule.max_length;
                        min_trace_length = current_rule.min_length;
                        use_via.extend(current_rule.use_via);
                        use_layer.extend(current_rule.use_layer);
                    }
                }
                Token::Kw(Keyword::ClearanceClass) => {
                    trace_clearance_class = Some(read_string_scope(scanner)?);
                }
                Token::Kw(Keyword::ShoveFixed) => shove_fixed = read_on_off_scope(scanner)?,
                Token::Kw(Keyword::PullTight) => pull_tight = read_on_off_scope(scanner)?,
                _ => {
                    let _ = skip_scope(scanner)?;
                }
            }
        }
        prev_was_open = is_open;
    }
    Ok(Some(DsnNetClass {
        name: class_name,
        trace_clearance_class,
        net_list,
        rules,
        layer_rules,
        use_via,
        use_layer,
        via_rule,
        shove_fixed,
        pull_tight,
        min_trace_length,
        max_trace_length,
    }))
}

#[derive(Debug, Clone, PartialEq)]
pub struct DsnClassClass {
    pub class_names: Vec<String>,
    pub rules: Vec<DsnRule>,
    pub layer_rules: Vec<DsnLayerRule>,
}

pub fn read_class_class_scope(scanner: &mut DsnScanner) -> Result<Option<DsnClassClass>, DsnError> {
    let mut classes: Vec<String> = Vec::new();
    let mut rules: Vec<DsnRule> = Vec::new();
    let mut layer_rules: Vec<DsnLayerRule> = Vec::new();
    let mut prev_was_open = false;
    loop {
        let Some(next_token) = scanner.next_token()? else {
            return Ok(None);
        };
        if next_token == Token::Close {
            break;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            match &next_token {
                Token::Kw(Keyword::Classes) => {
                    classes.extend(read_string_list_scope(scanner)?.unwrap_or_default());
                }
                Token::Kw(Keyword::Rule) => {
                    rules.extend(read_rule_scope(scanner)?.unwrap_or_default());
                }
                Token::Kw(Keyword::LayerRule) => {
                    if let Some(layer_rule) = read_layer_rule_scope(scanner)? {
                        layer_rules.push(layer_rule);
                    }
                }
                _ => {}
            }
        }
        prev_was_open = is_open;
    }
    Ok(Some(DsnClassClass {
        class_names: classes,
        rules,
        layer_rules,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PinRef {
    pub component_name: String,
    pub pin_name: String,
}

impl Ord for PinRef {
    fn cmp(&self, other: &PinRef) -> std::cmp::Ordering {
        java_string_cmp(&self.component_name, &other.component_name)
            .then_with(|| java_string_cmp(&self.pin_name, &other.pin_name))
    }
}

impl PartialOrd for PinRef {
    fn partial_cmp(&self, other: &PinRef) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PinRef {
    #[must_use]
    pub fn new(component_name: impl Into<String>, pin_name: impl Into<String>) -> PinRef {
        PinRef {
            component_name: component_name.into(),
            pin_name: pin_name.into(),
        }
    }
}

impl fmt::Display for PinRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Pin{{{}-{}}}", self.component_name, self.pin_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NetId {
    pub name: String,
    pub subnet_no: i32,
}

impl Ord for NetId {
    fn cmp(&self, other: &NetId) -> std::cmp::Ordering {
        java_string_cmp(&self.name, &other.name).then_with(|| self.subnet_no.cmp(&other.subnet_no))
    }
}

impl PartialOrd for NetId {
    fn partial_cmp(&self, other: &NetId) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl NetId {
    #[must_use]
    pub fn new(name: impl Into<String>, subnet_no: i32) -> NetId {
        NetId {
            name: name.into(),
            subnet_no,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsnNet {
    pub id: NetId,
    pins: Option<BTreeSet<PinRef>>,
}

impl DsnNet {
    #[must_use]
    pub fn new(id: NetId) -> DsnNet {
        DsnNet { id, pins: None }
    }

    #[must_use]
    pub fn get_pins(&self) -> Option<&BTreeSet<PinRef>> {
        self.pins.as_ref()
    }

    pub fn set_pins(&mut self, pin_list: impl IntoIterator<Item = PinRef>) {
        self.pins = Some(pin_list.into_iter().collect());
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetList {
    nets: BTreeMap<NetId, DsnNet>,
}

impl NetList {
    #[must_use]
    pub fn new() -> NetList {
        NetList::default()
    }

    #[must_use]
    pub fn contains(&self, net_id: &NetId) -> bool {
        self.nets.contains_key(net_id)
    }

    pub fn add_net(&mut self, net_id: NetId) -> Option<&mut DsnNet> {
        if self.nets.contains_key(&net_id) {
            return None;
        }
        Some(
            self.nets
                .entry(net_id.clone())
                .or_insert_with(|| DsnNet::new(net_id)),
        )
    }

    #[must_use]
    pub fn get_net(&self, net_id: &NetId) -> Option<&DsnNet> {
        self.nets.get(net_id)
    }

    pub fn get_net_mut(&mut self, net_id: &NetId) -> Option<&mut DsnNet> {
        self.nets.get_mut(net_id)
    }

    #[must_use]
    pub fn get_nets(&self, component_name: &str, pin_name: &str) -> Vec<&DsnNet> {
        let search_pin = PinRef::new(component_name, pin_name);
        self.nets
            .values()
            .filter(|net| {
                net.get_pins()
                    .is_some_and(|pins| pins.contains(&search_pin))
            })
            .collect()
    }

    pub fn values(&self) -> impl DoubleEndedIterator<Item = &DsnNet> + ExactSizeIterator {
        self.nets.values()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nets.is_empty()
    }
}

pub fn read_network_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let mut classes: Vec<DsnNetClass> = Vec::new();
    let mut class_class_list: Vec<DsnClassClass> = Vec::new();
    let mut via_infos: Vec<ViaInfo> = Vec::new();
    let mut via_rules: Vec<Vec<String>> = Vec::new();

    let mut prev_was_open = false;
    loop {
        let Some(next_token) = p.scanner.next_token()? else {
            return Ok(false);
        };
        if next_token == Token::Close {
            break;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            match next_token {
                Token::Kw(Keyword::Net) => {
                    read_net_scope(p)?;
                }
                Token::Kw(Keyword::Via) => {
                    let board = p.board.as_mut().expect(BOARD_EXPECTED);
                    match read_via_info(&mut p.scanner, board)? {
                        Some(via_info) => via_infos.push(via_info),
                        None => return Ok(false),
                    }
                }
                Token::Kw(Keyword::ViaRule) => match read_via_rule(&mut p.scanner)? {
                    Some(rule) => via_rules.push(rule),
                    None => return Ok(false),
                },
                Token::Kw(Keyword::Class) => match read_net_class_scope(&mut p.scanner)? {
                    Some(class) => classes.push(class),
                    None => return Ok(false),
                },
                Token::Kw(Keyword::ClassClass) => match read_class_class_scope(&mut p.scanner)? {
                    Some(class_class) => class_class_list.push(class_class),
                    None => return Ok(false),
                },
                _ => {
                    skip_scope(&mut p.scanner)?;
                }
            }
        }
        prev_was_open = is_open;
    }

    let mut aliased_class: Option<usize> = None;
    for (index, net_class) in classes.iter().enumerate() {
        match &mut p.via_padstack_names {
            Some(names) => names.extend(net_class.use_via.iter().cloned()),
            None => {
                p.via_padstack_names = Some(net_class.use_via.clone());
                aliased_class = Some(index);
            }
        }
    }
    if let Some(index) = aliased_class {
        classes[index].use_via = p.via_padstack_names.clone().unwrap_or_default();
    }

    if let Some(names) = p.via_padstack_names.clone() {
        let board = p.board.as_mut().expect(BOARD_EXPECTED);
        let mut via_padstacks: Vec<PadstackId> = Vec::with_capacity(names.len());
        for current_padstack_name in &names {
            let cleaned_name = strip_dot_digits(current_padstack_name);
            if let Some(padstack) = board.library.padstacks.get_by_name(&cleaned_name) {
                via_padstacks.push(PadstackId(padstack.no));
            }
        }
        board.library.set_via_padstacks(via_padstacks);
    }

    let via_at_smd_allowed = p.via_at_smd_allowed;
    {
        let board = p.board.as_mut().expect(BOARD_EXPECTED);
        insert_via_infos(via_infos, board, via_at_smd_allowed);
        insert_via_rules(&via_rules, board);
    }
    insert_net_classes(&classes, p);
    insert_class_pairs(&class_class_list, p);
    insert_components(p);
    insert_logical_parts(p);
    Ok(true)
}

const BOARD_EXPECTED: &str =
    "Network.readScope: the structure scope must have created the board (Java NPEs here too)";

fn java_split_underscore(text: &str) -> Vec<&str> {
    if !text.contains('_') {
        return vec![text];
    }
    let mut parts: Vec<&str> = text.split('_').collect();
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

pub const KICAD_DSN_DEFAULT: &str = "kicad_default";

#[must_use]
pub fn is_kicad_default_net_class_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    name.eq_ignore_ascii_case("default") || name.eq_ignore_ascii_case(KICAD_DSN_DEFAULT)
}

#[must_use]
pub fn resolve_net_class(rules: &mut BoardRules, name: &str) -> Option<NetClassId> {
    if is_kicad_default_net_class_name(name) {
        return Some(rules.get_default_net_class());
    }
    rules.net_classes.get_no(name)
}

fn read_net_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let net_name = p.scanner.next_string();

    let mut subnet_number = 1;
    let mut next_token = p.scanner.next_token()?;
    let scope_is_empty = next_token == Some(Token::Close);
    if let Some(Token::Int(value)) = next_token {
        subnet_number = value as i32;
    }
    let mut pin_order_found = false;
    let mut pin_list: Vec<PinRef> = Vec::new();
    let mut net_rules: Vec<DsnRule> = Vec::new();
    let mut subnet_pin_lists: Vec<Vec<PinRef>> = Vec::new();
    if !scope_is_empty {
        let mut prev_was_open = next_token == Some(Token::Open);
        loop {
            next_token = p.scanner.next_token()?;
            let Some(token) = next_token else {
                return Ok(false);
            };
            if token == Token::Close {
                break;
            }
            let is_open = token == Token::Open;
            if prev_was_open {
                match token {
                    Token::Kw(Keyword::Pins) => {
                        if !read_net_pins(&mut p.scanner, &mut pin_list)? {
                            return Ok(false);
                        }
                    }
                    Token::Kw(Keyword::Order) => {
                        pin_order_found = true;
                        if !read_net_pins(&mut p.scanner, &mut pin_list)? {
                            return Ok(false);
                        }
                    }
                    Token::Kw(Keyword::Fromto) => {
                        let mut current_subnet_pin_list: Vec<PinRef> = Vec::new();
                        if !read_net_pins(&mut p.scanner, &mut current_subnet_pin_list)? {
                            return Ok(false);
                        }
                        current_subnet_pin_list.sort();
                        current_subnet_pin_list.dedup();
                        subnet_pin_lists.push(current_subnet_pin_list);
                    }
                    Token::Kw(Keyword::Rule) => {
                        if let Some(rules) = read_rule_scope(&mut p.scanner)? {
                            net_rules.extend(rules);
                        }
                    }
                    _ => {
                        skip_scope(&mut p.scanner)?;
                    }
                }
            }
            prev_was_open = is_open;
        }
    }
    if subnet_pin_lists.is_empty() {
        if pin_order_found {
            subnet_pin_lists = create_ordered_subnets(&pin_list);
        } else {
            subnet_pin_lists.push(pin_list);
        }
    }
    let coordinate_transform = p.coordinate_transform.expect(TRANSFORM_EXPECTED);
    let contains_plane = p
        .layer_structure
        .as_ref()
        .expect(LAYER_STRUCTURE_EXPECTED)
        .contains_plane(&net_name);
    for current_pin_list in subnet_pin_lists {
        let net_id = NetId::new(net_name.clone(), subnet_number);
        if !p.netlist.contains(&net_id) {
            let board = p.board.as_mut().expect(BOARD_EXPECTED);
            let default_class = board.rules.get_default_net_class();
            if p.netlist.add_net(net_id.clone()).is_some() {
                let board = p.board.as_mut().expect(BOARD_EXPECTED);
                board.rules.nets.add(
                    net_id.name.clone(),
                    net_id.subnet_no,
                    contains_plane,
                    default_class,
                );
            }
        }
        let Some(current_subnet) = p.netlist.get_net_mut(&net_id) else {
            return Ok(false);
        };
        current_subnet.set_pins(current_pin_list);
        if !net_rules.is_empty() {
            let board = p.board.as_mut().expect(BOARD_EXPECTED);
            let Some(board_net_number) = board
                .rules
                .nets
                .get_by_name_and_subnet(&net_id.name, net_id.subnet_no)
                .map(|net| net.net_number)
            else {
                return Ok(false);
            };
            for current_object in &net_rules {
                if let DsnRule::Width(wire_width) = current_object {
                    let default_net_rule = board.rules.get_default_net_class();
                    let trace_half_width =
                        java_round_to_int(coordinate_transform.dsn_to_board(*wire_width) / 2.0);
                    let default_trace_clearance_class = board
                        .rules
                        .net_classes
                        .get(default_net_rule)
                        .get_trace_clearance_class();
                    let default_via_rule = board
                        .rules
                        .net_classes
                        .get(default_net_rule)
                        .get_via_rule()
                        .cloned();
                    let net_rule = board
                        .rules
                        .net_classes
                        .find(
                            trace_half_width,
                            default_trace_clearance_class,
                            default_via_rule.as_ref(),
                        )
                        .unwrap_or_else(|| board.rules.get_new_net_class());
                    board
                        .rules
                        .net_classes
                        .get_mut(net_rule)
                        .set_trace_half_width_on_all_layers(trace_half_width);
                    board
                        .rules
                        .nets
                        .get_mut(board_net_number)
                        .expect("looked up above")
                        .set_class(net_rule);
                }
            }
        }
        subnet_number += 1;
    }
    Ok(true)
}

const LAYER_STRUCTURE_EXPECTED: &str =
    "Network: the structure scope must have built the layer structure (Java NPEs here too)";

const TRANSFORM_EXPECTED: &str =
    "Network: the structure scope must have set the coordinate transform (Java NPEs here too)";

fn create_ordered_subnets(pin_list: &[PinRef]) -> Vec<Vec<PinRef>> {
    let mut result: Vec<Vec<PinRef>> = Vec::new();
    let mut it = pin_list.iter();
    let Some(mut prev_pin) = it.next() else {
        return result;
    };
    for next_pin in it {
        let mut current_subnet_pin_list = vec![prev_pin.clone(), next_pin.clone()];
        current_subnet_pin_list.sort();
        current_subnet_pin_list.dedup();
        result.push(current_subnet_pin_list);
        prev_pin = next_pin;
    }
    result
}

fn read_net_pins(scanner: &mut DsnScanner, pin_list: &mut Vec<PinRef>) -> Result<bool, DsnError> {
    loop {
        let component_name = scanner.next_string_with(true, '-');
        if component_name.is_empty() {
            break;
        }
        scanner.yybegin(LexicalState::SpecChar);
        scanner.next_token()?;
        let pin_name = scanner.next_string_ignoring_newline(true);
        pin_list.push(PinRef::new(component_name, pin_name));
    }

    let next_token = scanner.next_token()?;
    if next_token.is_none() {
        return Ok(false);
    }
    Ok(true)
}

pub(crate) fn read_via_info(
    scanner: &mut DsnScanner,
    board: &mut Board,
) -> Result<Option<ViaInfo>, DsnError> {
    scanner.yybegin(LexicalState::Name);
    let Some(Token::Str(name)) = scanner.next_token()? else {
        return Ok(None);
    };
    scanner.yybegin(LexicalState::Name);
    let Some(Token::Str(padstack_name)) = scanner.next_token()? else {
        return Ok(None);
    };
    scanner.set_scope_identifier(&padstack_name);
    let via_padstack = match board.library.get_via_padstack_by_name(&padstack_name) {
        Some(padstack) => padstack,
        None => {
            let Some(padstack) = board.library.padstacks.get_by_name(&padstack_name) else {
                return Ok(None);
            };
            let padstack = PadstackId(padstack.no);
            board.library.add_via_padstack(padstack);
            padstack
        }
    };
    scanner.yybegin(LexicalState::Name);
    let Some(Token::Str(clearance_class_name)) = scanner.next_token()? else {
        return Ok(None);
    };
    let clearance_class = board
        .rules
        .clearance_matrix
        .get_no(&clearance_class_name)
        .unwrap_or_else(BoardRules::default_clearance_class);
    let mut attach_allowed = false;
    let mut next_token = scanner.next_token()?;
    if next_token != Some(Token::Close) {
        if next_token != Some(Token::Kw(Keyword::Attach)) {
            return Ok(None);
        }
        attach_allowed = true;
        next_token = scanner.next_token()?;
        if next_token != Some(Token::Close) {
            return Ok(None);
        }
    }
    Ok(Some(ViaInfo::new(
        name,
        via_padstack,
        clearance_class,
        attach_allowed,
    )))
}

pub(crate) fn read_via_rule(scanner: &mut DsnScanner) -> Result<Option<Vec<String>>, DsnError> {
    let mut result: Vec<String> = Vec::new();
    loop {
        scanner.yybegin(LexicalState::Name);
        match scanner.next_token()? {
            Some(Token::Close) => break,
            Some(Token::Str(name)) => result.push(name),
            _ => return Ok(None),
        }
    }
    Ok(Some(result))
}

fn insert_via_infos(via_infos: Vec<ViaInfo>, board: &mut Board, attach_allowed: bool) {
    if via_infos.is_empty() {
        let default_net_class = board.rules.get_default_net_class();
        create_default_via_infos(board, default_net_class, attach_allowed);
        return;
    }
    for current_info in via_infos {
        board.rules.via_infos.add(current_info);
    }
}

fn create_default_via_infos(board: &mut Board, net_class: NetClassId, attach_allowed: bool) {
    let clearance_class_index = board
        .rules
        .net_classes
        .get(net_class)
        .default_item_clearance_classes
        .get(ItemClass::Via);
    let is_default_class = net_class == board.rules.get_default_net_class();
    let net_class_name = board
        .rules
        .net_classes
        .get(net_class)
        .get_name()
        .to_string();
    for i in 0..board.library.via_padstack_count() {
        let current_padstack = board
            .library
            .get_via_padstack(i)
            .expect("i < via_padstack_count()");
        let (padstack_name, padstack_attach_allowed) = board
            .library
            .get_padstack(current_padstack)
            .map(|p| (p.name.clone(), p.attach_allowed))
            .expect("a via padstack id resolves in library.padstacks");
        let via_attach_allowed = attach_allowed && padstack_attach_allowed;
        let via_name = if is_default_class {
            padstack_name
        } else {
            format!("{padstack_name}{CLASS_CLEARANCE_SEPARATOR}{net_class_name}")
        };
        board.rules.via_infos.add(ViaInfo::new(
            via_name,
            current_padstack,
            clearance_class_index,
            via_attach_allowed,
        ));
    }
}

fn insert_via_rules(via_rules: &[Vec<String>], board: &mut Board) {
    let mut rule_found = false;
    for current_list in via_rules {
        if current_list.len() < 2 {
            continue;
        }
        if add_via_rule(current_list, board) {
            rule_found = true;
        }
    }
    if !rule_found {
        let default_net_class = board.rules.get_default_net_class();
        board
            .rules
            .create_default_via_rule(default_net_class, "default", &board.library.padstacks);
    }
    let default_via_rule = board.rules.get_default_via_rule().cloned();
    for i in 0..board.rules.net_classes.count() {
        board
            .rules
            .net_classes
            .get_mut(NetClassId(i))
            .set_via_rule(default_via_rule.clone());
    }
}

pub fn add_via_rule(name_list: &[String], board: &mut Board) -> bool {
    let mut it = name_list.iter();
    let rule_name = it
        .next()
        .expect("Network.addViaRule: the name list is never empty");
    let existing_rule = board.rules.get_via_rule(rule_name);
    let mut current_rule = ViaRule::new(rule_name.clone());
    let mut rule_ok = true;
    for via_name in it {
        match board.rules.via_infos.get_by_name(via_name) {
            Some(current_via) => current_rule.append_via(current_via.clone()),
            None => rule_ok = false,
        }
    }
    if rule_ok {
        match existing_rule {
            Some(existing) => {
                board.rules.replace_via_rule(existing, current_rule);
            }
            None => board.rules.via_rules.push(current_rule),
        }
    }
    rule_ok
}

fn insert_net_classes(classes: &[DsnNetClass], p: &mut ReadScopeParameter<'_>) {
    let layer_structure = p.layer_structure.clone().expect(LAYER_STRUCTURE_EXPECTED);
    let coordinate_transform = p.coordinate_transform.expect(TRANSFORM_EXPECTED);
    let via_at_smd_allowed = p.via_at_smd_allowed;
    let board = p.board.as_mut().expect(BOARD_EXPECTED);
    for current_class in classes {
        insert_net_class(
            current_class,
            &layer_structure,
            board,
            &coordinate_transform,
            via_at_smd_allowed,
        );
    }
}

pub fn insert_net_class(
    net_class: &DsnNetClass,
    layer_structure: &DsnLayerStructure,
    board: &mut Board,
    coordinate_transform: &CoordinateTransform,
    via_at_smd_allowed: bool,
) {
    let board_net_class = if is_kicad_default_net_class_name(&net_class.name) {
        board.rules.get_default_net_class()
    } else {
        board.rules.append_net_class_named(&net_class.name)
    };
    if let Some(trace_clearance_class) = &net_class.trace_clearance_class {
        if let Some(no) = board.rules.clearance_matrix.get_no(trace_clearance_class) {
            board
                .rules
                .net_classes
                .get_mut(board_net_class)
                .set_trace_clearance_class(no);
        }
    }
    if let Some(via_rule_name) = &net_class.via_rule {
        if let Some(via_rule) = board.rules.get_via_rule(via_rule_name) {
            let via_rule = board.rules.via_rules[via_rule.0].clone();
            board
                .rules
                .net_classes
                .get_mut(board_net_class)
                .set_via_rule(Some(via_rule));
        }
    }
    if net_class.max_trace_length > 0.0 {
        let value = coordinate_transform.dsn_to_board(net_class.max_trace_length);
        board
            .rules
            .net_classes
            .get_mut(board_net_class)
            .set_maximum_trace_length(value);
    }
    if net_class.min_trace_length > 0.0 {
        let value = coordinate_transform.dsn_to_board(net_class.min_trace_length);
        board
            .rules
            .net_classes
            .get_mut(board_net_class)
            .set_minimum_trace_length(value);
    }
    for current_net_name in &net_class.net_list {
        let net_numbers: Vec<i32> = board
            .rules
            .nets
            .get_by_name(current_net_name)
            .iter()
            .map(|net| net.net_number)
            .collect();
        for net_number in net_numbers {
            board
                .rules
                .nets
                .get_mut(net_number)
                .expect("listed by get_by_name")
                .set_class(board_net_class);
        }
    }

    let mut clearance_rule_found = false;

    for current_rule in &net_class.rules {
        match current_rule {
            DsnRule::Width(value) => {
                let trace_half_width =
                    java_round_to_int(coordinate_transform.dsn_to_board(value / 2.0));
                board
                    .rules
                    .net_classes
                    .get_mut(board_net_class)
                    .set_trace_half_width_on_all_layers(trace_half_width);
            }
            DsnRule::Clearance(rule) => {
                add_clearance_rule(board, board_net_class, rule, None, coordinate_transform);
                clearance_rule_found = true;
            }
        }
    }

    for current_layer_rule in &net_class.layer_rules {
        for current_layer_name in &current_layer_rule.layer_names {
            let Some(layer_index) = board.layer_structure().get_no(current_layer_name) else {
                continue;
            };
            for current_rule in &current_layer_rule.rules {
                match current_rule {
                    DsnRule::Width(value) => {
                        let trace_half_width =
                            java_round_to_int(coordinate_transform.dsn_to_board(value / 2.0));
                        board
                            .rules
                            .net_classes
                            .get_mut(board_net_class)
                            .set_trace_half_width(layer_index, trace_half_width);
                    }
                    DsnRule::Clearance(rule) => {
                        add_clearance_rule(
                            board,
                            board_net_class,
                            rule,
                            Some(layer_index),
                            coordinate_transform,
                        );
                        clearance_rule_found = true;
                    }
                }
            }
        }
    }

    board
        .rules
        .net_classes
        .get_mut(board_net_class)
        .set_pull_tight(net_class.pull_tight);
    board
        .rules
        .net_classes
        .get_mut(board_net_class)
        .set_shove_fixed(net_class.shove_fixed);
    let mut via_infos_created = false;

    if clearance_rule_found && board_net_class != board.rules.get_default_net_class() {
        create_default_via_infos(board, board_net_class, via_at_smd_allowed);
        via_infos_created = true;
    }

    if net_class.use_via.is_empty() {
        if via_infos_created {
            let name = board
                .rules
                .net_classes
                .get(board_net_class)
                .get_name()
                .to_string();
            board
                .rules
                .create_default_via_rule(board_net_class, name, &board.library.padstacks);
        }
    } else {
        create_via_rule(&net_class.use_via, board_net_class, board);
    }
    if !net_class.use_layer.is_empty() {
        create_active_trace_layers(
            &net_class.use_layer,
            layer_structure,
            board,
            board_net_class,
        );
    }
}

fn insert_class_pairs(class_classes: &[DsnClassClass], p: &mut ReadScopeParameter<'_>) {
    let coordinate_transform = p.coordinate_transform.expect(TRANSFORM_EXPECTED);
    let board = p.board.as_mut().expect(BOARD_EXPECTED);
    for current_class_class in class_classes {
        let mut it1 = current_class_class.class_names.iter();
        while let Some(first_name) = it1.next() {
            let Some(first_class) = resolve_net_class(&mut board.rules, first_name) else {
                continue;
            };
            for second_name in it1.by_ref() {
                let Some(second_class) = resolve_net_class(&mut board.rules, second_name) else {
                    continue;
                };
                insert_class_pair_info(
                    current_class_class,
                    first_class,
                    second_class,
                    board,
                    &coordinate_transform,
                );
            }
        }
    }
}

fn insert_class_pair_info(
    class_class: &DsnClassClass,
    first_class: NetClassId,
    second_class: NetClassId,
    board: &mut Board,
    coordinate_transform: &CoordinateTransform,
) {
    for current_rule in &class_class.rules {
        if let DsnRule::Clearance(current_clearance_rule) = current_rule {
            add_mixed_clearance_rule(
                board,
                first_class,
                second_class,
                current_clearance_rule,
                None,
                coordinate_transform,
            );
        }
    }
    for current_layer_rule in &class_class.layer_rules {
        for current_layer_name in &current_layer_rule.layer_names {
            let Some(layer_index) = board.layer_structure().get_no(current_layer_name) else {
                continue;
            };
            for current_rule in &current_layer_rule.rules {
                if let DsnRule::Clearance(rule) = current_rule {
                    add_mixed_clearance_rule(
                        board,
                        first_class,
                        second_class,
                        rule,
                        Some(layer_index),
                        coordinate_transform,
                    );
                }
            }
        }
    }
}

fn add_mixed_clearance_rule(
    board: &mut Board,
    first_class: NetClassId,
    second_class: NetClassId,
    clearance_rule: &DsnClearanceRule,
    layer_index: Option<usize>,
    coordinate_transform: &CoordinateTransform,
) {
    let current_clearance =
        java_round_to_int(coordinate_transform.dsn_to_board(clearance_rule.value));
    let first_class_name = board
        .rules
        .net_classes
        .get(first_class)
        .get_name()
        .to_string();
    let first_class_no = match board.rules.clearance_matrix.get_no(&first_class_name) {
        Some(no) => no,
        None => {
            board.rules.clearance_matrix.append_class(&first_class_name);
            board
                .rules
                .clearance_matrix
                .get_no(&first_class_name)
                .expect("appendClass leaves the class present")
        }
    };
    let second_class_name = board
        .rules
        .net_classes
        .get(second_class)
        .get_name()
        .to_string();
    let second_class_no = match board.rules.clearance_matrix.get_no(&second_class_name) {
        Some(no) => no,
        None => {
            board
                .rules
                .clearance_matrix
                .append_class(&second_class_name);
            board
                .rules
                .clearance_matrix
                .get_no(&second_class_name)
                .expect("appendClass leaves the class present")
        }
    };
    if clearance_rule.clearance_class_pairs.is_empty() {
        set_clearance_both_ways(
            board,
            first_class_no,
            second_class_no,
            layer_index,
            current_clearance,
        );
        return;
    }
    for current_string in &clearance_rule.clearance_class_pairs {
        let current_pair = java_split_underscore(current_string);
        if current_pair.len() != 2 {
            continue;
        }
        for i in 0..2 {
            let (current_first_class_no, current_second_class_no) = if i == 0 {
                (
                    get_clearance_class(board, first_class, current_pair[0]),
                    get_clearance_class(board, second_class, current_pair[1]),
                )
            } else {
                (
                    get_clearance_class(board, second_class, current_pair[0]),
                    get_clearance_class(board, first_class, current_pair[1]),
                )
            };
            set_clearance_both_ways(
                board,
                current_first_class_no,
                current_second_class_no,
                layer_index,
                current_clearance,
            );
        }
    }
}

fn set_clearance_both_ways(
    board: &mut Board,
    first_class_no: usize,
    second_class_no: usize,
    layer_index: Option<usize>,
    value: i32,
) {
    match layer_index {
        None => {
            board.rules.clearance_matrix.set_value_on_all_layers(
                first_class_no,
                second_class_no,
                value,
            );
            board.rules.clearance_matrix.set_value_on_all_layers(
                second_class_no,
                first_class_no,
                value,
            );
        }
        Some(layer) => {
            board
                .rules
                .clearance_matrix
                .set_value(first_class_no, second_class_no, layer, value);
            board
                .rules
                .clearance_matrix
                .set_value(second_class_no, first_class_no, layer, value);
        }
    }
}

fn create_default_clearance_classes(board: &mut Board, net_class: NetClassId) {
    get_clearance_class(board, net_class, "via");
    get_clearance_class(board, net_class, "smd");
    get_clearance_class(board, net_class, "pin");
    get_clearance_class(board, net_class, "area");
}

fn create_via_rule(use_via: &[String], net_class: NetClassId, board: &mut Board) {
    let net_class_name = board
        .rules
        .net_classes
        .get(net_class)
        .get_name()
        .to_string();
    let mut new_via_rule = ViaRule::new(net_class_name);
    let default_via_cl_class = board
        .rules
        .net_classes
        .get(net_class)
        .default_item_clearance_classes
        .get(ItemClass::Via);
    for current_via_name in use_via {
        for i in 0..board.rules.via_infos.count() {
            let info = board.rules.via_infos.get(ViaInfoId(i));
            if info.get_clearance_class_index() != default_via_cl_class {
                continue;
            }
            let padstack_name = board
                .library
                .get_padstack(info.get_padstack())
                .map(|p| p.name.as_str());
            if padstack_name == Some(current_via_name.as_str()) {
                new_via_rule.append_via(info.clone());
            }
        }
    }
    board
        .rules
        .net_classes
        .get_mut(net_class)
        .set_via_rule(Some(new_via_rule.clone()));
    board.rules.via_rules.push(new_via_rule);
}

fn create_active_trace_layers(
    use_layer: &[String],
    layer_structure: &DsnLayerStructure,
    board: &mut Board,
    net_class: NetClassId,
) {
    let layer_count = layer_structure.layers.len();
    let board_net_class = board.rules.net_classes.get_mut(net_class);
    for i in 0..layer_count {
        board_net_class.set_active_routing_layer(i, false);
    }
    for cur_layer_name in use_layer {
        let Some(current_no) = layer_structure.get_no(cur_layer_name) else {
            continue;
        };
        board_net_class.set_active_routing_layer(current_no, true);
    }
    for i in 0..layer_count {
        if !board_net_class.is_active_routing_layer(i) {
            board_net_class.set_trace_half_width(i, 0);
        }
    }
}

fn add_clearance_rule(
    board: &mut Board,
    net_class: NetClassId,
    rule: &DsnClearanceRule,
    layer_index: Option<usize>,
    coordinate_transform: &CoordinateTransform,
) {
    let current_clearance = java_round_to_int(coordinate_transform.dsn_to_board(rule.value));
    let class_name = board
        .rules
        .net_classes
        .get(net_class)
        .get_name()
        .to_string();
    let class_no = match board.rules.clearance_matrix.get_no(&class_name) {
        Some(no) => no,
        None => {
            board.rules.clearance_matrix.append_class(&class_name);
            let class_no = board
                .rules
                .clearance_matrix
                .get_no(&class_name)
                .expect("appendClass leaves the class present");
            for i in 1..board.rules.clearance_matrix.get_class_count() {
                for j in 0..board.rules.clearance_matrix.get_layer_count() {
                    let current_value = board
                        .rules
                        .clearance_matrix
                        .get_value(class_no, i, j, false)
                        .max(current_clearance);
                    board
                        .rules
                        .clearance_matrix
                        .set_value(class_no, i, j, current_value);
                    board
                        .rules
                        .clearance_matrix
                        .set_value(i, class_no, j, current_value);
                }
            }
            board
                .rules
                .net_classes
                .get_mut(net_class)
                .default_item_clearance_classes
                .set_all(class_no);
            class_no
        }
    };
    board
        .rules
        .net_classes
        .get_mut(net_class)
        .set_trace_clearance_class(class_no);
    if rule.clearance_class_pairs.is_empty() {
        match layer_index {
            None => board.rules.clearance_matrix.set_value_on_all_layers(
                class_no,
                class_no,
                current_clearance,
            ),
            Some(layer) => {
                board.rules.clearance_matrix.set_value(
                    class_no,
                    class_no,
                    layer,
                    current_clearance,
                );
            }
        }
        return;
    }
    if contains_wire_clearance_pair(&rule.clearance_class_pairs) {
        create_default_clearance_classes(board, net_class);
    }
    for current_string in &rule.clearance_class_pairs {
        let current_pair = java_split_underscore(current_string);
        if current_pair.len() != 2 {
            continue;
        }
        let first_class_no = get_clearance_class(board, net_class, current_pair[0]);
        let second_class_no = get_clearance_class(board, net_class, current_pair[1]);
        set_clearance_both_ways(
            board,
            first_class_no,
            second_class_no,
            layer_index,
            current_clearance,
        );
    }
}

fn get_clearance_class(board: &mut Board, net_class: NetClassId, item_class_name: &str) -> usize {
    let net_class_name = board
        .rules
        .net_classes
        .get(net_class)
        .get_name()
        .to_string();
    let new_class_name = if item_class_name == "wire" {
        net_class_name.clone()
    } else {
        format!("{net_class_name}{CLASS_CLEARANCE_SEPARATOR}{item_class_name}")
    };
    if let Some(found_class_no) = board.rules.clearance_matrix.get_no(&new_class_name) {
        return found_class_no;
    }
    board.rules.clearance_matrix.append_class(&new_class_name);
    let result = board
        .rules
        .clearance_matrix
        .get_no(&new_class_name)
        .expect("appendClass leaves the class present");
    let Some(net_class_no) = board.rules.clearance_matrix.get_no(&net_class_name) else {
        return result;
    };
    for i in 1..board.rules.clearance_matrix.get_class_count() {
        for j in 0..board.rules.clearance_matrix.get_layer_count() {
            let current_value = board
                .rules
                .clearance_matrix
                .get_value(net_class_no, i, j, false);
            board
                .rules
                .clearance_matrix
                .set_value(result, i, j, current_value);
            board
                .rules
                .clearance_matrix
                .set_value(i, result, j, current_value);
        }
    }
    let default_item_clearance_classes = &mut board
        .rules
        .net_classes
        .get_mut(net_class)
        .default_item_clearance_classes;
    match item_class_name {
        "via" => default_item_clearance_classes.set(ItemClass::Via, result),
        "pin" => default_item_clearance_classes.set(ItemClass::Pin, result),
        "smd" => default_item_clearance_classes.set(ItemClass::Smd, result),
        "area" => default_item_clearance_classes.set(ItemClass::Area, result),
        _ => {}
    }
    result
}

fn insert_components(p: &mut ReadScopeParameter<'_>) {
    let placement_list = std::mem::take(&mut p.placement_list);
    for next_lib_component in &placement_list {
        for next_component in &next_lib_component.locations {
            if let Some(diagnostic) =
                insert_component(next_component, &next_lib_component.lib_name, p)
            {
                p.warnings.push(diagnostic);
            }
        }
    }
    p.placement_list = placement_list;
}

fn insert_component(
    location: &ComponentLocation,
    lib_key: &str,
    p: &mut ReadScopeParameter<'_>,
) -> Option<String> {
    let coordinate_transform = p.coordinate_transform.expect(TRANSFORM_EXPECTED);
    let netlist = &p.netlist;
    let board = p.board.as_mut().expect(BOARD_EXPECTED);

    let current_front_package = board
        .library
        .packages
        .get_by_name(lib_key, true)
        .map(|pkg| pkg.no);
    let current_back_package = board
        .library
        .packages
        .get_by_name(lib_key, false)
        .map(|pkg| pkg.no);
    let (Some(current_front_package), Some(current_back_package)) =
        (current_front_package, current_back_package)
    else {
        return None;
    };

    let package_no = if location.is_front {
        current_front_package
    } else {
        current_back_package
    };
    let package = board.library.packages.get(package_no);
    for i in 0..package.pin_count() {
        let pin = package.get_pin(i as i32).expect("i < pinCount");
        if board.library.padstacks.get(pin.padstack_no).is_none() {
            return Some(format!(
                "component {}: pin {} of package {lib_key} names padstack {:?}, which the \
                 library does not have — the whole component is rejected",
                location.name, pin.name, pin.padstack_no
            ));
        }
    }

    let component_location = location
        .coor
        .map(|coor| coordinate_transform.dsn_to_board_point(&coor).round());
    let rotation_in_degree = location.rotation;

    let new_component_id = board
        .components
        .add(
            location.name.clone(),
            component_location.map(Point::Int),
            rotation_in_degree,
            location.is_front,
            current_front_package,
            current_back_package,
            location.position_fixed,
            location.part_number.clone(),
        )
        .id;

    let Some(component_location) = component_location else {
        return None;
    };
    let component_translation = Point::Int(component_location).difference_by(&Point::ZERO);
    let fixed_state = if location.position_fixed {
        FixedState::SystemFixed
    } else {
        FixedState::Unfixed
    };
    let current_package = board.components.get(new_component_id).get_package();
    let pin_count = board.library.packages.get(current_package).pin_count();
    for i in 0..pin_count {
        let current_pin = board
            .library
            .packages
            .get(current_package)
            .get_pin(i as i32)
            .expect("i < pinCount")
            .clone();
        let current_padstack = board
            .library
            .padstacks
            .get(current_pin.padstack_no)
            .expect("every pin's padstack was checked before the component was added");
        let padstack_is_smd = current_padstack.from_layer() == current_padstack.to_layer();
        let pin_nets: Vec<(String, i32)> = netlist
            .get_nets(&location.name, &current_pin.name)
            .iter()
            .map(|net| (net.id.name.clone(), net.id.subnet_no))
            .collect();
        let mut net_number_array: Vec<i32> = Vec::with_capacity(pin_nets.len());
        for (net_name, subnet_no) in &pin_nets {
            if let Some(current_board_net) = board
                .rules
                .nets
                .get_by_name_and_subnet(net_name, *subnet_no)
            {
                net_number_array.push(current_board_net.net_number);
            }
        }
        let board_net_class = net_number_array
            .first()
            .and_then(|no| board.rules.nets.get(*no))
            .map(fr_board::Net::get_net_class);
        let net_class = board_net_class.unwrap_or_else(|| board.rules.get_default_net_class());
        let mut clearance_class = location
            .pin_infos
            .get(&current_pin.name)
            .and_then(|pin_info| {
                board
                    .rules
                    .clearance_matrix
                    .get_no(&pin_info.clearance_class)
            });
        if clearance_class.is_none() {
            let default_item_clearance_classes = &board
                .rules
                .net_classes
                .get(net_class)
                .default_item_clearance_classes;
            clearance_class = Some(if padstack_is_smd {
                default_item_clearance_classes.get(ItemClass::Smd)
            } else {
                default_item_clearance_classes.get(ItemClass::Pin)
            });
        }
        board.insert_pin(
            new_component_id,
            i as i32,
            net_number_array,
            clearance_class.expect("assigned above"),
            fixed_state,
        );
    }

    for k in 0..=2 {
        let package = board.library.packages.get(current_package);
        let (keepouts, current_keepout_infos) = match k {
            0 => (package.keepouts.clone(), &location.keepout_infos),
            1 => (package.via_keepouts.clone(), &location.via_keepout_infos),
            _ => (
                package.place_keepouts.clone(),
                &location.place_keepout_infos,
            ),
        };
        for current_keepout in &keepouts {
            let mut layer = current_keepout.layer;
            if layer >= board.get_layer_count() as i32 {
                continue;
            }
            if layer >= 0 && !location.is_front {
                layer = board.get_layer_count() as i32 - current_keepout.layer - 1;
            }
            let default_net_class = board.rules.get_default_net_class();
            let mut clearance_class = board
                .rules
                .net_classes
                .get(default_net_class)
                .default_item_clearance_classes
                .get(ItemClass::Area);
            if let Some(keepout_info) = current_keepout_infos.get(&current_keepout.name) {
                if let Some(current_clearance_class) = board
                    .rules
                    .clearance_matrix
                    .get_no(&keepout_info.clearance_class)
                    && current_clearance_class > 0
                {
                    clearance_class = current_clearance_class;
                }
            }
            if let Ok(layer) = usize::try_from(layer) {
                insert_package_keepout(
                    board,
                    k,
                    current_keepout,
                    layer,
                    &component_translation,
                    rotation_in_degree,
                    !location.is_front,
                    clearance_class,
                    new_component_id,
                    fixed_state,
                );
            } else {
                for j in 0..board.layer_structure().count() {
                    if board.layer_structure().layers[j].is_signal {
                        insert_package_keepout(
                            board,
                            k,
                            current_keepout,
                            j,
                            &component_translation,
                            rotation_in_degree,
                            !location.is_front,
                            clearance_class,
                            new_component_id,
                            fixed_state,
                        );
                    }
                }
            }
        }
    }

    let package = board.library.packages.get(current_package);
    let outline = package.outline.clone();
    let outline_widths = package.outline_widths.clone();
    let outline_is_closed = package.outline_is_closed.clone();
    let mut courtyard_idx: i32 = -1;
    if let Some(outline) = outline.as_ref()
        && outline.len() > 1
    {
        let mut max_area = -1.0;
        for (i, shape) in outline.iter().enumerate() {
            let area = shape.bounding_box().area();
            if area > max_area {
                max_area = area;
                courtyard_idx = i as i32;
            }
        }
    }
    if let Some(outline) = outline {
        for (i, shape) in outline.iter().enumerate() {
            let mut is_courtyard = i as i32 == courtyard_idx;
            if let Some(widths) = outline_widths.as_ref()
                && i < widths.len()
                && widths[i] == 0.0
            {
                is_courtyard = true;
            }
            let mut is_fabrication = false;
            if !is_courtyard
                && let Some(widths) = outline_widths.as_ref()
                && i < widths.len()
                && widths[i] <= 110.0
            {
                is_fabrication = true;
            }
            let mut is_closed = false;
            if let Some(closed) = outline_is_closed.as_ref()
                && i < closed.len()
            {
                is_closed = closed[i];
            }
            board.insert_component_outline(
                Area::Shape(shape.clone()),
                location.is_front,
                component_translation.clone(),
                rotation_in_degree,
                new_component_id,
                is_courtyard,
                is_fabrication,
                is_closed,
                fixed_state,
            );
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn insert_package_keepout(
    board: &mut Board,
    k: i32,
    keepout: &Keepout,
    layer: usize,
    translation: &Vector,
    rotation_in_degree: f64,
    side_changed: bool,
    clearance_class: usize,
    component_id: i32,
    fixed_state: FixedState,
) {
    let area = keepout.area.clone();
    let name = Some(keepout.name.clone());
    match k {
        0 => {
            board.insert_obstacle_of_component(
                area,
                layer,
                translation.clone(),
                rotation_in_degree,
                side_changed,
                clearance_class,
                component_id,
                name,
                fixed_state,
            );
        }
        1 => {
            board.insert_via_obstacle_of_component(
                area,
                layer,
                translation.clone(),
                rotation_in_degree,
                side_changed,
                clearance_class,
                component_id,
                name,
                fixed_state,
            );
        }
        _ => {
            board.insert_component_obstacle_of_component(
                area,
                layer,
                translation.clone(),
                rotation_in_degree,
                side_changed,
                clearance_class,
                component_id,
                name,
                fixed_state,
            );
        }
    }
}

fn insert_logical_parts(p: &mut ReadScopeParameter<'_>) -> bool {
    let logical_parts = std::mem::take(&mut p.logical_parts);
    let logical_part_mappings = std::mem::take(&mut p.logical_part_mappings);
    let result = insert_logical_parts_inner(&logical_parts, &logical_part_mappings, p);
    p.logical_parts = logical_parts;
    p.logical_part_mappings = logical_part_mappings;
    result
}

fn insert_logical_parts_inner(
    logical_parts: &[DsnLogicalPart],
    logical_part_mappings: &[DsnLogicalPartMapping],
    p: &mut ReadScopeParameter<'_>,
) -> bool {
    let board = p.board.as_mut().expect(BOARD_EXPECTED);
    for next_part in logical_parts {
        let Some(lib_package) = search_lib_package(&next_part.name, logical_part_mappings, board)
        else {
            return false;
        };
        let mut board_part_pins: Vec<PartPin> = Vec::with_capacity(next_part.part_pins.len());
        for current_part_pin in &next_part.part_pins {
            let Some(pin_index) = board
                .library
                .packages
                .get(lib_package)
                .get_pin_index(&current_part_pin.pin_name)
            else {
                return false;
            };
            board_part_pins.push(PartPin::new(
                pin_index as i32,
                current_part_pin.pin_name.clone(),
                current_part_pin.gate_name.clone(),
                current_part_pin.gate_swap_code,
                current_part_pin.gate_pin_name.clone(),
                current_part_pin.gate_pin_swap_code,
            ));
        }
        board
            .library
            .logical_parts
            .add(next_part.name.clone(), board_part_pins);
    }

    for next_mapping in logical_part_mappings {
        let current_logical_part = board
            .library
            .logical_parts
            .get_by_name(&next_mapping.name)
            .map(|part| part.no);
        for current_cmp_name in &next_mapping.components {
            if let Some(component_id) = board
                .components
                .get_by_name(current_cmp_name)
                .map(|component| component.id)
            {
                board
                    .components
                    .get_mut(component_id)
                    .set_logical_part(current_logical_part);
            }
        }
    }
    true
}

fn search_lib_package(
    part_name: &str,
    logical_part_mappings: &[DsnLogicalPartMapping],
    board: &Board,
) -> Option<usize> {
    for current_mapping in logical_part_mappings {
        if current_mapping.name == part_name {
            let component_name = current_mapping.components.first()?;
            let current_component = board.components.get_by_name(component_name)?;
            return Some(current_component.get_package());
        }
    }
    None
}

pub fn write_rule_scope(net_class: &NetClass, p: &mut WriteScopeParameter<'_>) {
    p.file.start_scope_nl();
    p.file.write("rule");

    let default_trace_half_width = net_class.get_trace_half_width(0);
    let trace_width = 2.0
        * p.coordinate_transform
            .board_to_dsn(f64::from(default_trace_half_width));
    p.file.new_line();
    p.file.write("(width ");
    p.file.write(&java_double_to_string(trace_width));
    p.file.write(")");
    p.file.end_scope();
    for i in 1..p.board.layer_structure().count() {
        if net_class.get_trace_half_width(i) != default_trace_half_width {
            write_layer_rule(net_class, i, p);
        }
    }
}

fn write_layer_rule(net_class: &NetClass, layer_index: usize, p: &mut WriteScopeParameter<'_>) {
    let board = p.board;
    p.file.start_scope_nl();
    p.file.write("layer_rule ");

    let current_board_layer_name = &board.layer_structure().layers[layer_index].name;

    p.file.write(current_board_layer_name);
    p.file.start_scope_nl();
    p.file.write("rule ");

    let current_trace_half_width = net_class.get_trace_half_width(layer_index);

    let trace_width = 2.0
        * p.coordinate_transform
            .board_to_dsn(f64::from(current_trace_half_width));
    p.file.new_line();
    p.file.write("(width ");
    p.file.write(&java_double_to_string(trace_width));
    p.file.write(") ");
    p.file.end_scope();
    p.file.end_scope();
}

pub fn write_default_rule(p: &mut WriteScopeParameter<'_>, layer: usize) {
    p.file.start_scope_nl();
    p.file.write("rule");
    let default_half_width =
        crate::parser::structure::default_net_class_trace_half_width(&p.board.rules, 0);
    let trace_width = 2.0
        * p.coordinate_transform
            .board_to_dsn(f64::from(default_half_width));
    p.file.new_line();
    p.file.write("(width ");
    p.file.write(&java_double_to_string(trace_width));
    p.file.write(")");
    let default_cl_no = BoardRules::default_clearance_class();
    let default_board_clearance =
        p.board
            .rules
            .clearance_matrix
            .get_value(default_cl_no, default_cl_no, layer, false);
    let default_clearance = p
        .coordinate_transform
        .board_to_dsn(f64::from(default_board_clearance));
    p.file.new_line();
    p.file.write("(clearance ");
    p.file.write(&java_double_to_string(default_clearance));
    p.file.write(")");
    let smd_to_turn_dist = p
        .coordinate_transform
        .board_to_dsn(p.board.rules.get_pin_edge_to_turn_dist());
    p.file.new_line();
    p.file.write("(clearance ");
    p.file.write(&java_double_to_string(smd_to_turn_dist));
    p.file.write(" (type smd_to_turn_gap))");

    write_named_clearance_rules(p, layer);

    p.file.end_scope();
}

const CLASS_CLEARANCE_SEPARATOR_STR: &str = "-";

#[allow(dead_code, reason = "dead in Java too — see the doc comment")]
fn write_non_default_clearance_rules(
    p: &mut WriteScopeParameter<'_>,
    layer: usize,
    default_clearance: i32,
) {
    let cl_count = p.board.rules.clearance_matrix.get_class_count();

    for i in 1..=cl_count {
        for j in i..cl_count {
            let current_board_clearance =
                p.board.rules.clearance_matrix.get_value(i, j, layer, false);

            if current_board_clearance == default_clearance {
                continue;
            }

            let current_clearance = p
                .coordinate_transform
                .board_to_dsn(f64::from(current_board_clearance));
            let name_i = clearance_class_name(&p.board.rules, i).to_string();
            let name_j = clearance_class_name(&p.board.rules, j).to_string();
            p.file.new_line();
            p.file.write("(clearance ");
            p.file.write(&java_double_to_string(current_clearance));
            p.file.write(" (type ");
            p.identifier_type.write(&name_i, &mut p.file);
            p.file.write(CLASS_CLEARANCE_SEPARATOR_STR);
            p.identifier_type.write(&name_j, &mut p.file);
            p.file.write("))");
        }
    }
}

fn write_named_clearance_rules(p: &mut WriteScopeParameter<'_>, layer: usize) {
    let cl_count = p.board.rules.clearance_matrix.get_class_count();

    for i in 1..cl_count {
        if clearance_class_name(&p.board.rules, i) == "default" {
            continue;
        }

        let current_board_clearance = p.board.rules.clearance_matrix.get_value(i, i, layer, false);
        let current_clearance = p
            .coordinate_transform
            .board_to_dsn(f64::from(current_board_clearance));
        let name_i = clearance_class_name(&p.board.rules, i).to_string();

        p.file.new_line();
        p.file.write("(clearance ");
        p.file.write(&java_double_to_string(current_clearance));
        p.file.write(" (type ");
        p.identifier_type.write(&name_i, &mut p.file);
        p.file.write("))");
    }
}

pub fn write_item_clearance_class<W: Write>(
    name: &str,
    file: &mut IndentFileWriter<W>,
    identifier_type: &IdentifierType,
) {
    file.new_line();
    file.write("(clearance_class ");
    identifier_type.write(name, file);
    file.write(")");
}

pub(crate) fn clearance_class_name(rules: &BoardRules, index: usize) -> &str {
    rules.clearance_matrix.get_name(index).unwrap_or("")
}

pub fn write_net_scope(p: &mut WriteScopeParameter<'_>, net_number: i32, pin_list: &[ItemId]) {
    let board = p.board;
    let Some(net) = board.rules.nets.get(net_number) else {
        return;
    };
    let net_name = net.name.clone();
    let subnet_number = net.subnet_number;
    p.file.start_scope_nl();
    write_net_id_parts(&net_name, subnet_number, &mut p.file, &p.identifier_type);
    p.file.start_scope_nl();
    p.file.write("pins");
    for pin_id in pin_list {
        let Some(pin) = board.items.get(pin_id) else {
            continue;
        };
        if pin.contains_net(net_number) {
            write_pin(p, *pin_id);
        }
    }
    p.file.end_scope();
    p.file.end_scope();
}

pub fn write_net_id<W: Write>(
    net: &fr_board::Net,
    file: &mut IndentFileWriter<W>,
    identifier_type: &IdentifierType,
) {
    write_net_id_parts(&net.name, net.subnet_number, file, identifier_type);
}

fn write_net_id_parts<W: Write>(
    name: &str,
    subnet_number: i32,
    file: &mut IndentFileWriter<W>,
    identifier_type: &IdentifierType,
) {
    file.write("net ");
    identifier_type.write(name, file);
    file.write(" ");
    file.write(&subnet_number.to_string());
}

pub fn write_pin(p: &mut WriteScopeParameter<'_>, pin_id: ItemId) {
    let board = p.board;
    let Some(item) = board.items.get(&pin_id) else {
        return;
    };
    let Item::Pin(pin) = item else {
        return;
    };
    // Java bug: Net.writePin — `FRLogger.warn("… at '" + currentComponent.name + "'")` inside the
    let component_id = item.component_id();
    if component_id < 1
        || component_id > i32::try_from(board.components.count()).unwrap_or(i32::MAX)
    {
        return;
    }
    let current_component = board.components.get(component_id);
    let component_name = current_component.name.clone();
    let package_no = current_component.get_package();
    let Some(lib_pin) = board
        .library
        .packages
        .get(package_no)
        .get_pin(pin.get_pin_index())
    else {
        return;
    };
    let lib_pin_name = lib_pin.name.clone();
    p.file.new_line();
    p.identifier_type.write(&component_name, &mut p.file);
    p.file.write("-");
    p.identifier_type.write(&lib_pin_name, &mut p.file);
}

pub fn write_network_scope(p: &mut WriteScopeParameter<'_>) {
    p.file.start_scope_nl();
    p.file.write("network");
    let board_pins = p.board.get_pins();
    for i in 1..=p.board.rules.nets.max_net_number() {
        write_net_scope(p, i, &board_pins);
    }
    write_via_infos(
        &p.board.rules,
        &p.board.library.padstacks,
        &mut p.file,
        &p.identifier_type,
    );
    write_via_rules(&p.board.rules, &mut p.file, &p.identifier_type);
    write_net_classes(p);
    p.file.end_scope();
}

pub fn write_via_infos<W: Write>(
    rules: &BoardRules,
    padstacks: &fr_board::Padstacks,
    file: &mut IndentFileWriter<W>,
    identifier_type: &IdentifierType,
) {
    for i in 0..rules.via_infos.count() {
        let current_via = rules.via_infos.get(ViaInfoId(i));
        file.start_scope_nl();
        file.write("via ");
        file.new_line();
        identifier_type.write(current_via.get_name(), file);
        file.write(" ");
        let padstack_name = padstacks
            .get(current_via.get_padstack())
            .map_or("", |padstack| padstack.name.as_str());
        identifier_type.write(padstack_name, file);
        file.write(" ");
        identifier_type.write(
            clearance_class_name(rules, current_via.get_clearance_class_index()),
            file,
        );
        if current_via.attach_smd_allowed() {
            file.write(" attach");
        }
        file.end_scope();
    }
}

pub fn write_via_rules<W: Write>(
    rules: &BoardRules,
    file: &mut IndentFileWriter<W>,
    identifier_type: &IdentifierType,
) {
    for current_rule in &rules.via_rules {
        file.start_scope_nl();
        file.write("via_rule");
        file.new_line();
        identifier_type.write(&current_rule.name, file);
        for i in 0..current_rule.via_count() {
            file.write(" ");
            identifier_type.write(current_rule.get_via(i).get_name(), file);
        }
        file.end_scope();
    }
}

pub fn write_net_classes(p: &mut WriteScopeParameter<'_>) {
    let board = p.board;
    for i in 0..board.rules.net_classes.count() {
        write_net_class(board.rules.net_classes.get(NetClassId(i)), NetClassId(i), p);
    }
}

pub fn write_net_class(
    net_class: &NetClass,
    net_class_id: NetClassId,
    p: &mut WriteScopeParameter<'_>,
) {
    let board = p.board;
    p.file.start_scope_nl();
    p.file.write("class ");
    p.identifier_type.write(net_class.get_name(), &mut p.file);
    const NETS_PER_ROW: usize = 8;
    let mut net_counter = 0usize;
    for i in 1..=board.rules.nets.max_net_number() {
        let Some(net) = board.rules.nets.get(i) else {
            continue;
        };
        if net.get_net_class() == net_class_id {
            if net_counter.is_multiple_of(NETS_PER_ROW) {
                p.file.new_line();
            } else {
                p.file.write(" ");
            }
            p.identifier_type.write(&net.name, &mut p.file);
            net_counter += 1;
        }
    }

    let trace_clearance_name =
        clearance_class_name(&board.rules, net_class.get_trace_clearance_class()).to_string();
    write_item_clearance_class(&trace_clearance_name, &mut p.file, &p.identifier_type);

    if let Some(via_rule) = net_class.get_via_rule() {
        let via_rule_name = via_rule.name.clone();
        p.file.new_line();
        p.file.write("(via_rule ");
        p.identifier_type.write(&via_rule_name, &mut p.file);
        p.file.write(")");
    }

    write_rule_scope(net_class, p);

    write_circuit(net_class, p);

    if !net_class.get_pull_tight() {
        p.file.new_line();
        p.file.write("(pull_tight off)");
    }

    if net_class.is_shove_fixed() {
        p.file.new_line();
        p.file.write("(shove_fixed on)");
    }

    p.file.end_scope();
}

fn write_circuit(net_class: &NetClass, p: &mut WriteScopeParameter<'_>) {
    let board = p.board;
    let min_trace_length = net_class.get_minimum_trace_length();
    let max_trace_length = net_class.get_maximum_trace_length();
    p.file.start_scope_nl();
    p.file.write("circuit ");
    p.file.new_line();
    p.file.write("(use_layer");
    let layer_count = net_class.layer_count();
    for i in 0..layer_count {
        if net_class.is_active_routing_layer(i) {
            let name = &board.layer_structure().layers[i].name;
            p.file.write(" ");
            p.file.write(name);
        }
    }
    p.file.write(")");
    if min_trace_length > 0.0 || max_trace_length > 0.0 {
        p.file.new_line();
        p.file.write("(length ");
        let transformed_max_length = if max_trace_length <= 0.0 {
            -1.0
        } else {
            p.coordinate_transform.board_to_dsn(max_trace_length)
        };
        p.file.write(&java_double_to_string(transformed_max_length));
        p.file.write(" ");
        let transformed_min_length = if min_trace_length <= 0.0 {
            0.0
        } else {
            p.coordinate_transform.board_to_dsn(min_trace_length)
        };
        p.file.write(&java_double_to_string(transformed_min_length));
        p.file.write(")");
    }
    p.file.end_scope();
}

#[cfg(test)]
mod tests {
    #[test]
    fn separator_str_matches_char() {
        assert_eq!(
            CLASS_CLEARANCE_SEPARATOR_STR,
            CLASS_CLEARANCE_SEPARATOR.to_string()
        );
    }

    use super::*;

    fn scan(text: &str) -> DsnScanner {
        DsnScanner::new(text)
    }

    #[test]
    fn net_id_orders_by_name_then_subnet() {
        let a = NetId {
            name: "GND".to_string(),
            subnet_no: 1,
        };
        let b = NetId {
            name: "GND".to_string(),
            subnet_no: 2,
        };
        let c = NetId {
            name: "VCC".to_string(),
            subnet_no: 0,
        };
        assert!(a < b);
        assert!(b < c);
    }

    #[test]
    fn read_rule_scope_reads_width_and_clearance() {
        let mut scanner = scan("(width 152.4) (clearance 200 (type via_smd)))");
        let rules = read_rule_scope(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0], DsnRule::Width(152.4));
        let DsnRule::Clearance(clearance) = &rules[1] else {
            panic!("expected a clearance rule");
        };
        assert!((clearance.value - 200.0).abs() < f64::EPSILON);
        assert_eq!(clearance.clearance_class_pairs, vec!["via_smd".to_string()]);
    }

    fn scan_body(text: &str) -> DsnScanner {
        let mut scanner = scan(text);
        assert_eq!(scanner.next_token().expect("scan"), Some(Token::Open));
        scanner.next_token().expect("scan");
        scanner
    }

    #[test]
    fn read_net_class_scope_reads_every_field_java_keeps() {
        let mut scanner = scan_body(concat!(
            "(class Power GND VCC (rule (width 200) (clearance 300 (type via_smd))) ",
            "(via_rule vr1) (clearance_class cc1) (shove_fixed on) (pull_tight off) ",
            "(circuit (length 5000 1000) (use_via v1 v2) (use_layer L1 L2)) ",
            "(layer_rule L1 (rule (width 150))) (junk x))",
        ));
        let net_class = read_net_class_scope(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert_eq!(net_class.name, "Power");
        assert_eq!(net_class.trace_clearance_class.as_deref(), Some("cc1"));
        assert_eq!(
            net_class.net_list,
            vec!["GND".to_string(), "VCC".to_string()]
        );
        assert_eq!(net_class.via_rule.as_deref(), Some("vr1"));
        assert!(net_class.shove_fixed);
        assert!(!net_class.pull_tight);
        assert!((net_class.min_trace_length - 1000.0).abs() < f64::EPSILON);
        assert!((net_class.max_trace_length - 5000.0).abs() < f64::EPSILON);
        assert_eq!(net_class.use_via, vec!["v1".to_string(), "v2".to_string()]);
        assert_eq!(
            net_class.use_layer,
            vec!["L1".to_string(), "L2".to_string()]
        );
        assert_eq!(net_class.rules.len(), 2);
        assert_eq!(net_class.rules[0], DsnRule::Width(200.0));
        assert_eq!(net_class.layer_rules.len(), 1);
        assert_eq!(net_class.layer_rules[0].layer_names, vec!["L1".to_string()]);
        assert_eq!(net_class.layer_rules[0].rules, vec![DsnRule::Width(150.0)]);
    }

    #[test]
    fn a_net_class_with_no_nets_gets_javas_defaults() {
        let mut scanner = scan_body("(class Empty (rule (width 1)))");
        let net_class = read_net_class_scope(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert_eq!(net_class.name, "Empty");
        assert!(net_class.net_list.is_empty());
        assert!(net_class.trace_clearance_class.is_none());
        assert!(net_class.via_rule.is_none());
        assert!(!net_class.shove_fixed);
        assert!(net_class.pull_tight);
        assert_eq!(net_class.rules, vec![DsnRule::Width(1.0)]);
    }

    #[test]
    fn read_circuit_scope_keeps_the_length_rule_and_the_use_lists() {
        let mut scanner = scan("(length 5000 1000) (use_via v1 v2) (use_layer L1 L2))");
        let circuit = read_circuit_scope(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert!((circuit.max_length - 5000.0).abs() < f64::EPSILON);
        assert!((circuit.min_length - 1000.0).abs() < f64::EPSILON);
        assert_eq!(circuit.use_via, vec!["v1".to_string(), "v2".to_string()]);
        assert_eq!(circuit.use_layer, vec!["L1".to_string(), "L2".to_string()]);
    }

    #[test]
    fn a_circuit_scope_skips_what_java_skips() {
        let mut scanner = scan("(length 5000 1000 (unit mil)))");
        let circuit = read_circuit_scope(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert!((circuit.max_length - 5000.0).abs() < f64::EPSILON);
        assert!((circuit.min_length - 1000.0).abs() < f64::EPSILON);

        let mut scanner = scan("(something else))");
        let circuit = read_circuit_scope(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert_eq!(circuit.max_length, 0.0);
        assert_eq!(circuit.min_length, 0.0);
        assert!(circuit.use_via.is_empty());
        assert!(circuit.use_layer.is_empty());
    }

    #[test]
    fn read_class_class_scope_reads_classes_rules_and_layer_rules() {
        let mut scanner =
            scan("(classes A B) (rule (width 10)) (layer_rule L1 (rule (width 20))))");
        let class_class = read_class_class_scope(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert_eq!(
            class_class.class_names,
            vec!["A".to_string(), "B".to_string()]
        );
        assert_eq!(class_class.rules, vec![DsnRule::Width(10.0)]);
        assert_eq!(class_class.layer_rules.len(), 1);
        assert_eq!(class_class.layer_rules[0].rules, vec![DsnRule::Width(20.0)]);
    }

    #[test]
    fn a_layer_rule_with_two_rule_scopes_is_rejected_the_way_java_rejects_it() {
        let mut scanner = scan("L1 L2 (rule (width 150)) (rule (clearance 20)))");
        assert_eq!(read_layer_rule_scope(&mut scanner).expect("scan"), None);

        let mut scanner = scan("L1 L2 (rule (width 150)))");
        let layer_rule = read_layer_rule_scope(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert_eq!(
            layer_rule.layer_names,
            vec!["L1".to_string(), "L2".to_string()]
        );
        assert_eq!(layer_rule.rules, vec![DsnRule::Width(150.0)]);
    }

    #[test]
    fn net_list_iterates_in_net_id_order_not_insertion_order() {
        let mut netlist = NetList::new();
        assert!(netlist.is_empty());
        for id in [
            NetId::new("VCC", 1),
            NetId::new("GND", 2),
            NetId::new("GND", 1),
        ] {
            assert!(netlist.add_net(id).is_some());
        }
        assert_eq!(
            netlist
                .values()
                .map(|net| (net.id.name.clone(), net.id.subnet_no))
                .collect::<Vec<_>>(),
            vec![
                ("GND".to_string(), 1),
                ("GND".to_string(), 2),
                ("VCC".to_string(), 1),
            ]
        );
        assert!(netlist.add_net(NetId::new("GND", 1)).is_none());
        assert!(netlist.contains(&NetId::new("GND", 1)));
        assert!(!netlist.contains(&NetId::new("GND", 3)));
    }

    #[test]
    fn net_id_and_pin_ref_order_by_utf16_code_units_like_string_compare_to() {
        assert!(NetId::new("\u{10000}", 1) < NetId::new("\u{FFFD}", 1));
        assert!("\u{10000}" > "\u{FFFD}");
        assert!(PinRef::new("\u{10000}", "1") < PinRef::new("\u{FFFD}", "1"));
        assert!(PinRef::new("U1", "\u{10000}") < PinRef::new("U1", "\u{FFFD}"));
        assert!(NetId::new("GND", 1) < NetId::new("GND", 2));
        assert!(NetId::new("GND", 9) < NetId::new("VCC", 1));
        assert!(PinRef::new("R1", "2") < PinRef::new("U1", "1"));
    }

    #[test]
    fn java_split_underscore_drops_trailing_empties_the_way_javas_split_does() {
        assert_eq!(java_split_underscore("via_smd"), vec!["via", "smd"]);
        assert_eq!(java_split_underscore("smd_via_same_net").len(), 4);
        assert_eq!(java_split_underscore("via_"), vec!["via"]);
        assert_eq!(java_split_underscore("_via"), vec!["", "via"]);
        assert_eq!(java_split_underscore("a__b"), vec!["a", "", "b"]);
        assert!(java_split_underscore("_").is_empty());
        assert_eq!(java_split_underscore(""), vec![""]);
        assert_eq!(java_split_underscore("wire"), vec!["wire"]);
    }

    #[test]
    fn kicad_default_net_class_names_are_matched_case_insensitively() {
        assert!(is_kicad_default_net_class_name("default"));
        assert!(is_kicad_default_net_class_name("Default"));
        assert!(is_kicad_default_net_class_name("DEFAULT"));
        assert!(is_kicad_default_net_class_name("kicad_default"));
        assert!(is_kicad_default_net_class_name("KiCad_Default"));
        assert!(!is_kicad_default_net_class_name(""));
        assert!(!is_kicad_default_net_class_name("Power"));
        assert!(!is_kicad_default_net_class_name("default2"));
    }

    #[test]
    fn create_ordered_subnets_pairs_consecutive_pins() {
        let pins = vec![
            PinRef::new("U1", "1"),
            PinRef::new("R1", "2"),
            PinRef::new("U1", "3"),
        ];
        let subnets = create_ordered_subnets(&pins);
        assert_eq!(subnets.len(), 2);
        assert_eq!(
            subnets[0],
            vec![PinRef::new("R1", "2"), PinRef::new("U1", "1")]
        );
        assert_eq!(
            subnets[1],
            vec![PinRef::new("R1", "2"), PinRef::new("U1", "3")]
        );
        assert!(create_ordered_subnets(&[]).is_empty());
        assert!(create_ordered_subnets(&pins[..1]).is_empty());
        let repeated = vec![PinRef::new("U1", "1"), PinRef::new("U1", "1")];
        assert_eq!(
            create_ordered_subnets(&repeated),
            vec![vec![PinRef::new("U1", "1")]]
        );
    }

    #[test]
    fn set_pins_sorts_and_deduplicates_and_get_nets_searches_every_net() {
        let mut netlist = NetList::new();
        netlist.add_net(NetId::new("GND", 1)).expect("new net");
        netlist.add_net(NetId::new("VCC", 1)).expect("new net");
        netlist
            .get_net_mut(&NetId::new("GND", 1))
            .expect("present")
            .set_pins([
                PinRef::new("U1", "2"),
                PinRef::new("R1", "1"),
                PinRef::new("U1", "2"),
            ]);
        let pins: Vec<String> = netlist
            .get_net(&NetId::new("GND", 1))
            .expect("present")
            .get_pins()
            .expect("set")
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(pins, vec!["Pin{R1-1}".to_string(), "Pin{U1-2}".to_string()]);

        assert!(
            netlist
                .get_net(&NetId::new("VCC", 1))
                .expect("present")
                .get_pins()
                .is_none()
        );
        assert_eq!(netlist.get_nets("U1", "2").len(), 1);
        assert!(netlist.get_nets("U1", "3").is_empty());
    }
}

#[cfg(test)]
mod component_rejection_tests {
    use fr_board::{PackagePin, Packages, PadstackId};
    use fr_geometry::{IntVector, Vector};

    use super::*;
    use crate::coordinate_transform::CoordinateTransform;
    use crate::error::BoardReadResult;
    use crate::parser::placement::{ComponentLocation, ComponentPlacement};
    use crate::parser::scope_parameter::DsnReadOptions;

    const DSN: &str = "(pcb t103.dsn\n  (parser\n    (string_quote \")\n  )\n  (resolution um \
                       10)\n  (unit um)\n  (structure\n    (layer F.Cu (type signal))\n    \
                       (layer B.Cu (type signal))\n    (boundary\n      (path pcb 0  0 0  \
                       100000 0  100000 100000  0 100000  0 0)\n    )\n  )\n  (library\n    \
                       (padstack PAD (shape (circle F.Cu 800)) (shape (circle B.Cu 800)) \
                       (attach off))\n  )\n)\n";

    fn board_and_transform() -> (Board, CoordinateTransform) {
        let options = DsnReadOptions::default();
        match crate::dsn_reader::read_board(DSN.as_bytes(), None, Some("t103"), &options) {
            BoardReadResult::Success {
                board,
                coordinate_transform,
                ..
            } => (
                *board.expect("a board"),
                coordinate_transform.expect("a transform"),
            ),
            other => panic!("the fixture must read cleanly: {other:?}"),
        }
    }

    fn pin(name: &str, padstack: PadstackId) -> PackagePin {
        PackagePin::new(name, padstack, Vector::Int(IntVector::new(0, 0)), 0.0)
    }

    fn package_pair(packages: &mut Packages, name: &str, padstacks: &[PadstackId]) {
        for is_front in [true, false] {
            let pins = padstacks
                .iter()
                .enumerate()
                .map(|(i, p)| pin(&(i + 1).to_string(), *p))
                .collect();
            packages.add(
                name,
                pins,
                None,
                None,
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                is_front,
            );
        }
    }

    fn placed(lib_name: &str, component_name: &str) -> ComponentPlacement {
        ComponentPlacement {
            lib_name: lib_name.to_string(),
            locations: vec![ComponentLocation {
                name: component_name.to_string(),
                coor: Some([1000.0, 1000.0]),
                is_front: true,
                rotation: 0.0,
                position_fixed: false,
                pin_infos: BTreeMap::new(),
                keepout_infos: BTreeMap::new(),
                via_keepout_infos: BTreeMap::new(),
                place_keepout_infos: BTreeMap::new(),
                part_number: None,
            }],
        }
    }

    struct Outcome {
        components: Vec<String>,
        pin_ids: Vec<fr_board::ItemId>,
        warnings: Vec<String>,
    }

    fn run(bad_padstack: Option<PadstackId>, placement_list: Vec<ComponentPlacement>) -> Outcome {
        let (mut board, ct) = board_and_transform();
        let good = PadstackId(
            board
                .library
                .padstacks
                .get_by_name("PAD")
                .expect("the fixture's padstack")
                .no,
        );
        board.library.packages = Packages::new();
        package_pair(
            &mut board.library.packages,
            "BAD",
            &[good, bad_padstack.unwrap_or(good)],
        );
        package_pair(&mut board.library.packages, "GOOD", &[good]);

        let options = DsnReadOptions::default();
        let mut p = ReadScopeParameter::new(DsnScanner::new(""), &options);
        p.board = Some(board);
        p.coordinate_transform = Some(ct);
        p.placement_list = placement_list;
        insert_components(&mut p);

        let board = p.board.take().expect("the board");
        Outcome {
            components: (0..board.components.count())
                .map(|i| board.components.get(i as i32 + 1).name.clone())
                .collect(),
            pin_ids: board
                .items
                .iter()
                .filter(|(_, item)| matches!(item, Item::Pin(_)))
                .map(|(id, _)| *id)
                .collect(),
            warnings: p.warnings,
        }
    }

    #[test]
    fn a_component_with_an_absent_padstack_is_rejected_whole() {
        let both = vec![placed("BAD", "C1"), placed("GOOD", "C2")];

        let control = run(None, both.clone());
        assert_eq!(control.components, vec!["C1", "C2"]);
        assert_eq!(control.pin_ids.len(), 3, "C1's two pins and C2's one");
        assert!(control.warnings.is_empty());

        let alone = run(None, vec![placed("GOOD", "C2")]);
        assert_eq!(alone.components, vec!["C2"]);

        let rejected = run(Some(PadstackId(9999)), both);
        assert_eq!(
            rejected.components,
            vec!["C2"],
            "the rejected component is not added at all — Java adds it and then abandons it"
        );
        assert_eq!(
            rejected.pin_ids, alone.pin_ids,
            "the following component's item ids are unshifted"
        );
        assert_eq!(rejected.warnings.len(), 1);
        assert!(
            rejected.warnings[0].contains("C1") && rejected.warnings[0].contains("rejected"),
            "the diagnostic names the component: {}",
            rejected.warnings[0]
        );
    }
}
