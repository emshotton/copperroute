//! Board structural model: layers, the layer stack, the small structural enums that describe
//! them, and the board's components.
//!
//! Java: `board/model/structure/*.java`. `BoardOutline` is an `Item`, so it lives in
//! [`crate::items`] with the other eight item variants; `ShapeEntrySide` and
//! `ShapeAndEntrySide` land in later Plan 2 tasks (see the crate-level file structure in the
//! plan).

pub mod component;
pub mod layer;

pub use component::{Component, Components};
pub use layer::{AngleRestriction, FixedState, Layer, LayerStructure, Unit};
