//! `io/specctra/parser/{Structure,LayerStructure,Layer}.java` — the `structure` scope, and
//! (nested inside it, Structure.java:1001-1005) the `plane` scope.
//!
//! # Nothing is inserted while parsing
//!
//! `Structure.readScope` accumulates everything it reads into four local lists (`keepoutList`,
//! `viaKeepoutList`, `placeKeepoutList` and `ReadScopeParameter.planeList`) plus a
//! `BoardConstructionInfo`, and only *after* the scope's closing bracket does it build the board
//! and insert anything (Structure.java:1030-1123). That ordering is load-bearing: item ids are
//! handed out in insertion order, so the sequence below is what every later parity test compares
//! against. In Java's order:
//!
//! 1. `createBoard` (Structure.java:1139-1290) — `BasicBoard`'s constructor inserts the outline,
//!    which therefore always takes id 1;
//! 2. still inside `createBoard` (:1279-1285), the **holes** in the outline, one obstacle per
//!    hole per board layer (the layer loop is the inner one);
//! 3. `keepoutList`, then `viaKeepoutList`, then `placeKeepoutList` (:1042-1067), each
//!    `FixedState::SystemFixed`;
//! 4. `planeList` → one conduction area per plane (:1069-1121), creating a missing net first;
//! 5. `insertMissingPowerPlanes` (:1123).
//!
//! # `Rule.java`'s read half lives here for now
//!
//! `Structure.readScope` and `Structure.readLayerScope` both call `Rule.readScope`, and
//! `Structure.updateBoardRules`/`setClearanceRule` consume its `WidthRule`/`ClearanceRule`
//! results, so the rule reader has to exist before the structure scope can do anything with a
//! `(rule …)`. Plan Task 8 owns `Rule.java` (its writers, its `layer_rule` scope and the
//! `NetClass`-side callers); this file carries only the three readers `Structure` needs, and
//! Task 8 should re-export or relocate [`DsnRule`] rather than define a second one.
// added in Task 8: Rule.readLayerRuleScope — the `(layer_rule …)` sub-scope, which no `structure` scope reaches (only `NetClass.readScope` does).
// added in Task 11: Rule.writeScope, Rule.writeDefaultRule, Rule.writeLayerRule, Rule.writeItemClearanceClass — the rule writers, which belong to the DSN writer half.
// added in Task 11: Structure.writeScope, Structure.writeSnapAngle, Structure.writeLayers, Structure.writeDefaultRules, Plane.writeScope — the `structure`/`plane` writers. `writeSnapAngle` must emit the 2.3.0 literal `"snap_angle "`, never HEAD's `"snapAngle "` (plan ruling 1).

use fr_board::{
    Board, BoardLibrary, BoardRules, ClearanceMatrix, Communication, Components, FixedState,
    ItemClass, ItemIdGenerator, Layer, LayerStructure, Packages, Padstacks, equals_ignore_case,
};
use fr_geometry::{Area, IntBox, PolylineShapeRef, Shape, TileShape};

use crate::coordinate_transform::CoordinateTransform;
use crate::error::DsnError;
use crate::format::java_round_to_int;
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::autoroute_settings::read_autoroute_settings_scope;
use crate::parser::dsn_file::{CLASS_CLEARANCE_SEPARATOR, read_string_scope};
use crate::parser::geometry::{self as shape, DsnPolygonPath, DsnShape, ReadAreaScopeResult};
use crate::parser::header::read_flip_style_rotate_first;
use crate::parser::network::{DsnNet, NetId};
use crate::parser::scope_parameter::{ReadScopeParameter, skip_scope};

// `LayerStructure.java` and `Layer.java` live in `parser/geometry.rs` (Plan 3 Task 5, whose file
// list puts them there next to the shapes that carry a `Layer`); re-exported here because this
// is the scope that reads them and the name is part of the `structure` scope's vocabulary.
pub use crate::parser::geometry::{DsnLayer, DsnLayerStructure};

/// `Limits.CRIT_INT` as a `f64`, for the scale-factor overflow loop (Structure.java:1211).
const CRIT_INT: f64 = fr_geometry::CRIT_INT as f64;

// ------------------------------------------------------------------------------ Rule.java

/// `io/specctra/parser/Rule.java`'s two concrete rule classes, as one enum (Rule is an abstract
/// class whose subclasses carry no shared state, and every consumer is an `instanceof` chain —
/// Structure.java:614,627,653,678).
#[derive(Debug, Clone, PartialEq)]
pub enum DsnRule {
    /// `Rule.WidthRule` (Rule.java:284-291): a trace width, in DSN units.
    Width(f64),
    /// `Rule.ClearanceRule` (Rule.java:293-302).
    Clearance(DsnClearanceRule),
}

/// `Rule.ClearanceRule` (Rule.java:293-302).
#[derive(Debug, Clone, PartialEq)]
pub struct DsnClearanceRule {
    /// `ClearanceRule.value`, in DSN units.
    pub value: f64,
    /// `ClearanceRule.clearanceClassPairs`, the `(type …)` list. Empty means "the default
    /// clearance", which [`set_clearance_rule`] handles separately.
    pub clearance_class_pairs: Vec<String>,
}

/// `Rule.readScope` (Rule.java:24-63): the `(rule …)` scope's body, a list of `width`/`clearance`
/// rules. `None` is Java's `null` (end of file).
// renamed: Rule.readScope -> read_rule_scope (see this module's docs on why it lives here).
pub fn read_rule_scope(scanner: &mut DsnScanner) -> Result<Option<Vec<DsnRule>>, DsnError> {
    let mut result = Vec::new();
    let mut prev_was_open = false;
    loop {
        let Some(current_token) = scanner.next_token()? else {
            // "unexpected end of file" (Rule.java:35-39).
            return Ok(None);
        };
        if current_token == Token::Close {
            // end of scope
            break;
        }
        let is_open = current_token == Token::Open;
        if prev_was_open {
            // every rule starts with a "("
            let current_rule = match current_token {
                Token::Kw(Keyword::Width) => read_width_rule(scanner)?,
                Token::Kw(Keyword::Clearance) => read_clearance_rule(scanner)?,
                _ => {
                    let _ = skip_scope(scanner)?;
                    None
                }
            };
            if let Some(rule) = current_rule {
                result.push(rule);
            }
        }
        prev_was_open = is_open;
    }
    Ok(Some(result))
}

/// `Rule.readWidthRule` (Rule.java:106-113).
///
// totalized: Rule.readWidthRule — Java's `double value = scanner.nextDouble()` unboxes a
// `Double` that `nextDouble` returns as `null` for a non-numeric token (IJFlexScanner.java:32),
// so `(width abc)` throws a `NullPointerException` that nothing between here and
// `DsnReader.readBoard` catches. The port answers `None`, which is the value Java's *other*
// failure branch on the very next line (a missing closing bracket) already returns and every
// caller already handles by dropping the rule (Rule.java:57).
pub fn read_width_rule(scanner: &mut DsnScanner) -> Result<Option<DsnRule>, DsnError> {
    let value = scanner.next_double();
    if !scanner.next_closing_bracket()? {
        return Ok(None);
    }
    Ok(value.map(DsnRule::Width))
}

