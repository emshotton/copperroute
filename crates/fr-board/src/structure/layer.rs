//! Board layers, the layer stack, and the small structural enums that describe them.
//!
//! Java: `board/model/structure/{Layer,LayerStructure,Unit,AngleRestriction,FixedState}.java`.

use std::fmt;

/// Port of `Layer` (`board/model/structure/Layer.java`): describes one board layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Layer {
    /// The layer's name (Layer.java:9).
    pub name: String,
    /// True if this is a signal layer usable for routing, as opposed to e.g. a power/ground
    /// plane (Layer.java:15).
    pub is_signal: bool,
}

impl Layer {
    /// Port of the `Layer(String, boolean)` constructor (Layer.java:19-22).
    pub fn new(name: impl Into<String>, is_signal: bool) -> Layer {
        Layer {
            name: name.into(),
            is_signal,
        }
    }
}

impl fmt::Display for Layer {
    // renamed: Layer.toString -> Display::fmt (Layer.java:24-27 returns `name`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// Port of `LayerStructure` (`board/model/structure/LayerStructure.java`): the ordered stack
/// of layers making up a board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerStructure {
    /// Java's public final `Layer[] layers` field (LayerStructure.java:8).
    pub layers: Vec<Layer>,
}

impl LayerStructure {
    /// Port of the `LayerStructure(Layer[])` constructor (LayerStructure.java:11-13).
    pub fn new(layers: Vec<Layer>) -> LayerStructure {
        LayerStructure { layers }
    }

    /// Not a Java method: `layers.length` is a direct field read in Java call sites
    /// (`layers` is a public field, LayerStructure.java:8); exposed here as a method for
    /// symmetry with the other accessors below.
    pub fn count(&self) -> usize {
        self.layers.len()
    }

    /// Port of `LayerStructure.getNo(String)` (LayerStructure.java:19-26): the index of the
    /// layer named `name`, or `None` (Java: `-1`) if there is none.
    pub fn get_no(&self, name: &str) -> Option<usize> {
        self.layers.iter().position(|l| l.name == name)
    }

    /// Port of the `getNo(Layer)` overload (LayerStructure.java:28-36). Java compares with
    /// `==` (object identity); ported as pointer identity against `self.layers`, which is the
    /// only sense that comparison can carry once `Layer` is a plain value type rather than a
    /// heap object with identity of its own.
    pub fn get_no_of_layer(&self, layer: &Layer) -> Option<usize> {
        self.layers.iter().position(|l| std::ptr::eq(l, layer))
    }

    /// Port of `LayerStructure.signalLayerCount` (LayerStructure.java:38-47).
    pub fn signal_layer_count(&self) -> usize {
        self.layers.iter().filter(|l| l.is_signal).count()
    }

    /// Port of `LayerStructure.getSignalLayer` (LayerStructure.java:49-61): the `no`-th signal
    /// layer, or `None` when there is no such layer.
    //
    // Java bug: when `no` is greater than or equal to the number of signal layers, Java falls
    // off the loop and returns `layers[layers.length - 1]` — the *last* layer, whether or not it
    // is even a signal layer — instead of erroring (LayerStructure.java:60); and with an empty
    // `layers` it throws `ArrayIndexOutOfBoundsException` on `layers[-1]`. See
    // docs/java-quirks.md #35.
    //
    // fixed: T10 (#35) — `Option<&Layer>`, as the sketch asks. Both of Java's failure modes
    // become `None`: a silent wrong answer and a crash are both replaced by an answer the caller
    // has to look at. The audit the sketch asks for came back empty — `ComboBoxLayer.java:45`,
    // `RouteState.java:265` and `InteractiveState.java:193` are the three Java call sites that
    // could rely on the fallback and all three are GUI, out of scope for this port, and
    // `get_signal_layer` has **no** caller in the port outside this file and
    // `get_layer_no_of_signal_layer` below. So the signature change costs nothing and removes a
    // trap.
    pub fn get_signal_layer(&self, no: usize) -> Option<&Layer> {
        let mut found = 0usize;
        for layer in &self.layers {
            if layer.is_signal {
                if no == found {
                    return Some(layer);
                }
                found += 1;
            }
        }
        None
    }

