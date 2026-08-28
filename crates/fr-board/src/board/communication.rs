//! Port of `board/state/Communication.java`: what the board remembers about the host system it
//! was imported from, plus the item-id generator every inserted item draws from.

use crate::ids::ItemIdGenerator;
use crate::structure::Unit;

/// Port of `Communication` (`board/state/Communication.java`).
///
/// Java's class carries six fields; three of them do not cross into this crate:
///
/// not ported: `Communication.observers` (Communication.java:31) — `global-constraints.md`
/// forbids board observers, and every call site (`BasicBoard.startNotifyObservers` and friends,
/// `BoardItemRepository.insertItem`/`removeItem`) is guarded by a `!= null` test that this port
/// answers with "no observers".
///
/// not ported: `Communication.coordinateTransform` (Communication.java:18), an
/// `io.CoordinateTransform` used only when writing a Specctra file back out — Plan 3 owns the
/// DSN/SES layer.
///
/// The fourth, `specctraParserInfo` (Communication.java:29), is **flattened onto this struct**:
/// its six fields become `string_quote`, `host_cad`, `host_version`, `constants`,
/// `write_resolution` and `dsn_file_generated_by_host` below, and its nullability is carried by
/// the `Option`s on the four fields Java allows to be `null`. Java's own
/// `specctraParserInfo == null` guard (Communication.java:61,68,88) therefore has no counterpart:
/// a `Communication` always has parser info, and "no parser info" is the all-defaults value
/// Java's own no-argument constructor already builds (`new SpecctraParserInfo("\"", null, null,
/// null, null, false)`, Communication.java:53). Every predicate below reads the same fields Java
/// reads through the nested object.
// renamed: the nested class `Communication.SpecctraParserInfo` (Communication.java:106-145) -> the six fields `string_quote`/`host_cad`/`host_version`/`constants`/`write_resolution`/`dsn_file_generated_by_host` on this struct.
#[derive(Debug, Clone, PartialEq)]
pub struct Communication {
    /// Java `final Unit unit` (Communication.java:20): mil, inch or mm.
    pub unit: Unit,
    /// Java `final int resolution` (Communication.java:27): `1 / unitFactor` of the host
    /// coordinate system.
    pub resolution: i32,
    /// Java `final IdGenerator idGenerator` (Communication.java:30), always an `ItemIdGenerator`
    /// in practice (Communication.java:56).
    pub id_gen: ItemIdGenerator,
    /// Java `specctraParserInfo.hostCad` (Communication.SpecctraParserInfo:111). `None` is
    /// Java's `null`, which the three predicates below all test for.
    pub host_cad: Option<String>,
    /// Java `specctraParserInfo.hostVersion` (Communication.SpecctraParserInfo:112).
    pub host_version: Option<String>,
    /// Java `specctraParserInfo.stringQuote` (Communication.java:109): "character for quoting
    /// strings in a dsn-File". Read by `DsnWriter.writePcbScope` (DsnWriter.java:67),
    /// `SesWriter.write` (SesWriter.java:64) and `RulesWriter.write` (RulesWriter.java:63) to
    /// build their `IdentifierType`, and written back out by `Parser.writeScope`.
    pub string_quote: String,
    /// Java `specctraParserInfo.constants` (Communication.java:113), a
    /// `Collection<String[]>` — the `(constant <name> <value>)` scopes of the DSN `parser`
    /// scope, kept verbatim so they can be written back out. Java's `null` and its empty
    /// collection are indistinguishable to `Parser.writeScope` (it `null`-checks and then
    /// iterates), so this is a plain `Vec`.
    pub constants: Vec<Vec<String>>,
    /// Java `specctraParserInfo.writeResolution` (Communication.java:114); `None` is Java's
    /// `null`, which `Parser.writeScope` tests for (Parser.java:131).
    pub write_resolution: Option<WriteResolution>,
    /// Java `specctraParserInfo.dsnFileGeneratedByHost` (Communication.java:115): false once the
    /// DSN reader has seen a `(generated_by_freerouting)` marker. Java's no-argument
    /// `Communication()` seeds it `false` (Communication.java:53), which is what
    /// [`Communication::default`] does too.
    pub dsn_file_generated_by_host: bool,
}

