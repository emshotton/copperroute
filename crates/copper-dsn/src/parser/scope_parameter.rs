use std::io::Write;
use std::time::Duration;

use copper_board::{AngleRestriction, Board, ItemIdGenerator, Unit};

use crate::coordinate_transform::CoordinateTransform;
use crate::error::DsnError;
use crate::format::{DSN_RESERVED, IdentifierType, IndentFileWriter};
use crate::keyword::ScopeKeyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::DsnRouterSettings;
use crate::parser::network::NetList;
use crate::parser::part_library::{DsnLogicalPart, DsnLogicalPartMapping};
use crate::parser::placement::ComponentPlacement;
use crate::parser::structure::{DsnLayerStructure, DsnPlane};
use crate::parser::{header, library, network, part_library, placement, structure, wiring};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsnReadOptions {
    pub normalize_time_limit: Duration,
}

impl Default for DsnReadOptions {
    fn default() -> DsnReadOptions {
        DsnReadOptions {
            normalize_time_limit: Duration::from_secs(60),
        }
    }
}

impl DsnReadOptions {
    #[must_use]
    pub fn normalize_time_limit_ms(&self) -> i32 {
        i32::try_from(self.normalize_time_limit.as_millis()).unwrap_or(i32::MAX)
    }
}

#[derive(Debug)]
pub struct ReadScopeParameter<'a> {
    pub scanner: DsnScanner,
    pub board: Option<Board>,
    pub netlist: NetList,
    pub plane_list: Vec<DsnPlane>,
    pub placement_list: Vec<ComponentPlacement>,
    pub logical_part_mappings: Vec<DsnLogicalPartMapping>,
    pub logical_parts: Vec<DsnLogicalPart>,
    pub constants: Vec<Vec<String>>,
    pub via_padstack_names: Option<Vec<String>>,
    pub string_quote: String,
    pub host_cad: Option<String>,
    pub host_version: Option<String>,
    pub dsn_file_generated_by_host: bool,
    pub write_resolution: Option<copper_board::WriteResolution>,
    pub via_at_smd_allowed: bool,
    pub board_outline_ok: bool,
    pub coordinate_transform: Option<CoordinateTransform>,
    pub layer_structure: Option<DsnLayerStructure>,
    pub autoroute_settings: Option<DsnRouterSettings>,
    pub unit: Unit,
    pub resolution: i32,
    pub snap_angle: AngleRestriction,
    pub warnings: Vec<String>,
    pub truncation: Option<String>,
    pub id_generator: ItemIdGenerator,
    pub options: &'a DsnReadOptions,
}

impl<'a> ReadScopeParameter<'a> {
    #[must_use]
    pub fn new(scanner: DsnScanner, options: &'a DsnReadOptions) -> ReadScopeParameter<'a> {
        ReadScopeParameter {
            scanner,
            board: None,
            netlist: NetList::new(),
            plane_list: Vec::new(),
            placement_list: Vec::new(),
            logical_part_mappings: Vec::new(),
            logical_parts: Vec::new(),
            constants: Vec::new(),
            via_padstack_names: None,
            string_quote: "\"".to_string(),
            host_cad: None,
            host_version: None,
            dsn_file_generated_by_host: true,
            write_resolution: None,
            via_at_smd_allowed: false,
            board_outline_ok: true,
            coordinate_transform: None,
            layer_structure: None,
            autoroute_settings: None,
            unit: Unit::Mil,
            resolution: 100,
            snap_angle: AngleRestriction::FortyFiveDegree,
            warnings: Vec::new(),
            truncation: None,
            id_generator: ItemIdGenerator::new(),
            options,
        }
    }
}

pub struct WriteScopeParameter<'a> {
    pub board: &'a Board,
    pub file: IndentFileWriter<&'a mut dyn Write>,
    pub identifier_type: IdentifierType,
    pub coordinate_transform: &'a CoordinateTransform,
    pub compat_mode: bool,
}

impl<'a> WriteScopeParameter<'a> {
    #[must_use]
    pub fn new(
        board: &'a Board,
        file: IndentFileWriter<&'a mut dyn Write>,
        string_quote: &str,
        coordinate_transform: &'a CoordinateTransform,
        compat_mode: bool,
    ) -> WriteScopeParameter<'a> {
        let identifier_type = IdentifierType::new(
            DSN_RESERVED.iter().map(|s| (*s).to_string()).collect(),
            string_quote.to_string(),
        );
        WriteScopeParameter {
            board,
            file,
            identifier_type,
            coordinate_transform,
            compat_mode,
        }
    }
}

pub fn skip_scope(scanner: &mut DsnScanner) -> Result<bool, DsnError> {
    let mut open_bracket_count: i32 = 1;
    while open_bracket_count > 0 {
        scanner.yybegin(LexicalState::Name);
        let token = scanner.next_token()?;
        let Some(token) = token else {
            return Ok(false);
        };
        match token {
            Token::Open => open_bracket_count += 1,
            Token::Close => open_bracket_count -= 1,
            _ => {}
        }
    }
    Ok(true)
}

fn read_scope_generic(
    p: &mut ReadScopeParameter<'_>,
    scope: ScopeKeyword,
) -> Result<bool, DsnError> {
    let mut prev_was_open = false;
    loop {
        let token = p.scanner.next_token()?;
        let Some(token) = token else {
            if p.truncation.is_none() {
                p.truncation = Some(format!(
                    "unexpected end of file inside the ({}) scope: its closing bracket is \
                     missing, so the board is only what the file held before the truncation",
                    scope.name()
                ));
            }
            return Ok(true);
        };
        if token == Token::Close {
            break;
        }
        let is_open = token == Token::Open;
        if prev_was_open {
            match token {
                Token::Kw(kw) => match ScopeKeyword::from_keyword(kw) {
                    Some(next_scope) => {
                        if !read_scope(next_scope, p)? {
                            return Ok(false);
                        }
                    }
                    None => {
                        skip_scope(&mut p.scanner)?;
                    }
                },
                _ => {
                    skip_scope(&mut p.scanner)?;
                }
            }
        }
        prev_was_open = is_open;
    }
    Ok(true)
}

pub fn read_scope(scope: ScopeKeyword, p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    match scope {
        ScopeKeyword::Pcb | ScopeKeyword::Placement => read_scope_generic(p, scope),
        ScopeKeyword::Structure => structure::read_structure_scope(p),
        ScopeKeyword::Plane => structure::read_plane_scope(p),
        ScopeKeyword::Network => network::read_network_scope(p),
        ScopeKeyword::Wiring => wiring::read_wiring_scope(p),
        ScopeKeyword::Library => library::read_library_scope(p),
        ScopeKeyword::PartLibrary => part_library::read_part_library_scope(p),
        ScopeKeyword::Component => placement::read_component_scope(p),
        ScopeKeyword::Parser => header::read_parser_scope(p),
        ScopeKeyword::Resolution => header::read_resolution_scope(p),
        ScopeKeyword::PlaceControl => header::read_place_control_scope(p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_scope_parameter_defaults_match_java() {
        let options = DsnReadOptions::default();
        let p = ReadScopeParameter::new(DsnScanner::new(""), &options);
        assert_eq!(p.resolution, 100);
        assert_eq!(p.snap_angle, AngleRestriction::FortyFiveDegree);
        assert_eq!(p.string_quote, "\"");
        assert!(p.board_outline_ok);
        assert!(p.dsn_file_generated_by_host);
        assert_eq!(p.unit, Unit::Mil);
        assert!(p.board.is_none());
        assert!(p.netlist.is_empty());
    }
}