    /// Port of `LayerStructure.getSignalLayerNo(Layer)` (LayerStructure.java:63-75), renamed
    /// to take a layer index instead of a `Layer` reference (`// renamed:` — see rationale in
    /// `get_no_of_layer`'s doc comment).
    ///
    /// Java's loop walks `layers` in order, tallying signal layers until it reaches the given
    /// object; for a valid index that is exactly "how many signal layers sit before this
    /// position" — so an index-based signature sidesteps the reference-identity dance and can
    /// never hit Java's "-1, not found" case for an in-range index. Returns `None` (Java: `-1`)
    /// for an out-of-range index.
    pub fn get_signal_layer_no(&self, no: usize) -> Option<usize> {
        if no >= self.layers.len() {
            return None;
        }
        Some(self.layers[..no].iter().filter(|l| l.is_signal).count())
    }

    /// Port of `LayerStructure.getLayerNo(int)` (LayerStructure.java:77-81), renamed to
    /// `get_layer_no_of_signal_layer` to disambiguate from [`Self::get_no`]: returns the
    /// overall layer index of the `signal_layer_no`-th signal layer
    /// (`getNo(getSignalLayer(signalLayerNo))` in Java).
    ///
    /// `None` when there is no such signal layer — it inherits [`Self::get_signal_layer`]'s
    /// answer, which quirk #35's fix (T10) turned from Java's silent last-layer fallback into an
    /// `Option`.
    pub fn get_layer_no_of_signal_layer(&self, signal_layer_no: usize) -> Option<usize> {
        let layer = self.get_signal_layer(signal_layer_no)?;
        Some(
            self.layers
                .iter()
                .position(|l| std::ptr::eq(l, layer))
                .expect("get_signal_layer always returns a reference into self.layers"),
        )
    }
}

/// Port of `Unit` (`board/model/structure/Unit.java`): the user units inch, mil, mm, or µm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    Mil,
    Inch,
    Mm,
    Um,
}

impl Unit {
    /// Java's private per-variant `micrometers` field (Unit.java:7-10,12,14-16).
    fn micrometers(self) -> f64 {
        match self {
            Unit::Mil => 25.4,
            Unit::Inch => 25_400.0,
            Unit::Mm => 1000.0,
            Unit::Um => 1.0,
        }
    }

    /// Port of `Unit.scale` (Unit.java:18-21): scales `value` from `from_unit` to `to_unit`.
    pub fn scale(value: f64, from_unit: Unit, to_unit: Unit) -> f64 {
        value * from_unit.micrometers() / to_unit.micrometers()
    }

    /// Port of `Unit.fromString` (Unit.java:23-33). Java upper-cases the input and matches it
    /// against the enum constant names, returning `null` on any other input; ported as
    /// `Option::None`.
    pub fn from_string(string: &str) -> Option<Unit> {
        match string.to_uppercase().as_str() {
            "MIL" => Some(Unit::Mil),
            "INCH" => Some(Unit::Inch),
            "MM" => Some(Unit::Mm),
            "UM" => Some(Unit::Um),
            _ => None,
        }
    }
}

impl fmt::Display for Unit {
    // renamed: Unit.toString -> Display::fmt (Unit.java:35-38: the lower-cased enum constant
    // name, e.g. `MIL` -> "mil").
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Unit::Mil => "mil",
            Unit::Inch => "inch",
            Unit::Mm => "mm",
            Unit::Um => "um",
        };
        f.write_str(s)
    }
}

/// Port of `AngleRestriction` (`board/model/structure/AngleRestriction.java`).
///
/// Declaration order matters here exactly as the Java comment warns ("ordinal() and values()
/// rely on the order", AngleRestriction.java:5): it is `NONE`, `FORTYFIVE_DEGREE`,
/// `NINETY_DEGREE` in both languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AngleRestriction {
    None,
    FortyFiveDegree,
    NinetyDegree,
}

