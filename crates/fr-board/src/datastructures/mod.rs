//! The generic data structures the board model and the router are built on.
//!
//! Java: `datastructures/{ShapeTree,MinAreaTree,TimeLimit,Stoppable,ArrayStack,
//! PlanarDelaunayTriangulation}.java`.
//!
//! Java splits the search tree into an abstract `ShapeTree` and its single concrete subclass
//! `MinAreaTree`; `MinAreaTree` is the only implementation in the tree (`ShapeSearchTree`
//! extends it), so the port merges the pair into one struct, [`ShapeTree`], and marks the
//! Java-side abstraction points in the source.
//!
//! A handful of `datastructures/*.java` classes are out of this plan's scope entirely. Each
//! marker line below is kept whole (method name and marker on the same line) so
//! `scripts/audit-port.sh`'s per-line grep finds it:
//!
//! not ported: lives in fr-geometry: `BigIntAux.determinant` (`datastructures/BigIntAux.java`).
//! not ported: lives in fr-geometry: `BigIntAux.addRationalCoordinates`.
//! not ported: lives in fr-geometry: `BigIntAux.binaryGcd`. All three are plain arithmetic
//! helpers with no board dependency, ported alongside the geometry types that use them as
//! `fr_geometry::bigint_aux::{determinant, add_rational_coordinates, binary_gcd}` (Plan 1).
//!
//! not ported: lives in fr-geometry: `Signum.asInt` (`datastructures/Signum.java`), ported as
//! `fr_geometry::Signum::as_int_i64`/`as_int_f64` (the two overloads, suffixed the same way as
//! the rest of `fr-geometry`'s naming scheme).
//! not ported: lives in fr-geometry: `Signum.negate`, ported as `fr_geometry::Signum::negate`.
//! not ported: lives in fr-geometry: `Signum.of` (Signum.java:16-25), ported as the two overloads `fr_geometry::Signum::of_i64`/`of_f64`; and `Signum.toString` (Signum.java:32-35), which is the enum constant's name and is `Debug` here.
//!
//! added in Plan 3: `IndentFileWriter.startScope` (`datastructures/IndentFileWriter.java`).
//! added in Plan 3: `IndentFileWriter.endScope`.
//! added in Plan 3: `IndentFileWriter.newLine`.
//! added in Plan 3: `IdentifierType.write` (`datastructures/IdentifierType.java`). All four are
//! SES-file-writer helpers with no board-model dependency; Plan 3 owns the `specctra`/`io` I/O
//! layer that calls them.
//!
//! `UndoableObjects` (`datastructures/UndoableObjects.java`), Java's undo-stack-aware sorted
//! object list, is replaced by the plain `BTreeMap<ItemId, Item>` on [`crate::Board::items`]
//! (plan-rulings.md #1) plus whole-board `Board::clone`/`Board::deep_copy` (`board/snapshot.rs`)
//! standing in for snapshot/undo/redo — see the `not ported:` note on `Board`'s doc comment for
//! the stack itself. Its per-object accessors become plain `BTreeMap` operations:
//!
//! renamed: `UndoableObjects.insert` (:66) -> `BTreeMap::insert` (`Board::items`, keyed by
//! `ItemId` rather than found by `Storable` identity).
//! renamed: `UndoableObjects.delete` (:76) -> `BTreeMap::remove`.
//! renamed: `UndoableObjects.startReadObject` (:46) -> the `BTreeMap`'s own iteration
//! (`Board::items_in_board_order`, `.values()`/`.values_mut()`).
//! renamed: `UndoableObjects.readObject` (:54) -> the same iteration; there are no undo levels
//! to filter by (Java's `currentNode.level <= this.stackLevel` guard), since there is no undo
//! stack in this port.
//! not ported: interactive undo: `UndoableObjects.generateSnapshot` (:131).
//! not ported: interactive undo: `UndoableObjects.undo` (:143).
//! not ported: interactive undo: `UndoableObjects.redo` (:183).
//! not ported: interactive undo: `UndoableObjects.popSnapshot` (:232).
//! not ported: interactive undo: `UndoableObjects.saveForUndo` (:273) — the undo stack itself;
//! see `Board`'s doc comment.

pub mod delaunay;
pub mod shape_tree;
pub mod time_limit;

pub use delaunay::{DelaunayCorner, DelaunayEdge, PlanarDelaunayTriangulation};
pub use shape_tree::{LeafId, Node, NodeId, ShapeTree, TreeEntry};
pub use time_limit::{StopCheck, TimeLimit};
