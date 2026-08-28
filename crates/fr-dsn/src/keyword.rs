//! `Keyword` — the identity the Specctra parser dispatches on.
//!
//! Java's `io/specctra/parser/Keyword.java` is not an `enum` but a class holding
//! `public static final Keyword` singletons, each constructed with its Specctra name; the
//! parser compares them with `==`, so what matters is the **identity**, not the name. The port
//! makes that identity a Rust `enum`, one variant per `return Keyword.X` in the scanner's
//! action switch (`SpecctraDsnStreamReader.nextToken`, lines 940-1728).
//!
//! `Keyword.OPEN_BRACKET`/`CLOSED_BRACKET` are deliberately absent: they are
//! [`crate::lexer::Token::Open`]/[`Close`](crate::lexer::Token::Close) instead, so that every
//! Java `nextToken() == Keyword.OPEN_BRACKET` test becomes a `matches!` on the token.
//! `// renamed: OPEN_BRACKET`, `// renamed: CLOSED_BRACKET`.
//!
//! Java's `Keyword` subclasses (`ScopeKeyword` and the ~25 scope classes deriving from it) carry
//! the scope-reading behaviour; that hierarchy is not modelled here — dispatch is a `match` on
//! this enum in [`crate::parser::scope_parameter`]. `Keyword::name()` (below) returns the 2.3.0
//! name strings (plan ruling 1), reflected off `tools/freerouting-2.3.0.jar`'s
//! `Keyword.get_name()` while writing this file — not HEAD's camelCased regression.
//!
//! **`Keyword::JUMPER`**: HEAD's DFA (`SpecctraDsnStreamReader.nextToken`'s action switch) never
//! returns a `Keyword::Jumper` token — "jumper" is not one of the DFA's recognised keyword
//! lexemes, so it lexes as a plain `Str("jumper")`. This is **not a Java bug**: the only site
//! that cares, `Structure.java:349`
//! (`!Objects.equals(nextToken.toString(), Keyword.JUMPER.getName())`), deliberately compares the
//! token's *string form* against `Keyword.JUMPER.getName()` rather than testing identity against
//! a keyword singleton the way `SIGNAL`/`POWER` are tested a few lines above — so a plain string
//! token is exactly what that call site expects. The variant exists so `Keyword::name()` has
//! something to hand back at that call site (ruling 2); it is simply never produced by
//! [`crate::lexer::DsnScanner::next_token`].
//!
//! **`Keyword::PN`** is likewise never returned by the DFA (`"PN"` is not a DFA keyword lexeme
//! either), and unlike `JUMPER` it is *not* added as a variant here: `Component.java:281` tests
//! `nextToken == Keyword.PN || (nextToken instanceof String && "PN".equalsIgnoreCase(...))` — an
//! identity comparison against a keyword instance the DFA can never produce, OR'd with a
//! case-insensitive string fallback that *is* reachable. Whichever task ports `Component`'s
//! place scope (pin/gate-swap "PN" property) should read this as: the identity half of that `||`
//! is dead code reachable only if some future DFA state change starts returning it, and the
//! string-fallback half is what actually fires — no `Keyword::Pn` variant is needed to reproduce
//! that behaviour, only a case-insensitive `"PN"` string match.

