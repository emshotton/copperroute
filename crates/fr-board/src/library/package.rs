//! Component package templates: padstacks and relative locations of package pins, plus optional
//! outline/keepout geometry.
//!
//! Java: `core/library/Package.java`, `core/library/Packages.java`.
//!
//! Java's `Package` file also imports `app.freerouting.board.model.items.Pin`
//! (Package.java:4), but that import is dead: the nested `Package.Pin` static class (below)
//! shadows it for every unqualified `Pin` use within the file, and `Packages.add` (Packages.java:
//! 69-94) is typed `Package.Pin[]`. So `pins` really is a list of the nested pin type ported
//! here as [`PackagePin`], not `board.model.items.Pin` (a later task's item type).

use std::cmp::Ordering;
use std::fmt;

use fr_geometry::{Area, Shape, Vector};

use crate::ids::PadstackId;
use crate::rules::{compare_to_ignore_case, equals_ignore_case};

/// Port of the nested `Package.Pin` (Package.java:130-151): a pin padstack of a package.
#[derive(Debug, Clone, PartialEq)]
pub struct PackagePin {
    /// `Pin.name` (Package.java:133).
    pub name: String,
    /// `Pin.padstackId` (Package.java:136), renamed per the task brief; the id of the
    /// padstack mask of the pin.
    pub padstack_no: PadstackId,
    /// `Pin.relativeLocation` (Package.java:139): the pin's location relative to the package.
    pub relative_location: Vector,
    /// `Pin.rotationInDegree` (Package.java:142): the rotation of the pin padstack.
    pub rotation_in_degree: f64,
}

impl PackagePin {
    /// Port of the `Pin(String, int, Vector, double)` constructor (Package.java:145-150).
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

/// Port of the nested `Package.Keepout` (Package.java:154-166): a named keepout area belonging
/// to a package.
#[derive(Debug, Clone, PartialEq)]
pub struct Keepout {
    /// `Keepout.name` (Package.java:156).
    pub name: String,
    /// `Keepout.area` (Package.java:157).
    pub area: Area,
    /// `Keepout.layer` (Package.java:158).
    pub layer: i32,
}

impl Keepout {
    // renamed: the nested `Package.Keepout(String, Area, int)` constructor -> `Keepout::new`
    // (Package.java:161-165). Flagged separately from the audit script's usual constructor
    // skip because the script only recognises `<Class>(...)` as a constructor when the method
    // name equals the *enclosing* class's file name (`Package`), not a nested class's own name.
    /// Port of the `Keepout(String, Area, int)` constructor (Package.java:161-165).
    pub fn new(name: impl Into<String>, area: Area, layer: i32) -> Keepout {
        Keepout {
            name: name.into(),
            area,
            layer,
        }
    }
}

/// Port of `Package` (`core/library/Package.java`): describes the padstacks and relative
/// locations of a component's pins, and optional outline/keepout geometry.
///
/// Java's `outline`/`outlineWidths`/`outlineIsClosed` fields (`Shape[]`/`double[]`/`boolean[]`)
/// may all be `null` — see `Packages.add(Pin[])` (Packages.java:97-110), which passes `null` for
/// all three — so they are `Option<Vec<_>>` here rather than the task brief's plain `Vec<Shape>`
/// (Java wins over the brief).
///
/// not ported: `Package.packageList` (Package.java:40) — a back-pointer to the owning
/// [`Packages`], read only by `Package.printInfo` (Package.java:106-127) to resolve a pin's
/// padstack name; `ItemInfoPrinter.Printable` is GUI-only and dropped (see `rules/mod.rs`'s
/// note).
///
/// not ported: `Package.printInfo` (Package.java:106-127) — GUI only.
#[derive(Debug, Clone, PartialEq)]
pub struct Package {
    /// `Package.name` (Package.java:20).
    pub name: String,
    /// `Package.id` (Package.java:23), renamed per the task brief; starts at 1
    /// (`Packages.add`, Packages.java:82).
    pub no: usize,
    /// `Package.outline` (Package.java:26): the outline of the component, `None` where Java has
    /// a `null` array.
    pub outline: Option<Vec<Shape>>,
    /// `Package.outlineWidths` (Package.java:28).
    pub outline_widths: Option<Vec<f64>>,
    /// `Package.outlineIsClosed` (Package.java:29).
    pub outline_is_closed: Option<Vec<bool>>,
    /// `Package.keepouts` (Package.java:30).
    pub keepouts: Vec<Keepout>,
    /// `Package.viaKeepouts` (Package.java:31).
    pub via_keepouts: Vec<Keepout>,
    /// `Package.placeKeepoutArr` (Package.java:32).
    pub place_keepouts: Vec<Keepout>,
    /// `Package.isFront` (Package.java:35): if false, the package is placed on the back side.
    pub is_front: bool,
    /// `Package.pins` (Package.java:38).
    pins: Vec<PackagePin>,
}

impl Package {
    /// Port of the public `Package` constructor (Package.java:43-66), minus the `Packages`
    /// back-pointer (see the type-level `not ported` note).
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

