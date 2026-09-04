use crate::ids::ItemIdGenerator;
use crate::structure::Unit;

#[derive(Debug, Clone, PartialEq)]
pub struct Communication {
        pub unit: Unit,
            pub resolution: i32,
            pub id_gen: ItemIdGenerator,
            pub host_cad: Option<String>,
        pub host_version: Option<String>,
                    pub string_quote: String,
                        pub constants: Vec<Vec<String>>,
            pub write_resolution: Option<WriteResolution>,
                    pub dsn_file_generated_by_host: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteResolution {
        pub char_name: String,
        pub positive_int: i32,
}

impl WriteResolution {
        pub fn new(char_name: impl Into<String>, positive_int: i32) -> WriteResolution {
        WriteResolution {
            char_name: char_name.into(),
            positive_int,
        }
    }
}

impl Communication {
                                            pub fn new(
        unit: Unit,
        resolution: i32,
        id_gen: ItemIdGenerator,
        host_cad: Option<String>,
        host_version: Option<String>,
    ) -> Communication {
        Communication {
            unit,
            resolution,
            id_gen,
            host_cad,
            host_version,
            ..Communication::default()
        }
    }

        pub fn host_cad_is_eagle(&self) -> bool {
        self.host_cad
            .as_deref()
            .is_some_and(|cad| cad.eq_ignore_ascii_case("CadSoft"))
    }

                            pub fn host_is_old_kicad(&self) -> bool {
        let (Some(host_cad), Some(host_version)) =
            (self.host_cad.as_deref(), self.host_version.as_deref())
        else {
            return false;
        };
        if !host_cad.to_lowercase().contains("kicad") {
            return false;
        }
        let Some(start) = host_version.find(|c: char| c.is_ascii_digit()) else {
            return false;
        };
        let digits: String = host_version[start..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        digits
            .parse::<i64>()
            .is_ok_and(|version_number| version_number <= 5)
    }

                        pub fn host_cad_exists(&self) -> bool {
        self.host_cad.is_some()
    }

        pub fn get_resolution(&self, unit: Unit) -> f64 {
        Unit::scale(f64::from(self.resolution), unit, self.unit)
    }
}

impl Default for Communication {
                    fn default() -> Communication {
        Communication {
            unit: Unit::Mil,
            resolution: 1,
            id_gen: ItemIdGenerator::new(),
            host_cad: None,
            host_version: None,
            string_quote: "\"".to_string(),
            constants: Vec::new(),
            write_resolution: None,
            dsn_file_generated_by_host: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_carries_javas_default_specctra_parser_info() {
        let communication = Communication::default();
        assert_eq!(communication.string_quote, "\"");
        assert!(communication.constants.is_empty());
        assert_eq!(communication.write_resolution, None);
        assert!(!communication.dsn_file_generated_by_host);
    }

    #[test]
    fn new_fills_the_parser_info_fields_it_does_not_take_from_the_default() {
        let communication = Communication::new(
            Unit::Mm,
            1000,
            ItemIdGenerator::new(),
            Some("KiCad".to_string()),
            None,
        );
        assert_eq!(communication.unit, Unit::Mm);
        assert_eq!(communication.resolution, 1000);
        assert_eq!(communication.string_quote, "\"");
        assert!(!communication.dsn_file_generated_by_host);
    }

    #[test]
    fn write_resolution_keeps_the_raw_char_name() {
        let write_resolution = WriteResolution::new("mil", 10);
        assert_eq!(write_resolution.char_name, "mil");
        assert_eq!(write_resolution.positive_int, 10);
    }

    #[test]
    fn the_default_matches_javas_no_argument_constructor() {
        let communication = Communication::default();
        assert_eq!(communication.unit, Unit::Mil);
        assert_eq!(communication.resolution, 1);
        assert!(!communication.host_cad_exists());
        assert!(!communication.host_cad_is_eagle());
        assert!(!communication.host_is_old_kicad());
    }

    #[test]
    fn host_cad_is_eagle_ignores_case() {
        let eagle = Communication {
            host_cad: Some("cadsoft".to_string()),
            ..Communication::default()
        };
        assert!(eagle.host_cad_is_eagle());
        let other = Communication {
            host_cad: Some("KiCad".to_string()),
            ..Communication::default()
        };
        assert!(!other.host_cad_is_eagle());
    }

    #[test]
    fn host_is_old_kicad_reads_the_first_digit_run() {
        let old = Communication {
            host_cad: Some("kicad".to_string()),
            host_version: Some("5.1.9".to_string()),
            ..Communication::default()
        };
        assert!(old.host_is_old_kicad());
        let new = Communication {
            host_cad: Some("KiCad's Pcbnew".to_string()),
            host_version: Some("6.0.0".to_string()),
            ..Communication::default()
        };
        assert!(!new.host_is_old_kicad());
        let versionless = Communication {
            host_cad: Some("kicad".to_string()),
            host_version: Some("unknown".to_string()),
            ..Communication::default()
        };
        assert!(!versionless.host_is_old_kicad());
        let prefixed = Communication {
            host_cad: Some("kicad".to_string()),
            host_version: Some("version 4".to_string()),
            ..Communication::default()
        };
        assert!(prefixed.host_is_old_kicad());
    }

    #[test]
    fn host_is_old_kicad_needs_both_strings() {
        let no_version = Communication {
            host_cad: Some("kicad".to_string()),
            ..Communication::default()
        };
        assert!(!no_version.host_is_old_kicad());
    }

    #[test]
    fn get_resolution_scales_between_units() {
        let communication = Communication {
            unit: Unit::Mil,
            resolution: 10,
            ..Communication::default()
        };
        assert!((communication.get_resolution(Unit::Mil) - 10.0).abs() < 1e-9);
        assert!((communication.get_resolution(Unit::Inch) - 10_000.0).abs() < 1e-6);
    }

    #[test]
    fn the_id_generator_hands_out_ids_from_one() {
        let mut communication = Communication::default();
        assert_eq!(communication.id_gen.new_id(), crate::ids::ItemId(1));
    }
}