/// A Specctra DSN keyword, as recognised by the lexer's DFA.
///
/// Variant names are the CamelCase spelling of the Java constant names; the *name strings*
/// (which plan ruling 1 pins to the 2.3.0 snake_case Specctra tokens, not the clone HEAD's
/// camelCased regression) arrive with `Keyword::name()` in Task 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Keyword {
    /// Java `Keyword.ABSOLUTE`.
    Absolute,
    /// Java `Keyword.ACTIVE`.
    Active,
    /// Java `Keyword.AGAINST_PREFERRED_DIRECTION_TRACE_COSTS`.
    AgainstPreferredDirectionTraceCosts,
    /// Java `Keyword.ATTACH`.
    Attach,
    /// Java `Keyword.AUTOROUTE`.
    Autoroute,
    /// Java `Keyword.AUTOROUTE_SETTINGS`.
    AutorouteSettings,
    /// Java `Keyword.BACK`.
    Back,
    /// Java `Keyword.BOUNDARY`.
    Boundary,
    /// Java `Keyword.CIRCLE`.
    Circle,
    /// Java `Keyword.CIRCUIT`.
    Circuit,
    /// Java `Keyword.CLASS`.
    Class,
    /// Java `Keyword.CLASSES`.
    Classes,
    /// Java `Keyword.CLASS_CLASS`.
    ClassClass,
    /// Java `Keyword.CLEARANCE`.
    Clearance,
    /// Java `Keyword.CLEARANCE_CLASS`.
    ClearanceClass,
    /// Java `Keyword.COMPONENT_SCOPE`.
    ComponentScope,
    /// Java `Keyword.CONSTANT`.
    Constant,
    /// Java `Keyword.CONTROL`.
    Control,
    /// Java `Keyword.FANOUT`.
    Fanout,
    /// Java `Keyword.FIX`.
    Fix,
    /// Java `Keyword.FLIP_STYLE`.
    FlipStyle,
    /// Java `Keyword.FORTYFIVE_DEGREE`.
    FortyfiveDegree,
    /// Java `Keyword.FROMTO`.
    Fromto,
    /// Java `Keyword.FRONT`.
    Front,
    /// Java `Keyword.GENERATED_BY_FREEROUTING`.
    GeneratedByFreerouting,
    /// Java `Keyword.HORIZONTAL`.
    Horizontal,
    /// Java `Keyword.HOST_CAD`.
    HostCad,
    /// Java `Keyword.HOST_VERSION`.
    HostVersion,
    /// Java `Keyword.IMAGE`.
    Image,
    /// Java `Keyword.JUMPER`. Never produced by the lexer's DFA — see the module docs above for
    /// why that is not a bug and `Keyword::name()` is still needed for it.
    Jumper,
    /// Java `Keyword.KEEPOUT`.
    Keepout,
    /// Java `Keyword.LAYER`.
    Layer,
    /// Java `Keyword.LAYER_RULE`.
    LayerRule,
    /// Java `Keyword.LENGTH`.
    Length,
    /// Java `Keyword.LIBRARY_SCOPE`.
    LibraryScope,
    /// Java `Keyword.LOCK_TYPE`.
    LockType,
    /// Java `Keyword.LOGICAL_PART`.
    LogicalPart,
    /// Java `Keyword.LOGICAL_PART_MAPPING`.
    LogicalPartMapping,
    /// Java `Keyword.NET`.
    Net,
    /// Java `Keyword.NETWORK_OUT`.
    NetworkOut,
    /// Java `Keyword.NETWORK_SCOPE`.
    NetworkScope,
    /// Java `Keyword.NINETY_DEGREE`.
    NinetyDegree,
    /// Java `Keyword.NONE`.
    None,
    /// Java `Keyword.NORMAL`.
    Normal,
    /// Java `Keyword.OFF`.
    Off,
    /// Java `Keyword.ON`.
    On,
    /// Java `Keyword.ORDER`.
    Order,
    /// Java `Keyword.OUTLINE`.
    Outline,
    /// Java `Keyword.PADSTACK`.
    Padstack,
    /// Java `Keyword.PARSER_SCOPE`.
    ParserScope,
    /// Java `Keyword.PART_LIBRARY_SCOPE`.
    PartLibraryScope,
    /// Java `Keyword.PCB_SCOPE`.
    PcbScope,
    /// Java `Keyword.PIN`.
    Pin,
    /// Java `Keyword.PINS`.
    Pins,
    /// Java `Keyword.PLACE`.
    Place,
    /// Java `Keyword.PLACEMENT_SCOPE`.
    PlacementScope,
    /// Java `Keyword.PLACE_CONTROL`.
    PlaceControl,
    /// Java `Keyword.PLACE_KEEPOUT`.
    PlaceKeepout,
    /// Java `Keyword.PLANE_SCOPE`.
    PlaneScope,
    /// Java `Keyword.PLANE_VIA_COSTS`.
    PlaneViaCosts,
    /// Java `Keyword.POLYGON`.
    Polygon,
    /// Java `Keyword.POLYGON_PATH`.
    PolygonPath,
    /// Java `Keyword.POLYLINE_PATH`.
    PolylinePath,
    /// Java `Keyword.POSITION`.
    Position,
    /// Java `Keyword.POSTROUTE`.
    Postroute,
    /// Java `Keyword.POWER`.
    Power,
    /// Java `Keyword.PREFERRED_DIRECTION`.
    PreferredDirection,
    /// Java `Keyword.PREFERRED_DIRECTION_TRACE_COSTS`.
    PreferredDirectionTraceCosts,
    /// Java `Keyword.PULL_TIGHT`.
    PullTight,
    /// Java `Keyword.RECTANGLE`.
    Rectangle,
    /// Java `Keyword.RESOLUTION_SCOPE`.
    ResolutionScope,
    /// Java `Keyword.ROTATE`.
    Rotate,
    /// Java `Keyword.ROTATE_FIRST`.
    RotateFirst,
    /// Java `Keyword.ROUTES`.
    Routes,
    /// Java `Keyword.RULE`.
    Rule,
    /// Java `Keyword.RULES`.
    Rules,
    /// Java `Keyword.SESSION`.
    Session,
    /// Java `Keyword.SHAPE`.
    Shape,
    /// Java `Keyword.SHOVE_FIXED`.
    ShoveFixed,
    /// Java `Keyword.SIDE`.
    Side,
    /// Java `Keyword.SIGNAL`.
    Signal,
    /// Java `Keyword.SNAP_ANGLE`.
    SnapAngle,
    /// Java `Keyword.SPARE`.
    Spare,
    /// Java `Keyword.START_PASS_NO`.
    StartPassNo,
    /// Java `Keyword.START_RIPUP_COSTS`.
    StartRipupCosts,
    /// Java `Keyword.STRING_QUOTE`.
    StringQuote,
    /// Java `Keyword.STRUCTURE_SCOPE`.
    StructureScope,
    /// Java `Keyword.TYPE`.
    Type,
    /// Java `Keyword.USE_LAYER`.
    UseLayer,
    /// Java `Keyword.USE_NET`.
    UseNet,
    /// Java `Keyword.USE_VIA`.
    UseVia,
    /// Java `Keyword.VERTICAL`.
    Vertical,
    /// Java `Keyword.VIA`.
    Via,
    /// Java `Keyword.VIAS`.
    Vias,
    /// Java `Keyword.VIA_AT_SMD`.
    ViaAtSmd,
    /// Java `Keyword.VIA_COSTS`.
    ViaCosts,
    /// Java `Keyword.VIA_KEEPOUT`.
    ViaKeepout,
    /// Java `Keyword.VIA_RULE`.
    ViaRule,
    /// Java `Keyword.WIDTH`.
    Width,
    /// Java `Keyword.WINDOW`.
    Window,
    /// Java `Keyword.WIRE`.
    Wire,
    /// Java `Keyword.WIRING_SCOPE`.
    WiringScope,
    /// Java `Keyword.WRITE_RESOLUTION`.
    WriteResolution,
}

