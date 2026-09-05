use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Layer {
    pub name: String,
    pub is_signal: bool,
}

impl Layer {
    pub fn new(name: impl Into<String>, is_signal: bool) -> Layer {
        Layer {
            name: name.into(),
            is_signal,
        }
    }
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerStructure {
    pub layers: Vec<Layer>,
}

impl LayerStructure {
    pub fn new(layers: Vec<Layer>) -> LayerStructure {
        LayerStructure { layers }
    }

    pub fn count(&self) -> usize {
        self.layers.len()
    }

    pub fn get_no(&self, name: &str) -> Option<usize> {
        self.layers.iter().position(|l| l.name == name)
    }

    pub fn get_no_of_layer(&self, layer: &Layer) -> Option<usize> {
        self.layers.iter().position(|l| std::ptr::eq(l, layer))
    }

    pub fn signal_layer_count(&self) -> usize {
        self.layers.iter().filter(|l| l.is_signal).count()
    }

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

    pub fn get_signal_layer_no(&self, no: usize) -> Option<usize> {
        if no >= self.layers.len() {
            return None;
        }
        Some(self.layers[..no].iter().filter(|l| l.is_signal).count())
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    Mil,
    Inch,
    Mm,
    Um,
}

impl Unit {
    fn micrometers(self) -> f64 {
        match self {
            Unit::Mil => 25.4,
            Unit::Inch => 25_400.0,
            Unit::Mm => 1000.0,
            Unit::Um => 1.0,
        }
    }

    pub fn scale(value: f64, from_unit: Unit, to_unit: Unit) -> f64 {
        value * from_unit.micrometers() / to_unit.micrometers()
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AngleRestriction {
    None,
    FortyFiveDegree,
    NinetyDegree,
}

impl AngleRestriction {
    pub fn value_of(i: usize) -> Option<AngleRestriction> {
        match i {
            0 => Some(AngleRestriction::None),
            1 => Some(AngleRestriction::FortyFiveDegree),
            2 => Some(AngleRestriction::NinetyDegree),
            _ => None,
        }
    }

    pub fn get_value(self) -> usize {
        match self {
            AngleRestriction::None => 0,
            AngleRestriction::FortyFiveDegree => 1,
            AngleRestriction::NinetyDegree => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FixedState {
    Unfixed,
    ShoveFixed,
    UserFixed,
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

    #[test]
    fn get_signal_layer_out_of_range_is_none() {
        let s = structure();
        assert_eq!(s.get_signal_layer(99), None);

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
