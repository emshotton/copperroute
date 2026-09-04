use std::cmp::Ordering;
use std::fmt;

use fr_geometry::{Area, Shape, Vector};

use crate::ids::PadstackId;
use crate::rules::{compare_to_ignore_case, equals_ignore_case};

#[derive(Debug, Clone, PartialEq)]
pub struct PackagePin {
        pub name: String,
            pub padstack_no: PadstackId,
        pub relative_location: Vector,
        pub rotation_in_degree: f64,
}

impl PackagePin {
        pub fn new(
        name: impl Into<String>,
        padstack_no: PadstackId,
        relative_location: Vector,
        rotation_in_degree: f64,
    ) -> PackagePin {
        PackagePin {
            name: name.into(),
            padstack_no,
            relative_location,
            rotation_in_degree,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Keepout {
        pub name: String,
        pub area: Area,
        pub layer: i32,
}

impl Keepout {
        pub fn new(name: impl Into<String>, area: Area, layer: i32) -> Keepout {
        Keepout {
            name: name.into(),
            area,
            layer,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Package {
        pub name: String,
            pub no: usize,
            pub outline: Option<Vec<Shape>>,
        pub outline_widths: Option<Vec<f64>>,
        pub outline_is_closed: Option<Vec<bool>>,
        pub keepouts: Vec<Keepout>,
        pub via_keepouts: Vec<Keepout>,
        pub place_keepouts: Vec<Keepout>,
        pub is_front: bool,
        pins: Vec<PackagePin>,
}

impl Package {
            #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: impl Into<String>,
        no: usize,
        pins: Vec<PackagePin>,
        outline: Option<Vec<Shape>>,
        outline_widths: Option<Vec<f64>>,
        outline_is_closed: Option<Vec<bool>>,
        keepouts: Vec<Keepout>,
        via_keepouts: Vec<Keepout>,
        place_keepouts: Vec<Keepout>,
        is_front: bool,
    ) -> Package {
        Package {
            name: name.into(),
            no,
            outline,
            outline_widths,
            outline_is_closed,
            keepouts,
            via_keepouts,
            place_keepouts,
            is_front,
            pins,
        }
    }

            pub fn compare_to(&self, other: &Package) -> Ordering {
        compare_to_ignore_case(&self.name, &other.name)
    }

            pub fn get_pin(&self, pin_index: i32) -> Option<&PackagePin> {
        if pin_index < 0 || pin_index as usize >= self.pins.len() {
            return None;
        }
        Some(&self.pins[pin_index as usize])
    }

                pub fn get_pin_index(&self, name: &str) -> Option<usize> {
        self.pins.iter().position(|p| p.name == name)
    }

            pub fn get_pin_by_name(&self, name: &str) -> Option<&PackagePin> {
        self.pins.iter().find(|p| p.name == name)
    }

        pub fn pin_count(&self) -> usize {
        self.pins.len()
    }
}

impl fmt::Display for Package {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Packages {
        list: Vec<Package>,
}

impl Packages {
        pub fn new() -> Packages {
        Packages::default()
    }

                                pub fn get_by_name(&self, name: &str, is_front: bool) -> Option<&Package> {
        let mut other_side_package = None;
        for pkg in &self.list {
            if equals_ignore_case(&pkg.name, name) {
                if pkg.is_front == is_front {
                    return Some(pkg);
                }
                other_side_package = Some(pkg);
            }
        }
        let base_name = strip_side_suffix(name);
        if !equals_ignore_case(base_name, name) {
            for pkg in &self.list {
                if equals_ignore_case(&pkg.name, base_name) {
                    if pkg.is_front == is_front {
                        return Some(pkg);
                    }
                    other_side_package = Some(pkg);
                }
            }
        }
        other_side_package
    }

                                    pub fn get(&self, no: usize) -> &Package {
        let result = &self.list[no - 1];
        debug_assert_eq!(
            result.no, no,
            "Packages.get: inconsistent package ID (Packages.java:57-59)"
        );
        result
    }

        pub fn count(&self) -> usize {
        self.list.len()
    }

                #[allow(clippy::too_many_arguments)]
    pub fn add(
        &mut self,
        name: impl Into<String>,
        pins: Vec<PackagePin>,
        outline: Option<Vec<Shape>>,
        outline_widths: Option<Vec<f64>>,
        outline_is_closed: Option<Vec<bool>>,
        keepouts: Vec<Keepout>,
        via_keepouts: Vec<Keepout>,
        place_keepouts: Vec<Keepout>,
        is_front: bool,
    ) -> usize {
        let no = self.list.len() + 1;
        self.list.push(Package::new(
            name,
            no,
            pins,
            outline,
            outline_widths,
            outline_is_closed,
            keepouts,
            via_keepouts,
            place_keepouts,
            is_front,
        ));
        no
    }

            pub fn add_pins(&mut self, pins: Vec<PackagePin>) -> usize {
        let name = format!("Package#{}", self.list.len() + 1);
        self.add(
            name,
            pins,
            None,
            None,
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            true,
        )
    }
}

fn strip_side_suffix(name: &str) -> &str {
    if let Some(idx) = name.rfind("::") {
        let suffix = &name[idx + 2..];
        if !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()) {
            return &name[..idx];
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;
    use fr_geometry::IntVector;

    fn pin(name: &str, padstack_no: usize) -> PackagePin {
        PackagePin::new(
            name,
            PadstackId(padstack_no),
            Vector::from(IntVector::new(0, 0)),
            0.0,
        )
    }

    #[test]
    fn package_pin_lookup_by_name() {
        let mut packages = Packages::new();
        let pins = vec![pin("1", 1), pin("2", 2), pin("GND", 3)];
        let no = packages.add_pins(pins);
        let pkg = packages.get(no);
        assert_eq!(pkg.pin_count(), 3);
        assert_eq!(pkg.get_pin_index("GND"), Some(2));
        assert_eq!(pkg.get_pin(2).unwrap().name, "GND");
        assert_eq!(pkg.get_pin_index("missing"), None);
        assert_eq!(pkg.get_pin_by_name("2").unwrap().padstack_no, PadstackId(2));
        assert!(pkg.get_pin_by_name("missing").is_none());
    }

    #[test]
    fn get_pin_index_is_case_sensitive() {
        let mut packages = Packages::new();
        let no = packages.add_pins(vec![pin("VCC", 1)]);
        let pkg = packages.get(no);
        assert_eq!(pkg.get_pin_index("VCC"), Some(0));
        assert_eq!(pkg.get_pin_index("vcc"), None);
    }

    #[test]
    fn get_pin_bounds_checked() {
        let mut packages = Packages::new();
        let no = packages.add_pins(vec![pin("1", 1)]);
        let pkg = packages.get(no);
        assert!(pkg.get_pin(-1).is_none());
        assert!(pkg.get_pin(1).is_none());
        assert!(pkg.get_pin(0).is_some());
    }

    #[test]
    fn add_pins_generates_sequential_names_and_defaults() {
        let mut packages = Packages::new();
        let a = packages.add_pins(vec![]);
        let b = packages.add_pins(vec![]);
        assert_eq!(packages.get(a).name, "Package#1");
        assert_eq!(packages.get(b).name, "Package#2");
        assert!(packages.get(a).is_front);
        assert!(packages.get(a).outline.is_none());
        assert!(packages.get(a).keepouts.is_empty());
    }

    #[test]
    #[should_panic]
    fn get_by_id_panics_out_of_range_like_java_elementat() {
        let packages = Packages::new();
        packages.get(1);
    }

    #[test]
    fn get_by_name_prefers_matching_side_falls_back_to_other_side() {
        let mut packages = Packages::new();
        packages.add(
            "SOIC8",
            vec![],
            None,
            None,
            None,
            vec![],
            vec![],
            vec![],
            true,
        );
        let front_id = packages.count();
        packages.add(
            "SOIC8",
            vec![],
            None,
            None,
            None,
            vec![],
            vec![],
            vec![],
            false,
        );
        let back_id = packages.count();

        assert_eq!(packages.get_by_name("soic8", true).unwrap().no, front_id);
        assert_eq!(packages.get_by_name("soic8", false).unwrap().no, back_id);
        assert!(packages.get_by_name("missing", true).is_none());
    }

    #[test]
    fn get_by_name_falls_back_to_the_other_side_when_only_side_present() {
        let mut packages = Packages::new();
        packages.add(
            "SOIC8",
            vec![],
            None,
            None,
            None,
            vec![],
            vec![],
            vec![],
            false,
        );
        assert!(packages.get_by_name("SOIC8", true).is_some());
        assert!(!packages.get_by_name("SOIC8", true).unwrap().is_front);
    }

    #[test]
    fn get_by_name_strips_double_colon_suffix() {
        let mut packages = Packages::new();
        packages.add(
            "R0805",
            vec![],
            None,
            None,
            None,
            vec![],
            vec![],
            vec![],
            true,
        );
        assert!(packages.get_by_name("R0805::2", true).is_some());
        assert!(packages.get_by_name("R0805::abc", true).is_none());
    }

    #[test]
    fn package_compare_to_is_case_insensitive() {
        let mut packages = Packages::new();
        packages.add(
            "Alpha",
            vec![],
            None,
            None,
            None,
            vec![],
            vec![],
            vec![],
            true,
        );
        packages.add(
            "beta",
            vec![],
            None,
            None,
            None,
            vec![],
            vec![],
            vec![],
            true,
        );
        let a = packages.get(1);
        let b = packages.get(2);
        assert_eq!(a.compare_to(b), Ordering::Less);
        assert_eq!(a.to_string(), "Alpha");
    }
}