impl Keyword {
    // renamed: getName (also `get_name` in the 2.3.0 jar) -> `name`, dropping the Java `get`
    // prefix per Rust accessor convention.
    /// `getName()` (Keyword.java:126-128 at HEAD; `get_name()` in the 2.3.0 jar). Returns the
    /// **2.3.0** Specctra token string (plan ruling 1), reflected off
    /// `tools/freerouting-2.3.0.jar` — not HEAD's camelCased regression (`Keyword.AUTOROUTE_SETTINGS
    /// = new Keyword("autorouteSettings")` at HEAD vs. 2.3.0's `"autoroute_settings"`, etc. for
    /// the fifteen keywords ruling 1 lists).
    ///
    /// Used only where Java calls `getName()`/`get_name()`: `Structure.java:349`,
    /// `Shape.java:81,83,262`, `Network.java:65,87,104,371` (ruling 2) — every other scope's
    /// *write* literal is transcribed separately, one `writeScope` at a time, in Tasks 11-14.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Keyword::Absolute => "absolute",
            Keyword::Active => "active",
            Keyword::AgainstPreferredDirectionTraceCosts => {
                "against_preferred_direction_trace_costs"
            }
            Keyword::Attach => "attach",
            Keyword::Autoroute => "autoroute",
            Keyword::AutorouteSettings => "autoroute_settings",
            Keyword::Back => "back",
            Keyword::Boundary => "boundary",
            Keyword::Circle => "circle",
            Keyword::Circuit => "circuit",
            Keyword::Class => "class",
            Keyword::Classes => "classes",
            Keyword::ClassClass => "class_class",
            Keyword::Clearance => "clearance",
            Keyword::ClearanceClass => "clearance_class",
            Keyword::ComponentScope => "component",
            Keyword::Constant => "constant",
            Keyword::Control => "control",
            Keyword::Fanout => "fanout",
            Keyword::Fix => "fix",
            Keyword::FlipStyle => "flip_style",
            Keyword::FortyfiveDegree => "fortyfive_degree",
            Keyword::Fromto => "fromto",
            Keyword::Front => "front",
            Keyword::GeneratedByFreerouting => "generated_by_freerouting",
            Keyword::Horizontal => "horizontal",
            Keyword::HostCad => "host_cad",
            Keyword::HostVersion => "host_version",
            Keyword::Image => "image",
            Keyword::Jumper => "jumper",
            Keyword::Keepout => "keepout",
            Keyword::Layer => "layer",
            Keyword::LayerRule => "layer_rule",
            Keyword::Length => "length",
            Keyword::LibraryScope => "library",
            Keyword::LockType => "lock_type",
            Keyword::LogicalPart => "logical_part",
            Keyword::LogicalPartMapping => "logical_part_mapping",
            Keyword::Net => "net",
            Keyword::NetworkOut => "network_out",
            Keyword::NetworkScope => "network",
            Keyword::NinetyDegree => "ninety_degree",
            Keyword::None => "none",
            Keyword::Normal => "normal",
            Keyword::Off => "off",
            Keyword::On => "on",
            Keyword::Order => "order",
            Keyword::Outline => "outline",
            Keyword::Padstack => "padstack",
            Keyword::ParserScope => "parser",
            Keyword::PartLibraryScope => "part_library",
            Keyword::PcbScope => "pcb",
            Keyword::Pin => "pin",
            Keyword::Pins => "pins",
            Keyword::Place => "place",
            Keyword::PlacementScope => "placement",
            Keyword::PlaceControl => "place_control",
            Keyword::PlaceKeepout => "place_keepout",
            Keyword::PlaneScope => "plane",
            Keyword::PlaneViaCosts => "plane_via_costs",
            Keyword::Polygon => "polygon",
            Keyword::PolygonPath => "polygon_path",
            Keyword::PolylinePath => "polyline_path",
            Keyword::Position => "position",
            Keyword::Postroute => "postroute",
            Keyword::Power => "power",
            Keyword::PreferredDirection => "preferred_direction",
            Keyword::PreferredDirectionTraceCosts => "preferred_direction_trace_costs",
            Keyword::PullTight => "pull_tight",
            Keyword::Rectangle => "rectangle",
            Keyword::ResolutionScope => "resolution",
            Keyword::Rotate => "rotate",
            Keyword::RotateFirst => "rotate_first",
            Keyword::Routes => "routes",
            Keyword::Rule => "rule",
            Keyword::Rules => "rules",
            Keyword::Session => "session",
            Keyword::Shape => "shape",
            Keyword::ShoveFixed => "shove_fixed",
            Keyword::Side => "side",
            Keyword::Signal => "signal",
            Keyword::SnapAngle => "snap_angle",
            Keyword::Spare => "spare",
            Keyword::StartPassNo => "start_pass_no",
            Keyword::StartRipupCosts => "start_ripup_costs",
            Keyword::StringQuote => "string_quote",
            Keyword::StructureScope => "structure",
            Keyword::Type => "type",
            Keyword::UseLayer => "use_layer",
            Keyword::UseNet => "use_net",
            Keyword::UseVia => "use_via",
            Keyword::Vertical => "vertical",
            Keyword::Via => "via",
            Keyword::Vias => "vias",
            Keyword::ViaAtSmd => "via_at_smd",
            Keyword::ViaCosts => "via_costs",
            Keyword::ViaKeepout => "via_keepout",
            Keyword::ViaRule => "via_rule",
            Keyword::Width => "width",
            Keyword::Window => "window",
            Keyword::Wire => "wire",
            Keyword::WiringScope => "wiring",
            Keyword::WriteResolution => "write_resolution",
        }
    }
}