/// `Rule.readClearanceRule` (Rule.java:244-282).
///
// totalized: Rule.readClearanceRule — same `nextDouble` unboxing NPE as `readWidthRule`, and the
// same totalization to the `null` its callers already handle.
pub fn read_clearance_rule(scanner: &mut DsnScanner) -> Result<Option<DsnRule>, DsnError> {
    let Some(value) = scanner.next_double() else {
        return Ok(None);
    };
    let mut class_pairs: Vec<String> = Vec::new();
    let next_token = scanner.next_token()?;
    if next_token != Some(Token::Close) {
        // look for "(type"
        if next_token != Some(Token::Open) {
            // "( expected" (Rule.java:251-255).
            return Ok(None);
        }
        if scanner.next_token()? != Some(Token::Kw(Keyword::Type)) {
            // "type expected" (Rule.java:257-261).
            return Ok(None);
        }
        class_pairs.extend(scanner.next_string_list_sep(CLASS_CLEARANCE_SEPARATOR));
        // check the closing ")" of "(type"
        if !scanner.next_closing_bracket()? {
            return Ok(None);
        }
        // check the closing ")" of "(clear"
        if !scanner.next_closing_bracket()? {
            return Ok(None);
        }
    }
    Ok(Some(DsnRule::Clearance(DsnClearanceRule {
        value,
        clearance_class_pairs: class_pairs,
    })))
}

// ----------------------------------------------------------------------------- Plane.java

/// `io/specctra/parser/ReadScopeParameter.java`'s nested `PlaneInfo` (ReadScopeParameter.java:
/// 176-190): a plane read from a `plane` scope, held until the whole `structure` scope has been
/// read before it is inserted into the board (`ReadScopeParameter.planeList`'s element type).
///
/// Java wins over the plan brief here: the brief's shape for this type was
/// `{ net_name, shape, clearance_class, windows }`, i.e. `ReadAreaScopeResult` flattened. Java's
/// `PlaneInfo` holds the `ReadAreaScopeResult` whole, and that matters — `readAreaScope` can put
/// a **`null`** into `shapeList` for a window it could not read (Shape.java:222-223), which a
/// `Vec<DsnShape>` of windows could not represent, and `transformAreaToBoard` consumes the border
/// and the windows as one list. So the field is the result object, not its parts.
#[derive(Debug, Clone, PartialEq)]
pub struct DsnPlane {
    /// `PlaneInfo.area` (ReadScopeParameter.java:179).
    pub area: ReadAreaScopeResult,
    /// `PlaneInfo.netName` (ReadScopeParameter.java:180).
    pub net_name: String,
}

impl DsnPlane {
    /// `PlaneInfo(Shape.ReadAreaScopeResult, String)` (ReadScopeParameter.java:182-185).
    #[must_use]
    pub fn new(area: ReadAreaScopeResult, net_name: impl Into<String>) -> DsnPlane {
        DsnPlane {
            area,
            net_name: net_name.into(),
        }
    }
}

/// `Plane.readScope` (Plane.java:52-83): a `(plane <net> <shape> (window …)*)` scope. Called only
/// from within `Structure.readScope` (Structure.java:1001-1005), not from the top-level `pcb`
/// scope.
///
/// `skipWindowScopes` is on when the host CAD is Allegro (Plane.java:57): "Cadence Allegro cutouts
/// the pins on power planes, which leads to performance problems when dividing a conduction area
/// into convex pieces." The comparison is `"allegro".equalsIgnoreCase(hostCad)`, i.e. Java's
/// *simple* case mapping ([`fr_board::equals_ignore_case`]), not an ASCII fold.
///
// totalized: Plane.readScope — `Shape.readAreaScope` can return `null` (end of file, or a border
// shape it could not read), and Java stores it in the `PlaneInfo` unchecked, so
// `Structure.readScope`'s plane loop then NPEs on `planeInfo.area.shapeList`
// (Structure.java:1084). The port drops the plane instead and returns `true`, matching Java's
// behaviour for every plane it *can* read and skipping only the one that would have crashed.
pub fn read_plane_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let skip_window_scopes = p
        .host_cad
        .as_deref()
        .is_some_and(|host| equals_ignore_case("allegro", host));

    // read the net name
    let Some(Token::Str(net_name)) = p.scanner.next_token()? else {
        // "String expected" (Plane.java:60-66).
        return Ok(false);
    };
    p.scanner.set_scope_identifier(&net_name);
    let conduction_area = shape::read_area_scope(
        &mut p.scanner,
        p.layer_structure.as_ref(),
        skip_window_scopes,
    )?;
    if let Some(conduction_area) = conduction_area {
        p.plane_list.push(DsnPlane::new(conduction_area, net_name));
    }
    Ok(true)
}

// ------------------------------------------------------------------------- Structure.java

/// `Structure.KeepoutType` (Structure.java:1295-1299).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeepoutType {
    Keepout,
    ViaKeepout,
    PlaceKeepout,
}

/// `Structure.BoardConstructionInfo` (Structure.java:1301-1310): everything the `structure` scope
/// accumulates before `createBoard` turns it into a [`Board`].
#[derive(Debug, Default)]
struct BoardConstructionInfo {
    /// `BoardConstructionInfo.layerInfo`.
    layer_info: Vec<DsnLayer>,
    /// `BoardConstructionInfo.boundingShape`.
    bounding_shape: Option<DsnShape>,
    /// `BoardConstructionInfo.outlineShapes`.
    outline_shapes: Vec<DsnShape>,
    /// `BoardConstructionInfo.outlineClearanceClassName`.
    outline_clearance_class_name: Option<String>,
    /// `BoardConstructionInfo.foundLayerCount`.
    found_layer_count: i32,
    /// `BoardConstructionInfo.defaultRules`.
    default_rules: Vec<DsnRule>,
    /// `BoardConstructionInfo.layerDependentRules`.
    layer_dependent_rules: Vec<StructureLayerRule>,
}

/// `Structure.LayerRule` (Structure.java:1312-1321) — the private nested class, not
/// `Rule.LayerRule`.
// renamed: Structure.LayerRule -> StructureLayerRule (Rule.java has a `LayerRule` of its own,
// which Task 8 ports; Rust has no nesting to keep the two names apart).
#[derive(Debug)]
struct StructureLayerRule {
    layer_name: String,
    rule: Vec<DsnRule>,
}

