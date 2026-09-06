use std::io::Write;

use fr_board::LayerStructure;

use crate::error::DsnError;
use crate::format::{IdentifierType, IndentFileWriter, format_float};
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::dsn_file::{read_float_scope, read_integer_scope, read_on_off_scope};
use crate::parser::geometry::DsnLayerStructure;
use crate::parser::scope_parameter::skip_scope;

#[derive(Debug, Clone, PartialEq)]
pub struct DsnRouterSettings {
    run_router: bool,
    run_optimizer: bool,
    vias_allowed: Option<bool>,
    via_costs: Option<i32>,
    plane_via_costs: Option<i32>,
    start_ripup_costs: Option<i32>,
    layer_active: Vec<bool>,
    preferred_direction_is_horizontal: Vec<Option<bool>>,
    preferred_direction_trace_costs: Vec<f64>,
    against_preferred_direction_trace_costs: Vec<f64>,
    board_specific_trace_costs_applied: bool,
}

impl Default for DsnRouterSettings {
    fn default() -> DsnRouterSettings {
        DsnRouterSettings::new()
    }
}

impl DsnRouterSettings {
    #[must_use]
    pub fn new() -> DsnRouterSettings {
        DsnRouterSettings {
            run_router: true,
            run_optimizer: false,
            vias_allowed: None,
            via_costs: None,
            plane_via_costs: None,
            start_ripup_costs: None,
            layer_active: Vec::new(),
            preferred_direction_is_horizontal: Vec::new(),
            preferred_direction_trace_costs: Vec::new(),
            against_preferred_direction_trace_costs: Vec::new(),
            board_specific_trace_costs_applied: false,
        }
    }

    pub fn set_layer_count(&mut self, layer_count: usize) {
        if self.layer_active.len() != layer_count {
            self.board_specific_trace_costs_applied = false;
        }
        self.layer_active = vec![true; layer_count];
        self.preferred_direction_is_horizontal = vec![None; layer_count];
        self.preferred_direction_trace_costs = vec![1.0; layer_count];
        self.against_preferred_direction_trace_costs = vec![1.0; layer_count];
    }

    #[must_use]
    pub fn get_layer_count(&self) -> usize {
        self.layer_active.len()
    }

    #[must_use]
    pub fn run_router(&self) -> bool {
        self.run_router
    }

    pub fn set_run_router(&mut self, value: bool) {
        self.run_router = value;
    }

    #[must_use]
    pub fn run_optimizer(&self) -> bool {
        self.run_optimizer
    }

    pub fn set_run_optimizer(&mut self, value: bool) {
        self.run_optimizer = value;
    }

    #[must_use]
    pub fn vias_allowed(&self) -> bool {
        self.vias_allowed.unwrap_or(true)
    }

    #[must_use]
    pub fn vias_allowed_raw(&self) -> Option<bool> {
        self.vias_allowed
    }

    pub fn set_vias_allowed(&mut self, value: bool) {
        self.vias_allowed = Some(value);
    }

    #[must_use]
    pub fn via_costs(&self) -> i32 {
        self.via_costs.unwrap_or(1)
    }

    #[must_use]
    pub fn via_costs_raw(&self) -> Option<i32> {
        self.via_costs
    }

    pub fn set_via_costs(&mut self, value: i32) {
        self.via_costs = Some(value.max(1));
    }

    #[must_use]
    pub fn plane_via_costs(&self) -> i32 {
        self.plane_via_costs.unwrap_or(1)
    }

    #[must_use]
    pub fn plane_via_costs_raw(&self) -> Option<i32> {
        self.plane_via_costs
    }

    pub fn set_plane_via_costs(&mut self, value: i32) {
        self.plane_via_costs = Some(value.max(1));
    }

    #[must_use]
    pub fn start_ripup_costs(&self) -> i32 {
        self.start_ripup_costs.unwrap_or(1)
    }

    #[must_use]
    pub fn start_ripup_costs_raw(&self) -> Option<i32> {
        self.start_ripup_costs
    }

    pub fn set_start_ripup_costs(&mut self, value: i32) {
        self.start_ripup_costs = Some(value.max(1));
    }

