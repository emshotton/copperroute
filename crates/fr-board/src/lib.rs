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
pub mod structure;

pub use error::BoardError;
pub use ids::{ItemId, ItemIdGenerator, RoomId, TreeId, TreeObject};
pub use structure::{AngleRestriction, FixedState, Layer, LayerStructure, Unit};

/// Re-exports every public type of the crate, for `use fr_board::prelude::*;`.
pub mod prelude {
    pub use crate::{
        AngleRestriction, BoardError, FixedState, ItemId, ItemIdGenerator, Layer, LayerStructure,
        RoomId, TreeId, TreeObject, Unit,
    };
}