/// A Specctra DSN **scope** keyword — the twelve `Keyword` constants Java constructs as
/// `ScopeKeyword` (or a subclass of it) rather than a plain `Keyword`
/// (`Keyword.java:26,49,58,65,66,70,72,73,88,90,109`). Java tests membership with
/// `nextToken instanceof ScopeKeyword` (`ScopeKeyword.java:59`); the port makes that a
/// dedicated enum plus [`ScopeKeyword::from_keyword`] instead.
///
/// `Pcb` is the odd one out: `Keyword.PCB_SCOPE = new ScopeKeyword("pcb")` (`Keyword.java:66`)
/// constructs a plain `ScopeKeyword`, not a subclass — there is no `Pcb.java`. Every other
/// variant here has a same-named Java scope class (`Component.java`, `Library.java`, …) that
/// overrides `readScope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ScopeKeyword {
    /// Java `Keyword.COMPONENT_SCOPE` / `Component.java`.
    Component,
    /// Java `Keyword.LIBRARY_SCOPE` / `Library.java`.
    Library,
    /// Java `Keyword.NETWORK_SCOPE` / `Network.java`.
    Network,
    /// Java `Keyword.PARSER_SCOPE` / `Parser.java`.
    Parser,
    /// Java `Keyword.PART_LIBRARY_SCOPE` / `PartLibrary.java`.
    PartLibrary,
    /// Java `Keyword.PCB_SCOPE` — no Java subclass; see the enum docs above.
    Pcb,
    /// Java `Keyword.PLACE_CONTROL` / `PlaceControl.java`.
    PlaceControl,
    /// Java `Keyword.PLACEMENT_SCOPE` / `Placement.java`.
    Placement,
    /// Java `Keyword.PLANE_SCOPE` / `Plane.java`.
    Plane,
    /// Java `Keyword.RESOLUTION_SCOPE` / `Resolution.java`.
    Resolution,
    /// Java `Keyword.STRUCTURE_SCOPE` / `Structure.java`.
    Structure,
    /// Java `Keyword.WIRING_SCOPE` / `Wiring.java`.
    Wiring,
}