/// `Structure.readScope` (Structure.java:936-1137): the `(structure …)` scope.
///
/// See this module's docs for the insertion order, which is the whole point of this function.
///
/// Java's `FRLogger.warn` calls are dropped (no `tracing` in `fr-dsn`, and none of them pushes
/// onto `ReadScopeParameter.warnings`).
// renamed: Structure.readScope -> read_structure_scope.
#[allow(clippy::too_many_lines)] // Java's own 200-line method, kept in one piece for auditability.
pub fn read_structure_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let mut info = BoardConstructionInfo::default();

    // If true, components on the back side are rotated before mirroring.
    // The correct location is the scope PlaceControl, but Electra writes it here.
    let mut flip_style_rotate_first = false;

    let mut keepout_list: Vec<Option<ReadAreaScopeResult>> = Vec::new();
    let mut via_keepout_list: Vec<Option<ReadAreaScopeResult>> = Vec::new();
    let mut place_keepout_list: Vec<Option<ReadAreaScopeResult>> = Vec::new();

    let mut prev_was_open = false;
    loop {
        let Some(next_token) = p.scanner.next_token()? else {
            // "unexpected end of file" (Structure.java:956-962).
            return Ok(false);
        };
        if next_token == Token::Close {
            // end of scope
            break;
        }
        let is_open = next_token == Token::Open;
        let mut read_ok = true;
        if prev_was_open {
            match next_token {
                Token::Kw(Keyword::Boundary) => {
                    // Java discards `readBoundaryScope`'s boolean (Structure.java:970).
                    let _ = read_boundary_scope(&mut p.scanner, &mut info)?;
                }
                Token::Kw(Keyword::Layer) => {
                    read_ok = read_layer_scope(&mut p.scanner, &mut info, &p.string_quote)?;
                    if p.layer_structure.is_some() {
                        // correct the layerStructure because another layer is read
                        p.layer_structure = Some(DsnLayerStructure::new(info.layer_info.clone()));
                    }
                }
                Token::Kw(Keyword::Via) => {
                    // totalized: Structure.readViaPadstacks returns `null` on a non-string
                    // entry, and Java assigns that `null` straight to
                    // `scopeParameter.viaPadstackNames` (Structure.java:980), where
                    // `Network.readScope` later NPEs on it. `ReadScopeParameter.via_padstack_names`
                    // is a plain `Vec` in this port (see its docs), so the failure becomes an
                    // empty list.
                    p.via_padstack_names = read_via_padstacks(&mut p.scanner)?.unwrap_or_default();
                }
                Token::Kw(Keyword::Rule) => {
                    // totalized: Rule.readScope's `null` — Java's `addAll(null)` NPEs
                    // (Structure.java:982).
                    if let Some(rules) = read_rule_scope(&mut p.scanner)? {
                        info.default_rules.extend(rules);
                    }
                }
                Token::Kw(Keyword::Keepout) => {
                    ensure_layer_structure(p, &info);
                    keepout_list.push(shape::read_area_scope(
                        &mut p.scanner,
                        p.layer_structure.as_ref(),
                        false,
                    )?);
                }
                Token::Kw(Keyword::ViaKeepout) => {
                    ensure_layer_structure(p, &info);
                    via_keepout_list.push(shape::read_area_scope(
                        &mut p.scanner,
                        p.layer_structure.as_ref(),
                        false,
                    )?);
                }
                Token::Kw(Keyword::PlaceKeepout) => {
                    ensure_layer_structure(p, &info);
                    place_keepout_list.push(shape::read_area_scope(
                        &mut p.scanner,
                        p.layer_structure.as_ref(),
                        false,
                    )?);
                }
                Token::Kw(Keyword::PlaneScope) => {
                    ensure_layer_structure(p, &info);
                    // Java discards `Keyword.PLANE_SCOPE.readScope`'s boolean
                    // (Structure.java:1004).
                    let _ = read_plane_scope(p)?;
                }
                Token::Kw(Keyword::AutorouteSettings) => {
                    // Java bug: Structure.readScope (Structure.java:1006-1012) puts the
                    // `AutorouteSettings.readScope` call **inside** the
                    // `if (scopeParameter.layerStructure == null)` guard that every sibling
                    // branch uses only to *create* the layer structure. Any `keepout`,
                    // `via_keepout`, `place_keepout` or `plane` scope earlier in the same
                    // `structure` scope has already created it, and then the whole
                    // `autoroute_settings` scope is neither read nor skipped: its body is
                    // re-tokenised by this loop (each `(autoroute …)`/`(via_costs …)` falls to
                    // `skipScope`) and its closing bracket ends the `structure` scope one scope
                    // early. Reproduced verbatim — see `docs/java-quirks.md`.
                    if p.layer_structure.is_none() {
                        p.layer_structure = Some(DsnLayerStructure::new(info.layer_info.clone()));
                        let layer_structure =
                            p.layer_structure.clone().expect("just assigned above");
                        p.autoroute_settings =
                            read_autoroute_settings_scope(&mut p.scanner, &layer_structure)?;
                    }
                }
                Token::Kw(Keyword::Control) => {
                    read_ok = read_control_scope(p)?;
                }
                Token::Kw(Keyword::FlipStyle) => {
                    flip_style_rotate_first = read_flip_style_rotate_first(&mut p.scanner)?;
                }
                Token::Kw(Keyword::SnapAngle) => {
                    if let Some(snap_angle) = read_snap_angle(&mut p.scanner)? {
                        p.snap_angle = snap_angle;
                    }
                }
                _ => {
                    let _ = skip_scope(&mut p.scanner)?;
                }
            }
        }
        if !read_ok {
            return Ok(false);
        }
        prev_was_open = is_open;
    }

    // let's create a board based on the data we read (Structure.java:1030-1041)
    let mut result = true;
    if p.board.is_none() {
        result = create_board(p, &mut info)?;
    }
    if p.board.is_none() {
        return Ok(false);
    }
    if flip_style_rotate_first {
        p.board
            .as_mut()
            .expect("checked just above")
            .components
            .set_flip_style_rotate_first(true);
    }

    // insert the keepouts (Structure.java:1043-1067)
    for current_area in &keepout_list {
        if !insert_keepout(
            current_area.as_ref(),
            p,
            KeepoutType::Keepout,
            FixedState::SystemFixed,
        )? {
            return Ok(false);
        }
    }
    for current_area in &via_keepout_list {
        if !insert_keepout(
            current_area.as_ref(),
            p,
            KeepoutType::ViaKeepout,
            FixedState::SystemFixed,
        )? {
            return Ok(false);
        }
    }
    for current_area in &place_keepout_list {
        if !insert_keepout(
            current_area.as_ref(),
            p,
            KeepoutType::PlaceKeepout,
            FixedState::SystemFixed,
        )? {
            return Ok(false);
        }
    }

    // insert the planes (Structure.java:1069-1121)
    if !insert_planes(p)? {
        return Ok(false);
    }

    insert_missing_power_planes(&info.layer_info, p);

    // not ported: Structure.readScope's routing-job hand-off (Structure.java:1125-1134) —
    // `RoutingJob`/`routerSettings.applyNewValuesFrom` is `core/`, outside this crate; the
    // settings this port read stay on `ReadScopeParameter::autoroute_settings` for the caller.

    Ok(result)
}

/// The `if (scopeParameter.layerStructure == null) { … = new LayerStructure(layerInfo); }` guard
/// that five of `Structure.readScope`'s branches share (Structure.java:986-1010).
fn ensure_layer_structure(p: &mut ReadScopeParameter<'_>, info: &BoardConstructionInfo) {
    if p.layer_structure.is_none() {
        p.layer_structure = Some(DsnLayerStructure::new(info.layer_info.clone()));
    }
}

/// `Structure.readBoundaryScope` (Structure.java:271-302).
fn read_boundary_scope(
    scanner: &mut DsnScanner,
    info: &mut BoardConstructionInfo,
) -> Result<bool, DsnError> {
    let current_shape = shape::read_scope(scanner, None)?;
    let mut prev_was_open = false;
    loop {
        let next_token = scanner.next_token()?;
        if next_token == Some(Token::Close) {
            break;
        }
        // Java's loop has no end-of-file guard at all (Structure.java:275-289): `nextToken()`
        // answers `null`, which is neither bracket, so it spins forever appending nothing.
        // totalized: Structure.readBoundaryScope loops forever at end of file; the port stops.
        let Some(token) = next_token else {
            return Ok(true);
        };
        let is_open = token == Token::Open;
        if prev_was_open {
            if token == Token::Kw(Keyword::ClearanceClass) {
                info.outline_clearance_class_name = Some(read_string_scope(scanner)?);
            } else {
                let additional_shape = shape::read_scope_from_keyword(scanner, Some(token), None)?;
                add_boundary_shape(info, additional_shape);
            }
        }
        prev_was_open = is_open;
    }
    // Java warns and returns `true` for a null shape here (Structure.java:292-296).
    add_boundary_shape(info, current_shape);
    Ok(true)
}

/// `Structure.addBoundaryShape` (Structure.java:304-321).
fn add_boundary_shape(info: &mut BoardConstructionInfo, shape: Option<DsnShape>) {
    let Some(shape) = shape else {
        return;
    };
    if matches!(shape, DsnShape::PolylinePath(_) | DsnShape::Path(_)) {
        info.outline_shapes.push(shape);
        return;
    }
    // Java compares against the shared `Layer.PCB`/`Layer.SIGNAL` constants by reference; the
    // port owns its layers, so this is value equality against the same two constants.
    if *shape.layer() == DsnLayer::pcb() {
        if info.bounding_shape.is_none() {
            info.bounding_shape = Some(shape);
        } else {
            info.outline_shapes.push(shape);
        }
    } else if *shape.layer() == DsnLayer::signal() {
        info.outline_shapes.push(shape);
    }
    // else: "unexpected layer at boundary", an `FRLogger.warn` this port drops.
}

