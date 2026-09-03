//! Board components and the list of them.
//!
//! Java: `board/model/structure/{Component,Components}.java`.
//!
//! A `Component` is one placed part: a name, a location and rotation, a placement side, and the
//! two library packages (front and back) it is drawn from. Its pins are separate [`crate::items::Item`]s
//! on the board — "the items of the component have to be inserted separately into the board"
//! (Components.java:26-28).

use std::cmp::Ordering;

use fr_geometry::{IntPoint, Point, Vector};

/// Port of `Component` (`board/model/structure/Component.java`).
///
/// not ported: `Component.printInfo` (Component.java:206-235) — `ItemInfoPrinter.Printable`,
/// GUI only, as everywhere else in this crate.
///
/// not ported: `Serializable` / `UndoableObjects.Storable`.
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    /// Java `public final String name` (Component.java:22).
    pub name: String,

    /// Java `public final int id` (Component.java:25): "internal generated unique
    /// identification number", assigned by [`Components::add`] as `count + 1`.
    ///
    /// The Task 5 brief calls this field `no`; Java calls it `id`, and `Components.get(int)`
    /// (Components.java:88-94) checks `result.id != componentId`, so `id` it is.
    pub id: i32,

    /// Java `public final boolean positionFixed` (Component.java:28).
    pub position_fixed: bool,

    /// Java `private final Package libPackageFront` (Component.java:31), stored as the package's
    /// `Package.no` rather than an object reference (the Plan 2 "no object references between
    /// model objects" rule; `Packages::get` indexes by exactly this number).
    lib_package_front: usize,

    /// Java `private final Package libPackageBack` (Component.java:34); see
    /// `lib_package_front`.
    lib_package_back: usize,

    /// Java `private final String partNumber` (Component.java:36); `null` at both of Java's own
    /// call sites that generate a name (Components.java:70).
    part_number: Option<String>,

    /// Java `private Point location` (Component.java:39); `null` until the component is placed
    /// (`isPlaced`, Component.java:91-93).
    location: Option<Point>,

    /// Java `private double rotationInDegree` (Component.java:42), normalised into `[0, 360)` by
    /// the constructor and by every mutator.
    rotation_in_degree: f64,

    /// Java `private LogicalPart logicalPart` (Component.java:45), stored as the part's
    /// `LogicalPart.no` rather than an object reference; `None` is Java's `null`.
    logical_part: Option<usize>,

    /// Java `private boolean onFront` (Component.java:48).
    on_front: bool,
}

