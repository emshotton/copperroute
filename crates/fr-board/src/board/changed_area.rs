use fr_geometry::{FloatPoint, IntBox, IntOctagon, TileShape};

#[derive(Debug, Clone, PartialEq)]
pub struct ChangedArea {
    layer_count: usize,
    arr: Vec<MutableOctagon>,
}

impl ChangedArea {
    pub fn new(layer_count: usize) -> ChangedArea {
        ChangedArea {
            layer_count,
            arr: vec![MutableOctagon::empty(); layer_count],
        }
    }

    pub fn layer_count(&self) -> usize {
        self.layer_count
    }

    pub fn join(&mut self, point: &FloatPoint, layer: usize) {
        let current = &mut self.arr[layer];
        current.lx = current.lx.min(point.x);
        current.ly = current.ly.min(point.y);
        current.rx = current.rx.max(point.x);
        current.uy = current.uy.max(point.y);

        let tmp = point.x - point.y;
        current.ulx = current.ulx.min(tmp);
        current.lrx = current.lrx.max(tmp);

        let tmp = point.x + point.y;
        current.llx = current.llx.min(tmp);
        current.urx = current.urx.max(tmp);
    }

    pub fn join_shape(&mut self, shape: &TileShape, layer: usize) {
        for i in 0..shape.border_line_count() {
            if let Some(corner) = shape.corner_approx(i) {
                self.join(&corner, layer);
            }
        }
    }

    pub fn get_area(&self, layer: usize) -> IntOctagon {
        self.arr[layer].to_int()
    }

    pub fn surrounding_box(&self) -> IntBox {
        let mut llx = i32::MAX;
        let mut lly = i32::MAX;
        let mut urx = i32::MIN;
        let mut ury = i32::MIN;
        for current in &self.arr {
            llx = llx.min(floor_to_i32(current.lx));
            lly = lly.min(floor_to_i32(current.ly));
            urx = urx.max(ceil_to_i32(current.rx));
            ury = ury.max(ceil_to_i32(current.uy));
        }
        if llx > urx || lly > ury {
            return IntBox::EMPTY;
        }
        IntBox::from_coords(llx, lly, urx, ury)
    }

    pub fn set_empty(&mut self, layer: usize) {
        self.arr[layer] = MutableOctagon::empty();
    }
}

fn floor_to_i32(value: f64) -> i32 {
    value.floor() as i32
}

fn ceil_to_i32(value: f64) -> i32 {
    value.ceil() as i32
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MutableOctagon {
    lx: f64,
    ly: f64,
    rx: f64,
    uy: f64,
    ulx: f64,
    lrx: f64,
    llx: f64,
    urx: f64,
}

impl MutableOctagon {
    fn empty() -> MutableOctagon {
        MutableOctagon {
            lx: f64::from(i32::MAX),
            ly: f64::from(i32::MAX),
            rx: f64::from(i32::MIN),
            uy: f64::from(i32::MIN),
            ulx: f64::from(i32::MAX),
            lrx: f64::from(i32::MIN),
            llx: f64::from(i32::MAX),
            urx: f64::from(i32::MIN),
        }
    }

    fn to_int(self) -> IntOctagon {
        if self.rx < self.lx || self.uy < self.ly || self.lrx < self.ulx || self.urx < self.llx {
            return IntOctagon::EMPTY;
        }
        IntOctagon::new(
            floor_to_i32(self.lx),
            floor_to_i32(self.ly),
            ceil_to_i32(self.rx),
            ceil_to_i32(self.uy),
            floor_to_i32(self.ulx),
            ceil_to_i32(self.lrx),
            floor_to_i32(self.llx),
            ceil_to_i32(self.urx),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_area_is_empty_on_every_layer() {
        let area = ChangedArea::new(3);
        for layer in 0..3 {
            assert_eq!(area.get_area(layer), IntOctagon::EMPTY);
        }
        assert_eq!(area.surrounding_box(), IntBox::EMPTY);
    }

    #[test]
    fn joining_one_point_makes_a_degenerate_octagon_around_it() {
        let mut area = ChangedArea::new(2);
        area.join(&FloatPoint::new(100.0, 40.0), 0);
        assert_eq!(
            area.get_area(0),
            IntOctagon::new(100, 40, 100, 40, 60, 60, 140, 140)
        );
        assert_eq!(area.get_area(1), IntOctagon::EMPTY);
    }

    #[test]
    fn joining_rounds_outward() {
        let mut area = ChangedArea::new(1);
        area.join(&FloatPoint::new(10.5, -3.25), 0);
        assert_eq!(
            area.get_area(0),
            IntOctagon::new(10, -4, 11, -3, 13, 14, 7, 8)
        );
    }

    #[test]
    fn joining_a_shape_visits_every_corner() {
        let mut area = ChangedArea::new(1);
        area.join_shape(&TileShape::Box(IntBox::from_coords(-10, -20, 30, 40)), 0);
        assert_eq!(
            area.get_area(0),
            IntOctagon::new(-10, -20, 30, 40, -50, 50, -30, 70)
        );
    }

    #[test]
    fn surrounding_box_unions_every_layer() {
        let mut area = ChangedArea::new(2);
        area.join(&FloatPoint::new(0.0, 0.0), 0);
        area.join(&FloatPoint::new(500.0, -200.0), 1);
        assert_eq!(area.surrounding_box(), IntBox::from_coords(0, -200, 500, 0));
    }

    #[test]
    fn set_empty_clears_one_layer_only() {
        let mut area = ChangedArea::new(2);
        area.join(&FloatPoint::new(1.0, 1.0), 0);
        area.join(&FloatPoint::new(2.0, 2.0), 1);
        area.set_empty(0);
        assert_eq!(area.get_area(0), IntOctagon::EMPTY);
        assert_ne!(area.get_area(1), IntOctagon::EMPTY);
    }
}