/// `Structure.readLayerScope` (Structure.java:323-403).
///
/// `_string_quote` is Java's `stringQuote` parameter (Structure.java:324), which its body never
/// reads; kept so the signature matches the Java method it ports.
fn read_layer_scope(
    scanner: &mut DsnScanner,
    info: &mut BoardConstructionInfo,
    _string_quote: &str,
) -> Result<bool, DsnError> {
    let mut layer_ok = true;
    let mut is_signal = true;

    let layer_string = scanner.next_string();
    let mut net_names: Vec<String> = Vec::new();

    let mut next_token = scanner.next_token()?;
    while next_token != Some(Token::Close) {
        if next_token != Some(Token::Open) {
            // "( expected" (Structure.java:333-337).
            return Ok(false);
        }
        next_token = scanner.next_token()?;
        match next_token {
            Some(Token::Kw(Keyword::Type)) => {
                next_token = scanner.next_token()?;
                if next_token == Some(Token::Kw(Keyword::Power)) {
                    is_signal = false;
                } else if next_token != Some(Token::Kw(Keyword::Signal))
                    && !matches!(&next_token, Some(Token::Str(s)) if s == Keyword::Jumper.name())
                {
                    // Java compares `nextToken.toString()` against `Keyword.JUMPER.getName()`
                    // (Structure.java:349). `Keyword` has no `toString` override, so that test
                    // can only succeed for a `String` token — which is exactly what the lexer
                    // produces for `jumper` (quirk: the DFA never yields `Keyword.JUMPER`).
                    layer_ok = false;
                }
                if scanner.next_token()? != Some(Token::Close) {
                    // ") expected" (Structure.java:367-371).
                    return Ok(false);
                }
            }
            Some(Token::Kw(Keyword::Rule)) => {
                // totalized: Rule.readScope's `null` — Java's `new LayerRule(name, null)` then
                // NPEs when `updateBoardRules` iterates it (Structure.java:646).
                let current_rules = read_rule_scope(scanner)?.unwrap_or_default();
                info.layer_dependent_rules.push(StructureLayerRule {
                    layer_name: layer_string.clone(),
                    rule: current_rules,
                });
            }
            Some(Token::Kw(Keyword::UseNet)) => loop {
                scanner.yybegin(LexicalState::Name);
                next_token = scanner.next_token()?;
                match next_token {
                    Some(Token::Close) => break,
                    Some(Token::Str(ref s)) => net_names.push(s.clone()),
                    // "string expected", an `FRLogger.warn` this port drops. Java keeps looping,
                    // which spins forever at end of file; the port stops.
                    // totalized: Structure.readLayerScope's `use_net` loop at end of file.
                    None => return Ok(false),
                    Some(_) => {}
                }
            },
            _ => {
                let _ = skip_scope(scanner)?;
            }
        }
        next_token = scanner.next_token()?;
    }
    if layer_ok {
        let current_layer =
            DsnLayer::with_nets(layer_string, info.found_layer_count, is_signal, net_names);
        info.layer_info.push(current_layer);
        info.found_layer_count += 1;
    }
    Ok(true)
}

/// `Structure.readViaPadstacks` (Structure.java:405-441): the `(via <name>* (spare <name>*))`
/// scope. `None` is Java's `null`.
fn read_via_padstacks(scanner: &mut DsnScanner) -> Result<Option<Vec<String>>, DsnError> {
    let mut normal_vias: Vec<String> = Vec::new();
    let mut spare_vias: Vec<String> = Vec::new();
    loop {
        let next_token = scanner.next_token()?;
        match next_token {
            Some(Token::Close) => break,
            Some(Token::Open) => {
                if scanner.next_token()? == Some(Token::Kw(Keyword::Spare)) {
                    spare_vias = read_via_padstacks(scanner)?.unwrap_or_default();
                } else {
                    let _ = skip_scope(scanner)?;
                }
            }
            Some(Token::Str(s)) => normal_vias.push(s),
            // "String expected" (Structure.java:428-433), and end of file, where Java's loop
            // would spin: both answer `null` here.
            _ => return Ok(None),
        }
    }
    // add the spare vias to the end of the list
    normal_vias.extend(spare_vias);
    Ok(Some(normal_vias))
}

/// `Structure.readControlScope` (Structure.java:443-476).
fn read_control_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let mut prev_was_open = false;
    loop {
        let Some(next_token) = p.scanner.next_token()? else {
            // "unexpected end of file" (Structure.java:452-458).
            return Ok(false);
        };
        if next_token == Token::Close {
            // end of scope
            break;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            if next_token == Token::Kw(Keyword::ViaAtSmd) {
                p.via_at_smd_allowed = crate::parser::dsn_file::read_on_off_scope(&mut p.scanner)?;
            } else {
                let _ = skip_scope(&mut p.scanner)?;
            }
        }
        prev_was_open = is_open;
    }
    Ok(true)
}

/// `Structure.readSnapAngle` (Structure.java:478-509). `None` is Java's `null`, which the caller
/// treats as "leave the current snap angle alone" (Structure.java:1017-1020).
pub fn read_snap_angle(
    scanner: &mut DsnScanner,
) -> Result<Option<fr_board::AngleRestriction>, DsnError> {
    use fr_board::AngleRestriction;
    let snap_angle = match scanner.next_token()? {
        Some(Token::Kw(Keyword::NinetyDegree)) => AngleRestriction::NinetyDegree,
        Some(Token::Kw(Keyword::FortyfiveDegree)) => AngleRestriction::FortyFiveDegree,
        Some(Token::Kw(Keyword::None)) => AngleRestriction::None,
        // "unexpected token" (Structure.java:491-496).
        _ => return Ok(None),
    };
    if scanner.next_token()? != Some(Token::Close) {
        // "closing bracket expected" (Structure.java:498-504).
        return Ok(None);
    }
    Ok(Some(snap_angle))
}

// ------------------------------------------------------------- keepouts, planes, power planes

/// `Structure.insertKeepout(ReadAreaScopeResult, …)` (Structure.java:834-882).
///
// totalized: Structure.insertKeepout — Java dereferences `area` (which `Shape.readAreaScope` may
// have returned as `null`), `area.shapeList`'s first element and
// `Shape.transformAreaToBoard`'s result, none of them checked. Each of those NPEs aborts the
// whole read. The port folds them into the branch Java already has one line further down for a
// keepout it cannot enforce (`dimension() < 2`, Structure.java:840-853): the keepout is dropped
// and the read continues, which is the same observable board for every input Java does not crash
// on.
fn insert_keepout(
    area: Option<&ReadAreaScopeResult>,
    p: &mut ReadScopeParameter<'_>,
    keepout_type: KeepoutType,
    fixed_state: FixedState,
) -> Result<bool, DsnError> {
    let Some(area) = area else {
        return Ok(true);
    };
    let Some(coordinate_transform) = p.coordinate_transform else {
        return Ok(true);
    };
    let Some(keepout_area) =
        shape::transform_area_to_board(&area.shape_list, &coordinate_transform)
    else {
        return Ok(true);
    };
    if keepout_area.dimension() < 2 {
        // A degenerate keepout (e.g. all polygon vertices identical, exported incorrectly by the
        // EDA tool) cannot be enforced as a routing constraint. The board remains valid — the
        // keepout restriction is simply not applied. Java's `FRLogger.warn` is dropped.
        return Ok(true);
    }
    let Some(current_layer) = area
        .shape_list
        .first()
        .and_then(Option::as_ref)
        .map(DsnShape::layer)
    else {
        return Ok(true);
    };
    let current_layer = current_layer.clone();
    let clearance_class_name = area.clearance_class_name.clone();

    let layer_structure = p.layer_structure.clone().unwrap_or_default();
    let Some(board) = p.board.as_mut() else {
        // "board not initialized" (Structure.java:855-858).
        return Ok(false);
    };
    if current_layer == DsnLayer::signal() {
        for i in 0..board.get_layer_count() {
            if layer_structure.layers.get(i).is_some_and(|l| l.is_signal) {
                insert_keepout_on_layer(
                    board,
                    keepout_area.clone(),
                    i,
                    clearance_class_name.as_deref(),
                    keepout_type,
                    fixed_state,
                );
            }
        }
    } else if current_layer.no >= 0 {
        insert_keepout_on_layer(
            board,
            keepout_area,
            current_layer.no as usize,
            clearance_class_name.as_deref(),
            keepout_type,
            fixed_state,
        );
    } else {
        // "unknown layer name" (Structure.java:875-880).
        return Ok(false);
    }
    Ok(true)
}

