//! Board structural model: layers, the layer stack, and the small structural enums that
//! describe them.
//!
//! Java: `board/model/structure/*.java`. `Component`, `Components`, `BoardOutline`,
//! `ShapeEntrySide`, and `ShapeAndEntrySide` are out of scope for this task; they land in
//! later Plan 2 tasks (see the crate-level file structure in the plan).

pub mod layer;

pub use layer::{AngleRestriction, FixedState, Layer, LayerStructure, Unit};
