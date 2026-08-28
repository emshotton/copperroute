//! `io/specctra/parser/AutorouteSettings.java` — the `autoroute_settings` scope.

use std::io::Write;

use fr_board::LayerStructure;

use crate::error::DsnError;
use crate::format::{IdentifierType, IndentFileWriter, java_float_to_string};
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::dsn_file::{read_float_scope, read_integer_scope, read_on_off_scope};
use crate::parser::geometry::DsnLayerStructure;
use crate::parser::scope_parameter::skip_scope;

/// The router settings the DSN `autoroute_settings` scope reads and writes (plan ruling 5).
///
/// Java's `AutorouteSettings.readScope` returns an `app.freerouting.settings.RouterSettings` and
/// `BoardMetadata` carries one; Plan 4 owns `fr-settings`, which cannot be a dependency of Plan 3
/// without inverting the build order. This type therefore holds **exactly** the fields the
/// DSN/rules scopes read and write, with the same defaults and the same setter clamping as the
/// Java class, and Plan 4 writes the `From`/`Into` pair.
///
// obligation: settings/RouterSettings.java — Plan 4 owns the real `RouterSettings`; this
// `DsnRouterSettings` is the `fr-dsn`-local subset the DSN reader/writer needs (plan ruling 5),
// and Plan 4 must supply the conversion in both directions rather than a second parser.
///
/// # Defaults, and where they come from
///
/// Every getter below reproduces `RouterSettings`' own null-coalescing default, because Java's
/// fields are boxed and start `null`:
///
/// | Field | Java | Default |
/// |---|---|---|
/// | `run_router` | `enabled != null ? enabled : true` (:553) | `true` |
/// | `run_optimizer` | `optimizer != null && optimizer.enabled != null ? … : false` (:562) | `false` |
/// | `vias_allowed` | `viasAllowed != null ? viasAllowed : true` (:596) | `true` |
/// | `via_costs` / `plane_via_costs` / `start_ripup_costs` | `scoring…!= null ? … : 1` (:538,601,614) | `1` |
/// | layer `active` | `layers[layer].routable`, seeded `true` by `setLayerCount` (:471) | `true` |
/// | `preferred_direction_is_horizontal` | `layer % 2 == 1` when unset (:743,747) | alternating |
/// | both trace costs | `1.0` when unset (:786,806), seeded `1.0` by `setLayerCount` (:475-476) | `1.0` |
#[derive(Debug, Clone, PartialEq)]
pub struct DsnRouterSettings {
    /// `RouterSettings.enabled` (`getRunRouter`/`setRunRouter`, RouterSettings.java:552-557).
    run_router: bool,
    /// `RouterSettings.optimizer.enabled` (`getRunOptimizer`/`setRunOptimizer`, :561-568).
    run_optimizer: bool,
    /// `RouterSettings.viasAllowed` (`getViasAllowed`, :595-597).
    vias_allowed: bool,
    /// `RouterSettings.scoring.viaCosts` (:600-611).
    via_costs: i32,
    /// `RouterSettings.scoring.planeViaCosts` (:613-624).
    plane_via_costs: i32,
    /// `RouterSettings.scoring.startRipupCosts` (:537-548).
    start_ripup_costs: i32,
    /// `RouterSettings.layers[i].routable` (:629-670).
    layer_active: Vec<bool>,
    /// `RouterSettings.layers[i].preferredDirectionHorizontal` (:709-749). `None` is Java's
    /// `null`, whose default is the alternating `layer % 2 == 1` — not a constant, so the
    /// nullability has to survive into this port.
    preferred_direction_is_horizontal: Vec<Option<bool>>,
    /// `RouterSettings.scoring.preferredDirectionTraceCost` (:751-793).
    preferred_direction_trace_costs: Vec<f64>,
    /// `RouterSettings.scoring.undesiredDirectionTraceCost` (:795-813,838-853).
    against_preferred_direction_trace_costs: Vec<f64>,
}

impl Default for DsnRouterSettings {
    fn default() -> DsnRouterSettings {
        DsnRouterSettings::new()
    }
}

impl DsnRouterSettings {
    /// `new RouterSettings()` (RouterSettings.java:13) with no layers yet — every field at the
    /// default its Java getter coalesces to.
    #[must_use]
    pub fn new() -> DsnRouterSettings {
        DsnRouterSettings {
            run_router: true,
            run_optimizer: false,
            vias_allowed: true,
            via_costs: 1,
            plane_via_costs: 1,
            start_ripup_costs: 1,
            layer_active: Vec::new(),
            preferred_direction_is_horizontal: Vec::new(),
            preferred_direction_trace_costs: Vec::new(),
            against_preferred_direction_trace_costs: Vec::new(),
        }
    }

