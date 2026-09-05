use std::io::Read;

use fr_board::{Board, FixedState, ItemClass, PadstackId};
use fr_geometry::{IntPoint, Point, Polyline};

use crate::coordinate_transform::CoordinateTransform;
use crate::error::DsnError;
use crate::format::java_round_to_int;
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::geometry::{DsnLayerStructure, DsnPolygonPath, read_polygon_path_scope};
use crate::parser::library::strip_dot_digits;
use crate::parser::scope_parameter::skip_scope;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SesImportSummary {
    pub wires_imported: usize,
    pub vias_imported: usize,
    pub errors_encountered: usize,
}

pub fn read(
    input: impl Read,
    board: &mut Board,
    ct: &CoordinateTransform,
) -> Result<SesImportSummary, DsnError> {
    let text = quote_brace_identifiers(&read_to_string(input)?);
    let scanner = DsnScanner::new(&text);

    let scale_factor = ct.dsn_to_board(1.0) / f64::from(board.communication.resolution);

    let specctra_layer_structure = DsnLayerStructure::from_board(board.layer_structure());

    let mut reader = SesReader {
        scanner,
        board,
        specctra_layer_structure,
        session_file_scale_denominator: scale_factor,
        wires_imported: 0,
        vias_imported: 0,
        errors_encountered: 0,
    };
    reader.process_session_scope()?;
    Ok(SesImportSummary {
        wires_imported: reader.wires_imported,
        vias_imported: reader.vias_imported,
        errors_encountered: reader.errors_encountered,
    })
}

struct SesReader<'a> {
    scanner: DsnScanner,
    board: &'a mut Board,
    specctra_layer_structure: DsnLayerStructure,
    session_file_scale_denominator: f64,
    wires_imported: usize,
    vias_imported: usize,
    errors_encountered: usize,
}

