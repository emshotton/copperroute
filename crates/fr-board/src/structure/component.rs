use std::cmp::Ordering;

use fr_geometry::{IntPoint, Point, Vector};

#[derive(Debug, Clone, PartialEq)]
pub struct Component {
        pub name: String,

                        pub id: i32,

        pub position_fixed: bool,

                lib_package_front: usize,

            lib_package_back: usize,

            part_number: Option<String>,

            location: Option<Point>,

            rotation_in_degree: f64,

            logical_part: Option<usize>,

        on_front: bool,
}

fn normalize_rotation(mut rotation_in_degree: f64) -> f64 {
    while rotation_in_degree >= 360.0 {
        rotation_in_degree -= 360.0;
    }
    while rotation_in_degree < 0.0 {
        rotation_in_degree += 360.0;
    }
    rotation_in_degree
}

impl Component {
                    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        name: impl Into<String>,
        location: Option<Point>,
        rotation_in_degree: f64,
        on_front: bool,
        package_front: usize,
        package_back: usize,
        id: i32,
        position_fixed: bool,
        part_number: Option<String>,
    ) -> Component {
        Component {
            name: name.into(),
            location,
            rotation_in_degree: normalize_rotation(rotation_in_degree),
            on_front,
            lib_package_front: package_front,
            lib_package_back: package_back,
            id,
            position_fixed,
            part_number,
            logical_part: None,
        }
    }

        pub fn get_location(&self) -> Option<&Point> {
        self.location.as_ref()
    }

        pub fn get_rotation_in_degree(&self) -> f64 {
        self.rotation_in_degree
    }

        pub fn is_placed(&self) -> bool {
        self.location.is_some()
    }

        pub fn placed_on_front(&self) -> bool {
        self.on_front
    }

        pub fn get_part_number(&self) -> Option<&str> {
        self.part_number.as_deref()
    }

            pub fn get_package(&self) -> usize {
        if self.on_front {
            self.lib_package_front
        } else {
            self.lib_package_back
        }
    }

        pub fn get_logical_part(&self) -> Option<usize> {
        self.logical_part
    }

        pub fn set_logical_part(&mut self, logical_part: Option<usize>) {
        self.logical_part = logical_part;
    }

            pub fn translate_by(&mut self, vector: &Vector) {
        if let Some(location) = &self.location {
            self.location = Some(location.translate_by(vector));
        }
    }

                    pub fn turn_90_degree(&mut self, factor: i32, pole: &IntPoint) {
        if factor == 0 {
            return;
        }
        self.rotation_in_degree =
            normalize_rotation(self.rotation_in_degree + f64::from(factor) * 90.0);
        if let Some(location) = &self.location {
            self.location = Some(location.turn_90_degree(factor, &Point::Int(*pole)));
        }
    }

                            pub fn rotate(&mut self, angle_in_degree: f64, pole: &IntPoint, flip_style_rotate_first: bool) {
        if angle_in_degree == 0.0 {
            return;
        }
        let mut turn_angle = angle_in_degree;
        if flip_style_rotate_first && !self.placed_on_front() {
            turn_angle = 360.0 - angle_in_degree;
        }
        self.rotation_in_degree = normalize_rotation(self.rotation_in_degree + turn_angle);
        if let Some(location) = &self.location {
            self.location = Some(Point::Int(
                location
                    .to_float()
                    .rotate(angle_in_degree.to_radians(), &pole.to_float())
                    .round(),
            ));
        }
    }

            pub fn change_side(&mut self, pole: &IntPoint) {
        if let Some(location) = &self.location {
            let mirrored = location.mirror_vertical(&Point::Int(*pole));
            self.on_front = !self.on_front;
            self.location = Some(mirrored);
        }
    }

                        pub fn compare_to(&self, other: &Component) -> Ordering {
        self.name.to_lowercase().cmp(&other.name.to_lowercase())
    }
}

impl std::fmt::Display for Component {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Components {
            components: Vec<Component>,

            flip_style_rotate_first: bool,
}

impl Components {
            pub fn new() -> Components {
        Components::default()
    }