    /// `RouterSettings.setLayerCount` (RouterSettings.java:455-477): resizes the per-layer arrays
    /// and re-seeds every entry — `routable = true`, `preferredDirectionHorizontal = null`, both
    /// trace costs `1.0`.
    pub fn set_layer_count(&mut self, layer_count: usize) {
        self.layer_active = vec![true; layer_count];
        self.preferred_direction_is_horizontal = vec![None; layer_count];
        self.preferred_direction_trace_costs = vec![1.0; layer_count];
        self.against_preferred_direction_trace_costs = vec![1.0; layer_count];
    }

    /// `RouterSettings.getLayerCount` (RouterSettings.java:442-448).
    #[must_use]
    pub fn get_layer_count(&self) -> usize {
        self.layer_active.len()
    }

    /// `RouterSettings.getRunRouter` (RouterSettings.java:552-554).
    #[must_use]
    pub fn run_router(&self) -> bool {
        self.run_router
    }

    /// `RouterSettings.setRunRouter` (RouterSettings.java:555-557).
    pub fn set_run_router(&mut self, value: bool) {
        self.run_router = value;
    }

    /// `RouterSettings.getRunOptimizer` (RouterSettings.java:561-563).
    #[must_use]
    pub fn run_optimizer(&self) -> bool {
        self.run_optimizer
    }

    /// `RouterSettings.setRunOptimizer` (RouterSettings.java:564-569).
    pub fn set_run_optimizer(&mut self, value: bool) {
        self.run_optimizer = value;
    }

    /// `RouterSettings.getViasAllowed` (RouterSettings.java:595-597).
    #[must_use]
    pub fn vias_allowed(&self) -> bool {
        self.vias_allowed
    }

    /// `RouterSettings.setViasAllowed(boolean)` (RouterSettings.java:216-218).
    pub fn set_vias_allowed(&mut self, value: bool) {
        self.vias_allowed = value;
    }

    /// `RouterSettings.getViaCosts` (RouterSettings.java:600-602).
    #[must_use]
    pub fn via_costs(&self) -> i32 {
        self.via_costs
    }

    /// `RouterSettings.setViaCosts` (RouterSettings.java:604-611): clamped up to 1.
    pub fn set_via_costs(&mut self, value: i32) {
        self.via_costs = value.max(1);
    }

    /// `RouterSettings.getPlaneViaCosts` (RouterSettings.java:613-615).
    #[must_use]
    pub fn plane_via_costs(&self) -> i32 {
        self.plane_via_costs
    }

    /// `RouterSettings.setPlaneViaCosts` (RouterSettings.java:617-624): clamped up to 1.
    pub fn set_plane_via_costs(&mut self, value: i32) {
        self.plane_via_costs = value.max(1);
    }

    /// `RouterSettings.getStartRipupCosts` (RouterSettings.java:537-539).
    #[must_use]
    pub fn start_ripup_costs(&self) -> i32 {
        self.start_ripup_costs
    }

    /// `RouterSettings.setStartRipupCosts` (RouterSettings.java:541-548): clamped up to 1.
    pub fn set_start_ripup_costs(&mut self, value: i32) {
        self.start_ripup_costs = value.max(1);
    }

    /// `RouterSettings.getLayerActive` (RouterSettings.java:658-670): `false` for an
    /// out-of-range layer, exactly as Java's guarded getter answers.
    #[must_use]
    pub fn get_layer_active(&self, layer: usize) -> bool {
        self.layer_active.get(layer).copied().unwrap_or(false)
    }

    /// `RouterSettings.setLayerActive` (RouterSettings.java:635-650): an out-of-range layer is
    /// a no-op (Java warns and returns).
    pub fn set_layer_active(&mut self, layer: usize, value: bool) {
        if let Some(slot) = self.layer_active.get_mut(layer) {
            *slot = value;
        }
    }

