use std::io::Read;
use std::path::Path;

use copper_board::Board;
use copper_dsn::keyword::Keyword;
use copper_dsn::lexer::{DsnScanner, LexicalState, Token};
use copper_dsn::parser::autoroute_settings::read_autoroute_settings_scope;
use copper_dsn::parser::geometry::DsnLayerStructure;
use copper_dsn::parser::scope_parameter::skip_scope;

use crate::{RouterSettings, SettingsSource, SourceKind, merger::priority};

#[derive(Debug, Clone)]
pub struct RulesFileSettings {
    settings: RouterSettings,
    file_name: String,
}

impl RulesFileSettings {
    const PRIORITY: i32 = priority::RULES_FILE;

    #[must_use]
    pub fn new(rules: impl Read, file_name: &str) -> Self {
        let settings = match copper_dsn::rules_reader::read_router_settings(rules) {
            Ok(Some(extracted)) => RouterSettings::from(extracted),
            Ok(None) | Err(_) => RouterSettings::new(),
        };
        Self {
            settings,
            file_name: file_name.to_string(),
        }
    }

    #[must_use]
    pub fn from_path(path: &Path) -> Self {
        let file_name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        match std::fs::File::open(path) {
            Ok(file) => Self::new(file, &file_name),
            Err(_) => Self {
                settings: RouterSettings::new(),
                file_name,
            },
        }
    }
}

impl SettingsSource for RulesFileSettings {
    fn get_settings(&self) -> Option<&RouterSettings> {
        Some(&self.settings)
    }

    fn get_source_name(&self) -> String {
        format!("RULES file: {}", self.file_name)
    }

    fn get_priority(&self) -> i32 {
        Self::PRIORITY
    }

    fn kind(&self) -> SourceKind {
        SourceKind::RulesFile
    }
}

pub fn apply_rules_file_against_board(
    bytes: &[u8],
    board: &Board,
    target: &mut RouterSettings,
) -> bool {
    let text = String::from_utf8_lossy(bytes).into_owned();
    let layer_structure = DsnLayerStructure::from_board(board.layer_structure());
    let mut scanner = DsnScanner::new(&text);

    for expected in [
        Token::Open,
        Token::Kw(Keyword::Rules),
        Token::Kw(Keyword::PcbScope),
    ] {
        match scanner.next_token() {
            Ok(Some(token)) if token == expected => {}
            _ => return false,
        }
    }
    scanner.yybegin(LexicalState::Name);
    if scanner.next_token().is_err() {
        return false;
    }

    let mut prev_was_open = false;
    loop {
        let next_token = match scanner.next_token() {
            Ok(Some(token)) => token,
            Ok(None) | Err(_) => return false,
        };
        if next_token == Token::Close {
            return true;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            if next_token == Token::Kw(Keyword::AutorouteSettings) {
                match read_autoroute_settings_scope(&mut scanner, &layer_structure) {
                    Ok(Some(parsed)) => {
                        target.apply_new_values_from(&RouterSettings::from(&parsed));
                    }
                    Ok(None) => {}
                    Err(_) => return false,
                }
            } else if skip_scope(&mut scanner).is_err() {
                return false;
            }
        }
        prev_was_open = is_open;
    }
}