                            #[allow(clippy::too_many_arguments)]
    pub fn add(
        &mut self,
        name: impl Into<String>,
        location: Option<Point>,
        rotation_in_degree: f64,
        on_front: bool,
        package_front: usize,
        package_back: usize,
        position_fixed: bool,
        part_number: Option<String>,
    ) -> &Component {
        let new_component = Component::new(
            name,
            location,
            rotation_in_degree,
            on_front,
            package_front,
            package_back,
            self.components.len() as i32 + 1,
            position_fixed,
            part_number,
        );
        self.components.push(new_component);
        self.components.last().expect("just pushed")
    }

                pub fn add_with_generated_name(
        &mut self,
        location: Option<Point>,
        rotation: f64,
        on_front: bool,
        component_package: usize,
    ) -> &Component {
        let component_name = format!("Component#{}", self.components.len() + 1);
        self.add(
            component_name,
            location,
            rotation,
            on_front,
            component_package,
            component_package,
            false,
            None,
        )
    }

            pub fn get_by_name(&self, name: &str) -> Option<&Component> {
        self.components.iter().find(|c| c.name == name)
    }

                                pub fn get(&self, component_id: i32) -> &Component {
        let index = usize::try_from(component_id - 1).unwrap_or_else(|_| {
            panic!(
                "Components.get({component_id}): Java's elementAt({}) throws \
                 ArrayIndexOutOfBoundsException (Components.java:89)",
                component_id - 1
            )
        });
        let result = &self.components[index];
        debug_assert_eq!(
            result.id, component_id,
            "Components.get: inconsistent component ID (Components.java:90-92)"
        );
        result
    }

        pub fn get_mut(&mut self, component_id: i32) -> &mut Component {
        let index = usize::try_from(component_id - 1).unwrap_or_else(|_| {
            panic!(
                "Components.get({component_id}): Java's elementAt({}) throws \
                 ArrayIndexOutOfBoundsException (Components.java:89)",
                component_id - 1
            )
        });
        &mut self.components[index]
    }

        pub fn count(&self) -> usize {
        self.components.len()
    }

        pub fn get_all(&self) -> std::slice::Iter<'_, Component> {
        self.components.iter()
    }

        pub fn move_component(&mut self, component_id: i32, vector: &Vector) {
        self.get_mut(component_id).translate_by(vector);
    }

        pub fn turn_90_degree(&mut self, component_id: i32, factor: i32, pole: &IntPoint) {
        self.get_mut(component_id).turn_90_degree(factor, pole);
    }

            pub fn rotate(&mut self, component_id: i32, rotation_in_degree: f64, pole: &IntPoint) {
        let flip_style_rotate_first = self.flip_style_rotate_first;
        self.get_mut(component_id)
            .rotate(rotation_in_degree, pole, flip_style_rotate_first);
    }

        pub fn change_side(&mut self, component_id: i32, pole: &IntPoint) {
        self.get_mut(component_id).change_side(pole);
    }

        pub fn get_flip_style_rotate_first(&self) -> bool {
        self.flip_style_rotate_first
    }