    /// `RouterSettings.getPreferredDirectionIsHorizontal` (RouterSettings.java:733-749): `false`
    /// out of range, otherwise the stored value, otherwise the alternating `layer % 2 == 1`.
    #[must_use]
    pub fn get_preferred_direction_is_horizontal(&self, layer: usize) -> bool {
        match self.preferred_direction_is_horizontal.get(layer) {
            None => false,
            Some(None) => layer % 2 == 1,
            Some(Some(value)) => *value,
        }
    }

    /// `RouterSettings.setPreferredDirectionIsHorizontal` (RouterSettings.java:709-725).
    pub fn set_preferred_direction_is_horizontal(&mut self, layer: usize, value: bool) {
        if let Some(slot) = self.preferred_direction_is_horizontal.get_mut(layer) {
            *slot = Some(value);
        }
    }

    /// `RouterSettings.getPreferredDirectionTraceCosts` (RouterSettings.java:779-793): `0` out of
    /// range, `1.0` when unset.
    #[must_use]
    pub fn get_preferred_direction_trace_costs(&self, layer: usize) -> f64 {
        self.preferred_direction_trace_costs
            .get(layer)
            .copied()
            .unwrap_or(0.0)
    }

    /// `RouterSettings.setPreferredDirectionTraceCosts` (RouterSettings.java:756-773): clamped up
    /// to 0.1.
    pub fn set_preferred_direction_trace_costs(&mut self, layer: usize, value: f64) {
        if let Some(slot) = self.preferred_direction_trace_costs.get_mut(layer) {
            *slot = value.max(0.1);
        }
    }

    /// `RouterSettings.getAgainstPreferredDirectionTraceCosts` (RouterSettings.java:795-813).
    #[must_use]
    pub fn get_against_preferred_direction_trace_costs(&self, layer: usize) -> f64 {
        self.against_preferred_direction_trace_costs
            .get(layer)
            .copied()
            .unwrap_or(0.0)
    }

    /// `RouterSettings.setAgainstPreferredDirectionTraceCosts` (RouterSettings.java:838-853):
    /// clamped up to 0.1.
    pub fn set_against_preferred_direction_trace_costs(&mut self, layer: usize, value: f64) {
        if let Some(slot) = self.against_preferred_direction_trace_costs.get_mut(layer) {
            *slot = value.max(0.1);
        }
    }
}

/// `AutorouteSettings.readScope` (AutorouteSettings.java:18-70): the `(autoroute_settings …)`
/// scope, whose opening bracket and keyword the caller has already consumed.
///
/// `None` is Java's `null` — end of file, or a `layer_rule` sub-scope that named an unknown
/// layer. Java's `FRLogger.warn`/`error` calls are dropped (no `tracing` in `fr-dsn`); a genuine
/// scanner error still propagates as `Err`.
///
/// `withAutoroute`/`withPostroute` default to `true` and land in `run_router`/`run_optimizer`
/// after the loop (:67-68) — note that this is what makes a DSN-read `run_optimizer` default to
/// `true` even though the plain `RouterSettings` getter defaults it to `false`.
// renamed: AutorouteSettings.readScope -> read_autoroute_settings_scope (this module has no
// `AutorouteSettings` type to hang it on: the Java class is a static-method holder).
pub fn read_autoroute_settings_scope(
    scanner: &mut DsnScanner,
    layer_structure: &DsnLayerStructure,
) -> Result<Option<DsnRouterSettings>, DsnError> {
    let mut result = DsnRouterSettings::new();
    result.set_layer_count(layer_structure.layers.len());
    let mut with_autoroute = true;
    let mut with_postroute = true;

    let mut prev_was_open = false;
    loop {
        let Some(next_token) = scanner.next_token()? else {
            // "unexpected end of file" (AutorouteSettings.java:31-36).
            return Ok(None);
        };
        if next_token == Token::Close {
            // end of scope
            break;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            match next_token {
                Token::Kw(Keyword::Fanout) => {
                    // Java reads it and throws the value away (AutorouteSettings.java:41-42).
                    let _ = read_on_off_scope(scanner)?;
                }
                Token::Kw(Keyword::Autoroute) => with_autoroute = read_on_off_scope(scanner)?,
                Token::Kw(Keyword::Postroute) => with_postroute = read_on_off_scope(scanner)?,
                Token::Kw(Keyword::Vias) => result.set_vias_allowed(read_on_off_scope(scanner)?),
                Token::Kw(Keyword::ViaCosts) => result.set_via_costs(read_integer_scope(scanner)?),
                Token::Kw(Keyword::PlaneViaCosts) => {
                    result.set_plane_via_costs(read_integer_scope(scanner)?);
                }
                Token::Kw(Keyword::StartRipupCosts) => {
                    result.set_start_ripup_costs(read_integer_scope(scanner)?);
                }
                Token::Kw(Keyword::LayerRule) => {
                    match read_layer_rule(scanner, layer_structure, result)? {
                        Some(updated) => result = updated,
                        None => return Ok(None),
                    }
                }
                _ => {
                    let _ = skip_scope(scanner)?;
                }
            }
        }
        prev_was_open = is_open;
    }
    result.set_run_router(with_autoroute);
    result.set_run_optimizer(with_postroute);
    Ok(Some(result))
}

