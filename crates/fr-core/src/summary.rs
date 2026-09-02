//! The board summary shared by `freerouting info` (spec §12) and the `board_info` MCP tool
//! (spec §13).
//!
//! **New — Java has no `info` mode and no `board_info` tool.** The closest thing in the jar is
//! `BoardStatistics.toString()` (`core/scoring/BoardStatistics.java:589-591`), whose JSON this
//! embeds verbatim under `statistics`, so the two surfaces cannot drift from the jar's own
//! counters and cannot drift from each other: `info` and `board_info` call
//! [`summarise`] and nothing else.
//!
//! # What this adds over the statistics, and what it deliberately does not
//!
//! Every **count** in the answer comes from [`BoardStatistics`] and from nowhere else — there is
//! no second, port-only tally of layers, nets or components anywhere in this file. What the three
//! vectors add is the half a counter cannot carry: the **names**. A caller asking "what is on
//! this board" wants `GND`/`+3V3` and `R1`/`U2`, and the statistics document has no room for
//! either. `the_summary_counts_agree_with_the_statistics` pins the two halves against each other
//! so a future edit cannot make them disagree.
//!
//! # Key order
//!
//! [`BoardSummary`] and its three row types are plain `derive(Serialize)` structs, so
//! `serde_json` writes their keys in **declaration order** (Convention 8) — which is what makes
//! `freerouting info`'s stdout stable enough to diff. The one exception is inside `statistics`:
//! it is a [`serde_json::Value`], and this workspace does not enable `serde_json`'s
//! `preserve_order`, so its object keys come back **alphabetised** rather than in Gson's
//! declaration order. That is [`crate::to_gson_json`]'s own documented loss ("use it to *inspect*
//! statistics, never to write them"), and it is the right trade here: both surfaces embed the
//! same `Value`, so the alphabetisation is stable and identical on both, and the byte-exact
//! Gson rendering stays available through [`crate::to_gson_string`] for the callers that need it
//! (the result manifest, `p8t2`).

use fr_board::Board;
use fr_dsn::BoardMetadata;
use fr_router::score::BoardStatistics;
use serde::Serialize;

/// One board layer, in stack order — `board/model/structure/Layer.java:9,15`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LayerSummary {
    /// The layer's index in the stack, which is what every `layers[i]` setting is keyed by.
    pub index: usize,
    /// `Layer.name`.
    pub name: String,
    /// `Layer.isSignal` — false for a power/ground plane, which the router will not route on.
    pub signal: bool,
}

/// One electrical net, in net-number order — `rules/Net.java:23,32,38,41`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NetSummary {
    /// `Net.netNumber`, the strictly positive number every item's net list holds.
    pub number: i32,
    /// `Net.name`.
    pub name: String,
    /// The name of `Net.netClass` (`rules/NetClass.java:27`), resolved through
    /// `BoardRules.netClasses`; Java's field is an object reference and this port's is an index.
    pub class: String,
    /// `Net.containsPlane` — the net has a copper pour, which changes how it is routed.
    pub contains_plane: bool,
}

/// One component, in `Component.id` order — `board/model/structure/Component.java:22,25,39,48`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComponentSummary {
    /// `Component.id`, assigned by `Components.add` as `count + 1`.
    pub id: i32,
    /// `Component.name`.
    pub name: String,
    /// `Component.isPlaced()` — `Component.location != null`.
    pub placed: bool,
    /// `Component.onFront`.
    pub on_front: bool,
}

/// The file-level facts about the board, as the reader recorded them.
///
/// These are `Communication.SpecctraParserInfo`'s (`board/model/Communication.java:109-115`) plus
/// the two the board itself owns, and they are read off the **board** rather than off
/// [`BoardMetadata`] for the reason [`summarise`]'s `metadata` argument documents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SummaryMetadata {
    /// `specctraParserInfo.hostCad` — `(parser (host_cad …))`. Java's `null` is `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_cad: Option<String>,
    /// `specctraParserInfo.hostVersion` — `(parser (host_version …))`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_version: Option<String>,
    /// `Communication.unit` (`:20`) — `mil`, `inch`, `mm` or `um`, spelled as `Unit.toString()`.
    pub unit: String,
    /// `Communication.resolution` (`:27`).
    pub resolution: i32,
    /// `BoardRules.traceAngleRestriction` (`rules/BoardRules.java:31`), spelled as the DSN
    /// `(snap_angle …)` keyword the reader accepted.
    pub snap_angle: String,
}