/// Port of `Communication.SpecctraParserInfo.WriteResolution` (Communication.java:133-143):
/// "resolution metadata for Specctra DSN export".
///
/// `char_name` stays a `String` rather than becoming a [`Unit`]: the DSN reader stores whatever
/// token the `(write_resolution …)` scope carried (`Parser.readWriteSolution`, Parser.java:22-27,
/// accepts any string), and `Parser.writeScope` writes only its **first character**
/// (`charName.substring(0, 1)`, Parser.java:134) — neither of which round-trips through a
/// four-variant enum.
///
/// not ported: `Serializable` — this port has no Java object serialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteResolution {
    /// Java `WriteResolution.charName` (Communication.java:136).
    pub char_name: String,
    /// Java `WriteResolution.positiveInt` (Communication.java:137).
    pub positive_int: i32,
}

impl WriteResolution {
    /// Port of `WriteResolution(String, int)` (Communication.java:140-143).
    pub fn new(char_name: impl Into<String>, positive_int: i32) -> WriteResolution {
        WriteResolution {
            char_name: char_name.into(),
            positive_int,
        }
    }
}

impl Communication {
    /// Port of the six-argument `Communication` constructor (Communication.java:34-47), minus
    /// the three not-ported fields and minus the four flattened `SpecctraParserInfo` fields that
    /// no `Communication::new` caller sets.
    ///
    /// Java passes a whole `SpecctraParserInfo`; flattening it would make this a nine-argument
    /// constructor. The four fields not named here take the values Java's own no-argument
    /// constructor gives them (`stringQuote = "\""`, no constants, no write resolution,
    /// `dsnFileGeneratedByHost = false`, Communication.java:53); a caller that has real parser
    /// info — the DSN reader — sets them with struct-update syntax over this or
    /// [`Communication::default`], since every field is `pub`.
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

    /// Port of `Communication.hostCadIsEagle` (Communication.java:61-65).
    pub fn host_cad_is_eagle(&self) -> bool {
        self.host_cad
            .as_deref()
            .is_some_and(|cad| cad.eq_ignore_ascii_case("CadSoft"))
    }

    /// Port of `Communication.hostIsOldKicad` (Communication.java:68-85): a KiCad host whose
    /// version string starts with a number `<= 5`.
    ///
    /// Java lower-cases `hostCad` and tests `contains("kicad")`, then pulls the **first** run of
    /// digits out of `hostVersion` with `Pattern.compile("\\d+")` and parses it. A version with
    /// no digits at all leaves `matcher.find()` false and the method returns `false`.
    //
    // Java bug: `Integer.parseInt(matcher.group())` throws `NumberFormatException` on a digit run
    // longer than an `int` — e.g. a `hostVersion` of "99999999999". Reproduced: the `parse::<i32>`
    // below `expect`s, panicking on exactly the same inputs. See docs/java-quirks.md.
    pub fn host_is_old_kicad(&self) -> bool {
        let (Some(host_cad), Some(host_version)) =
            (self.host_cad.as_deref(), self.host_version.as_deref())
        else {
            return false;
        };
        if !host_cad.to_lowercase().contains("kicad") {
            return false;
        }
        // `Pattern.compile("\\d+").matcher(versionString).find()` — the first maximal run of
        // ASCII digits anywhere in the string.
        let Some(start) = host_version.find(|c: char| c.is_ascii_digit()) else {
            return false;
        };
        let digits: String = host_version[start..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        let version_number: i32 = digits.parse().expect(
            "Communication.hostIsOldKicad: Integer.parseInt throws NumberFormatException on a \
             digit run that overflows an int (Communication.java:79)",
        );
        version_number <= 5
    }

    /// Port of `Communication.hostCadExists` (Communication.java:88-90).
    ///
    /// This is what decides `ItemCtx::max_tree_shape_width`: a board with a named host CAD system
    /// divides obstacle areas into much narrower sections
    /// (`ShapeSearchTree.calculateTreeShapes(ObstacleArea)`, ShapeSearchTree.java:916-920).
    pub fn host_cad_exists(&self) -> bool {
        self.host_cad.is_some()
    }

    /// Port of `Communication.getResolution(Unit)` (Communication.java:93-95).
    pub fn get_resolution(&self, unit: Unit) -> f64 {
        Unit::scale(f64::from(self.resolution), unit, self.unit)
    }
}

impl Default for Communication {
    /// Port of the no-argument `Communication()` constructor (Communication.java:50-58):
    /// `Unit.MIL`, resolution 1, a fresh `ItemIdGenerator`, and `new SpecctraParserInfo("\"",
    /// null, null, null, null, false)` — note `dsnFileGeneratedByHost` is seeded **false**, not
    /// true (Communication.java:53).
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
        // Communication.java:53 — `new SpecctraParserInfo("\"", null, null, null, null, false)`.
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
        // Communication.java:140-143 — `charName` is whatever token the DSN carried, not a Unit.
        let write_resolution = WriteResolution::new("mil", 10);
        assert_eq!(write_resolution.char_name, "mil");
        assert_eq!(write_resolution.positive_int, 10);
    }

