//! The board's own outline.
//!
//! Java: `board/model/structure/BoardOutline.java`. `BoardOutline` extends `Item`, so
//! [`BoardOutline`] is one of the nine [`Item`](crate::items::Item) variants even though it
//! lives in this module — it is placed here to match its Java package, the way the Plan 2 file
//! layout asks.
//!
//! # The two keepout modes
//!
//! An outline obstructs the router in one of two ways, chosen by `keepoutOutsideOutline`
//! (BoardOutline.java:43, flipped by `generateKeepoutOutside`):
//!
//! * **off** (the default): only the outline *curves* are obstacles, as
//!   `BOARD_OUTLINE_HALF_WIDTH`-wide bands around each border line, so `tileShapeCount` is
//!   `lineCount() * layerCount`.
//! * **on**: the whole board area *outside* the outline is a keepout. Java builds it as
//!   `new PolylineArea(board.boundingBox, shapes)` — the outline polygons become the **holes**
//!   of the board box (BoardOutline.java:183-189) — and `tileShapeCount` is that area's convex
//!   pieces times the layer count.
//!
//! `board.boundingBox` and `board.layerStructure` are reached through [`ItemCtx`], not a
//! back-pointer; `ItemCtx::bounding_box` exists for this class alone.
//!
//! renamed: the protected `BoardOutline.calculateTreeShapes` (BoardOutline.java:259-262)
//! and the `ShapeSearchTree.calculateTreeShapes(BoardOutline)` body it delegates to
//! (ShapeSearchTree.java:940-988) — the same two-mode split as `tileShapeCount`, with the
//! clearance-compensation enlargement in the keepout branch and `Polyline.offsetShape(halfWidth
//! + cmpValue, 0)` over consecutive border-line triples in the line branch.

use std::sync::OnceLock;

use fr_geometry::{
    Area, FloatPoint, IntBox, IntPoint, PolylineArea, PolylineShapeRef, TileShape, Vector,
};

use crate::ids::ItemId;
use crate::items::header::ItemHeader;
use crate::items::{BOARD_OUTLINE_HALF_WIDTH, ItemCtx};
use crate::structure::FixedState;

/// Port of `BoardOutline` (`board/model/structure/BoardOutline.java`): the board's outline,
/// which is an `Item` so it can live in the search tree.
#[derive(Debug, Clone)]
pub struct BoardOutline {
    /// The `Item` base-class state (Item.java:41-67).
    pub hdr: ItemHeader,
    /// Java `private final PolylineShape[] shapes` (BoardOutline.java:30): the board shapes
    /// inside the outline curves.
    shapes: Vec<PolylineShapeRef>,
    /// Java `private Area keepoutArea` (BoardOutline.java:36): the board shape *outside* the
    /// outline curves, memoised on first use.
    ///
    /// A [`OnceLock`] rather than a `Cell`/`RefCell`, so that `getKeepoutArea` stays `&self` and
    /// [`Item`](crate::items::Item) stays `Send + Sync`. Java's four transforms rewrite it
    /// through `get_mut`, and nothing else touches it once filled — Java has no `keepoutArea =
    /// null` anywhere.
    keepout_area: OnceLock<Area>,
    /// Java `private TileShape[] keepoutLines` (BoardOutline.java:41), "used instead of
    /// keepoutArea if only the line shapes of the outlines are inserted as keepout".
    ///
    /// `None` is Java's `null`. In practice the field never holds anything else: the only writer
    /// is `getKeepoutLines` (BoardOutline.java:191-196), which stores an **empty** array, and
    /// the four transforms, which reset it to `null`. Kept so those five assignments have
    /// something to write.
    keepout_lines: Option<Vec<TileShape>>,
    /// The convex division of [`Self::get_keepout_area`], memoised.
    ///
    /// No Java field: Java memoises one level down, in `PolylineArea.precalculatedConvexPieces`
    /// (PolylineArea.java:31), and `keepoutArea` is a single long-lived `PolylineArea`, so
    /// `getKeepoutArea().splitToConvex()` divides once per outline. `fr-geometry` does not
    /// memoise there (quirk #30 made the divider's `Random` per-call), so this lock restores
    /// Java's amortised cost — `tileShapeCount`, `shapeLayer` and `calculateTreeShapes` all
    /// call it, the last once per layer. `Option` inside the lock is the failed division Java
    /// returns as `null`.
    // Plan-1 obligation: "memo cache for convex pieces" (docs/plan-1-handoff.md), discharged
    // here per the Task 7 ruling.
    keepout_convex_pieces: OnceLock<Option<Vec<TileShape>>>,
    /// Java `private boolean keepoutOutsideOutline` (BoardOutline.java:43); `false` until
    /// `generateKeepoutOutside(true)`.
    keepout_outside_outline: bool,
}