/// Java's rotation normalisation, written three times over in `Component.java` (lines 67-72,
/// 116-121, 138-143): bring the angle into `[0, 360)` by repeated addition/subtraction.
///
/// The loops are kept rather than replaced by `rem_euclid` because they are not the same
/// function on a non-finite input: `rem_euclid(NaN)` is NaN, while Java's `while` conditions are
/// both false for NaN so the angle survives unchanged — and `Component.rotate` is reachable with
/// whatever `angleInDegree` the caller supplies.
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
    /// Port of the `Component(String, Point, double, boolean, Package, Package, int, boolean,
    /// String)` constructor (Component.java:50-79).
    ///
    /// Package-private in Java — only `Components.add` calls it — so it is `pub(crate)` here.
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
            // Java leaves `logicalPart` at its `null` default; `Components.add` never sets it,
            // and `Component.clone` copies it explicitly (Component.java:187).
            logical_part: None,
        }
    }

    /// Port of `Component.getLocation` (Component.java:81-84).
    pub fn get_location(&self) -> Option<&Point> {
        self.location.as_ref()
    }

    /// Port of `Component.getRotationInDegree` (Component.java:86-89).
    pub fn get_rotation_in_degree(&self) -> f64 {
        self.rotation_in_degree
    }

    /// Port of `Component.isPlaced` (Component.java:91-93).
    pub fn is_placed(&self) -> bool {
        self.location.is_some()
    }

    /// Port of `Component.placedOnFront` (Component.java:95-98).
    pub fn placed_on_front(&self) -> bool {
        self.on_front
    }

    /// Port of `Component.getPartNumber` (Component.java:169-171).
    pub fn get_part_number(&self) -> Option<&str> {
        self.part_number.as_deref()
    }

    /// Port of `Component.getPackage` (Component.java:237-246): the front package when the
    /// component is on the front side, the back package otherwise.
    pub fn get_package(&self) -> usize {
        if self.on_front {
            self.lib_package_front
        } else {
            self.lib_package_back
        }
    }

    /// Port of `Component.getLogicalPart` (Component.java:196-199).
    pub fn get_logical_part(&self) -> Option<usize> {
        self.logical_part
    }

    /// Port of `Component.setLogicalPart` (Component.java:201-204).
    pub fn set_logical_part(&mut self, logical_part: Option<usize>) {
        self.logical_part = logical_part;
    }

    /// Port of `Component.translateBy` (Component.java:100-108). An unplaced component is left
    /// alone (Java's `location != null` guard).
    pub fn translate_by(&mut self, vector: &Vector) {
        if let Some(location) = &self.location {
            self.location = Some(location.translate_by(vector));
        }
    }

    /// Port of `Component.turn90Degree` (Component.java:110-125).
    ///
    /// `factor == 0` returns immediately (Component.java:112-114), so a zero factor does not even
    /// normalise the rotation.
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

    /// Port of `Component.rotate` (Component.java:127-148).
    ///
    /// `flip_style_rotate_first` comes from [`Components::get_flip_style_rotate_first`]; when it
    /// is set and the component is on the back side, the *rotation field* is advanced by
    /// `360 - angleInDegree` instead of `angleInDegree` "to take care of the order of mirroring
    /// and rotating on the back side of the board" (Component.java:133-136).
    //
    // Java quirk: the location is rotated by `angleInDegree`, not by `turnAngle`
    // (Component.java:146), so on the back side with `flipStyleRotateFirst` the stored rotation
    // and the moved location disagree by `360 - 2*angle`. Ported verbatim; see
    // docs/java-quirks.md.
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

    /// Port of `Component.changeSide` (Component.java:150-156): flips the placement side and
    /// mirrors the location at the vertical line through `pole`.
    //
    // Java bug: unlike `translateBy`, `turn90Degree` and `rotate`, this one has **no**
    // `location != null` guard (Component.java:155), so changing the side of a component that
    // has not been placed yet throws a `NullPointerException` — after already flipping
    // `onFront`, which leaves the component half-mutated. See docs/java-quirks.md.
    //
    // fixed: T6 (#47) — the same `if (location != null)` guard the three sibling mutators have,
    // **and** the flip moved below it. Both halves matter and the second is the one a reader
    // could miss: `:152`'s `onFront = !onFront` ran before the throw, so a caught exception left
    // a component whose side had changed and whose location had not. An unplaced component is now
    // left alone exactly as `translateBy` (`:105`) leaves it.
    pub fn change_side(&mut self, pole: &IntPoint) {
        if let Some(location) = &self.location {
            let mirrored = location.mirror_vertical(&Point::Int(*pole));
            self.on_front = !self.on_front;
            self.location = Some(mirrored);
        }
    }

    /// Port of `Component.compareTo` (Component.java:158-167): by name, ignoring case.
    ///
    /// Exposed as an inherent method rather than an `Ord` impl, as elsewhere in this crate:
    /// Java's comparator looks only at the name while `PartialEq` here is structural, so an
    /// `Ord` impl would break the `Eq`/`Ord` consistency contract for two same-named components.
    pub fn compare_to(&self, other: &Component) -> Ordering {
        // Java `String.compareToIgnoreCase` compares char by char after `toUpperCase` then
        // `toLowerCase` on each character; for the ASCII reference designators that reach here
        // that is the same as comparing the lowercased strings.
        self.name.to_lowercase().cmp(&other.name.to_lowercase())
    }
}

impl std::fmt::Display for Component {
    // renamed: Component.toString -> Display::fmt (Component.java:191-194: the name).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

/// Port of `Components` (`board/model/structure/Components.java`): the list of components on the
/// board.
///
/// not ported: `Components.undoList` (Components.java:16), a `UndoableObjects` — Task 12 gives
/// the board `Board::clone`/`Board::deep_copy` (`board/snapshot.rs`) instead of a per-field undo
/// stack, so there is no `undoList` here for these to act on:
///
/// not ported: `Components.generateSnapshot` (:105-108) — the undo stack; a Plan-7 caller takes
/// `board.clone()` instead of generating a snapshot.
/// not ported: `Components.undo`/`redo` (:110-128) and the private
/// `restoreComponentArrFromUndoList` (:130-146) — interactive undo/redo.
// not ported: the `undoList.saveForUndo(currentComponent)` call that opens each of `move`,
// `turn90Degree`, `rotate` and `changeSide` (Components.java:154,164,174,184) — the four
// mutators below do the geometry but take no undo record; whole-board `deep_copy` is this port's
// snapshot instead.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Components {
    /// Java `private final Vector<Component> componentArr` (Components.java:17). The component
    /// with id `n` is at index `n - 1` (Components.java:48,89).
    components: Vec<Component>,