/// `Structure.insertKeepout(BasicBoard, Area, int, String, KeepoutType, FixedState)`
/// (Structure.java:884-916).
// renamed: the six-argument `insertKeepout` overload -> insert_keepout_on_layer.
fn insert_keepout_on_layer(
    board: &mut Board,
    area: Area,
    layer: usize,
    clearance_class_name: Option<&str>,
    keepout_type: KeepoutType,
    fixed_state: FixedState,
) {
    let clearance_class_index = match clearance_class_name {
        None => {
            let default_class = board.rules.get_default_net_class();
            board
                .rules
                .net_classes
                .get(default_class)
                .default_item_clearance_classes
                .get(ItemClass::Area)
        }
        Some(name) => board
            .rules
            .clearance_matrix
            .get_no(name)
            // "clearance class not found" (Structure.java:902-906).
            .unwrap_or_else(BoardRules::clearance_class_none),
    };
    match keepout_type {
        KeepoutType::ViaKeepout => {
            board.insert_via_obstacle(area, layer, clearance_class_index, fixed_state);
        }
        KeepoutType::PlaceKeepout => {
            board.insert_component_obstacle(area, layer, clearance_class_index, fixed_state);
        }
        KeepoutType::Keepout => {
            board.insert_obstacle(area, layer, clearance_class_index, fixed_state);
        }
    }
}

/// `Structure.readScope`'s plane loop (Structure.java:1069-1121), one conduction area per entry
/// of `ReadScopeParameter.planeList`, creating any missing net first.
fn insert_planes(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    // Java iterates `scopeParameter.planeList` in place; nothing inside the loop appends to it,
    // so taking it out and putting it back is equivalent and keeps the borrows disjoint.
    let plane_list = std::mem::take(&mut p.plane_list);
    let result = insert_planes_inner(p, &plane_list);
    p.plane_list = plane_list;
    result
}

fn insert_planes_inner(
    p: &mut ReadScopeParameter<'_>,
    plane_list: &[DsnPlane],
) -> Result<bool, DsnError> {
    let Some(coordinate_transform) = p.coordinate_transform else {
        return Ok(true);
    };
    for plane_info in plane_list {
        let net_id = NetId {
            name: plane_info.net_name.clone(),
            subnet_no: 1,
        };
        if !p.netlist.contains_key(&net_id) {
            p.netlist
                .insert(net_id.clone(), DsnNet::new(net_id.clone()));
            let Some(board) = p.board.as_mut() else {
                return Ok(false);
            };
            let default_class = board.rules.get_default_net_class();
            board
                .rules
                .nets
                .add(net_id.name.clone(), net_id.subnet_no, true, default_class);
        }
        let Some(board) = p.board.as_mut() else {
            return Ok(false);
        };
        let Some(current_net) = board
            .rules
            .nets
            .get_by_name_and_subnet(&plane_info.net_name, 1)
        else {
            // "net not found" (Structure.java:1078-1083).
            continue;
        };
        let net_number = current_net.net_number;
        let net_class = current_net.get_net_class();

        // totalized: Structure.readScope's plane loop — `Shape.transformAreaToBoard` and
        // `planeInfo.area.shapeList`'s first element are both dereferenced unchecked
        // (Structure.java:1084-1086). Same treatment as `insert_keepout`: drop the plane.
        let Some(plane_area) =
            shape::transform_area_to_board(&plane_info.area.shape_list, &coordinate_transform)
        else {
            continue;
        };
        let Some(current_layer) = plane_info
            .area
            .shape_list
            .first()
            .and_then(Option::as_ref)
            .map(DsnShape::layer)
        else {
            continue;
        };
        if current_layer.no < 0 {
            // "unexpected layer name" (Structure.java:1113-1118) — the one branch here that
            // fails the whole read.
            return Ok(false);
        }
        let layer_no = current_layer.no as usize;
        let clearance_class_index = match &plane_info.area.clearance_class_name {
            Some(name) => board
                .rules
                .clearance_matrix
                .get_no(name)
                // "clearance class not found" (Structure.java:1091-1097).
                .unwrap_or_else(BoardRules::clearance_class_none),
            None => board
                .rules
                .net_classes
                .get(net_class)
                .default_item_clearance_classes
                .get(ItemClass::Area),
        };
        board.insert_conduction_area(
            plane_area,
            layer_no,
            vec![net_number],
            clearance_class_index,
            false,
            FixedState::SystemFixed,
        );
    }
    Ok(true)
}

/// `Structure.insertMissingPowerPlanes` (Structure.java:539-571): every non-signal layer that
/// carries a net name but no conduction area gets one covering the whole board.
///
/// Java fetches `board.getConductionAreas()` **once**, before the loop (Structure.java:541), so a
/// conduction area this method itself inserts is not visible to later iterations. Reproduced.
fn insert_missing_power_planes(layer_info: &[DsnLayer], p: &mut ReadScopeParameter<'_>) {
    let Some(board) = p.board.as_mut() else {
        return;
    };
    let conduction_area_layers: Vec<i32> = board
        .get_conduction_areas()
        .into_iter()
        .filter_map(|id| match board.get_item(id) {
            Some(fr_board::Item::ConductionArea(area)) => i32::try_from(area.area.get_layer()).ok(),
            _ => None,
        })
        .collect();

    for current_layer in layer_info {
        if current_layer.is_signal {
            continue;
        }
        let conduction_area_found = conduction_area_layers.contains(&current_layer.no);
        if conduction_area_found || current_layer.net_names.is_empty() {
            continue;
        }
        let current_net_name = current_layer.net_names[0].clone();
        let current_net_id = NetId {
            name: current_net_name.clone(),
            subnet_no: 1,
        };
        if !p.netlist.contains_key(&current_net_id) {
            p.netlist
                .insert(current_net_id.clone(), DsnNet::new(current_net_id.clone()));
            let Some(board) = p.board.as_mut() else {
                return;
            };
            let default_class = board.rules.get_default_net_class();
            board.rules.nets.add(
                current_net_id.name.clone(),
                current_net_id.subnet_no,
                true,
                default_class,
            );
        }
        let Some(board) = p.board.as_mut() else {
            return;
        };
        let Some(current_net) = board
            .rules
            .nets
            .get_by_name_and_subnet(&current_net_id.name, current_net_id.subnet_no)
        else {
            // "net not found" (Structure.java:555-561).
            continue;
        };
        let net_number = current_net.net_number;
        if current_layer.no < 0 {
            continue;
        }
        let bounding_box = board.bounding_box;
        board.insert_conduction_area(
            Area::Shape(Shape::Tile(TileShape::Box(bounding_box))),
            current_layer.no as usize,
            vec![net_number],
            BoardRules::clearance_class_none(),
            false,
            FixedState::SystemFixed,
        );
    }
}

// --------------------------------------------------------------------------- board rules