/// Spec §12's `info` document and spec §13's `board_info` result — one value, two surfaces.
///
/// See the module docs for where each half comes from and for the one place key order is not
/// declaration order.
#[derive(Debug, Clone, Serialize)]
pub struct BoardSummary {
    /// The layer stack, bottom index first.
    pub layers: Vec<LayerSummary>,
    /// Every net, in net-number order.
    pub nets: Vec<NetSummary>,
    /// Every component, in id order.
    pub components: Vec<ComponentSummary>,
    /// `BoardStatistics.toString()`'s document, embedded whole — the **only** source of counts in
    /// this answer.
    pub statistics: serde_json::Value,
    /// The file-level facts.
    pub metadata: SummaryMetadata,
}

impl BoardSummary {
    /// The document `freerouting info` writes to stdout, and the text block the MCP tool's result
    /// carries.
    ///
    /// `serde_json::to_string_pretty`, not [`crate::to_gson_string`]: this is a **port-only**
    /// surface with no jar counterpart to be byte-compatible with (the delta table in
    /// `crates/freerouting/README.md` says so), and Gson's formatter is reserved for the three
    /// documents that do have one.
    ///
    /// # Panics
    ///
    /// Never in practice: every field is a `String`, an integer, a `bool` or a `Value` that
    /// [`crate::to_gson_json`] already produced, so there is no non-finite float and no map with
    /// a non-string key for `serde_json` to refuse.
    #[must_use]
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("a BoardSummary serializes")
    }
}

/// Summarise a loaded board.
///
/// `board` is `&mut` because [`BoardStatistics::new`] is — the computing constructor runs
/// `DesignRulesChecker` (`core/scoring/BoardStatistics.java:265-268`, `:338-341`), and every
/// clearance query lowers an item's `smallestClearance` (plan-5 ruling 8). Both callers own the
/// board and drop it afterwards, so the mutation is invisible.
///
/// `metadata` is an [`Option`] where the task brief drafted a plain `&BoardMetadata`, and the
/// correction is Task 3's, made for the same reason: `DsnReader.readBoard` constructs its
/// `Success` with a **null** metadata (`DsnReader.java:146`) — only `readMetadata` fills one — so
/// [`crate::LoadedBoard::metadata`] is `None` on every load path the CLI and the MCP take. The
/// values are therefore read off `board.communication`, which is where the reader actually put
/// them, and `metadata` is consulted only for what the board does not carry. Passing `None` is
/// not a degraded answer; it is the ordinary one.
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
        // `Communication` is what `Structure.createBoard` filled from the `(parser …)` scope; the
        // `BoardMetadata` argument carries the same two values when a caller has one, and it is
        // preferred only where the board's is absent, so neither source can silently win over a
        // real value from the other.
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

    // The statistics **last**, because the computing constructor mutates the board (see above)
    // and the three vectors above must be read from the board as it arrived.
    let statistics = crate::to_gson_json(&BoardStatistics::new(board));

    BoardSummary {
        layers,
        nets,
        components,
        statistics,
        metadata,
    }
}

/// `AngleRestriction` as the DSN `(snap_angle …)` keyword — `io/specctra/Structure.java`'s own
/// three spellings, which is what a caller reading this document would put back into a file.
fn snap_angle_keyword(restriction: fr_board::AngleRestriction) -> String {
    match restriction {
        fr_board::AngleRestriction::None => "none",
        fr_board::AngleRestriction::FortyFiveDegree => "fortyfive_degree",
        fr_board::AngleRestriction::NinetyDegree => "ninety_degree",
    }
    .to_string()
}