impl AngleRestriction {
    /// Port of `AngleRestriction.valueOf(int)` (AngleRestriction.java:11-13). Java indexes
    /// `values()` directly and throws `ArrayIndexOutOfBoundsException` for `i` out of range;
    /// ported as `Option` per the totalization ruling (`docs/plan-1-handoff.md` #12): no
    /// reachable caller may silently observe a fabricated value, so out-of-range is `None`.
    pub fn value_of(i: usize) -> Option<AngleRestriction> {
        match i {
            0 => Some(AngleRestriction::None),
            1 => Some(AngleRestriction::FortyFiveDegree),
            2 => Some(AngleRestriction::NinetyDegree),
            _ => None,
        }
    }

    /// Port of `AngleRestriction.getValue` (AngleRestriction.java:16-18): `ordinal()`.
    pub fn get_value(self) -> usize {
        match self {
            AngleRestriction::None => 0,
            AngleRestriction::FortyFiveDegree => 1,
            AngleRestriction::NinetyDegree => 2,
        }
    }
}

/// Port of `FixedState` (`board/model/structure/FixedState.java`): how strongly a board item
/// is pinned in place. Declaration order is the sort order in both languages — "the strongest
/// fixed states came last" (FixedState.java:3) — so deriving `Ord` over these variants in Java
/// declaration order reproduces Java's ordinal-based comparison exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FixedState {
    /// Not fixed: free to move or delete (FixedState.java:5).
    Unfixed,
    /// Fixed by the shove algorithm (FixedState.java:6).
    ShoveFixed,
    /// Fixed by the user (FixedState.java:7).
    UserFixed,
    /// Fixed by the system (FixedState.java:8).
    SystemFixed,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(name: &str, is_signal: bool) -> Layer {
        Layer::new(name, is_signal)
    }

    fn structure() -> LayerStructure {
        LayerStructure::new(vec![
            layer("F.Cu", true),
            layer("In1.Cu", false), // ground plane, e.g.
            layer("In2.Cu", true),
            layer("B.Cu", true),
        ])
    }

    #[test]
    fn layer_display_is_name() {
        assert_eq!(layer("F.Cu", true).to_string(), "F.Cu");
    }

    #[test]
    fn get_no_finds_by_name() {
        let s = structure();
        assert_eq!(s.get_no("F.Cu"), Some(0));
        assert_eq!(s.get_no("In2.Cu"), Some(2));
        assert_eq!(s.get_no("nonexistent"), None);
    }

    #[test]
    fn get_no_of_layer_uses_identity() {
        let s = structure();
        assert_eq!(s.get_no_of_layer(&s.layers[2]), Some(2));
        let other = layer("In2.Cu", true); // same contents, different object
        assert_eq!(s.get_no_of_layer(&other), None);
    }

    #[test]
    fn count_and_signal_layer_count() {
        let s = structure();
        assert_eq!(s.count(), 4);
        assert_eq!(s.signal_layer_count(), 3);
    }

    #[test]
    fn get_signal_layer_indexes_only_signal_layers() {
        let s = structure();
        assert_eq!(s.get_signal_layer(0).map(|l| l.name.as_str()), Some("F.Cu"));
        assert_eq!(
            s.get_signal_layer(1).map(|l| l.name.as_str()),
            Some("In2.Cu")
        );
        assert_eq!(s.get_signal_layer(2).map(|l| l.name.as_str()), Some("B.Cu"));
    }

    /// Quirk #35, fixed at Plan 9 Task 10. Java's `:60` answered `layers[layers.length - 1]` —
    /// the last layer of the *whole stack*, signal or not — for an out-of-range `no`, and threw
    /// `ArrayIndexOutOfBoundsException` on an empty stack. Both are `None` now. The binding
    /// version of this test is `crates/fr-board/tests/layer_structure.rs`.
    #[test]
    fn get_signal_layer_out_of_range_is_none() {
        let s = structure();
        assert_eq!(s.get_signal_layer(99), None);

        // The case that made Java's fallback a *wrong answer* rather than merely an odd one: the
        // last layer of the stack is not a signal layer at all.
        let non_signal_last = LayerStructure::new(vec![layer("F.Cu", true), layer("Adhes", false)]);
        assert_eq!(non_signal_last.get_signal_layer(99), None);

        // And the crash.
        assert_eq!(LayerStructure::new(vec![]).get_signal_layer(0), None);
    }

    #[test]
    fn get_signal_layer_no_counts_preceding_signal_layers() {
        let s = structure();
        assert_eq!(s.get_signal_layer_no(0), Some(0)); // F.Cu: 0 signal layers before it
        assert_eq!(s.get_signal_layer_no(2), Some(1)); // In2.Cu: only F.Cu before it is signal
        assert_eq!(s.get_signal_layer_no(3), Some(2)); // B.Cu: F.Cu, In2.Cu before it
        assert_eq!(s.get_signal_layer_no(4), None); // out of range
    }

    #[test]
    fn get_layer_no_of_signal_layer_round_trips_get_signal_layer() {
        let s = structure();
        assert_eq!(s.get_layer_no_of_signal_layer(0), Some(0)); // F.Cu
        assert_eq!(s.get_layer_no_of_signal_layer(1), Some(2)); // In2.Cu
        assert_eq!(s.get_layer_no_of_signal_layer(2), Some(3)); // B.Cu
    }

    #[test]
    fn unit_scale_inch_to_mil() {
        // Unit.java:7-10: MIL(25.4), INCH(25_400), MM(1000), UM(1) micrometers.
        // scale(1.0, INCH, MIL) = 1.0 * 25_400 / 25.4 = 1000.0.
        assert_eq!(Unit::scale(1.0, Unit::Inch, Unit::Mil), 1000.0);
    }

    #[test]
    fn unit_scale_mm_to_inch() {
        // scale(25.4, MM, INCH) = 25.4 * 1000 / 25_400 = 1.0.
        assert_eq!(Unit::scale(25.4, Unit::Mm, Unit::Inch), 1.0);
    }

    #[test]
    fn unit_scale_identity() {
        assert_eq!(Unit::scale(42.0, Unit::Um, Unit::Um), 42.0);
    }

    #[test]
    fn unit_from_string_is_case_insensitive() {
        assert_eq!(Unit::from_string("mil"), Some(Unit::Mil));
        assert_eq!(Unit::from_string("INCH"), Some(Unit::Inch));
        assert_eq!(Unit::from_string("Mm"), Some(Unit::Mm));
        assert_eq!(Unit::from_string("um"), Some(Unit::Um));
        assert_eq!(Unit::from_string("furlong"), None);
    }

    #[test]
    fn unit_display_is_lowercase() {
        assert_eq!(Unit::Mil.to_string(), "mil");
        assert_eq!(Unit::Inch.to_string(), "inch");
        assert_eq!(Unit::Mm.to_string(), "mm");
        assert_eq!(Unit::Um.to_string(), "um");
    }

    #[test]
    fn angle_restriction_value_of_and_get_value_round_trip() {
        assert_eq!(AngleRestriction::value_of(0), Some(AngleRestriction::None));
        assert_eq!(
            AngleRestriction::value_of(1),
            Some(AngleRestriction::FortyFiveDegree)
        );
        assert_eq!(
            AngleRestriction::value_of(2),
            Some(AngleRestriction::NinetyDegree)
        );
        assert_eq!(AngleRestriction::value_of(3), None);

        for a in [
            AngleRestriction::None,
            AngleRestriction::FortyFiveDegree,
            AngleRestriction::NinetyDegree,
        ] {
            assert_eq!(AngleRestriction::value_of(a.get_value()), Some(a));
        }
    }

    #[test]
    fn fixed_state_ordering_matches_java_ordinal_order() {
        assert!(FixedState::Unfixed < FixedState::ShoveFixed);
        assert!(FixedState::ShoveFixed < FixedState::UserFixed);
        assert!(FixedState::UserFixed < FixedState::SystemFixed);
        assert!(FixedState::Unfixed < FixedState::SystemFixed);
    }
}