    /// Java `private boolean flipStyleRotateFirst` (Components.java:23): "if true, components on
    /// the back side are rotated before mirroring, else they are mirrored before rotating".
    flip_style_rotate_first: bool,
}

impl Components {
    /// An empty component list; Java uses the implicit constructor plus the field initialisers
    /// at Components.java:16-17.
    pub fn new() -> Components {
        Components::default()
    }

    /// Port of `Components.add(String, Point, double, boolean, Package, Package, boolean,
    /// String)` (Components.java:24-54).
    ///
    /// The packages are `Package.no` values rather than object references — see
    /// [`Component::get_package`]. Java returns the new component; this returns a shared
    /// reference to it, which is the same thing minus Java's aliasing.
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
        // not ported: `undoList.insert(newComponent)` (Components.java:52), which would register
        // the new component with the undo stack right after this push — no undo stack in this
        // port (see the `not ported:` note on `Components.undoList` above).
        self.components.push(new_component);
        self.components.last().expect("just pushed")
    }

    /// Port of the `Components.add(Point, double, boolean, Package)` overload
    /// (Components.java:56-72), which generates the name `Component#<count + 1>` and uses the
    /// same package for both sides, with `positionFixed == false` and no part number.
    // renamed: the second `Components.add` overload -> add_with_generated_name (Rust has no
    // overloading).
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

    /// Port of `Components.get(String)` (Components.java:74-82): the component with exactly this
    /// name (Java uses `String.equals`, not `equalsIgnoreCase`), or `None`.
    // renamed: Components.get(String) -> get_by_name, to keep it apart from the `get(int)`
    // overload below.
    pub fn get_by_name(&self, name: &str) -> Option<&Component> {
        self.components.iter().find(|c| c.name == name)
    }

    /// Port of `Components.get(int)` (Components.java:84-94): the component with this id, which
    /// lives at index `id - 1`.
    ///
    /// Java calls `Vector.elementAt(componentId - 1)` with no bounds check, so an id of 0 or one
    /// past the end throws `ArrayIndexOutOfBoundsException`; the port panics on the same input.
    /// Java's `result.id != componentId` warning becomes a `debug_assert!`
    /// (`global-constraints.md`).
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

    /// Mutable counterpart of [`Self::get`]; Java's `get` already hands back a mutable object.
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

    /// Port of `Components.count` (Components.java:96-99).
    pub fn count(&self) -> usize {
        self.components.len()
    }

    /// Port of `Components.getAll` (Components.java:101-103), in id order.
    pub fn get_all(&self) -> std::slice::Iter<'_, Component> {
        self.components.iter()
    }

    /// Port of `Components.move(int, Vector)` (Components.java:148-156).
    // renamed: Components.move -> move_component (`move` is a Rust keyword).
    pub fn move_component(&mut self, component_id: i32, vector: &Vector) {
        self.get_mut(component_id).translate_by(vector);
    }

    /// Port of `Components.turn90Degree(int, int, IntPoint)` (Components.java:158-166).
    pub fn turn_90_degree(&mut self, component_id: i32, factor: i32, pole: &IntPoint) {
        self.get_mut(component_id).turn_90_degree(factor, pole);
    }

    /// Port of `Components.rotate(int, double, IntPoint)` (Components.java:168-176), which
    /// supplies the list-wide `flipStyleRotateFirst` to [`Component::rotate`].
    pub fn rotate(&mut self, component_id: i32, rotation_in_degree: f64, pole: &IntPoint) {
        let flip_style_rotate_first = self.flip_style_rotate_first;
        self.get_mut(component_id)
            .rotate(rotation_in_degree, pole, flip_style_rotate_first);
    }

    /// Port of `Components.changeSide(int, IntPoint)` (Components.java:178-186).
    pub fn change_side(&mut self, component_id: i32, pole: &IntPoint) {
        self.get_mut(component_id).change_side(pole);
    }

    /// Port of `Components.getFlipStyleRotateFirst` (Components.java:188-194).
    pub fn get_flip_style_rotate_first(&self) -> bool {
        self.flip_style_rotate_first
    }

    /// Port of `Components.setFlipStyleRotateFirst` (Components.java:196-202).
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

    // ---- Components::add / get / get_by_name / count -------------------------------------------

    #[test]
    fn add_assigns_consecutive_ids_from_one() {
        // Components.java:48: `componentArr.size() + 1`.
        let c = components();
        assert_eq!(c.count(), 2);
        assert_eq!(c.get(1).name, "R1");
        assert_eq!(c.get(1).id, 1);
        assert_eq!(c.get(2).name, "C2");
        assert_eq!(c.get(2).id, 2);
    }

    #[test]
    fn add_stores_every_constructor_argument() {
        // Components.java:40-50 -> Component.java:64-78.
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
        // Components.java:77 uses `String.equals`.
        let c = components();
        assert_eq!(c.get_by_name("C2").map(|x| x.id), Some(2));
        assert!(c.get_by_name("c2").is_none());
        assert!(c.get_by_name("nope").is_none());
    }

    #[test]
    #[should_panic(expected = "ArrayIndexOutOfBoundsException")]
    fn get_by_id_zero_panics_like_java_element_at() {
        // Components.java:89: `componentArr.elementAt(componentId - 1)` with componentId 0.
        components().get(0);
    }

    #[test]
    #[should_panic]
    fn get_by_id_past_the_end_panics_like_java_element_at() {
        components().get(3);
    }

    #[test]
    fn get_all_yields_the_components_in_id_order() {
        // Components.java:101-103.
        let c = components();
        assert_eq!(c.get_all().map(|x| x.id).collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn add_with_generated_name_numbers_from_one() {
        // Components.java:62: `"Component#" + (componentArr.size() + 1)`, and the same package on
        // both sides with positionFixed false and no part number (Components.java:63-71).
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
        // The same package is used whichever side the component is on.
        assert_eq!(c.get(1).get_package(), 5);
        assert_eq!(c.get(2).get_package(), 5);
    }

    #[test]
    fn flip_style_rotate_first_round_trips_and_defaults_to_false() {
        // Components.java:23 leaves the `boolean` at its Java default.
        let mut c = Components::new();
        assert!(!c.get_flip_style_rotate_first());
        c.set_flip_style_rotate_first(true);
        assert!(c.get_flip_style_rotate_first());
    }

    // ---- Component --------------------------------------------------------------------------

    #[test]
    fn the_constructor_normalises_the_rotation_into_zero_to_360() {
        // Component.java:67-72.
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
        // Component.java:91-93.
        let c = components();
        assert!(c.get(1).is_placed());
        assert!(!c.get(2).is_placed());
    }

    #[test]
    fn get_package_picks_the_side() {
        // Component.java:237-246.
        let c = components();
        assert_eq!(c.get(1).get_package(), 1); // on the front
        assert_eq!(c.get(2).get_package(), 4); // on the back
    }

    #[test]
    fn translate_by_moves_a_placed_component_and_spares_an_unplaced_one() {
        // Component.java:104-108.
        let mut c = components();
        let v = Vector::Int(IntVector::new(5, -5));
        c.move_component(1, &v);
        c.move_component(2, &v);
        assert_eq!(c.get(1).get_location(), Some(&point(15, 15)));
        assert_eq!(c.get(2).get_location(), None);
    }

    #[test]
    fn turn_90_degree_advances_the_rotation_and_moves_the_location() {
        // Component.java:111-125.
        let mut c = components();
        c.turn_90_degree(1, 1, &IntPoint::new(0, 0));
        assert_eq!(c.get(1).get_rotation_in_degree(), 180.0);
        // (10,20) turned 90 degrees about the origin is (-20,10).
        assert_eq!(c.get(1).get_location(), Some(&point(-20, 10)));
    }

    #[test]
    fn turn_90_degree_by_zero_is_a_no_op() {
        // Component.java:112-114 returns before normalising anything.
        let mut c = components();
        c.turn_90_degree(1, 0, &IntPoint::new(3, 3));
        assert_eq!(c.get(1).get_rotation_in_degree(), 90.0);
        assert_eq!(c.get(1).get_location(), Some(&point(10, 20)));
    }

    #[test]
    fn turn_90_degree_wraps_the_rotation() {
        // Component.java:116-121.
        let mut c = components();
        c.turn_90_degree(1, 3, &IntPoint::new(0, 0));
        assert_eq!(c.get(1).get_rotation_in_degree(), 0.0);
        c.turn_90_degree(1, -1, &IntPoint::new(0, 0));
        assert_eq!(c.get(1).get_rotation_in_degree(), 270.0);
    }

    #[test]
    fn rotate_by_zero_is_a_no_op() {
        // Component.java:129-131.
        let mut c = components();
        c.rotate(1, 0.0, &IntPoint::new(0, 0));
        assert_eq!(c.get(1).get_rotation_in_degree(), 90.0);
        assert_eq!(c.get(1).get_location(), Some(&point(10, 20)));
    }

    #[test]
    fn rotate_advances_the_rotation_and_moves_the_location() {
        // Component.java:137-147.
        let mut c = components();
        c.rotate(1, 90.0, &IntPoint::new(0, 0));
        assert_eq!(c.get(1).get_rotation_in_degree(), 180.0);
        assert_eq!(c.get(1).get_location(), Some(&point(-20, 10)));
    }

    #[test]
    fn rotate_on_the_back_side_with_flip_style_rotate_first_uses_the_complement() {
        // Component.java:133-136: `turnAngle = 360 - angleInDegree` for the *rotation field*,
        // while the location is still rotated by `angleInDegree` (Component.java:146) — the
        // Java quirk noted on `Component::rotate`.
        let mut c = Components::new();
        c.add("B1", Some(point(10, 0)), 0.0, false, 1, 1, false, None);
        c.set_flip_style_rotate_first(true);
        c.rotate(1, 90.0, &IntPoint::new(0, 0));
        assert_eq!(c.get(1).get_rotation_in_degree(), 270.0);
        // ... but the location moved by +90 degrees, not -90.
        assert_eq!(c.get(1).get_location(), Some(&point(0, 10)));
    }

    #[test]
    fn rotate_on_the_front_side_ignores_flip_style_rotate_first() {
        // Component.java:133: the branch also needs `!placedOnFront()`.
        let mut c = components();
        c.set_flip_style_rotate_first(true);
        c.rotate(1, 90.0, &IntPoint::new(0, 0));
        assert_eq!(c.get(1).get_rotation_in_degree(), 180.0);
    }

    #[test]
    fn change_side_flips_and_mirrors() {
        // Component.java:153-156.
        let mut c = components();
        c.change_side(1, &IntPoint::new(0, 0));
        assert!(!c.get(1).placed_on_front());
        assert_eq!(c.get(1).get_location(), Some(&point(-10, 20)));
        assert_eq!(c.get(1).get_package(), 2); // now the back package
    }

    /// Quirk #47, inverted. Component.java:155 had no null guard, unlike its three sibling
    /// mutators, so changing the side of an unplaced component threw — **after** `:152` had
    /// already flipped `onFront`, leaving it half-mutated.
    #[test]
    fn change_side_on_an_unplaced_component_is_guarded() {
        let mut c = components();
        let before_side = c.get(2).placed_on_front();
        assert_eq!(c.get(2).get_location(), None, "component 2 is unplaced");

        c.change_side(2, &IntPoint::new(0, 0));

        // Left alone, exactly as `translateBy` (Component.java:105) leaves it — and in
        // particular `onFront` did **not** flip, which is the half-mutation the throw used to
        // leave behind.
        assert_eq!(c.get(2).get_location(), None);
        assert_eq!(c.get(2).placed_on_front(), before_side);
    }

    #[test]
    fn compare_to_orders_by_name_ignoring_case() {
        // Component.java:161-167.
        let c = components();
        assert_eq!(c.get(2).compare_to(c.get(1)), Ordering::Less); // "C2" < "R1"
        assert_eq!(c.get(1).compare_to(c.get(1)), Ordering::Equal);
        let mut other = Components::new();
        other.add("r1", None, 0.0, true, 1, 1, false, None);
        assert_eq!(c.get(1).compare_to(other.get(1)), Ordering::Equal);
    }

    #[test]
    fn logical_part_round_trips() {
        // Component.java:196-204; the constructor leaves it null (Component.java:45).
        let mut c = components();
        assert_eq!(c.get(1).get_logical_part(), None);
        c.get_mut(1).set_logical_part(Some(7));
        assert_eq!(c.get(1).get_logical_part(), Some(7));
    }

    #[test]
    fn clone_carries_the_logical_part() {
        // Component.java:174-189: Java's `clone` copies `logicalPart` explicitly after
        // re-running the constructor.
        let mut c = components();
        c.get_mut(1).set_logical_part(Some(4));
        let dup = c.get(1).clone();
        assert_eq!(dup.get_logical_part(), Some(4));
        assert_eq!(dup.id, 1);
        assert_eq!(dup.name, "R1");
    }

    #[test]
    fn display_is_the_name() {
        // Component.java:191-194.
        assert_eq!(components().get(1).to_string(), "R1");
    }
}
