//! Port of `autoroute.drill.DrillPageArray` (DrillPageArray.java:15-120) — "the 2 dimensional
//! array of pages of `ExpansionDrill`s used in the maze search algorithm. The pages are
//! rectangles of about equal width and height covering the bounding box of the board area."

use fr_board::Board;
use fr_geometry::{IntBox, TileShape};

use crate::Arena;
use crate::arena::PageId;
use crate::autoroute::drill::{DrillPage, ExpansionDrill};

/// Port of `DrillPageArray` (DrillPageArray.java:15-120).
///
/// # Addressing
///
/// Java addresses a page by reference: `pages[j][i]`, and the objects themselves are the
/// identities the maze search stores. The port addresses one by [`PageId`], the **row-major flat
/// index** `j * columnCount + i` — the same identity, because the grid is built once in the
/// constructor and never resized. [`page_id`](Self::page_id) is the `(i, j)` → `PageId` map, in
/// Java's `(column, row)` argument order.
#[derive(Debug, Clone, PartialEq)]
pub struct DrillPageArray {
    /// `private final IntBox bounds` (`:17`) — `board.boundingBox` (`:35`).
    bounds: IntBox,
    /// `private final int columnCount` (`:20`).
    column_count: i32,
    /// `private final int rowCount` (`:23`).
    row_count: i32,
    /// `private final int pageWidth` (`:26`).
    page_width: i32,
    /// `private final int pageHeight` (`:29`).
    page_height: i32,
    /// `private final DrillPage[][] pages` (`:31`), `[rowCount][columnCount]`.
    pages: Vec<Vec<DrillPage>>,
}

impl DrillPageArray {
    /// Port of `DrillPageArray(RoutingBoard, int)` (DrillPageArray.java:33-62).
    ///
    /// # The `ceil` chain, transcribed rather than tidied
    ///
    /// ```java
    /// double length = bounds.ur.x - bounds.ll.x;          // int subtraction, then widened
    /// double height = bounds.ur.y - bounds.ll.y;
    /// this.columnCount = (int) Math.ceil(length / maxPageWidth);
    /// this.rowCount    = (int) Math.ceil(height / maxPageWidth);
    /// this.pageWidth   = (int) Math.ceil(length / columnCount);
    /// this.pageHeight  = (int) Math.ceil(height / rowCount);
    /// ```
    ///
    /// Three details the port keeps:
    ///
    /// * the subtraction is **`int`** arithmetic and only then widened, so a bounding box wider
    ///   than `i32::MAX` wraps before the division ever sees it — `wrapping_sub` reproduces that;
    /// * `pageWidth` is **recomputed from `columnCount`**, not reused from `maxPageWidth`, so a
    ///   20 000-unit board over 7 000-unit pages gets three 6 667-wide columns rather than two of
    ///   7 000 and one of 6 000;
    /// * Java's `(int)` cast of a `double` truncates toward zero, maps `NaN` to 0 and saturates
    ///   at the `int` bounds — which is exactly Rust's `as i32`, so a degenerate zero-width board
    ///   gives `columnCount = 0`, `length / 0` = `NaN`, `pageWidth = 0` and an empty grid on both
    ///   sides.
    pub fn new(board: &Board, max_page_width: i32) -> DrillPageArray {
        // :35-37.
        let bounds = board.bounding_box;
        let length = f64::from(bounds.ur.x.wrapping_sub(bounds.ll.x));
        let height = f64::from(bounds.ur.y.wrapping_sub(bounds.ll.y));
        // :38-41.
        let column_count = (length / f64::from(max_page_width)).ceil() as i32;
        let row_count = (height / f64::from(max_page_width)).ceil() as i32;
        let page_width = (length / f64::from(column_count)).ceil() as i32;
        let page_height = (height / f64::from(row_count)).ceil() as i32;

        // :42-61.
        let mut pages: Vec<Vec<DrillPage>> = Vec::with_capacity(row_count.max(0) as usize);
        for j in 0..row_count {
            let mut row = Vec::with_capacity(column_count.max(0) as usize);
            for i in 0..column_count {
                // :45-51.
                let ll_x = bounds.ll.x + i * page_width;
                let ur_x = if i == column_count - 1 {
                    bounds.ur.x
                } else {
                    ll_x + page_width
                };
                // :52-58.
                let ll_y = bounds.ll.y + j * page_height;
                let ur_y = if j == row_count - 1 {
                    bounds.ur.y
                } else {
                    ll_y + page_height
                };
                // :59.
                row.push(DrillPage::new(
                    IntBox::from_coords(ll_x, ll_y, ur_x, ur_y),
                    board,
                ));
            }
            pages.push(row);
        }

        DrillPageArray {
            bounds,
            column_count,
            row_count,
            page_width,
            page_height,
            pages,
        }
    }