/// `Structure.updateBoardRules` (Structure.java:608-667): applies the DSN `rule` scopes to the
/// board rules that are about to be handed to [`Board::new`].
fn update_board_rules(
    p: &ReadScopeParameter<'_>,
    info: &BoardConstructionInfo,
    board_rules: &mut BoardRules,
) {
    let Some(coordinate_transform) = p.coordinate_transform else {
        return;
    };
    let mut smd_to_turn_gap_found = false;
    // update the clearance matrix
    for current_object in &info.default_rules {
        if let DsnRule::Clearance(current_rule) = current_object
            && set_clearance_rule(
                current_rule,
                None,
                &coordinate_transform,
                board_rules,
                &p.string_quote,
            )
        {
            smd_to_turn_gap_found = true;
        }
    }
    // update width rules
    for current_object in &info.default_rules {
        if let DsnRule::Width(wire_width) = current_object {
            let trace_halfwidth =
                java_round_to_int(coordinate_transform.dsn_to_board(*wire_width) / 2.0);
            board_rules.set_default_trace_half_widths(trace_halfwidth);
        }
    }
    let layer_structure = p.layer_structure.clone().unwrap_or_default();
    for layer_rule in &info.layer_dependent_rules {
        let Some(layer_index) = layer_structure.get_no(&layer_rule.layer_name) else {
            continue;
        };
        for current_object in &layer_rule.rule {
            match current_object {
                DsnRule::Width(wire_width) => {
                    let trace_halfwidth =
                        java_round_to_int(coordinate_transform.dsn_to_board(*wire_width) / 2.0);
                    board_rules.set_default_trace_half_width(layer_index, trace_halfwidth);
                }
                DsnRule::Clearance(current_rule) => {
                    set_clearance_rule(
                        current_rule,
                        Some(layer_index),
                        &coordinate_transform,
                        board_rules,
                        &p.string_quote,
                    );
                }
            }
        }
    }
    if !smd_to_turn_gap_found {
        board_rules.set_pin_edge_to_turn_dist(f64::from(board_rules.get_min_trace_half_width()));
    }
}

/// `Structure.setClearanceRule` (Structure.java:673-808): "converts a dsn clearance rule into a
/// board clearance rule. If layerIndex is negative, the rule is set on all layers. Returns true,
/// if the string smd_to_turn_gap was found."
///
/// `layer_index` is `None` for Java's negative "all layers".
///
/// **Both `setValue` orders are mandatory.** Java writes `setValue(first, second, …)` *and*
/// `setValue(second, first, …)` (Structure.java:768-769,785-787) because
/// [`ClearanceMatrix::set_value`] writes exactly one cell with quirk #83's J-then-I indexing —
/// the symmetric pair is the only thing that keeps a DSN-sourced matrix symmetric.
pub fn set_clearance_rule(
    rule: &DsnClearanceRule,
    layer_index: Option<usize>,
    coordinate_transform: &CoordinateTransform,
    board_rules: &mut BoardRules,
    string_quote: &str,
) -> bool {
    let mut result = false;
    let current_clearance = java_round_to_int(coordinate_transform.dsn_to_board(rule.value));
    if rule.clearance_class_pairs.is_empty() {
        match layer_index {
            None => board_rules
                .clearance_matrix
                .set_default_value(current_clearance),
            Some(layer) => board_rules
                .clearance_matrix
                .set_default_value_on_layer(layer, current_clearance),
        }
        return result;
    }
    if contains_wire_clearance_pair(&rule.clearance_class_pairs) {
        create_default_clearance_classes(board_rules);
    }

    for current_string in &rule.clearance_class_pairs {
        if equals_ignore_case("smd_to_turn_gap", current_string) {
            board_rules.set_pin_edge_to_turn_dist(f64::from(current_clearance));
            result = true;
            continue;
        }
        let current_pair: [String; 2] = if rule.clearance_class_pairs.len() == 2 {
            // Java bug: this branch ignores `currentString` entirely and re-reads the first two
            // entries of the whole list on **every** iteration (Structure.java:734-744), so a
            // two-entry `(type a b)` applies the same pair twice; and the `for i` loop that
            // strips the quotes tests `currentPair[1]`'s leading `_` on both iterations, i.e.
            // once before `currentPair[1]`'s own quotes have been stripped and once after.
            // Reproduced verbatim.
            let mut pair = [
                rule.clearance_class_pairs[0].clone(),
                rule.clearance_class_pairs[1].clone(),
            ];
            for i in 0..2 {
                pair[i] = pair[i].replace('"', "");
                if let Some(stripped) = pair[1].strip_prefix('_') {
                    pair[1] = stripped.to_string();
                }
            }
            pair
        } else if let Some(rest) = current_string.strip_prefix(string_quote) {
            // split at the second occurrence of stringQuote
            let mut parts = rest.splitn(2, string_quote);
            let first = parts.next().unwrap_or_default().to_string();
            let Some(second) = parts.next() else {
                // "'_' expected" (Structure.java:750-756).
                continue;
            };
            let Some(second) = second.strip_prefix('_') else {
                continue;
            };
            [first, second.to_string()]
        } else {
            let mut parts = current_string.splitn(2, '_');
            let first = parts.next().unwrap_or_default().to_string();
            let Some(second) = parts.next() else {
                // pairs with more than 1 underline like smd_via_same_net are not implemented
                continue;
            };
            [first, second.to_string()]
        };

        let mut first_class_no = if current_pair[0] == "wire" {
            Some(1) // default class
        } else {
            board_rules.clearance_matrix.get_no(&current_pair[0])
        };
        if first_class_no.is_none() {
            first_class_no = Some(append_clearance_class(board_rules, &current_pair[0]));
        }
        let mut second_class_no = if current_pair[1] == "wire" {
            Some(1) // default class
        } else {
            board_rules.clearance_matrix.get_no(&current_pair[1])
        };
        if second_class_no.is_none() {
            second_class_no = Some(append_clearance_class(board_rules, &current_pair[1]));
        }
        let first_class_no = first_class_no.expect("assigned above");
        let second_class_no = second_class_no.expect("assigned above");

        match layer_index {
            None => {
                board_rules.clearance_matrix.set_value_on_all_layers(
                    first_class_no,
                    second_class_no,
                    current_clearance,
                );
                board_rules.clearance_matrix.set_value_on_all_layers(
                    second_class_no,
                    first_class_no,
                    current_clearance,
                );
            }
            Some(layer) => {
                board_rules.clearance_matrix.set_value(
                    first_class_no,
                    second_class_no,
                    layer,
                    current_clearance,
                );
                board_rules.clearance_matrix.set_value(
                    second_class_no,
                    first_class_no,
                    layer,
                    current_clearance,
                );
            }
        }
    }
    result
}

/// `Structure.containsWireClearancePair` (Structure.java:810-817).
fn contains_wire_clearance_pair(clearance_pairs: &[String]) -> bool {
    clearance_pairs
        .iter()
        .any(|p| p.starts_with("wire_") || p.ends_with("_wire"))
}

/// `Structure.createDefaultClearanceClasses` (Structure.java:819-824).
fn create_default_clearance_classes(board_rules: &mut BoardRules) {
    append_clearance_class(board_rules, "via");
    append_clearance_class(board_rules, "smd");
    append_clearance_class(board_rules, "pin");
    append_clearance_class(board_rules, "area");
}

