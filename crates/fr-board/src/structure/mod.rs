//! Board structural model: layers, the layer stack, the small structural enums that describe
//! them, and the board's components.
//!
//! Java: `board/model/structure/*.java`. `BoardOutline` is an `Item` — one of the nine
//! [`crate::items::Item`] variants — but it lives here, in [`board_outline`], to match its Java
//! package; [`crate::items`] re-exports it.

pub mod board_outline;
pub mod component;
pub mod layer;
pub mod shape_entry_side;

pub use board_outline::BoardOutline;
pub use component::{Component, Components};
pub use layer::{AngleRestriction, FixedState, Layer, LayerStructure, Unit};
pub use shape_entry_side::{ShapeAndEntrySide, ShapeEntrySide, free_trace_tree_shapes};