/// `AutorouteSettings.readLayerRule` (AutorouteSettings.java:73-158): one `(layer_rule <layer> …)`
/// sub-scope, taking and returning the settings object Java threads through it.
pub fn read_layer_rule(
    scanner: &mut DsnScanner,
    layer_structure: &DsnLayerStructure,
    mut settings: DsnRouterSettings,
) -> Result<Option<DsnRouterSettings>, DsnError> {
    scanner.yybegin(LexicalState::Name);
    let Some(Token::Str(layer_name)) = scanner.next_token()? else {
        // "String expected" (AutorouteSettings.java:83-89).
        return Ok(None);
    };
    // Java: `layerStructure.getNo(name)` answers `-1` for an unknown layer (:91-98).
    let Some(layer_index) = layer_structure.get_no(&layer_name) else {
        return Ok(None);
    };

    // Java seeds `prevToken` from the layer-name token, which is a `String`, so the first
    // iteration's `prevToken == OPEN_BRACKET` test is false either way.
    let mut prev_was_open = false;
    loop {
        let Some(next_token) = scanner.next_token()? else {
            // "unexpected end of file" (AutorouteSettings.java:104-110).
            return Ok(None);
        };
        if next_token == Token::Close {
            // end of scope
            break;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            match next_token {
                Token::Kw(Keyword::Active) => {
                    settings.set_layer_active(layer_index, read_on_off_scope(scanner)?);
                }
                Token::Kw(Keyword::PreferredDirection) => {
                    let mut pref_dir_is_horizontal = true;
                    match scanner.next_token()? {
                        Some(Token::Kw(Keyword::Vertical)) => pref_dir_is_horizontal = false,
                        Some(Token::Kw(Keyword::Horizontal)) => {}
                        // "unexpected key word" (AutorouteSettings.java:124-130).
                        _ => return Ok(None),
                    }
                    settings
                        .set_preferred_direction_is_horizontal(layer_index, pref_dir_is_horizontal);
                    if scanner.next_token()? != Some(Token::Close) {
                        // "closing bracket expected" (AutorouteSettings.java:133-139).
                        return Ok(None);
                    }
                }
                Token::Kw(Keyword::PreferredDirectionTraceCosts) => {
                    settings.set_preferred_direction_trace_costs(
                        layer_index,
                        read_float_scope(scanner)?,
                    );
                }
                Token::Kw(Keyword::AgainstPreferredDirectionTraceCosts) => {
                    settings.set_against_preferred_direction_trace_costs(
                        layer_index,
                        read_float_scope(scanner)?,
                    );
                }
                _ => {
                    let _ = skip_scope(scanner)?;
                }
            }
        }
        prev_was_open = is_open;
    }
    Ok(Some(settings))
}

