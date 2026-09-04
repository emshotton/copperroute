use fr_board::Board;
use fr_geometry::{IntBox, TileShape};

use crate::Arena;
use crate::arena::PageId;
use crate::autoroute::drill::{DrillPage, ExpansionDrill};

#[derive(Debug, Clone, PartialEq)]
pub struct DrillPageArray {
        bounds: IntBox,
        column_count: i32,
        row_count: i32,
        page_width: i32,
        page_height: i32,
        pages: Vec<Vec<DrillPage>>,
}

impl DrillPageArray {
                                                                                                                pub fn new(
        board: &Board,
        max_page_width: i32,
        rooms: &mut crate::autoroute::expansion::ExpansionRoomStore,
    ) -> DrillPageArray {
        let bounds = board.bounding_box;
        let length = f64::from(bounds.ur.x.wrapping_sub(bounds.ll.x));
        let height = f64::from(bounds.ur.y.wrapping_sub(bounds.ll.y));
        let column_count = (length / f64::from(max_page_width)).ceil() as i32;
        let row_count = (height / f64::from(max_page_width)).ceil() as i32;
        let page_width = (length / f64::from(column_count)).ceil() as i32;
        let page_height = (height / f64::from(row_count)).ceil() as i32;

        let mut pages: Vec<Vec<DrillPage>> = Vec::with_capacity(row_count.max(0) as usize);
        for j in 0..row_count {
            let mut row = Vec::with_capacity(column_count.max(0) as usize);
            for i in 0..column_count {
                let ll_x = bounds.ll.x + i * page_width;
                let ur_x = if i == column_count - 1 {
                    bounds.ur.x
                } else {
                    ll_x + page_width
                };
                let ll_y = bounds.ll.y + j * page_height;
                let ur_y = if j == row_count - 1 {
                    bounds.ur.y
                } else {
                    ll_y + page_height
                };
                row.push(DrillPage::new(
                    IntBox::from_coords(ll_x, ll_y, ur_x, ur_y),
                    board,
                    rooms.next_room_id_no(),
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

                            pub fn invalidate(&mut self, shape: &TileShape, drills: &mut Arena<ExpansionDrill>) {
        for page in self.overlapping_pages(shape) {
            self.page_mut(page).invalidate(drills);
        }
    }

                                                                pub fn overlapping_pages(&self, shape: &TileShape) -> Vec<PageId> {
        let mut result = Vec::new();
        let shape_box = shape.bounding_box().intersection(&self.bounds);

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

        let mut j = min_j;
        while f64::from(j) < max_j {
            let mut i = min_i;
            while f64::from(i) < max_i {
                let page = self.page_id(i, j);
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

                        pub fn reset(&mut self, drills: &mut Arena<ExpansionDrill>) {
        for row in &mut self.pages {
            for page in row {
                page.reset(drills);
            }
        }
    }

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

                    pub fn page(&self, id: PageId) -> &DrillPage {
        let (i, j) = self.split(id);
        &self.pages[j][i]
    }

        pub fn page_mut(&mut self, id: PageId) -> &mut DrillPage {
        let (i, j) = self.split(id);
        &mut self.pages[j][i]
    }

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

        pub fn bounds(&self) -> IntBox {
        self.bounds
    }

        pub fn column_count(&self) -> i32 {
        self.column_count
    }

        pub fn row_count(&self) -> i32 {
        self.row_count
    }

        pub fn page_width(&self) -> i32 {
        self.page_width
    }

        pub fn page_height(&self) -> i32 {
        self.page_height
    }

                                            pub(crate) fn take_pages(&mut self) -> Vec<Vec<DrillPage>> {
        std::mem::take(&mut self.pages)
    }

        pub(crate) fn restore_pages(&mut self, pages: Vec<Vec<DrillPage>>) {
        self.pages = pages;
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flat_page_id_round_trips_through_the_grid() {
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