/// Compares the three real fields **and** the keepout-area memo.
///
/// The memo is not purely derived: Java's transforms rewrite it in place while leaving `shapes`
/// untouched (the `for`-loop bug, quirk #55), so two outlines with equal `shapes` can hold
/// genuinely different keepout areas and answer differently from `getKeepoutArea`.
impl PartialEq for BoardOutline {
    fn eq(&self, other: &BoardOutline) -> bool {
        self.hdr == other.hdr
            && self.shapes == other.shapes
            && self.keepout_outside_outline == other.keepout_outside_outline
            && self.keepout_area.get() == other.keepout_area.get()
            && self.keepout_lines == other.keepout_lines
    }
}

impl BoardOutline {
    /// Port of `BoardOutline(PolylineShape[], int, int, BasicBoard)` (BoardOutline.java:45-49).
    ///
    /// Java's constructor hard-codes `new int[0]`, component id `0` and
    /// `FixedState.SYSTEM_FIXED` (BoardOutline.java:47), so the caller must hand in a matching
    /// `hdr`.
    pub fn new(hdr: ItemHeader, shapes: Vec<PolylineShapeRef>) -> BoardOutline {
        BoardOutline {
            hdr,
            shapes,
            keepout_area: OnceLock::new(),
            keepout_lines: None,
            keepout_convex_pieces: OnceLock::new(),
            keepout_outside_outline: false,
        }
    }

    /// Port of `BoardOutline.copy` (BoardOutline.java:198-201).
    ///
    /// Java passes only `shapes` and the clearance class to the new outline, so the copy starts
    /// with `keepoutOutsideOutline == false` and an empty keepout cache however the original was
    /// set.
    pub fn copy(&self, new_id: ItemId) -> BoardOutline {
        BoardOutline::new(
            // BoardOutline.java:47: the constructor passes `new int[0]`, component id 0 and
            // SYSTEM_FIXED, keeping only the clearance class.
            ItemHeader::new(
                new_id,
                Vec::new(),
                self.hdr.clearance_class(),
                0,
                FixedState::SystemFixed,
            ),
            self.shapes.clone(),
        )
    }

    /// Port of `BoardOutline.getHalfWidth` (BoardOutline.java:254-257).
    pub fn get_half_width(&self) -> i32 {
        BOARD_OUTLINE_HALF_WIDTH
    }

    /// Port of `BoardOutline.lastLayer` (BoardOutline.java:102-105):
    /// `board.layerStructure.layers.length - 1`.
    pub fn last_layer(&self, ctx: &ItemCtx<'_>) -> usize {
        ctx.rules.layer_structure().count().checked_sub(1).expect(
            "BoardOutline.lastLayer: the board has no layers — Java answers -1 here \
                 (BoardOutline.java:104)",
        )
    }

    /// Port of `BoardOutline.tileShapeCount` (BoardOutline.java:51-66).
    pub fn tile_shape_count(&self, ctx: &ItemCtx<'_>) -> usize {
        let layer_count = ctx.rules.layer_structure().count();
        if self.keepout_outside_outline {
            // BoardOutline.java:54-61: a failed division answers 0, not `tiles * layers`.
            self.keepout_convex_pieces(ctx)
                .map_or(0, |tiles| tiles.len() * layer_count)
        } else {
            self.line_count() * layer_count
        }
    }

    /// Port of `BoardOutline.shapeLayer(int)` (BoardOutline.java:68-81).
    ///
    /// Java's out-of-range `FRLogger.warn` (BoardOutline.java:77-79) is a diagnostic only — it
    /// still returns the computed value — so it is dropped, as `global-constraints.md` requires.
    pub fn shape_layer(&self, index: usize, ctx: &ItemCtx<'_>) -> usize {
        let layer_count = ctx.rules.layer_structure().count();
        let shape_count = self.tile_shape_count(ctx);
        if shape_count > 0 {
            index * layer_count / shape_count
        } else {
            0
        }
    }