    /// Port of `Package.compareTo` (Package.java:69-72): compares by name, case-insensitively.
    /// Not an `Ord` impl, for the same reason as `Padstack::compare_to`.
    pub fn compare_to(&self, other: &Package) -> Ordering {
        compare_to_ignore_case(&self.name, &other.name)
    }

    /// Port of `Package.getPin` (Package.java:74-81): the pin at `pin_index`, or `None` when out
    /// of range. Java's out-of-range branch also logs a warning (dropped).
    pub fn get_pin(&self, pin_index: i32) -> Option<&PackagePin> {
        if pin_index < 0 || pin_index as usize >= self.pins.len() {
            return None;
        }
        Some(&self.pins[pin_index as usize])
    }

    /// Port of `Package.getPinIndex` (Package.java:84-94): the index of the pin named `name`
    /// (`equals`, case-sensitive), or `None` if no such pin exists. Pin indices range from `0`
    /// to `pin_count() - 1`, matching Java's `-1`-for-"not found" convention as `None`.
    pub fn get_pin_index(&self, name: &str) -> Option<usize> {
        self.pins.iter().position(|p| p.name == name)
    }

    /// Not a Java method: combines [`Self::get_pin_index`] and [`Self::get_pin`] for direct
    /// name-based lookup, matching the task brief's `get_pin(name)` shorthand.
    pub fn get_pin_by_name(&self, name: &str) -> Option<&PackagePin> {
        self.pins.iter().find(|p| p.name == name)
    }

    /// Port of `Package.pinCount` (Package.java:96-99).
    pub fn pin_count(&self) -> usize {
        self.pins.len()
    }
}

impl fmt::Display for Package {
    // renamed: Package.toString -> Display::fmt (Package.java:101-104 returns `name`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// Port of `Packages` (`core/library/Packages.java`): a library of component packages.
///
/// not ported: the `Packages(Padstacks)` constructor parameter (Packages.java:20-22), stored as
/// `Packages.padstackList` (Packages.java:11) — read only by `Package.printInfo`
/// (Package.java:119), which is GUI-only and dropped (see the `Package` doc comment).
/// [`Packages::new`] therefore takes no argument.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Packages {
    /// `Packages.packages` (Packages.java:14, a `Vector`).
    list: Vec<Package>,
}

impl Packages {
    /// An empty package library.
    pub fn new() -> Packages {
        Packages::default()
    }

    /// Port of `Packages.get(String, boolean)` (Packages.java:24-52): the package named `name`
    /// on side `is_front`, falling back to a package on the other side (preferring the
    /// input-side match if any package by that name exists), and — when `name` ends in
    /// `"::<digits>"` — retrying the whole search against the name with that suffix stripped.
    ///
    /// Java's leading `if (name == null) return null;` (Packages.java:28-30) is dropped: `name`
    /// is a non-nullable `&str` in the port.
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

    /// Port of `Packages.get(int)` (Packages.java:54-61): the package with this id (ids start
    /// at 1).
    ///
    /// Java performs **no bounds check** here (unlike `Padstacks.get(int)`): `Vector.elementAt`
    /// throws `ArrayIndexOutOfBoundsException` out of range, and the port panics identically via
    /// slice indexing. See `docs/java-quirks.md`.
    pub fn get(&self, no: usize) -> &Package {
        &self.list[no - 1]
    }

    /// Port of `Packages.count` (Packages.java:63-66).
    pub fn count(&self) -> usize {
        self.list.len()
    }

    /// Port of `Packages.add(String, Pin[], Shape[], double[], boolean[], Keepout[], Keepout[],
    /// Keepout[], boolean)` (Packages.java:68-94): appends a new package, returning its freshly
    /// assigned id.
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

    /// Port of `Packages.add(Pin[])` (Packages.java:96-110): appends a new package with an
    /// internally generated name (`"Package#" + no`), no outline, no keepouts, front side.
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

/// Java's `name.replaceAll("::\\d+$", "")` (Packages.java:40): strips a trailing `"::<digits>"`
/// suffix. Only the *last* `"::"` in the string can begin an all-digit run to the end (an
/// earlier one is always followed by another `"::"`, which is not a digit), so `rfind` alone
/// reproduces the regex's leftmost-match semantics.
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
        // Package.getPinIndex (Package.java:84-94) + getPin (Package.java:74-81).
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
        // Package.java:89 uses `equals`, not `equalsIgnoreCase`.
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
        // Packages.java:97-110: "Package#" + no, front side, no outline/keepouts.
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
        // Packages.java:54-61 has no bounds check, unlike Padstacks.get(int).
        let packages = Packages::new();
        packages.get(1);
    }

    #[test]
    fn get_by_name_prefers_matching_side_falls_back_to_other_side() {
        // Packages.java:24-52.
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
        // No front-side SOIC8 exists, so the back-side one is returned as a fallback.
        assert!(packages.get_by_name("SOIC8", true).is_some());
        assert!(!packages.get_by_name("SOIC8", true).unwrap().is_front);
    }

    #[test]
    fn get_by_name_strips_double_colon_suffix() {
        // Packages.java:40-50: "R0805::2" retries against "R0805" if the exact name fails.
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