    #[must_use]
    pub fn get_layer_active(&self, layer: usize) -> bool {
        self.layer_active.get(layer).copied().unwrap_or(false)
    }

    pub fn set_layer_active(&mut self, layer: usize, value: bool) {
        if let Some(slot) = self.layer_active.get_mut(layer) {
            *slot = value;
        }
    }

    #[must_use]
    pub fn get_preferred_direction_is_horizontal(&self, layer: usize) -> bool {
        match self.preferred_direction_is_horizontal.get(layer) {
            None => false,
            Some(None) => layer % 2 == 1,
            Some(Some(value)) => *value,
        }
    }

    #[must_use]
    pub fn preferred_direction_is_horizontal_raw(&self, layer: usize) -> Option<bool> {
        self.preferred_direction_is_horizontal
            .get(layer)
            .copied()
            .flatten()
    }

    pub fn set_preferred_direction_is_horizontal(&mut self, layer: usize, value: bool) {
        if let Some(slot) = self.preferred_direction_is_horizontal.get_mut(layer) {
            *slot = Some(value);
        }
    }

    #[must_use]
    pub fn get_preferred_direction_trace_costs(&self, layer: usize) -> f64 {
        self.preferred_direction_trace_costs
            .get(layer)
            .copied()
            .unwrap_or(0.0)
    }

    pub fn set_preferred_direction_trace_costs(&mut self, layer: usize, value: f64) {
        if let Some(slot) = self.preferred_direction_trace_costs.get_mut(layer) {
            *slot = value.max(0.1);
            self.board_specific_trace_costs_applied = true;
        }
    }

    #[must_use]
    pub fn get_against_preferred_direction_trace_costs(&self, layer: usize) -> f64 {
        self.against_preferred_direction_trace_costs
            .get(layer)
            .copied()
            .unwrap_or(0.0)
    }

    pub fn set_against_preferred_direction_trace_costs(&mut self, layer: usize, value: f64) {
        if let Some(slot) = self.against_preferred_direction_trace_costs.get_mut(layer) {
            *slot = value.max(0.1);
            self.board_specific_trace_costs_applied = true;
        }
    }

    #[must_use]
    pub fn are_board_specific_trace_costs_applied(&self) -> bool {
        self.board_specific_trace_costs_applied
    }

    pub fn apply_new_values_from(&mut self, other: &DsnRouterSettings) {
        self.run_router = other.run_router;
        self.run_optimizer = other.run_optimizer;

        if other.vias_allowed.is_some() {
            self.vias_allowed = other.vias_allowed;
        }
        if other.via_costs.is_some() {
            self.via_costs = other.via_costs;
        }
        if other.plane_via_costs.is_some() {
            self.plane_via_costs = other.plane_via_costs;
        }
        if other.start_ripup_costs.is_some() {
            self.start_ripup_costs = other.start_ripup_costs;
        }

        if self.layer_active.len() >= other.layer_active.len() {
            for (i, active) in other.layer_active.iter().enumerate() {
                if let Some(slot) = self.layer_active.get_mut(i) {
                    *slot = *active;
                }
            }
            for (i, horizontal) in other.preferred_direction_is_horizontal.iter().enumerate() {
                if horizontal.is_some()
                    && let Some(slot) = self.preferred_direction_is_horizontal.get_mut(i)
                {
                    *slot = *horizontal;
                }
            }
        } else {
            self.layer_active = other.layer_active.clone();
            self.preferred_direction_is_horizontal =
                other.preferred_direction_is_horizontal.clone();
        }

        if self.preferred_direction_trace_costs.is_empty() {
            self.preferred_direction_trace_costs = other.preferred_direction_trace_costs.clone();
        }
        if self.against_preferred_direction_trace_costs.is_empty() {
            self.against_preferred_direction_trace_costs =
                other.against_preferred_direction_trace_costs.clone();
        }
    }
}