    /// Port of `BoardOutline.boundingBox` (BoardOutline.java:88-95): the union of the outline
    /// shapes' boxes, starting from `IntBox.EMPTY`.
    pub fn bounding_box(&self) -> IntBox {
        self.shapes.iter().fold(IntBox::EMPTY, |acc, shape| {
            acc.union(&shape.as_ops().bounding_box())
        })
    }

    /// Port of `BoardOutline.translateBy` (BoardOutline.java:112-121).
    //
    // Java bug: the loop body is `currentShape = currentShape.translateBy(vector);`, which assigns
    // to the **loop variable** of an enhanced `for` — Java binds a *copy of the reference* there,
    // so `this.shapes` is never written and the outline polygons do not move, turn, rotate or
    // mirror. The same defect is in all four transforms (BoardOutline.java:114-116, 125-127,
    // 137-139, 148-150).
    //
    // Only the lazily built `keepoutArea` followed the transform, because that one *is* a field
    // assignment. So after any of the four, the outline's curves and its outside-keepout
    // disagreed — and worse, they disagreed *depending on build order*, since the keepout is
    // rebuilt from the (untransformed) shapes if it had not been built yet. `boundingBox()`,
    // `lineCount()`, `getShape()` and the search-tree line bands all kept answering from the
    // untransformed shapes.
    //
    // fixed: T11 (#55) — Java's own suggested remedy: write back into the array, in all four.
    // Measured: no golden moved, because the four transforms are reachable only from
    // `BasicBoard.moveItems`/`changePlacementSide`, which the headless pipeline never calls.
    pub fn translate_by(&mut self, vector: &Vector) {
        for shape in &mut self.shapes {
            *shape = shape.translate_by(vector);
        }
        if let Some(keepout_area) = self.keepout_area.get_mut() {
            *keepout_area = keepout_area.translate_by(vector);
        }
        self.keepout_lines = None;
        self.keepout_convex_pieces.take();
    }

    /// Port of `BoardOutline.turn90Degree` (BoardOutline.java:123-132). See
    /// [`Self::translate_by`] for the `for`-loop bug — fixed: T11 (#55) here too.
    pub fn turn_90_degree(&mut self, factor: i32, pole: &IntPoint) {
        for shape in &mut self.shapes {
            *shape = shape.turn_90_degree(factor, pole);
        }
        if let Some(keepout_area) = self.keepout_area.get_mut() {
            *keepout_area = keepout_area.turn_90_degree(factor, pole);
        }
        self.keepout_lines = None;
        self.keepout_convex_pieces.take();
    }

    /// Port of `BoardOutline.rotateApprox` (BoardOutline.java:134-144). See
    /// [`Self::translate_by`] for the `for`-loop bug — fixed: T11 (#55) here too.
    pub fn rotate_approx(&mut self, angle_in_degree: f64, pole: &FloatPoint) {
        // BoardOutline.java:136 converts to radians once, before the loop.
        let angle = angle_in_degree.to_radians();
        for shape in &mut self.shapes {
            *shape = shape.rotate_approx(angle, pole);
        }
        if let Some(keepout_area) = self.keepout_area.get_mut() {
            *keepout_area = keepout_area.rotate_approx(angle, pole);
        }
        self.keepout_lines = None;
        self.keepout_convex_pieces.take();
    }

    /// Port of `BoardOutline.changePlacementSide` (BoardOutline.java:146-155). See
    /// [`Self::translate_by`] for the `for`-loop bug — fixed: T11 (#55) here too.
    pub fn change_placement_side(&mut self, pole: &IntPoint) {
        for shape in &mut self.shapes {
            *shape = shape.mirror_vertical(pole);
        }
        if let Some(keepout_area) = self.keepout_area.get_mut() {
            *keepout_area = keepout_area.mirror_vertical(pole);
        }
        self.keepout_lines = None;
        self.keepout_convex_pieces.take();
    }

    /// Port of `BoardOutline.shapeCount` (BoardOutline.java:157-160).
    pub fn shape_count(&self) -> usize {
        self.shapes.len()
    }

