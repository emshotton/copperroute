//! `fr-board`: the board-model crate — items, rules, library, search trees, and `Board` itself.
//! Faithful port of freerouting's `board/model/**`, `board/searchtree/**`, `rules/**`, and
//! `core/library/**` package family (see the Plan 2 design doc).
//!
//! This crate must not depend on `tracing`
//! (`.superpowers/sdd/2026-08-28-plan-2-board-model/global-constraints.md`): diagnostic
//! `FRLogger` calls from the Java source are dropped during porting, and invariant-guard logs
//! become `debug_assert!`.

pub mod board;
pub mod datastructures;
pub mod error;
pub mod ids;
pub mod items;
pub mod library;
pub mod rules;
pub mod searchtree;
pub mod structure;

pub use board::{Board, ChangedArea, Communication, ShapeTraceEntries, StopConnectionOption};
pub use datastructures::{
    DelaunayCorner, DelaunayEdge, LeafId, Node, NodeId, PlanarDelaunayTriangulation, ShapeTree,
    StopCheck, TimeLimit, TreeEntry,
};
pub use error::BoardError;
pub use ids::{
    ItemId, ItemIdGenerator, NetClassId, PadstackId, RoomId, TreeId, TreeObject, ViaInfoId,
    ViaRuleId,
};
pub use items::{
    AutorouteInfo, BOARD_OUTLINE_HALF_WIDTH, ComponentObstacleArea, ComponentOutline,
    ConductionArea, Connectable, ConnectableRef, DEFAULT_MAX_TREE_SHAPE_WIDTH, DrillItemData, Item,
    ItemCtx, ItemHeader, ItemKind, ObstacleArea, ObstacleAreaData, Pin, PolylineTrace,
    TraceExitRestriction, TreeEntries, Via, ViaObstacleArea,
};
pub use library::{
    BoardLibrary, DrillItemPadstackLookup, Keepout, LogicalPart, LogicalParts, Package, PackagePin,
    Packages, Padstack, Padstacks, PartPin,
};
pub use rules::{
    BoardRules, ClearanceClassIndexed, ClearanceMatrix, DefaultItemClearanceClasses, ItemClass,
    Net, NetClass, NetClasses, Nets, PadstackLookup, ViaInfo, ViaInfos, ViaRule,
};
pub use searchtree::{ItemLookup, SearchTreeManager, ShapeSearchTree};
pub use structure::{
    AngleRestriction, BoardOutline, Component, Components, FixedState, Layer, LayerStructure,
    ShapeAndEntrySide, ShapeEntrySide, Unit,
};

/// Re-exports every public type of the crate, for `use fr_board::prelude::*;`.
pub mod prelude {
    pub use crate::{
        AngleRestriction, AutorouteInfo, BOARD_OUTLINE_HALF_WIDTH, Board, BoardError, BoardLibrary,
        BoardOutline, BoardRules, ChangedArea, ClearanceClassIndexed, ClearanceMatrix,
        Communication, Component, ComponentObstacleArea, ComponentOutline, Components,
        ConductionArea, Connectable, ConnectableRef, DEFAULT_MAX_TREE_SHAPE_WIDTH,
        DefaultItemClearanceClasses, DelaunayCorner, DelaunayEdge, DrillItemData,
        DrillItemPadstackLookup, FixedState, Item, ItemClass, ItemCtx, ItemHeader, ItemId,
        ItemIdGenerator, ItemKind, ItemLookup, Keepout, Layer, LayerStructure, LeafId, LogicalPart,
        LogicalParts, Net, NetClass, NetClassId, NetClasses, Nets, Node, NodeId, ObstacleArea,
        ObstacleAreaData, Package, PackagePin, Packages, Padstack, PadstackId, PadstackLookup,
        Padstacks, PartPin, Pin, PlanarDelaunayTriangulation, PolylineTrace, RoomId,
        SearchTreeManager, ShapeAndEntrySide, ShapeEntrySide, ShapeSearchTree, ShapeTraceEntries,
        ShapeTree, StopCheck, StopConnectionOption, TimeLimit, TraceExitRestriction, TreeEntries,
        TreeEntry, TreeId, TreeObject, Unit, Via, ViaInfo, ViaInfoId, ViaInfos, ViaObstacleArea,
        ViaRule, ViaRuleId,
    };
}