/// `Structure.appendClearanceClass` (Structure.java:826-832).
fn append_clearance_class(board_rules: &mut BoardRules, name: &str) -> usize {
    board_rules.clearance_matrix.append_class(name);
    let result = board_rules
        .clearance_matrix
        .get_no(name)
        // `appendClass` either added the class or found it already present, so the lookup that
        // follows it in Java can never miss.
        .expect("appendClass leaves the class present");
    let default_net_class = board_rules.get_default_net_class();
    let net_class = board_rules.net_classes.get_mut(default_net_class);
    match name {
        "via" => net_class
            .default_item_clearance_classes
            .set(ItemClass::Via, result),
        "pin" => net_class
            .default_item_clearance_classes
            .set(ItemClass::Pin, result),
        "smd" => net_class
            .default_item_clearance_classes
            .set(ItemClass::Smd, result),
        "area" => net_class
            .default_item_clearance_classes
            .set(ItemClass::Area, result),
        // Ignore unsupported item classes.
        _ => {}
    }
    result
}

// ------------------------------------------------------------------------------ createBoard

/// `Structure.OutlineShape` (Structure.java:1324-1353): "used to separate the holes in the
/// outline".
// renamed: OutlineShape — Java's nested-class constructor `OutlineShape(PolylineShape)` becomes `OutlineShape::new` (Rust has no implicit constructors).
struct OutlineShape {
    shape: PolylineShapeRef,
    bounding_box: IntBox,
    convex_shapes: Option<Vec<TileShape>>,
    is_hole: bool,
}

impl OutlineShape {
    /// `OutlineShape(PolylineShape)` (Structure.java:1331-1336).
    fn new(shape: PolylineShapeRef) -> OutlineShape {
        let bounding_box = shape.as_ops().bounding_box();
        let convex_shapes = shape.split_to_convex();
        OutlineShape {
            shape,
            bounding_box,
            convex_shapes,
            is_hole: false,
        }
    }

    /// `OutlineShape.containsAllCorners` (Structure.java:1339-1352).
    fn contains_all_corners(&self, other_shape: &OutlineShape) -> bool {
        let Some(convex_shapes) = &self.convex_shapes else {
            // calculation of the convex shapes failed
            return false;
        };
        let corner_count = other_shape.shape.as_ops().border_line_count();
        for i in 0..corner_count {
            let current_corner = other_shape.shape.as_ops().corner(i);
            if !convex_shapes.iter().any(|s| s.contains(&current_corner)) {
                return false;
            }
        }
        true
    }
}

/// `Structure.separateHoles` (Structure.java:573-603): "calculates shapes in outlineShapes, which
/// are holes in the outline and returns them in the result list", **removing them from
/// `outline_shapes`** as Java's `outlineShapes.remove(...)` does.
fn separate_holes(outline_shapes: &mut Vec<PolylineShapeRef>) -> Vec<PolylineShapeRef> {
    let mut shapes: Vec<OutlineShape> = outline_shapes
        .iter()
        .cloned()
        .map(OutlineShape::new)
        .collect();
    for i in 0..shapes.len() {
        for j in 0..shapes.len() {
            // check if shapes[j] may be contained in shapes[i]
            if i == j || shapes[j].is_hole {
                continue;
            }
            if !shapes[j].bounding_box.contains(&shapes[i].bounding_box) {
                continue;
            }
            shapes[i].is_hole = shapes[j].contains_all_corners(&shapes[i]);
        }
    }
    let mut hole_list: Vec<PolylineShapeRef> = Vec::new();
    let mut kept: Vec<PolylineShapeRef> = Vec::new();
    for entry in shapes {
        if entry.is_hole {
            hole_list.push(entry.shape);
        } else {
            kept.push(entry.shape);
        }
    }
    *outline_shapes = kept;
    hole_list
}