    /// Port of `BoardOutline.getShape(int)` (BoardOutline.java:162-169). Java's out-of-range
    /// warning path returns `null`.
    pub fn get_shape(&self, index: usize) -> Option<&PolylineShapeRef> {
        self.shapes.get(index)
    }

    /// Port of `BoardOutline.getKeepoutArea` (BoardOutline.java:179-189): the board area
    /// *outside* the outline curves, memoised in `keepoutArea`. Java builds
    /// `new PolylineArea(board.boundingBox, shapes.clone())`, so the outline curves become the
    /// holes of the returned area.
    ///
    /// `ctx.bounding_box` replaces Java's `board.boundingBox` (BoardOutline.java:186).
    /// The convex division of [`Self::get_keepout_area`], memoised (see
    /// [`Self::keepout_convex_pieces`]). Java writes
    /// `getKeepoutArea().splitToConvex()` at each of its three call sites
    /// (BoardOutline.java:54, ShapeSearchTree.java:946) and relies on `PolylineArea`'s own memo.
    pub fn keepout_convex_pieces(&self, ctx: &ItemCtx<'_>) -> Option<&[TileShape]> {
        self.keepout_convex_pieces
            .get_or_init(|| self.get_keepout_area(ctx).split_to_convex())
            .as_deref()
    }

    pub fn get_keepout_area(&self, ctx: &ItemCtx<'_>) -> &Area {
        self.keepout_area.get_or_init(|| {
            Area::Polyline(PolylineArea::new(
                PolylineShapeRef::Tile(TileShape::Box(*ctx.bounding_box)),
                self.shapes.clone(),
            ))
        })
    }

    /// Port of the package-private `BoardOutline.getKeepoutLines` (BoardOutline.java:191-196),
    /// which fills the field with an empty array on first use and never with anything else.
    ///
    /// Java's is package-private and has **no caller anywhere in the Java tree**; this one is
    /// `pub` because Rust has no package visibility and a `pub(crate)` method with no in-crate
    /// caller is dead code. The same call the four transforms make (`keepoutLines = null`) is
    /// reproduced there.
    pub fn get_keepout_lines(&mut self) -> &[TileShape] {
        self.keepout_lines.get_or_insert_with(Vec::new)
    }

    /// Port of `BoardOutline.keepoutOutsideOutlineGenerated` (BoardOutline.java:221-227).
    pub fn keepout_outside_outline_generated(&self) -> bool {
        self.keepout_outside_outline
    }

    /// Port of `BoardOutline.generateKeepoutOutside(boolean)` (BoardOutline.java:229-243): makes
    /// the area outside this outline a keepout, and re-inserts the outline into the search trees
    /// if the value changed.
    // renamed: the search-tree half (BoardOutline.java:238-242,
    // `board.searchTreeManager.remove(this)` + `insert(this)`) is
    // `Board::generate_keepout_outside(ItemId, bool)` — it needs the `SearchTreeManager`, which
    // only `Board` owns. Java's `board == null || searchTreeManager == null` guard
    // (BoardOutline.java:238-240) is exactly the "flag set, trees untouched" behaviour this
    // method has on its own.
    pub fn generate_keepout_outside(&mut self, value: bool) {
        if value == self.keepout_outside_outline {
            return;
        }
        self.keepout_outside_outline = value;
    }

    /// Port of `BoardOutline.lineCount` (BoardOutline.java:245-252): the sum of the border lines
    /// of all outline polygons.
    pub fn line_count(&self) -> usize {
        self.shapes
            .iter()
            .map(|shape| shape.as_ops().border_line_count())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_outline_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<BoardOutline>();
    }

    #[test]
    fn get_keepout_lines_is_always_empty() {
        // BoardOutline.java:191-196 stores `new TileShape[0]` and nothing ever replaces it.
        let mut outline = BoardOutline::new(
            ItemHeader::new(ItemId(1), Vec::new(), 0, 0, FixedState::SystemFixed),
            Vec::new(),
        );
        assert!(outline.get_keepout_lines().is_empty());
        outline.translate_by(&Vector::new(1, 1));
        assert!(outline.get_keepout_lines().is_empty());
    }
}
