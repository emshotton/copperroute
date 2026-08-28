//! The board library: padstacks, packages, logical parts, and the aggregate `BoardLibrary`.
//!
//! Java: `core/library/{Padstack,Padstacks,Package,Packages,LogicalPart,LogicalParts,
//! BoardLibrary}.java`.

pub mod board_library;
pub mod logical_part;
pub mod package;
pub mod padstack;

pub use board_library::{BoardLibrary, DrillItemPadstackLookup};
pub use logical_part::{LogicalPart, LogicalParts, PartPin};
pub use package::{Keepout, Package, PackagePin, Packages};
pub use padstack::{Padstack, Padstacks};