impl SesReader<'_> {
    fn process_session_scope(&mut self) -> Result<(), DsnError> {
        let mut next_token: Option<Token> = None;
        for i in 0..3 {
            next_token = self.scanner.next_token()?;
            let keyword_ok = match i {
                0 => next_token == Some(Token::Open),
                1 => {
                    let ok = next_token == Some(Token::Kw(Keyword::Session));
                    self.scanner.yybegin(LexicalState::Name);
                    ok
                }
                _ => true,
            };
            if !keyword_ok {
                return Err(DsnError::NotASessionFile {
                    got: describe_token(next_token.as_ref()),
                });
            }
        }

        loop {
            let prev_token = next_token;
            next_token = self.scanner.next_token()?;
            let Some(token) = next_token.clone() else {
                return Ok(());
            };
            if token == Token::Close {
                break;
            }
            if prev_token == Some(Token::Open) {
                if token == Token::Kw(Keyword::Routes) {
                    self.process_routes_scope()?;
                } else {
                    skip_scope(&mut self.scanner)?;
                }
            }
        }
        Ok(())
    }

    // not ported: the `FRLogger.warn("… unexpected end of file at '…'")` at :166-170.
    fn process_routes_scope(&mut self) -> Result<(), DsnError> {
        self.process_container_scope(Keyword::NetworkOut, Self::process_network_scope)
    }

    fn process_network_scope(&mut self) -> Result<(), DsnError> {
        self.process_container_scope(Keyword::Net, Self::process_net_scope)
    }

    fn process_container_scope(
        &mut self,
        wanted: Keyword,
        mut on_match: impl FnMut(&mut Self) -> Result<(), DsnError>,
    ) -> Result<(), DsnError> {
        let mut next_token: Option<Token> = None;
        loop {
            let prev_token = next_token;
            next_token = self.scanner.next_token()?;
            let Some(token) = next_token.clone() else {
                return Ok(());
            };
            if token == Token::Close {
                break;
            }
            if prev_token == Some(Token::Open) {
                if token == Token::Kw(wanted) {
                    on_match(self)?;
                } else {
                    skip_scope(&mut self.scanner)?;
                }
            }
        }
        Ok(())
    }

    fn process_net_scope(&mut self) -> Result<(), DsnError> {
        let mut next_token = self.scanner.next_token()?;
        let Some(Token::Str(net_name)) = next_token.clone() else {
            self.errors_encountered += 1;
            return Ok(());
        };
        self.scanner.set_scope_identifier(&net_name);

        let Some(net) = self.board.rules.nets.get_by_name_and_subnet(&net_name, 1) else {
            self.errors_encountered += 1;
            skip_scope(&mut self.scanner)?;
            return Ok(());
        };
        let net_numbers = vec![net.net_number];

        loop {
            let prev_token = next_token;
            next_token = self.scanner.next_token()?;
            let Some(token) = next_token.clone() else {
                return Ok(());
            };
            if token == Token::Close {
                break;
            }
            if prev_token == Some(Token::Open) {
                if token == Token::Kw(Keyword::Wire) {
                    if !self.process_wire_scope(&net_numbers)? {
                        self.errors_encountered += 1;
                    }
                } else if token == Token::Kw(Keyword::Via) {
                    if !self.process_via_scope(&net_numbers)? {
                        self.errors_encountered += 1;
                    }
                } else {
                    skip_scope(&mut self.scanner)?;
                }
            }
        }
        Ok(())
    }

    fn process_wire_scope(&mut self, net_numbers: &[i32]) -> Result<bool, DsnError> {
        let mut wire_path: Option<DsnPolygonPath> = None;
        let mut next_token: Option<Token> = None;
        loop {
            let prev_token = next_token;
            next_token = self.scanner.next_token()?;
            let Some(token) = next_token.clone() else {
                return Ok(false);
            };
            if token == Token::Close {
                break;
            }
            if prev_token == Some(Token::Open) {
                if token == Token::Kw(Keyword::PolygonPath) {
                    wire_path = read_polygon_path_scope(
                        &mut self.scanner,
                        Some(&self.specctra_layer_structure),
                    )?;
                } else {
                    skip_scope(&mut self.scanner)?;
                }
            }
        }

        let Some(wire_path) = wire_path else {
            return Ok(true);
        };

        let denominator = self.session_file_scale_denominator;
        let coordinate_arr = &wire_path.coordinate_arr;
        let mut points: Vec<Point> = Vec::with_capacity(coordinate_arr.len() / 2);
        for i in 0..coordinate_arr.len() / 2 {
            points.push(Point::Int(IntPoint::new(
                java_round_to_int(coordinate_arr[2 * i] / denominator),
                java_round_to_int(coordinate_arr[2 * i + 1] / denominator),
            )));
        }

        let polyline = Polyline::from_points(&points);
        let half_width = java_round_to_int(wire_path.width / (2.0 * denominator));

        let layer_index = usize::try_from(wire_path.layer.no).unwrap_or(0);

        let clearance_class = self.default_clearance_class(ItemClass::Trace);

        self.board.insert_trace(
            polyline,
            layer_index,
            half_width,
            net_numbers.to_vec(),
            clearance_class,
            FixedState::UserFixed,
        );
        self.wires_imported += 1;
        Ok(true)
    }

    fn process_via_scope(&mut self, net_numbers: &[i32]) -> Result<bool, DsnError> {
        let mut next_token = self.scanner.next_token()?;
        let Some(Token::Str(padstack_name)) = next_token.clone() else {
            return Ok(false);
        };
        self.scanner.set_scope_identifier(&padstack_name);

        let mut location = [0.0f64; 2];
        for slot in &mut location {
            next_token = self.scanner.next_token()?;
            match next_token {
                Some(Token::Float(value)) => *slot = value,
                #[allow(clippy::cast_precision_loss)]
                Some(Token::Int(value)) => *slot = value as f64,
                _ => return Ok(false),
            }
        }

        next_token = self.scanner.next_token()?;
        while next_token == Some(Token::Open) {
            skip_scope(&mut self.scanner)?;
            next_token = self.scanner.next_token()?;
        }
        if next_token != Some(Token::Close) {
            return Ok(false);
        }

        let cleaned_name = strip_dot_digits(&padstack_name);
        let Some(via_padstack) = self
            .board
            .library
            .padstacks
            .get_by_name(&cleaned_name)
            .map(|padstack| PadstackId(padstack.no))
        else {
            return Ok(false);
        };

        let denominator = self.session_file_scale_denominator;
        let via_location = Point::Int(IntPoint::new(
            java_round_to_int(location[0] / denominator),
            java_round_to_int(location[1] / denominator),
        ));

        let clearance_class = self.default_clearance_class(ItemClass::Via);

        if self
            .board
            .insert_via(
                via_padstack,
                via_location,
                net_numbers.to_vec(),
                clearance_class,
                FixedState::UserFixed,
                true,
            )
            .is_err()
        {
            return Ok(false);
        }
        self.vias_imported += 1;
        Ok(true)
    }

    fn default_clearance_class(&mut self, item_class: ItemClass) -> usize {
        let default_net_class = self.board.rules.get_default_net_class();
        self.board
            .rules
            .net_classes
            .get(default_net_class)
            .default_item_clearance_classes
            .get(item_class)
    }
}