/// `Structure.createBoard` (Structure.java:1139-1290): everything between the end of the
/// `structure` scope's token loop and a live [`Board`].
///
/// `Ok(false)` is Java's `false`: no layers, no outline (with `boardOutlineOk` cleared), a
/// degenerate bounding box, or an illegal layer number.
// renamed: Structure.createBoard -> create_board.
#[allow(clippy::too_many_lines)] // Java's own 150-line method, kept in one piece.
fn create_board(
    p: &mut ReadScopeParameter<'_>,
    info: &mut BoardConstructionInfo,
) -> Result<bool, DsnError> {
    let layer_count = info.layer_info.len();
    if layer_count == 0 {
        // "layers missing in structure scope" (Structure.java:1141-1147).
        return Ok(false);
    }
    if info.bounding_shape.is_none() {
        // happens if the boundary shape with layer pcb is missing
        if info.outline_shapes.is_empty() {
            // "outline missing" (Structure.java:1150-1157).
            p.board_outline_ok = false;
            return Ok(false);
        }
        // totalized: Structure.createBoard — `Shape.boundingBox()` is `null` for a
        // `PolylinePath` with fewer than two corners (PolylinePath.java:98-101), which Java
        // dereferences unchecked here (:1160-1164). The port skips such a shape; if every
        // outline shape is one, it takes the same "outline missing" exit two lines above.
        let mut boxes = info
            .outline_shapes
            .iter()
            .filter_map(DsnShape::bounding_box);
        let Some(mut bounding_box) = boxes.next() else {
            p.board_outline_ok = false;
            return Ok(false);
        };
        for other in boxes {
            bounding_box = bounding_box.union(&other);
        }
        info.bounding_shape = Some(DsnShape::Rect(bounding_box));
    }
    let bounding_shape = info
        .bounding_shape
        .as_ref()
        .expect("assigned just above when it was None");
    // totalized: the same `Shape.boundingBox()` null as above (Structure.java:1169).
    let Some(bounding_box) = bounding_shape.bounding_box() else {
        p.board_outline_ok = false;
        return Ok(false);
    };

    let mut board_layer_arr: Vec<Layer> = Vec::with_capacity(layer_count);
    for current_layer in &info.layer_info {
        if current_layer.no < 0 || current_layer.no as usize >= layer_count {
            // "illegal layer number" (Structure.java:1175-1181).
            return Ok(false);
        }
        board_layer_arr.push(Layer::new(
            current_layer.name.clone(),
            current_layer.is_signal,
        ));
    }
    let board_layer_structure = LayerStructure::new(board_layer_arr);
    p.layer_structure = Some(DsnLayerStructure::new(info.layer_info.clone()));

    // Calculate an approximate scaling between dsn coordinates and board coordinates.
    let mut scale_factor: i32 = p.resolution.max(1);

    let mut max_coor = 0.0_f64;
    for coordinate in bounding_box.coor {
        max_coor = max_coor.max((coordinate * f64::from(p.resolution)).abs());
    }
    if max_coor == 0.0 {
        p.board_outline_ok = false;
        return Ok(false);
    }
    // make scalefactor smaller, if there is a danger of integer overflow.
    //
    // Java bug: `scaleFactor` is an `int` and `/= 10` is **integer** division
    // (Structure.java:1211-1214), so it truncates to 0 as soon as the loop runs more times than
    // the resolution has decimal digits — which happens for any board whose boundary reaches
    // `CRIT_INT / 5 == 6_710_886` in DSN units, whatever the resolution. `CoordinateTransform`
    // then divides by zero and every DSN coordinate written back out is `Infinity`/`NaN`
    // (quirk #89). Reproduced exactly; do not widen to `f64`.
    //
    // This loop is also what makes quirk #82's Delaunay `positionLocate` unreachable for
    // imported boards: it keeps `5 * maxCoor` below `Limits.CRIT_INT` (2^25), so an imported
    // board's coordinates never reach the bounding triangle's corners.
    while 5.0 * max_coor >= CRIT_INT {
        scale_factor /= 10;
        max_coor /= 10.0;
    }

    let coordinate_transform = CoordinateTransform::new(f64::from(scale_factor), 0.0, 0.0);
    p.coordinate_transform = Some(coordinate_transform);

    let Shape::Tile(TileShape::Box(bounds)) =
        bounding_box.transform_to_board(&coordinate_transform)
    else {
        // `Rectangle.transformToBoard` always builds an `IntBox` (Rectangle.java:58-69); Java's
        // `(IntBox)` cast at Structure.java:1218 relies on exactly that.
        unreachable!("Rectangle.transformToBoard returns an IntBox");
    };
    let bounds = bounds.offset(1000.0);

    let mut board_outline_shapes: Vec<PolylineShapeRef> = Vec::new();
    for current_shape in &info.outline_shapes {
        let mut current_shape = current_shape.clone();
        if let DsnShape::Path(current_path) = &current_shape
            && current_path.width != 0.0
        {
            // set the width to 0, because the offset function used in transform_to_board is not
            // implemented for shapes, which are not convex.
            current_shape = DsnShape::Path(DsnPolygonPath::new(
                current_path.layer.clone(),
                0.0,
                current_path.coordinate_arr.clone(),
            ));
        }
        // totalized: Structure.createBoard casts `transformToBoard`'s result to `PolylineShape`
        // (:1237), which throws a `ClassCastException` for a `Circle` boundary (reachable: a
        // second `(circle pcb …)` inside a `boundary` scope lands in `outlineShapes`). The port
        // drops such a shape, which is what the `dimension() > 0` test on the next line would do
        // for any shape that carries no area anyway.
        let Some(current_board_shape) = current_shape
            .transform_to_board(&coordinate_transform)
            .and_then(to_polyline_shape)
        else {
            continue;
        };
        if current_board_shape.as_ops().dimension() > 0 {
            board_outline_shapes.push(current_board_shape);
        }
    }
    if board_outline_shapes.is_empty() {
        // construct an outline from the boundingShape, if the outline is missing.
        let bounding_shape = info.bounding_shape.as_ref().expect("set above");
        if let Some(current_board_shape) = bounding_shape
            .transform_to_board(&coordinate_transform)
            .and_then(to_polyline_shape)
        {
            board_outline_shapes.push(current_board_shape);
        }
    }
    let hole_shapes = separate_holes(&mut board_outline_shapes);

    let clearance_matrix = ClearanceMatrix::get_default_instance(&board_layer_structure, 0);
    let mut board_rules = BoardRules::new(board_layer_structure.clone(), clearance_matrix);
    let board_communication = Communication {
        string_quote: p.string_quote.clone(),
        constants: p.constants.clone(),
        write_resolution: p.write_resolution.clone(),
        dsn_file_generated_by_host: p.dsn_file_generated_by_host,
        ..Communication::new(
            p.unit,
            p.resolution,
            ItemIdGenerator::new(),
            p.host_cad.clone(),
            p.host_version.clone(),
        )
    };

    // not ported: Structure.createBoard's old-KiCad warning (Structure.java:1263-1268) — an
    // `FRLogger.warn` and nothing else: it is neither a `ReadScopeParameter.warnings` entry nor
    // a `BoardReadResult` variant, so it leaves no trace in anything this port returns.
    // `Communication::host_is_old_kicad` is already ported in `fr-board` for whoever wants it.

    update_board_rules(p, info, &mut board_rules);
    board_rules.trace_angle_restriction = p.snap_angle;

    // `ReadScopeParameter.MinimalBoardManager.createBoard` (ReadScopeParameter.java:139-166)
    // resolves the outline clearance class *name* to an index here, not in `BasicBoard`.
    let outline_clearance_no = match &info.outline_clearance_class_name {
        Some(name) => board_rules.clearance_matrix.get_no(name).unwrap_or(0),
        None => {
            let default_class = board_rules.get_default_net_class();
            board_rules
                .net_classes
                .get(default_class)
                .default_item_clearance_classes
                .get(ItemClass::Area)
        }
    };

    // Plan 2 obligation, phase 1 of 2: `BoardLibrary::via_padstacks` must be `Some` before any
    // via lookup, because `remove_via_padstack`/`get_mirrored_via_padstack` reproduce Java's NPE
    // on the `null` list (quirks #42-43), and `Board::new`'s own doc names the DSN reader as the
    // caller that owes this. The *names* are read here (`p.via_padstack_names`) but the padstacks
    // they refer to only exist once the `library` scope has been read, and Structure's names are
    // merged with `Network`'s at Network.java:1276 — so this is deliberately the empty list, and
    // phase 2 (Task 9's `Network.readScope`) replaces it with the resolved set.
    let mut library = BoardLibrary::new(
        Padstacks::new(board_layer_structure.clone()),
        Packages::new(),
    );
    library.set_via_padstacks(Vec::new());

    let board = Board::new(
        board_outline_shapes,
        outline_clearance_no,
        bounds,
        board_rules,
        library,
        Components::new(),
        board_communication,
    );
    p.board = Some(board);

    // Insert the holes in the board outline as keepouts (Structure.java:1279-1285). The layer
    // loop is the **inner** one: hole 0 on every layer, then hole 1 on every layer, …
    let board = p.board.as_mut().expect("just assigned");
    for current_outline_hole in &hole_shapes {
        for i in 0..board_layer_structure.layers.len() {
            board.insert_obstacle(
                Area::Shape(current_outline_hole.to_shape()),
                i,
                0,
                FixedState::SystemFixed,
            );
        }
    }

    Ok(true)
}

/// Java's `(PolylineShape)` narrowing of a `geometry.planar.Shape` — `TileShape` and
/// `PolygonShape` implement `PolylineShape`, `Circle` does not.
fn to_polyline_shape(shape: Shape) -> Option<PolylineShapeRef> {
    match shape {
        Shape::Tile(t) => Some(PolylineShapeRef::Tile(t)),
        Shape::Polygon(p) => Some(PolylineShapeRef::Polygon(p)),
        Shape::Circle(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::DsnScanner;

    fn scan(input: &str) -> DsnScanner {
        DsnScanner::new(input).expect("fits the buffer")
    }

    #[test]
    fn read_snap_angle_reads_the_three_keywords() {
        assert_eq!(
            read_snap_angle(&mut scan("ninety_degree)")).expect("scan"),
            Some(fr_board::AngleRestriction::NinetyDegree)
        );
        assert_eq!(
            read_snap_angle(&mut scan("fortyfive_degree)")).expect("scan"),
            Some(fr_board::AngleRestriction::FortyFiveDegree)
        );
        assert_eq!(
            read_snap_angle(&mut scan("none)")).expect("scan"),
            Some(fr_board::AngleRestriction::None)
        );
        // "unexpected token" and "closing bracket expected" both answer `null`.
        assert_eq!(read_snap_angle(&mut scan("on)")).expect("scan"), None);
        assert_eq!(
            read_snap_angle(&mut scan("none none)")).expect("scan"),
            None
        );
    }

    #[test]
    fn read_via_padstacks_appends_the_spare_vias_last() {
        // Structure.java:437-438: "add the spare vias to the end of the list".
        let mut scanner = scan("Via_1 Via_2 (spare Spare_1))");
        let vias = read_via_padstacks(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert_eq!(vias, vec!["Via_1", "Via_2", "Spare_1"]);
    }

    #[test]
    fn read_rule_scope_reads_width_and_clearance() {
        let mut scanner = scan("(width 152.4) (clearance 200 (type via_smd)))");
        let rules = read_rule_scope(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0], DsnRule::Width(152.4));
        let DsnRule::Clearance(clearance) = &rules[1] else {
            panic!("expected a clearance rule");
        };
        assert!((clearance.value - 200.0).abs() < f64::EPSILON);
        assert_eq!(clearance.clearance_class_pairs, vec!["via_smd".to_string()]);
    }

    #[test]
    fn contains_wire_clearance_pair_matches_either_end() {
        assert!(contains_wire_clearance_pair(&["wire_via".to_string()]));
        assert!(contains_wire_clearance_pair(&["via_wire".to_string()]));
        assert!(!contains_wire_clearance_pair(&["via_smd".to_string()]));
    }
}