        pub fn set_flip_style_rotate_first(&mut self, value: bool) {
        self.flip_style_rotate_first = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fr_geometry::{IntVector, Point};

    fn point(x: i32, y: i32) -> Point {
        Point::new(x, y)
    }

    fn components() -> Components {
        let mut c = Components::new();
        c.add("R1", Some(point(10, 20)), 90.0, true, 1, 2, false, None);
        c.add(
            "C2",
            None,
            0.0,
            false,
            3,
            4,
            true,
            Some("100nF".to_string()),
        );
        c
    }


    #[test]
    fn add_assigns_consecutive_ids_from_one() {
        let c = components();
        assert_eq!(c.count(), 2);
        assert_eq!(c.get(1).name, "R1");
        assert_eq!(c.get(1).id, 1);
        assert_eq!(c.get(2).name, "C2");
        assert_eq!(c.get(2).id, 2);
    }

    #[test]
    fn add_stores_every_constructor_argument() {
        let c = components();
        let r1 = c.get(1);
        assert_eq!(r1.get_location(), Some(&point(10, 20)));
        assert_eq!(r1.get_rotation_in_degree(), 90.0);
        assert!(r1.placed_on_front());
        assert!(!r1.position_fixed);
        assert_eq!(r1.get_part_number(), None);
        let c2 = c.get(2);
        assert_eq!(c2.get_location(), None);
        assert!(!c2.placed_on_front());
        assert!(c2.position_fixed);
        assert_eq!(c2.get_part_number(), Some("100nF"));
    }

    #[test]
    fn get_by_name_is_case_sensitive() {
        let c = components();
        assert_eq!(c.get_by_name("C2").map(|x| x.id), Some(2));
        assert!(c.get_by_name("c2").is_none());
        assert!(c.get_by_name("nope").is_none());
    }

    #[test]
    #[should_panic(expected = "ArrayIndexOutOfBoundsException")]
    fn get_by_id_zero_panics_like_java_element_at() {
        components().get(0);
    }

    #[test]
    #[should_panic]
    fn get_by_id_past_the_end_panics_like_java_element_at() {
        components().get(3);
    }

    #[test]
    fn get_all_yields_the_components_in_id_order() {
        let c = components();
        assert_eq!(c.get_all().map(|x| x.id).collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn add_with_generated_name_numbers_from_one() {
        let mut c = Components::new();
        assert_eq!(
            c.add_with_generated_name(Some(point(0, 0)), 0.0, true, 5)
                .name,
            "Component#1"
        );
        assert_eq!(
            c.add_with_generated_name(None, 0.0, false, 5).name,
            "Component#2"
        );
        assert!(!c.get(1).position_fixed);
        assert_eq!(c.get(1).get_part_number(), None);
        assert_eq!(c.get(1).get_package(), 5);
        assert_eq!(c.get(2).get_package(), 5);
    }

    #[test]
    fn flip_style_rotate_first_round_trips_and_defaults_to_false() {
        let mut c = Components::new();
        assert!(!c.get_flip_style_rotate_first());
        c.set_flip_style_rotate_first(true);
        assert!(c.get_flip_style_rotate_first());
    }


    #[test]
    fn the_constructor_normalises_the_rotation_into_zero_to_360() {
        let mut c = Components::new();
        c.add("A", None, 450.0, true, 1, 1, false, None);
        c.add("B", None, -90.0, true, 1, 1, false, None);
        c.add("C", None, 360.0, true, 1, 1, false, None);
        assert_eq!(c.get(1).get_rotation_in_degree(), 90.0);
        assert_eq!(c.get(2).get_rotation_in_degree(), 270.0);
        assert_eq!(c.get(3).get_rotation_in_degree(), 0.0);
    }

    #[test]
    fn is_placed_follows_the_location() {
        let c = components();
        assert!(c.get(1).is_placed());
        assert!(!c.get(2).is_placed());
    }

    #[test]
    fn get_package_picks_the_side() {
        let c = components();
        assert_eq!(c.get(1).get_package(), 1); 
        assert_eq!(c.get(2).get_package(), 4); 
    }

    #[test]
    fn translate_by_moves_a_placed_component_and_spares_an_unplaced_one() {
        let mut c = components();
        let v = Vector::Int(IntVector::new(5, -5));
        c.move_component(1, &v);
        c.move_component(2, &v);
        assert_eq!(c.get(1).get_location(), Some(&point(15, 15)));
        assert_eq!(c.get(2).get_location(), None);
    }

    #[test]
    fn turn_90_degree_advances_the_rotation_and_moves_the_location() {
        let mut c = components();
        c.turn_90_degree(1, 1, &IntPoint::new(0, 0));
        assert_eq!(c.get(1).get_rotation_in_degree(), 180.0);
        assert_eq!(c.get(1).get_location(), Some(&point(-20, 10)));
    }

    #[test]
    fn turn_90_degree_by_zero_is_a_no_op() {
        let mut c = components();
        c.turn_90_degree(1, 0, &IntPoint::new(3, 3));
        assert_eq!(c.get(1).get_rotation_in_degree(), 90.0);
        assert_eq!(c.get(1).get_location(), Some(&point(10, 20)));
    }

    #[test]
    fn turn_90_degree_wraps_the_rotation() {
        let mut c = components();
        c.turn_90_degree(1, 3, &IntPoint::new(0, 0));
        assert_eq!(c.get(1).get_rotation_in_degree(), 0.0);
        c.turn_90_degree(1, -1, &IntPoint::new(0, 0));
        assert_eq!(c.get(1).get_rotation_in_degree(), 270.0);
    }

    #[test]
    fn rotate_by_zero_is_a_no_op() {
        let mut c = components();
        c.rotate(1, 0.0, &IntPoint::new(0, 0));
        assert_eq!(c.get(1).get_rotation_in_degree(), 90.0);
        assert_eq!(c.get(1).get_location(), Some(&point(10, 20)));
    }

    #[test]
    fn rotate_advances_the_rotation_and_moves_the_location() {
        let mut c = components();
        c.rotate(1, 90.0, &IntPoint::new(0, 0));
        assert_eq!(c.get(1).get_rotation_in_degree(), 180.0);
        assert_eq!(c.get(1).get_location(), Some(&point(-20, 10)));
    }

    #[test]
    fn rotate_on_the_back_side_with_flip_style_rotate_first_uses_the_complement() {
        let mut c = Components::new();
        c.add("B1", Some(point(10, 0)), 0.0, false, 1, 1, false, None);
        c.set_flip_style_rotate_first(true);
        c.rotate(1, 90.0, &IntPoint::new(0, 0));
        assert_eq!(c.get(1).get_rotation_in_degree(), 270.0);
        assert_eq!(c.get(1).get_location(), Some(&point(0, 10)));
    }

    #[test]
    fn rotate_on_the_front_side_ignores_flip_style_rotate_first() {
        let mut c = components();
        c.set_flip_style_rotate_first(true);
        c.rotate(1, 90.0, &IntPoint::new(0, 0));
        assert_eq!(c.get(1).get_rotation_in_degree(), 180.0);
    }

    #[test]
    fn change_side_flips_and_mirrors() {
        let mut c = components();
        c.change_side(1, &IntPoint::new(0, 0));
        assert!(!c.get(1).placed_on_front());
        assert_eq!(c.get(1).get_location(), Some(&point(-10, 20)));
        assert_eq!(c.get(1).get_package(), 2); 
    }

                #[test]
    fn change_side_on_an_unplaced_component_is_guarded() {
        let mut c = components();
        let before_side = c.get(2).placed_on_front();
        assert_eq!(c.get(2).get_location(), None, "component 2 is unplaced");

        c.change_side(2, &IntPoint::new(0, 0));

        assert_eq!(c.get(2).get_location(), None);
        assert_eq!(c.get(2).placed_on_front(), before_side);
    }

    #[test]
    fn compare_to_orders_by_name_ignoring_case() {
        let c = components();
        assert_eq!(c.get(2).compare_to(c.get(1)), Ordering::Less); 
        assert_eq!(c.get(1).compare_to(c.get(1)), Ordering::Equal);
        let mut other = Components::new();
        other.add("r1", None, 0.0, true, 1, 1, false, None);
        assert_eq!(c.get(1).compare_to(other.get(1)), Ordering::Equal);
    }

    #[test]
    fn logical_part_round_trips() {
        let mut c = components();
        assert_eq!(c.get(1).get_logical_part(), None);
        c.get_mut(1).set_logical_part(Some(7));
        assert_eq!(c.get(1).get_logical_part(), Some(7));
    }

    #[test]
    fn clone_carries_the_logical_part() {
        let mut c = components();
        c.get_mut(1).set_logical_part(Some(4));
        let dup = c.get(1).clone();
        assert_eq!(dup.get_logical_part(), Some(4));
        assert_eq!(dup.id, 1);
        assert_eq!(dup.name, "R1");
    }

    #[test]
    fn display_is_the_name() {
        assert_eq!(components().get(1).to_string(), "R1");
    }
}
