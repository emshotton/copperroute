//! The edits that turn a KiCad board into its routed version: tracks and vias to add, and the
//! KiCad items that rip-up removed. Copper the router kept is matched geometrically, because the
//! router splits and renumbers the traces it keeps.

use std::collections::BTreeSet;

use copper_board::Board;
use serde::{Deserialize, Serialize};

use super::dto::{KiCadBoardJson, Point2D, TraceJson, ViaJson};

const TOLERANCE_MM: f64 = 1e-4;

#[allow(non_snake_case)]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TrackChange {
    pub start: Point2D,
    pub end: Point2D,
    pub width: f64,
    pub layer: String,
    pub net: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ViaChange {
    pub position: Point2D,
    pub diameter: f64,
    pub drill: f64,
    pub startLayer: String,
    pub endLayer: String,
    pub net: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RoutingChanges {
    pub addTracks: Vec<TrackChange>,
    pub addVias: Vec<ViaChange>,
    /// KiCad UUIDs of tracks, arcs and vias to delete.
    pub removeIds: Vec<String>,
}

struct Segment {
    a: Point2D,
    b: Point2D,
    layer: i32,
    width: f64,
}

fn near(a: &Point2D, b: &Point2D) -> bool {
    (a.x - b.x).hypot(a.y - b.y) < TOLERANCE_MM
}

fn on_segment(p: &Point2D, a: &Point2D, b: &Point2D) -> bool {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length = dx.hypot(dy);
    if length < 1e-9 {
        return near(p, a);
    }
    let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / (length * length);
    t > -1e-6
        && t < 1.0 + 1e-6
        && ((p.x - a.x) * dy - (p.y - a.y) * dx).abs() / length < TOLERANCE_MM
}

fn segments(traces: &[TraceJson]) -> Vec<Segment> {
    traces
        .iter()
        .flat_map(|trace| {
            let points = trace.points.as_deref().unwrap_or_default();
            points.windows(2).map(|pair| Segment {
                a: pair[0],
                b: pair[1],
                layer: trace.layerIndex,
                width: trace.width,
            })
        })
        .collect()
}

fn layer_name(board: &KiCadBoardJson, index: i32) -> String {
    board
        .layers
        .iter()
        .flatten()
        .find(|layer| layer.index == index)
        .and_then(|layer| layer.name.clone())
        .unwrap_or_default()
}

/// `original` is the imported board before rip-up and `kept` the one that was routed.
#[must_use]
pub fn routing_changes(
    original: &KiCadBoardJson,
    kept: &KiCadBoardJson,
    routed: &Board,
) -> RoutingChanges {
    let routed: KiCadBoardJson = serde_json::from_str(&super::writer::write(routed, "routed"))
        .expect("the KiCad writer emits the board JSON it reads");

    let kept_ids: BTreeSet<&str> = kept
        .traces
        .iter()
        .flatten()
        .filter_map(|trace| trace.sourceId.as_deref())
        .chain(
            kept.vias
                .iter()
                .flatten()
                .filter_map(|via| via.sourceId.as_deref()),
        )
        .collect();
    let remove_ids = original
        .traces
        .iter()
        .flatten()
        .filter_map(|trace| trace.sourceId.as_deref())
        .chain(
            original
                .vias
                .iter()
                .flatten()
                .filter_map(|via| via.sourceId.as_deref()),
        )
        .filter(|id| !kept_ids.contains(id))
        .map(str::to_string)
        .collect();

    let kept_segments = segments(kept.traces.as_deref().unwrap_or_default());
    let is_kept = |segment: &Segment| {
        kept_segments.iter().any(|existing| {
            existing.layer == segment.layer
                && (existing.width - segment.width).abs() < TOLERANCE_MM
                && on_segment(&segment.a, &existing.a, &existing.b)
                && on_segment(&segment.b, &existing.a, &existing.b)
        })
    };
    let net_of = |name: Option<&str>| name.unwrap_or_default().to_string();

    let add_tracks = routed
        .traces
        .iter()
        .flatten()
        .flat_map(|trace| {
            segments(std::slice::from_ref(trace))
                .into_iter()
                .filter(|segment| !is_kept(segment) && !near(&segment.a, &segment.b))
                .map(|segment| TrackChange {
                    layer: layer_name(original, segment.layer),
                    start: segment.a,
                    end: segment.b,
                    width: segment.width,
                    net: net_of(trace.netName.as_deref()),
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let kept_vias: &[ViaJson] = kept.vias.as_deref().unwrap_or_default();
    let add_vias = routed
        .vias
        .iter()
        .flatten()
        .filter(|via| {
            !kept_vias.iter().any(|existing| {
                existing.startLayerIndex == via.startLayerIndex
                    && existing.endLayerIndex == via.endLayerIndex
                    && match (&existing.position, &via.position) {
                        (Some(a), Some(b)) => near(a, b),
                        _ => false,
                    }
            })
        })
        .map(|via| ViaChange {
            position: via.position.unwrap_or_default(),
            diameter: via.diameter,
            drill: via.drill,
            startLayer: layer_name(original, via.startLayerIndex),
            endLayer: layer_name(original, via.endLayerIndex),
            net: net_of(via.netName.as_deref()),
        })
        .collect();

    RoutingChanges {
        addTracks: add_tracks,
        addVias: add_vias,
        removeIds: remove_ids,
    }
}

fn fixed(value: f64) -> String {
    let text = format!("{value:.6}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_string()
    } else {
        text.to_string()
    }
}

fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// KiCad accepts any unique ID; deriving it from the item keeps the output reproducible.
fn item_uuid(seed: &str) -> String {
    let mut hash: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    for byte in seed.bytes() {
        hash ^= u128::from(byte);
        hash = hash.wrapping_mul(0x0000_0000_0100_0000_0000_0000_0000_013b);
    }
    let hex = format!("{hash:032x}");
    format!(
        "{}-{}-4{}-8{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[13..16],
        &hex[17..20],
        &hex[20..32]
    )
}

/// Writes `changes` into the KiCad board file text: removed items are cut out and new tracks
/// and vias are appended, leaving every other byte as it was.
pub fn apply_changes_to_text(
    text: &str,
    changes: &RoutingChanges,
) -> Result<String, super::pcb::PcbError> {
    let root = super::sexpr::parse(text)
        .map_err(|error| super::pcb::PcbError::new("changes", &error.to_string()))?;
    let named_nets = root.number("version").is_some_and(|v| v >= 20_260_101.0);
    let net_ref = |name: &str| -> String {
        if named_nets {
            return quoted(name);
        }
        root.children("net")
            .find(|net| net.atom(2) == Some(name))
            .and_then(|net| net.atom(1))
            .unwrap_or("0")
            .to_string()
    };
    let remove: BTreeSet<&str> = changes.removeIds.iter().map(String::as_str).collect();
    let mut cuts: Vec<(usize, usize)> = root
        .values
        .iter()
        .filter_map(|value| match value {
            super::sexpr::Value::Node(node)
                if matches!(node.name(), "segment" | "arc" | "via")
                    && super::pcb::routing::source_id(node)
                        .is_some_and(|id| remove.contains(id.as_str())) =>
            {
                Some((node.start, node.end))
            }
            _ => None,
        })
        .collect();
    cuts.sort_unstable();

    let mut added = Vec::new();
    for (index, track) in changes.addTracks.iter().enumerate() {
        let id = item_uuid(&format!("track{index}{track:?}"));
        added.push(format!(
            "\t(segment (start {} {}) (end {} {}) (width {}) (layer {}) (net {}) (uuid {}))",
            fixed(track.start.x),
            fixed(track.start.y),
            fixed(track.end.x),
            fixed(track.end.y),
            fixed(track.width),
            quoted(&track.layer),
            net_ref(&track.net),
            quoted(&id)
        ));
    }
    for (index, via) in changes.addVias.iter().enumerate() {
        let id = item_uuid(&format!("via{index}{via:?}"));
        let through = root
            .child("layers")
            .map(|layers| {
                let copper: Vec<&str> = layers
                    .values
                    .iter()
                    .filter_map(|value| match value {
                        super::sexpr::Value::Node(layer) => layer.atom(1),
                        super::sexpr::Value::Atom(_) => None,
                    })
                    .filter(|name| name.ends_with(".Cu"))
                    .collect();
                copper.first() == Some(&via.startLayer.as_str())
                    && copper.last() == Some(&via.endLayer.as_str())
            })
            .unwrap_or(true);
        added.push(format!(
            "\t(via{} (at {} {}) (size {}) (drill {}) (layers {} {}) (net {}) (uuid {}))",
            if through { "" } else { " blind" },
            fixed(via.position.x),
            fixed(via.position.y),
            fixed(via.diameter),
            fixed(via.drill),
            quoted(&via.startLayer),
            quoted(&via.endLayer),
            net_ref(&via.net),
            quoted(&id)
        ));
    }

    let close = text[..root.end].rfind(')').unwrap_or(root.end);
    let mut out = String::with_capacity(text.len() + added.len() * 120);
    let mut cursor = 0;
    for (start, end) in cuts {
        out.push_str(&text[cursor..start]);
        cursor = end;
    }
    out.push_str(&text[cursor..close]);
    if !added.is_empty() {
        out.push('\n');
        out.push_str(&added.join("\n"));
        out.push('\n');
    }
    out.push_str(&text[close..]);
    Ok(out)
}
