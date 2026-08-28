//! `fr-board`: the board-model crate — items, rules, library, search trees, and `Board` itself.
//! Faithful port of freerouting's `board/model/**`, `board/searchtree/**`, `rules/**`, and
//! `core/library/**` package family (see the Plan 2 design doc).
//!
//! This crate must not depend on `tracing`
//! (`.superpowers/sdd/2026-08-28-plan-2-board-model/global-constraints.md`): diagnostic
//! `FRLogger` calls from the Java source are dropped during porting, and invariant-guard logs
//! become `debug_assert!`.

pub mod datastructures;
pub mod error;
pub mod ids;
pub mod items;
pub mod library;
pub mod rules;
pub mod structure;

pub use datastructures::{LeafId, Node, NodeId, ShapeTree, StopCheck, TimeLimit, TreeEntry};
pub use error::BoardError;
pub use ids::{
    ItemId, ItemIdGenerator, NetClassId, PadstackId, RoomId, TreeId, TreeObject, ViaInfoId,
    ViaRuleId,
};
pub use items::{
    AutorouteInfo, BoardOutline, ComponentObstacleArea, ComponentOutline, ConductionArea,
    Connectable, ConnectableRef, DrillItemData, Item, ItemCtx, ItemHeader, ItemKind, ObstacleArea,
    Pin, PolylineTrace, TraceExitRestriction, TreeEntries, Via, ViaObstacleArea,
};
pub use library::{
    BoardLibrary, DrillItemPadstackLookup, Keepout, LogicalPart, LogicalParts, Package, PackagePin,
    Packages, Padstack, Padstacks, PartPin,
};
pub use rules::{
    BoardRules, ClearanceClassIndexed, ClearanceMatrix, DefaultItemClearanceClasses, ItemClass,
    Net, NetClass, NetClasses, Nets, PadstackLookup, ViaInfo, ViaInfos, ViaRule,
};
pub use structure::{
    AngleRestriction, Component, Components, FixedState, Layer, LayerStructure, Unit,
};

/// Re-exports every public type of the crate, for `use fr_board::prelude::*;`.
pub mod prelude {
    pub use crate::{
        AngleRestriction, AutorouteInfo, BoardError, BoardLibrary, BoardOutline, BoardRules,
        ClearanceClassIndexed, ClearanceMatrix, Component, ComponentObstacleArea, ComponentOutline,
        Components, ConductionArea, Connectable, ConnectableRef, DefaultItemClearanceClasses,
        DrillItemData, DrillItemPadstackLookup, FixedState, Item, ItemClass, ItemCtx, ItemHeader,
        ItemId, ItemIdGenerator, ItemKind, Keepout, Layer, LayerStructure, LeafId, LogicalPart,
        LogicalParts, Net, NetClass, NetClassId, NetClasses, Nets, Node, NodeId, ObstacleArea,
        Package, PackagePin, Packages, Padstack, PadstackId, PadstackLookup, Padstacks, PartPin,
        Pin, PolylineTrace, RoomId, ShapeTree, StopCheck, TimeLimit, TraceExitRestriction,
        TreeEntries, TreeEntry, TreeId, TreeObject, Unit, Via, ViaInfo, ViaInfoId, ViaInfos,
        ViaObstacleArea, ViaRule, ViaRuleId,
    };
}