    /// Port of `invalidate(TileShape)` (DrillPageArray.java:64-73): "invalidates all drill pages
    /// intersecting with shape so they must be recalculated at the next call of `getDrills()`."
    pub fn invalidate(&mut self, shape: &TileShape) {
        // :69-72.
        for page in self.overlapping_pages(shape) {
            self.page_mut(page).invalidate();
        }
    }

    /// Port of `overlappingPages(TileShape)` (DrillPageArray.java:75-97): "collects all drill
    /// pages with a 2-dimensional overlap with shape."
    ///
    /// # The mixed-type loop bounds (`:81-88`)
    ///
    /// `minJ`/`minI` are `int`s from `Math.floor`, but `maxJ`/`maxI` are left as **`double`s**
    /// and the loops are `for (int j = minJ; j < maxJ; j++)` — so `j` is widened per comparison.
    /// Truncating `maxJ` to an `int` loses the page the fractional bound falls in; taking its
    /// ceiling happens to agree, but only by accident of `j` being an integer, so the port keeps
    /// the `f64` comparison Java wrote. `overlapping_pages_uses_javas_mixed_loop_bounds`
    /// (`crates/fr-router/tests/drill.rs`) is the test that fails if this is "cleaned up".
    ///
    /// The subtractions at `:81-85` are again `int` arithmetic cast to `double` afterwards, and
    /// `shapeBox` is the shape's bounding box **intersected with the board's** (`:79`), so a
    /// shape outside the board yields `IntBox::EMPTY` and an inverted range that runs zero times.
    pub fn overlapping_pages(&self, shape: &TileShape) -> Vec<PageId> {
        let mut result = Vec::new();
        // :79.
        let shape_box = shape.bounding_box().intersection(&self.bounds);

        // :81-85.
        let min_j = (f64::from(shape_box.ll.y.wrapping_sub(self.bounds.ll.y))
            / f64::from(self.page_height))
        .floor() as i32;
        let max_j =
            f64::from(shape_box.ur.y.wrapping_sub(self.bounds.ll.y)) / f64::from(self.page_height);
        let min_i = (f64::from(shape_box.ll.x.wrapping_sub(self.bounds.ll.x))
            / f64::from(self.page_width))
        .floor() as i32;
        let max_i =
            f64::from(shape_box.ur.x.wrapping_sub(self.bounds.ll.x)) / f64::from(self.page_width);

        // :87-95. `pages[j][i]` is unchecked in Java; the bounds hold because `pageHeight` is
        // `ceil(height / rowCount)`, so `height / pageHeight <= rowCount`.
        let mut j = min_j;
        while f64::from(j) < max_j {
            let mut i = min_i;
            while f64::from(i) < max_i {
                let page = self.page_id(i, j);
                // :90-93.
                let intersection = shape.intersection(&self.page(page).get_shape());
                if intersection.dimension() > 1 {
                    result.push(page);
                }
                i += 1;
            }
            j += 1;
        }
        result
    }

    /// Port of `reset()` (DrillPageArray.java:99-107): "resets all drill pages for autorouting
    /// the next connection."
    ///
    /// The maze scratch only — every page keeps its memoised drill list, exactly as
    /// [`DrillPage::reset`] does. Takes the drill arena because the drills live there.
    pub fn reset(&mut self, drills: &mut Arena<ExpansionDrill>) {
        for row in &mut self.pages {
            for page in row {
                page.reset(drills);
            }
        }
    }

    /// The [`PageId`] of the page in column `i`, row `j` — Java's `pages[j][i]`, in Java's
    /// argument order.
    ///
    /// # Panics
    /// If either index is outside the grid. Java's array access throws
    /// `ArrayIndexOutOfBoundsException`.
    pub fn page_id(&self, i: i32, j: i32) -> PageId {
        assert!(
            i >= 0 && i < self.column_count && j >= 0 && j < self.row_count,
            "DrillPageArray: pages[{j}][{i}] is outside a {}x{} grid — Java throws \
             ArrayIndexOutOfBoundsException",
            self.row_count,
            self.column_count
        );
        PageId((j * self.column_count + i) as u32)
    }