/// `AutorouteSettings.writeScope` (AutorouteSettings.java:159-239).
///
/// The per-layer trace costs go through [`java_float_to_string`], because Java writes
/// `String.valueOf((float) settings.getPreferredDirectionTraceCosts(i))` (:228,234) — a
/// `Float.toString`, not a `Double.toString`, so a cost of 1.0 is `1.0` rather than
/// `1.0000000149011612`. Everything else here is an `int` or an on/off literal.
///
/// `layer_structure` is the **board's** layer structure (`board/model/structure/LayerStructure`),
/// not the DSN parser's, exactly as Java's parameter is.
// renamed: AutorouteSettings.writeScope -> write_autoroute_settings_scope.
pub fn write_autoroute_settings_scope<W: Write>(
    file: &mut IndentFileWriter<W>,
    settings: &DsnRouterSettings,
    layer_structure: &LayerStructure,
    identifier_type: &IdentifierType,
) {
    file.start_scope_nl();
    file.write("autoroute_settings");
    file.new_line();
    file.write("(autoroute ");
    file.write(if settings.run_router() { "on)" } else { "off)" });
    file.new_line();
    file.write("(postroute ");
    file.write(if settings.run_optimizer() {
        "on)"
    } else {
        "off)"
    });
    file.new_line();
    file.write("(vias ");
    file.write(if settings.vias_allowed() {
        "on)"
    } else {
        "off)"
    });
    file.new_line();
    file.write("(via_costs ");
    file.write(&settings.via_costs().to_string());
    file.write(")");
    file.new_line();
    file.write("(plane_via_costs ");
    file.write(&settings.plane_via_costs().to_string());
    file.write(")");
    file.new_line();
    file.write("(start_ripup_costs ");
    file.write(&settings.start_ripup_costs().to_string());
    file.write(")");
    file.new_line();
    for (i, current_layer) in layer_structure.layers.iter().enumerate() {
        file.start_scope_nl();
        file.write("layer_rule ");
        identifier_type.write(&current_layer.name, file);
        file.new_line();
        file.write("(active ");
        file.write(if settings.get_layer_active(i) {
            "on)"
        } else {
            "off)"
        });
        file.new_line();
        file.write("(preferred_direction ");
        file.write(if settings.get_preferred_direction_is_horizontal(i) {
            "horizontal)"
        } else {
            "vertical)"
        });
        file.new_line();
        file.write("(preferred_direction_trace_costs ");
        #[allow(clippy::cast_possible_truncation)] // Java's own `(float)` cast (:228).
        let trace_costs = settings.get_preferred_direction_trace_costs(i) as f32;
        file.write(&java_float_to_string(trace_costs));
        file.write(")");
        file.new_line();
        file.write("(against_preferred_direction_trace_costs ");
        #[allow(clippy::cast_possible_truncation)] // Java's own `(float)` cast (:234).
        let trace_costs = settings.get_against_preferred_direction_trace_costs(i) as f32;
        file.write(&java_float_to_string(trace_costs));
        file.write(")");
        file.end_scope();
    }
    file.end_scope();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_router_settings_null_coalescing() {
        let settings = DsnRouterSettings::new();
        assert!(settings.run_router());
        assert!(!settings.run_optimizer());
        assert!(settings.vias_allowed());
        assert_eq!(settings.via_costs(), 1);
        assert_eq!(settings.plane_via_costs(), 1);
        assert_eq!(settings.start_ripup_costs(), 1);
        assert_eq!(settings.get_layer_count(), 0);
    }

    #[test]
    fn set_layer_count_seeds_javas_alternating_preferred_direction() {
        let mut settings = DsnRouterSettings::new();
        settings.set_layer_count(4);
        // `layer % 2 == 1` (RouterSettings.java:743,747).
        assert!(!settings.get_preferred_direction_is_horizontal(0));
        assert!(settings.get_preferred_direction_is_horizontal(1));
        assert!(!settings.get_preferred_direction_is_horizontal(2));
        assert!(settings.get_preferred_direction_is_horizontal(3));
        assert!(settings.get_layer_active(2));
        assert!((settings.get_preferred_direction_trace_costs(3) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn the_cost_setters_clamp_the_way_java_does() {
        let mut settings = DsnRouterSettings::new();
        settings.set_layer_count(1);
        settings.set_via_costs(-5);
        settings.set_plane_via_costs(0);
        settings.set_start_ripup_costs(-1);
        assert_eq!(settings.via_costs(), 1);
        assert_eq!(settings.plane_via_costs(), 1);
        assert_eq!(settings.start_ripup_costs(), 1);
        settings.set_preferred_direction_trace_costs(0, 0.0);
        assert!((settings.get_preferred_direction_trace_costs(0) - 0.1).abs() < 1e-12);
        settings.set_against_preferred_direction_trace_costs(0, -3.0);
        assert!((settings.get_against_preferred_direction_trace_costs(0) - 0.1).abs() < 1e-12);
    }

    #[test]
    fn an_out_of_range_layer_is_a_no_op_or_false() {
        let mut settings = DsnRouterSettings::new();
        settings.set_layer_count(2);
        settings.set_layer_active(9, false);
        assert!(!settings.get_layer_active(9));
        assert!(!settings.get_preferred_direction_is_horizontal(9));
        assert!((settings.get_preferred_direction_trace_costs(9) - 0.0).abs() < f64::EPSILON);
    }
}
