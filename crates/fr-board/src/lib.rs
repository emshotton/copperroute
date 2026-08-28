//! `fr-board`: the board-model crate — items, rules, library, search trees, and `Board` itself.
//! Faithful port of freerouting's `board/model/**`, `board/searchtree/**`, `rules/**`, and
//! `core/library/**` package family (see the Plan 2 design doc).
//!
//! This crate must not depend on `tracing`
//! (`.superpowers/sdd/2026-08-28-plan-2-board-model/global-constraints.md`): diagnostic
//! `FRLogger` calls from the Java source are dropped during porting, and invariant-guard logs
//! become `debug_assert!`.

pub mod error;
pub mod ids;
pub mod rules;
pub mod structure;

pub use error::BoardError;
pub use ids::{
    ItemId, ItemIdGenerator, NetClassId, PadstackId, RoomId, TreeId, TreeObject, ViaInfoId,
    ViaRuleId,
};
pub use rules::{
    BoardRules, ClearanceClassIndexed, ClearanceMatrix, DefaultItemClearanceClasses, ItemClass,
    Net, NetClass, NetClasses, Nets, PadstackLookup, ViaInfo, ViaInfos, ViaRule,
};
pub use structure::{AngleRestriction, FixedState, Layer, LayerStructure, Unit};

/// Re-exports every public type of the crate, for `use fr_board::prelude::*;`.
pub mod prelude {
    pub use crate::{
        AngleRestriction, BoardError, BoardRules, ClearanceClassIndexed, ClearanceMatrix,
        DefaultItemClearanceClasses, FixedState, ItemClass, ItemId, ItemIdGenerator, Layer,
        LayerStructure, Net, NetClass, NetClassId, NetClasses, Nets, PadstackId, PadstackLookup,
        RoomId, TreeId, TreeObject, Unit, ViaInfo, ViaInfoId, ViaInfos, ViaRule, ViaRuleId,
    };
}