    /// The page at a [`PageId`].
    ///
    /// # Panics
    /// On an id from a different array, for the reason [`page_id`](Self::page_id) gives.
    pub fn page(&self, id: PageId) -> &DrillPage {
        let (i, j) = self.split(id);
        &self.pages[j][i]
    }

    /// [`page`](Self::page), mutably.
    pub fn page_mut(&mut self, id: PageId) -> &mut DrillPage {
        let (i, j) = self.split(id);
        &mut self.pages[j][i]
    }

    /// The `(column, row)` a [`PageId`] names — the inverse of [`page_id`](Self::page_id).
    pub(crate) fn split(&self, id: PageId) -> (usize, usize) {
        assert!(
            self.column_count > 0,
            "DrillPageArray: an empty grid has no page {id:?}"
        );
        let flat = id.0 as i32;
        (
            (flat % self.column_count) as usize,
            (flat / self.column_count) as usize,
        )
    }

    /// `bounds` (`:17`) — the board's bounding box the grid covers.
    pub fn bounds(&self) -> IntBox {
        self.bounds
    }

    /// `columnCount` (`:20`).
    pub fn column_count(&self) -> i32 {
        self.column_count
    }

    /// `rowCount` (`:23`).
    pub fn row_count(&self) -> i32 {
        self.row_count
    }

    /// `pageWidth` (`:26`).
    pub fn page_width(&self) -> i32 {
        self.page_width
    }

    /// `pageHeight` (`:29`).
    pub fn page_height(&self) -> i32 {
        self.page_height
    }

    /// Takes the grid out, leaving an array with no pages.
    ///
    /// This is half of the borrow bridge [`AutorouteEngine::drill_page_drills`]:
    /// `DrillPage.getDrills(AutorouteEngine, boolean)` is a method on an object the engine owns
    /// that takes the engine, which no pair of Rust borrows can express. Pair every call with
    /// [`restore_pages`](Self::restore_pages) — including on an unwind, or the engine is left
    /// with a grid whose `columnCount` no longer matches its pages.
    ///
    /// [`AutorouteEngine::drill_page_drills`]:
    ///     crate::autoroute::maze::AutorouteEngine::drill_page_drills
    pub(crate) fn take_pages(&mut self) -> Vec<Vec<DrillPage>> {
        std::mem::take(&mut self.pages)
    }

    /// Puts the grid back — see [`take_pages`](Self::take_pages).
    pub(crate) fn restore_pages(&mut self, pages: Vec<Vec<DrillPage>>) {
        self.pages = pages;
    }
}

// =================================================================================================
// The deferral roster for `autoroute/drill/DrillPageArray.java`
// =================================================================================================

// not ported: `DrillPageArray.emitDiagnostics` — it feeds `AutorouteDiagnostic.Sink`, a GUI
// overlay (`global-constraints.md`: no GUI, no observers), and no routing decision reads it.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flat_page_id_round_trips_through_the_grid() {
        // A 3x2 grid, built by hand so the test needs no board.
        let array = DrillPageArray {
            bounds: IntBox::from_coords(0, 0, 30, 20),
            column_count: 3,
            row_count: 2,
            page_width: 10,
            page_height: 10,
            pages: Vec::new(),
        };
        assert_eq!(array.page_id(0, 0), PageId(0));
        assert_eq!(array.page_id(2, 0), PageId(2));
        assert_eq!(array.page_id(0, 1), PageId(3));
        assert_eq!(array.page_id(2, 1), PageId(5));
        for j in 0..2 {
            for i in 0..3 {
                assert_eq!(array.split(array.page_id(i, j)), (i as usize, j as usize));
            }
        }
    }

    #[test]
    #[should_panic(expected = "outside a 2x3 grid")]
    fn an_out_of_range_index_panics_like_javas_array_access() {
        let array = DrillPageArray {
            bounds: IntBox::from_coords(0, 0, 30, 20),
            column_count: 3,
            row_count: 2,
            page_width: 10,
            page_height: 10,
            pages: Vec::new(),
        };
        array.page_id(3, 0);
    }
}
