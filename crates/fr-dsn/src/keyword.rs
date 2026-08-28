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
//! this enum in the parser. // added in Plan 3: Keyword.getName / ScopeKeyword.readScope
//! (Task 4 adds `name()` with the 2.3.0 name strings, the scope dispatch and
//! `skip_scope`/`read_scope`).

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