impl ScopeKeyword {
    /// `getName()`/`get_name()` for the twelve scope keywords, reflected off
    /// `tools/freerouting-2.3.0.jar` (identical to HEAD for every one of these twelve — the
    /// ruling-1 rename only ever touched plain `Keyword` constants, never a `ScopeKeyword`).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            ScopeKeyword::Component => "component",
            ScopeKeyword::Library => "library",
            ScopeKeyword::Network => "network",
            ScopeKeyword::Parser => "parser",
            ScopeKeyword::PartLibrary => "part_library",
            ScopeKeyword::Pcb => "pcb",
            ScopeKeyword::PlaceControl => "place_control",
            ScopeKeyword::Placement => "placement",
            ScopeKeyword::Plane => "plane",
            ScopeKeyword::Resolution => "resolution",
            ScopeKeyword::Structure => "structure",
            ScopeKeyword::Wiring => "wiring",
        }
    }

    /// `nextToken instanceof ScopeKeyword` (`ScopeKeyword.java:59`), as a lookup instead of a
    /// runtime type test: `None` for every plain (non-scope) `Keyword`.
    #[must_use]
    pub fn from_keyword(keyword: Keyword) -> Option<ScopeKeyword> {
        match keyword {
            Keyword::ComponentScope => Some(ScopeKeyword::Component),
            Keyword::LibraryScope => Some(ScopeKeyword::Library),
            Keyword::NetworkScope => Some(ScopeKeyword::Network),
            Keyword::ParserScope => Some(ScopeKeyword::Parser),
            Keyword::PartLibraryScope => Some(ScopeKeyword::PartLibrary),
            Keyword::PcbScope => Some(ScopeKeyword::Pcb),
            Keyword::PlaceControl => Some(ScopeKeyword::PlaceControl),
            Keyword::PlacementScope => Some(ScopeKeyword::Placement),
            Keyword::PlaneScope => Some(ScopeKeyword::Plane),
            Keyword::ResolutionScope => Some(ScopeKeyword::Resolution),
            Keyword::StructureScope => Some(ScopeKeyword::Structure),
            Keyword::WiringScope => Some(ScopeKeyword::Wiring),
            _ => None,
        }
    }
}