fn describe_token(token: Option<&Token>) -> String {
    match token {
        None => "null".to_string(),
        Some(Token::Open) => "(".to_string(),
        Some(Token::Close) => ")".to_string(),
        Some(Token::Kw(keyword)) => keyword.name().to_string(),
        Some(Token::Str(value)) => value.clone(),
        Some(Token::Int(value)) => value.to_string(),
        Some(Token::Float(value)) => crate::format::java_double_to_string(*value),
    }
}

fn read_to_string(mut input: impl Read) -> Result<String, std::io::Error> {
    let mut bytes = Vec::new();
    input.read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Quotes an unquoted run that contains `{` or `}`.
///
/// `SesWriter` quotes a net name containing `~`, `{` or `}` (KiCad's `~{SIGNAL}` convention for an
/// active-low net), but the Specctra scanner's identifier character class never included `{`/`}` —
/// only `~` — so a session written before that quoting rule existed reads back as a lone `~`
/// followed by characters no keyword or identifier rule matches
/// (`fixtures/Issue191-processor.Z80/processor.ses`, a real bug-report attachment, has exactly
/// this for 44 nets). Quoting such a run before scanning makes it one token, the same as the
/// quoted spelling the current writer already produces for the identical name.
fn quote_brace_identifiers(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    let mut run_has_brace = false;
    let mut quote: Option<char> = None;
    for ch in text.chars() {
        if let Some(q) = quote {
            out.push(ch);
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => {
                flush_identifier_run(&mut out, &mut run, &mut run_has_brace);
                quote = Some(ch);
                out.push(ch);
            }
            '(' | ')' | ' ' | '\t' | '\r' | '\n' => {
                flush_identifier_run(&mut out, &mut run, &mut run_has_brace);
                out.push(ch);
            }
            '{' | '}' => {
                run_has_brace = true;
                run.push(ch);
            }
            _ => run.push(ch),
        }
    }
    flush_identifier_run(&mut out, &mut run, &mut run_has_brace);
    out
}

fn flush_identifier_run(out: &mut String, run: &mut String, run_has_brace: &mut bool) {
    if !run.is_empty() {
        if *run_has_brace {
            out.push('"');
            out.push_str(run);
            out.push('"');
        } else {
            out.push_str(run);
        }
        run.clear();
    }
    *run_has_brace = false;
}

#[cfg(test)]
mod tests {
    use super::quote_brace_identifiers;

    #[test]
    fn an_unquoted_brace_identifier_is_quoted() {
        assert_eq!(
            quote_brace_identifiers("(net ~{WAIT}\n"),
            "(net \"~{WAIT}\"\n"
        );
    }

    #[test]
    fn an_already_quoted_brace_identifier_is_left_alone() {
        assert_eq!(
            quote_brace_identifiers("(net \"~{IM2-EN}\"\n"),
            "(net \"~{IM2-EN}\"\n"
        );
    }

    #[test]
    fn text_without_braces_is_unchanged() {
        let text = "(net GND\n  (wire\n    (path F.Cu 250 0 0 1 1)\n  )\n)\n";
        assert_eq!(quote_brace_identifiers(text), text);
    }
}