pub fn read_autoroute_settings_scope(
    scanner: &mut DsnScanner,
    layer_structure: &DsnLayerStructure,
) -> Result<Option<DsnRouterSettings>, DsnError> {
    let mut result = DsnRouterSettings::new();
    result.set_layer_count(layer_structure.layers.len());
    let mut with_autoroute = true;
    let mut with_postroute = true;

    let mut depth: u32 = 0;
    let mut prev_was_open = false;
    loop {
        let Some(next_token) = scanner.next_token()? else {
            return Ok(None);
        };
        if next_token == Token::Close {
            let Some(outer) = depth.checked_sub(1) else {
                break;
            };
            depth = outer;
            prev_was_open = false;
            continue;
        }
        if next_token == Token::Open {
            depth += 1;
            prev_was_open = true;
            continue;
        }
        if prev_was_open {
            match next_token {
                Token::Kw(Keyword::Fanout) => {
                    let _ = read_on_off_scope(scanner)?;
                }
                Token::Kw(Keyword::Autoroute) => with_autoroute = read_on_off_scope(scanner)?,
                Token::Kw(Keyword::Postroute) => with_postroute = read_on_off_scope(scanner)?,
                Token::Kw(Keyword::Vias) => result.set_vias_allowed(read_on_off_scope(scanner)?),
                Token::Kw(Keyword::ViaCosts) => {
                    if let Some(value) = read_integer_scope(scanner)? {
                        result.set_via_costs(value);
                    }
                }
                Token::Kw(Keyword::PlaneViaCosts) => {
                    if let Some(value) = read_integer_scope(scanner)? {
                        result.set_plane_via_costs(value);
                    }
                }
                Token::Kw(Keyword::StartRipupCosts) => {
                    if let Some(value) = read_integer_scope(scanner)? {
                        result.set_start_ripup_costs(value);
                    }
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
            depth = depth.saturating_sub(1);
        }
        prev_was_open = false;
    }
    result.set_run_router(with_autoroute);
    result.set_run_optimizer(with_postroute);
    Ok(Some(result))
}

pub fn read_layer_rule(
    scanner: &mut DsnScanner,
    layer_structure: &DsnLayerStructure,
    mut settings: DsnRouterSettings,
) -> Result<Option<DsnRouterSettings>, DsnError> {
    scanner.yybegin(LexicalState::Name);
    let Some(Token::Str(layer_name)) = scanner.next_token()? else {
        return Ok(None);
    };
    let Some(layer_index) = layer_structure.get_no(&layer_name) else {
        return Ok(None);
    };

    let mut depth: u32 = 0;
    let mut prev_was_open = false;
    loop {
        let Some(next_token) = scanner.next_token()? else {
            return Ok(None);
        };
        if next_token == Token::Close {
            let Some(outer) = depth.checked_sub(1) else {
                break;
            };
            depth = outer;
            prev_was_open = false;
            continue;
        }
        if next_token == Token::Open {
            depth += 1;
            prev_was_open = true;
            continue;
        }
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
                        _ => return Ok(None),
                    }
                    settings
                        .set_preferred_direction_is_horizontal(layer_index, pref_dir_is_horizontal);
                    if scanner.next_token()? != Some(Token::Close) {
                        return Ok(None);
                    }
                }
                Token::Kw(Keyword::PreferredDirectionTraceCosts) => {
                    if let Some(value) = read_float_scope(scanner)? {
                        settings.set_preferred_direction_trace_costs(layer_index, value);
                    }
                }
                Token::Kw(Keyword::AgainstPreferredDirectionTraceCosts) => {
                    if let Some(value) = read_float_scope(scanner)? {
                        settings.set_against_preferred_direction_trace_costs(layer_index, value);
                    }
                }
                _ => {
                    let _ = skip_scope(scanner)?;
                }
            }
            depth = depth.saturating_sub(1);
        }
        prev_was_open = false;
    }
    Ok(Some(settings))
}

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
        #[allow(clippy::cast_possible_truncation)]
        let trace_costs = settings.get_preferred_direction_trace_costs(i) as f32;
        file.write(&format_float(trace_costs));
        file.write(")");
        file.new_line();
        file.write("(against_preferred_direction_trace_costs ");
        #[allow(clippy::cast_possible_truncation)]
        let trace_costs = settings.get_against_preferred_direction_trace_costs(i) as f32;
        file.write(&format_float(trace_costs));
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
