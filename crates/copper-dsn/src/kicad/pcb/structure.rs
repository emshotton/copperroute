use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use super::PcbError;
use crate::kicad::sexpr::{Node, Value};
use crate::kicad::{LayerJson, NetClassJson, NetJson};

const SECTION: &str = "structure";
const NAMED_NET_VERSION: f64 = 20_260_101.0;

fn number(text: &str) -> Result<f64, PcbError> {
    let value: f64 = text.parse().unwrap_or(f64::NAN);
    if !value.is_finite() || value.abs() > 100_000.0 {
        return Err(PcbError::new(
            SECTION,
            "Invalid or excessive board coordinate.",
        ));
    }
    Ok(value)
}

#[derive(Debug)]
pub struct Layers {
    pub entries: Vec<LayerJson>,
}

impl Layers {
    pub fn read(root: &Node) -> Result<Layers, PcbError> {
        let mut entries = Vec::new();
        if let Some(layers) = root.child("layers") {
            for value in &layers.values {
                let Value::Node(entry) = value else {
                    continue;
                };
                let Some(name) = entry.atom(1) else {
                    continue;
                };
                if !name.ends_with(".Cu") {
                    continue;
                }
                let r#type = match entry.atom(2).unwrap_or("") {
                    "signal" | "mixed" => "signal",
                    "power" => "plane",
                    other => {
                        return Err(PcbError::new(
                            SECTION,
                            &format!("Unsupported copper layer type: {other}"),
                        ));
                    }
                };
                entries.push(LayerJson {
                    index: entries.len() as i32,
                    name: Some(name.to_string()),
                    r#type: Some(r#type.to_string()),
                });
            }
        }
        if entries.is_empty() || entries.len() > 32 {
            return Err(PcbError::new(SECTION, "Expected 1\u{2013}32 copper layers."));
        }
        Ok(Layers { entries })
    }

    pub fn index_of(&self, name: &str) -> Result<i32, PcbError> {
        self.entries
            .iter()
            .find(|layer| layer.name.as_deref() == Some(name))
            .map(|layer| layer.index)
            .ok_or_else(|| PcbError::new(SECTION, &format!("Unknown copper layer: {name}")))
    }
}

#[derive(Debug)]
pub struct NetTable {
    nets: RefCell<Vec<NetJson>>,
    named_nets: bool,
}

impl NetTable {
    pub fn read(root: &Node) -> Result<NetTable, PcbError> {
        let mut nets = Vec::new();
        for net in root.children("net") {
            let id = number(net.atom(1).unwrap_or(""))? as i32;
            nets.push(NetJson {
                id,
                name: net.atom(2).map(str::to_string),
                className: Some("Default".to_string()),
                containsPlane: false,
            });
        }
        let named_nets = root.number("version").unwrap_or(0.0) >= NAMED_NET_VERSION;
        Ok(NetTable {
            nets: RefCell::new(nets),
            named_nets,
        })
    }

    pub fn name_of(&self, node: &Node) -> Result<String, PcbError> {
        if self.named_nets {
            let name = node.value("net").unwrap_or("");
            if name.is_empty() {
                return Ok(String::new());
            }
            let mut nets = self.nets.borrow_mut();
            if !nets.iter().any(|net| net.name.as_deref() == Some(name)) {
                let id = nets.len() as i32 + 1;
                nets.push(NetJson {
                    id,
                    name: Some(name.to_string()),
                    className: Some("Default".to_string()),
                    containsPlane: false,
                });
            }
            return Ok(name.to_string());
        }
        let id = node.number("net").unwrap_or(0.0) as i32;
        if id == 0 {
            return Ok(String::new());
        }
        let nets = self.nets.borrow();
        nets.iter()
            .find(|net| net.id == id)
            .map(|net| net.name.clone().unwrap_or_default())
            .ok_or_else(|| PcbError::new(SECTION, &format!("Unknown net {id}")))
    }

    pub fn finish(self, mut net_classes: Vec<NetClassJson>) -> (Vec<NetJson>, Vec<NetClassJson>) {
        let mut assignments: HashMap<String, String> = HashMap::new();
        for class in &net_classes {
            let class_name = class.name.clone().unwrap_or_default();
            for net_name in class.netNames.iter().flatten() {
                assignments.insert(net_name.clone(), class_name.clone());
            }
        }
        for class in &mut net_classes {
            class.netNames = Some(Vec::new());
        }

        let mut nets: Vec<NetJson> = self
            .nets
            .into_inner()
            .into_iter()
            .filter(|net| net.id > 0)
            .collect();
        for net in &mut nets {
            let net_name = net.name.clone().unwrap_or_default();
            let class_name = assignments
                .get(&net_name)
                .cloned()
                .unwrap_or_else(|| "Default".to_string());
            net.className = Some(class_name.clone());
            if let Some(names) = net_classes
                .iter_mut()
                .find(|class| class.name.as_deref() == Some(class_name.as_str()))
                .and_then(|class| class.netNames.as_mut())
            {
                names.push(net_name);
            }
        }
        (nets, net_classes)
    }
}

pub fn read_net_classes(
    root: &Node,
    defaults: &NetClassJson,
) -> Result<Vec<NetClassJson>, PcbError> {
    let mut classes = Vec::new();
    for node in root.children("net_class") {
        let clearance = number(node.value("clearance").unwrap_or(""))?;
        let trace_width = number(node.value("trace_width").unwrap_or(""))?;
        let via_diameter = number(node.value("via_dia").unwrap_or(""))?;
        let via_drill = number(node.value("via_drill").unwrap_or(""))?;
        let net_names: Vec<String> = node
            .children("add_net")
            .filter_map(|net| net.atom(1).map(str::to_string))
            .collect();
        classes.push(NetClassJson {
            viaInPadAllowed: None,
            name: node.atom(1).map(str::to_string),
            clearance,
            traceWidth: trace_width,
            viaDiameter: via_diameter,
            viaDrill: via_drill,
            netNames: Some(net_names),
        });
    }

    let mut seen_names = HashSet::new();
    let mut assignments: HashMap<String, String> = HashMap::new();
    for class in &classes {
        let name = match &class.name {
            Some(name) if !name.is_empty() && seen_names.insert(name.clone()) => name.clone(),
            _ => {
                return Err(PcbError::new(
                    SECTION,
                    "Embedded net class names must be nonempty and unique.",
                ));
            }
        };
        if class.clearance < 0.0
            || class.traceWidth <= 0.0
            || class.viaDrill <= 0.0
            || class.viaDiameter <= class.viaDrill
        {
            return Err(PcbError::new(
                SECTION,
                &format!("Invalid embedded routing rules for {name}."),
            ));
        }
        for net_name in class.netNames.iter().flatten() {
            match assignments.get(net_name) {
                Some(existing) if existing != &name => {
                    return Err(PcbError::new(
                        SECTION,
                        &format!("Net {net_name} belongs to multiple embedded net classes."),
                    ));
                }
                Some(_) => {}
                None => {
                    assignments.insert(net_name.clone(), name.clone());
                }
            }
        }
    }

    if seen_names.contains("Default") {
        classes.sort_by_key(|class| class.name.as_deref() != Some("Default"));
    } else {
        let mut default_class = defaults.clone();
        default_class.netNames = Some(Vec::new());
        classes.insert(0, default_class);
    }

    Ok(classes)
}
