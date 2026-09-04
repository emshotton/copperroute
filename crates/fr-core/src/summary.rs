use fr_board::Board;
use fr_dsn::BoardMetadata;
use fr_router::score::BoardStatistics;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LayerSummary {
    pub index: usize,
    pub name: String,
    pub signal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NetSummary {
    pub number: i32,
    pub name: String,
    pub class: String,
    pub contains_plane: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComponentSummary {
    pub id: i32,
    pub name: String,
    pub placed: bool,
    pub on_front: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SummaryMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_cad: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_version: Option<String>,
    pub unit: String,
    pub resolution: i32,
    pub snap_angle: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BoardSummary {
    pub layers: Vec<LayerSummary>,
    pub nets: Vec<NetSummary>,
    pub components: Vec<ComponentSummary>,
    pub statistics: serde_json::Value,
    pub metadata: SummaryMetadata,
}

impl BoardSummary {
    #[must_use]
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("a BoardSummary serializes")
    }
}

#[must_use]
pub fn summarise(board: &mut Board, metadata: Option<&BoardMetadata>) -> BoardSummary {
    let layers = board
        .layer_structure()
        .layers
        .iter()
        .enumerate()
        .map(|(index, layer)| LayerSummary {
            index,
            name: layer.name.clone(),
            signal: layer.is_signal,
        })
        .collect();

    let nets = board
        .rules
        .nets
        .iter()
        .map(|net| NetSummary {
            number: net.net_number,
            name: net.name.clone(),
            class: board
                .rules
                .net_classes
                .get(net.net_class)
                .get_name()
                .to_string(),
            contains_plane: net.contains_plane,
        })
        .collect();

    let components = board
        .components
        .get_all()
        .map(|component| ComponentSummary {
            id: component.id,
            name: component.name.clone(),
            placed: component.is_placed(),
            on_front: component.placed_on_front(),
        })
        .collect();

    let metadata = SummaryMetadata {
        host_cad: board
            .communication
            .host_cad
            .clone()
            .or_else(|| metadata.and_then(|m| m.host_cad.clone())),
        host_version: board
            .communication
            .host_version
            .clone()
            .or_else(|| metadata.and_then(|m| m.host_version.clone())),
        unit: board.communication.unit.to_string(),
        resolution: board.communication.resolution,
        snap_angle: snap_angle_keyword(board.rules.trace_angle_restriction),
    };

    let statistics = crate::to_gson_json(&BoardStatistics::new(board));

    BoardSummary {
        layers,
        nets,
        components,
        statistics,
        metadata,
    }
}

fn snap_angle_keyword(restriction: fr_board::AngleRestriction) -> String {
    match restriction {
        fr_board::AngleRestriction::None => "none",
        fr_board::AngleRestriction::FortyFiveDegree => "fortyfive_degree",
        fr_board::AngleRestriction::NinetyDegree => "ninety_degree",
    }
    .to_string()
}