    #[test]
    fn the_default_matches_javas_no_argument_constructor() {
        // Communication.java:50-58: MIL, resolution 1, no parser host strings.
        let communication = Communication::default();
        assert_eq!(communication.unit, Unit::Mil);
        assert_eq!(communication.resolution, 1);
        assert!(!communication.host_cad_exists());
        assert!(!communication.host_cad_is_eagle());
        assert!(!communication.host_is_old_kicad());
    }

    #[test]
    fn host_cad_is_eagle_ignores_case() {
        // Communication.java:64: `"CadSoft".equalsIgnoreCase(hostCad)`.
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
        // Communication.java:75-82.
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
        // No digits anywhere: `matcher.find()` is false and the method falls through to `false`
        // (Communication.java:84).
        let versionless = Communication {
            host_cad: Some("kicad".to_string()),
            host_version: Some("unknown".to_string()),
            ..Communication::default()
        };
        assert!(!versionless.host_is_old_kicad());
        // A version whose digits do not start the string: Java's regex finds them anywhere.
        let prefixed = Communication {
            host_cad: Some("kicad".to_string()),
            host_version: Some("version 4".to_string()),
            ..Communication::default()
        };
        assert!(prefixed.host_is_old_kicad());
    }

    #[test]
    fn host_is_old_kicad_needs_both_strings() {
        // Communication.java:69-73 returns false if either is null.
        let no_version = Communication {
            host_cad: Some("kicad".to_string()),
            ..Communication::default()
        };
        assert!(!no_version.host_is_old_kicad());
    }

    #[test]
    fn get_resolution_scales_between_units() {
        // Communication.java:94: `Unit.scale(resolution, unit, this.unit)`. Note the argument
        // order — the *requested* unit is the `from`, and the board's own unit is the `to`.
        let communication = Communication {
            unit: Unit::Mil,
            resolution: 10,
            ..Communication::default()
        };
        assert!((communication.get_resolution(Unit::Mil) - 10.0).abs() < 1e-9);
        // 10 inch expressed in mil is 10 * 25400 / 25.4.
        assert!((communication.get_resolution(Unit::Inch) - 10_000.0).abs() < 1e-6);
    }

    #[test]
    fn the_id_generator_hands_out_ids_from_one() {
        // Communication.java:56: `new ItemIdGenerator()`; Item.java:86-90 draws from it.
        let mut communication = Communication::default();
        assert_eq!(communication.id_gen.new_id(), crate::ids::ItemId(1));
    }
}
