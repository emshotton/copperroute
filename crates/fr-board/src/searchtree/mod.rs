//! The board's spatial index: the search trees the router queries.
//!
//! Java: `board/searchtree/{ShapeSearchTree,ShapeSearchTree45Degree,ShapeSearchTree90Degree,
//! SearchTreeManager}.java`.
//!
//! # Shape of the port
//!
//! Java has one concrete tree class plus two subclasses that differ only in *which shapes they
//! store*: `ShapeSearchTree45Degree` forces every stored shape to an `IntOctagon`,
//! `ShapeSearchTree90Degree` to an `IntBox`, and the base class stores whatever
//! `boundingTile()`/the board's angle restriction produces. Every override is a
//! `calculateTreeShapes` variant or an `offsetShape`/`offsetShapes` variant — except
//! `completeShape`, which is the autoroute expansion-room algorithm and belongs to Plan 6.
//!
//! The port collapses the three classes into one [`ShapeSearchTree`] parameterised by
//! [`AngleRestriction`](crate::structure::AngleRestriction), which stands for *which subclass
//! this tree is*:
//!
//! | Java class | `angle` | bounding directions | drill / area / outline shapes | trace shapes |
//! |---|---|---|---|---|
//! | `ShapeSearchTree` | `AngleRestriction::None` | `FortyfiveDegree` | `boundingTile`/`boundingBox`/`boundingOctagon` per `rules.traceAngleRestriction`, then `enlarge` | `Polyline.offsetShape` |
//! | `ShapeSearchTree45Degree` | `AngleRestriction::FortyFiveDegree` | `FortyfiveDegree` | forced to `IntOctagon`, `offset` not `enlarge` | `Polyline.offsetShape` |
//! | `ShapeSearchTree90Degree` | `AngleRestriction::NinetyDegree` | `Orthogonal` | forced to `IntBox` | `Polyline.offsetBox` |
//!
//! Note the two angles in play. A tree's own `angle` is fixed when
//! [`SearchTreeManager::get_autoroute_tree`] builds it from `rules.trace_angle_restriction`
//! (SearchTreeManager.java:149-160) — but the **default** tree is always the base class
//! (SearchTreeManager.java:33), whatever the board's angle restriction is, and the base class's
//! drill-item body reads `board.rules.getTraceAngleRestriction()` *again* at shape-calculation
//! time (ShapeSearchTree.java:885-892). So [`ShapeSearchTree::calculate_tree_shapes`] consults
//! both: `self.angle` for which subclass body to run, and `rules.trace_angle_restriction` inside
//! the base-class body.
//!
//! # What replaces Java's `board` back-pointer
//!
//! `ShapeSearchTree.board` (ShapeSearchTree.java:64) is read for `board.rules`,
//! `board.library`, `board.components`, `board.layerStructure`, `board.communication`,
//! `board.getBoundingBox()` and `board.itemList`. The first six travel in the
//! [`ItemCtx`](crate::items::ItemCtx) that Tasks 6-8 already thread through the item bodies; the
//! item list is passed explicitly to the query methods as an [`ItemLookup`], because only they
//! need it.
//!
//! `board == null` is unrepresentable here, so the four `if (this.board == null) return new
//! TileShape[0];` guards (ShapeSearchTree.java:872, 909, 941, 993) are not ported: Java reaches
//! them only from a tree built for a null board, which `SearchTreeManager`'s constructor cannot
//! produce.

pub mod manager;
pub mod shape_search_tree;

pub use manager::SearchTreeManager;
pub use shape_search_tree::{ItemLookup, NoRooms, RoomLookup, ShapeSearchTree};
