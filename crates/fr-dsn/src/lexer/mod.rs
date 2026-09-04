pub mod tables;
pub mod token;

pub use token::Token;

use crate::error::DsnError;
use crate::keyword::Keyword;

const YYEOF: i32 = -1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LexicalState {
        YyInitial = 0,
        String1 = 1,
        String2 = 2,
            Name = 3,
            LayerName = 4,
            ComponentName = 5,
        SpecChar = 6,
            IgnoreQuote = 7,
}

#[derive(Debug)]
pub struct DsnScanner {
        buffer: Vec<u16>,
            zz_start_read: isize,
        zz_marked_pos: isize,
        zz_current_pos: isize,
        zz_state: usize,
        zz_lexical_state: LexicalState,
                string_buffer: Vec<u16>,
            scope_identifier: String,
}

impl DsnScanner {
                            #[must_use]
    pub fn new(input: &str) -> Self {
        Self {
            buffer: input.encode_utf16().collect(),
            zz_start_read: 0,
            zz_marked_pos: 0,
            zz_current_pos: 0,
            zz_state: 0,
            zz_lexical_state: LexicalState::YyInitial,
            string_buffer: Vec::new(),
            scope_identifier: String::new(),
        }
    }

        pub fn yybegin(&mut self, new_state: LexicalState) {
        self.zz_lexical_state = new_state;
    }

        pub fn yystate(&self) -> LexicalState {
        self.zz_lexical_state
    }

        pub fn scope_identifier(&self) -> &str {
        &self.scope_identifier
    }

        pub fn set_scope_identifier(&mut self, identifier: &str) {
        self.scope_identifier = identifier.to_string();
    }

                        pub fn yytext(&self) -> String {
        String::from_utf16_lossy(self.matched_units())
    }

        fn matched_units(&self) -> &[u16] {
        let start = self.zz_start_read.max(0) as usize;
        let end = self.zz_marked_pos.max(0) as usize;
        &self.buffer[start.min(self.buffer.len())..end.min(self.buffer.len())]
    }

            fn unit_at(&self, pos: isize) -> Option<u16> {
        if pos < 0 {
            return None;
        }
        self.buffer.get(pos as usize).copied()
    }


                        pub fn next_token(&mut self) -> Result<Option<Token>, DsnError> {
        let zz_end_read_l = self.buffer.len() as isize;

        loop {
            let mut zz_marked_pos_l = self.zz_marked_pos;
            let mut zz_action: isize = -1;

            let mut zz_current_pos_l = zz_marked_pos_l;
            self.zz_current_pos = zz_marked_pos_l;
            self.zz_start_read = zz_marked_pos_l;

            self.zz_state = self.zz_lexical_state as usize;

            let mut zz_input: i32;
            loop {
                if zz_current_pos_l >= 0 && zz_current_pos_l < zz_end_read_l {
                    zz_input = self.buffer[zz_current_pos_l as usize] as i32;
                    zz_current_pos_l += 1;
                } else {
                    zz_input = YYEOF;
                    break;
                }

                let row = tables::ZZ_ROWMAP[self.zz_state];
                let class = tables::ZZ_CMAP[zz_input as usize] as i32;
                let zz_next = tables::ZZ_TRANS[(row + class) as usize];
                if zz_next == -1 {
                    break;
                }
                self.zz_state = zz_next as usize;

                let zz_attributes = tables::ZZ_ATTRIBUTE[self.zz_state];
                if (zz_attributes & 1) == 1 {
                    zz_action = self.zz_state as isize;
                    zz_marked_pos_l = zz_current_pos_l;
                    if (zz_attributes & 8) == 8 {
                        break;
                    }
                }
            }

            self.zz_marked_pos = zz_marked_pos_l;

            let action = if zz_action < 0 {
                zz_action as i32
            } else {
                tables::ZZ_ACTION[zz_action as usize]
            };

            match action {
                1 => {
                    self.yybegin(LexicalState::YyInitial);
                    return Ok(Some(Token::Str(self.yytext())));
                }
                // SpecctraDsnStreamReader.java:1650 — `FRLogger.warn("Non-ansi character ...")`,
                2 => {}
                3 => {}
                4 => {
                    return Ok(Some(Token::Str(self.yytext())));
                }
                5 => {
                    let text = self.yytext();
                    let value: i32 = text.parse().map_err(|_| {
                        DsnError::Scan(format!("For input string: \"{text}\" (Integer.valueOf)"))
                    })?;
                    return Ok(Some(Token::Int(i64::from(value))));
                }
                6 => {
                    self.string_buffer.clear();
                    self.yybegin(LexicalState::String1);
                }
                7 => {
                    self.string_buffer.clear();
                    self.yybegin(LexicalState::String2);
                }
                10 => {
                    let start = self.zz_start_read.max(0) as usize;
                    let end = self.zz_marked_pos.max(0) as usize;
                    let (start, end) = (start.min(self.buffer.len()), end.min(self.buffer.len()));
                    self.string_buffer
                        .extend_from_slice(&self.buffer[start..end]);
                }
                11 => {
                    self.yybegin(LexicalState::YyInitial);
                    return Ok(Some(Token::Str(String::from_utf16_lossy(
                        &self.string_buffer,
                    ))));
                }
                12 => {
                    self.string_buffer.push(u16::from(b'\\'));
                }
                15 => {
                    let text = self.yytext();
                    let value: f64 = text.parse().map_err(|_| {
                        DsnError::Scan(format!("For input string: \"{text}\" (Double.valueOf)"))
                    })?;
                    return Ok(Some(Token::Float(value)));
                }
                8 => {
                    return Ok(Some(Token::Open));
                }
                9 => {
                    return Ok(Some(Token::Close));
                }
                13 => {
                    self.yybegin(LexicalState::YyInitial);
                    return Ok(Some(Token::Open));
                }
                14 => {
                    self.yybegin(LexicalState::YyInitial);
                    return Ok(Some(Token::Close));
                }
                16 => {
                    return Ok(Some(Token::Kw(Keyword::On)));
                }
                17 => {
                    return Ok(Some(Token::Kw(Keyword::Off)));
                }
                18 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::Via)));
                }
                19 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::Net)));
                }
                20 => {
                    return Ok(Some(Token::Kw(Keyword::PcbScope)));
                }
                21 => {
                    return Ok(Some(Token::Kw(Keyword::Pin)));
                }
                22 => {
                    return Ok(Some(Token::Kw(Keyword::Fix)));
                }
                23 => {
                    self.yybegin(LexicalState::YyInitial);
                    return Ok(Some(Token::Kw(Keyword::PcbScope)));
                }
                24 => {
                    return Ok(Some(Token::Kw(Keyword::Back)));
                }
                25 => {
                    return Ok(Some(Token::Kw(Keyword::Side)));
                }
                26 => {
                    return Ok(Some(Token::Kw(Keyword::Type)));
                }
                27 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::ComponentScope)));
                }
                28 => {
                    self.yybegin(LexicalState::LayerName);
                    return Ok(Some(Token::Kw(Keyword::Circle)));
                }
                29 => {
                    return Ok(Some(Token::Kw(Keyword::Vias)));
                }
                30 => {
                    return Ok(Some(Token::Kw(Keyword::None)));
                }
                31 => {
                    self.yybegin(LexicalState::LayerName);
                    return Ok(Some(Token::Kw(Keyword::PolygonPath)));
                }
                32 => {
                    self.yybegin(LexicalState::LayerName);
                    return Ok(Some(Token::Kw(Keyword::Polygon)));
                }
                33 => {
                    return Ok(Some(Token::Kw(Keyword::Pins)));
                }
                34 => {
                    self.yybegin(LexicalState::LayerName);
                    return Ok(Some(Token::Kw(Keyword::Rectangle)));
                }
                35 => {
                    return Ok(Some(Token::Kw(Keyword::Rule)));
                }
                36 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::Wire)));
                }
                37 => {
                    return Ok(Some(Token::Kw(Keyword::Spare)));
                }
                38 => {
                    return Ok(Some(Token::Kw(Keyword::Shape)));
                }
                39 => {
                    return Ok(Some(Token::Kw(Keyword::Order)));
                }
                40 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::Layer)));
                }
                41 => {
                    return Ok(Some(Token::Kw(Keyword::Clearance)));
                }
                42 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::Class)));
                }
                43 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::Image)));
                }
                44 => {
                    return Ok(Some(Token::Kw(Keyword::Power)));
                }
                45 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::Place)));
                }
                46 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::PlaneScope)));
                }
                47 => {
                    return Ok(Some(Token::Kw(Keyword::Rules)));
                }
                48 => {
                    return Ok(Some(Token::Kw(Keyword::Front)));
                }
                49 => {
                    return Ok(Some(Token::Kw(Keyword::Width)));
                }
                50 => {
                    return Ok(Some(Token::Kw(Keyword::Attach)));
                }
                51 => {
                    return Ok(Some(Token::Kw(Keyword::Active)));
                }
                52 => {
                    return Ok(Some(Token::Kw(Keyword::Signal)));
                }
                53 => {
                    return Ok(Some(Token::Kw(Keyword::Length)));
                }
                54 => {
                    return Ok(Some(Token::Kw(Keyword::Normal)));
                }
                55 => {
                    return Ok(Some(Token::Kw(Keyword::ParserScope)));
                }
                56 => {
                    return Ok(Some(Token::Kw(Keyword::Routes)));
                }
                57 => {
                    return Ok(Some(Token::Kw(Keyword::Rotate)));
                }
                58 => {
                    return Ok(Some(Token::Kw(Keyword::Fanout)));
                }
                59 => {
                    return Ok(Some(Token::Kw(Keyword::Fromto)));
                }
                60 => {
                    return Ok(Some(Token::Kw(Keyword::Window)));
                }
                61 => {
                    return Ok(Some(Token::Kw(Keyword::WiringScope)));
                }
                62 => {
                    self.yybegin(LexicalState::YyInitial);
                    return Ok(Some(Token::Kw(Keyword::Signal)));
                }
                63 => {
                    return Ok(Some(Token::Kw(Keyword::Session)));
                }
                64 => {
                    return Ok(Some(Token::Kw(Keyword::Outline)));
                }
                65 => {
                    return Ok(Some(Token::Kw(Keyword::LibraryScope)));
                }
                66 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::UseVia)));
                }
                67 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::UseNet)));
                }
                68 => {
                    return Ok(Some(Token::Kw(Keyword::Control)));
                }
                69 => {
                    return Ok(Some(Token::Kw(Keyword::Classes)));
                }
                70 => {
                    return Ok(Some(Token::Kw(Keyword::Circuit)));
                }
                71 => {
                    return Ok(Some(Token::Kw(Keyword::NetworkScope)));
                }
                72 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::Keepout)));
                }
                73 => {
                    return Ok(Some(Token::Kw(Keyword::Absolute)));
                }
                74 => {
                    return Ok(Some(Token::Kw(Keyword::Boundary)));
                }
                75 => {
                    return Ok(Some(Token::Kw(Keyword::Constant)));
                }
                76 => {
                    return Ok(Some(Token::Kw(Keyword::Vertical)));
                }
                77 => {
                    return Ok(Some(Token::Kw(Keyword::ViaRule)));
                }
                78 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::Padstack)));
                }
                79 => {
                    return Ok(Some(Token::Kw(Keyword::Position)));
                }
                80 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::HostCad)));
                }
                81 => {
                    return Ok(Some(Token::Kw(Keyword::Autoroute)));
                }
                82 => {
                    return Ok(Some(Token::Kw(Keyword::StructureScope)));
                }
                83 => {
                    return Ok(Some(Token::Kw(Keyword::LockType)));
                }
                84 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::UseLayer)));
                }
                85 => {
                    return Ok(Some(Token::Kw(Keyword::ViaCosts)));
                }
                86 => {
                    return Ok(Some(Token::Kw(Keyword::Postroute)));
                }
                87 => {
                    return Ok(Some(Token::Kw(Keyword::PlacementScope)));
                }
                88 => {
                    return Ok(Some(Token::Kw(Keyword::SnapAngle)));
                }
                89 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::LayerRule)));
                }
                90 => {
                    return Ok(Some(Token::Kw(Keyword::ViaAtSmd)));
                }
                91 => {
                    return Ok(Some(Token::Kw(Keyword::PullTight)));
                }
                92 => {
                    return Ok(Some(Token::Kw(Keyword::ResolutionScope)));
                }
                93 => {
                    return Ok(Some(Token::Kw(Keyword::FlipStyle)));
                }
                94 => {
                    return Ok(Some(Token::Kw(Keyword::Horizontal)));
                }
                95 => {
                    return Ok(Some(Token::Kw(Keyword::ShoveFixed)));
                }
                96 => {
                    return Ok(Some(Token::Kw(Keyword::ClassClass)));
                }
                97 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::ViaKeepout)));
                }
                98 => {
                    return Ok(Some(Token::Kw(Keyword::NetworkOut)));
                }
                99 => {
                    self.yybegin(LexicalState::IgnoreQuote);
                    return Ok(Some(Token::Kw(Keyword::StringQuote)));
                }
                100 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::LogicalPart)));
                }
                101 => {
                    return Ok(Some(Token::Kw(Keyword::PartLibraryScope)));
                }
                102 => {
                    return Ok(Some(Token::Kw(Keyword::RotateFirst)));
                }
                103 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::HostVersion)));
                }
                104 => {
                    return Ok(Some(Token::Kw(Keyword::Keepout)));
                }
                105 => {
                    return Ok(Some(Token::Kw(Keyword::StartPassNo)));
                }
                106 => {
                    return Ok(Some(Token::Kw(Keyword::NinetyDegree)));
                }
                107 => {
                    self.yybegin(LexicalState::LayerName);
                    return Ok(Some(Token::Kw(Keyword::PolylinePath)));
                }
                108 => {
                    return Ok(Some(Token::Kw(Keyword::PlaceControl)));
                }
                109 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::PlaceKeepout)));
                }
                110 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::ClearanceClass)));
                }
                111 => {
                    return Ok(Some(Token::Kw(Keyword::PlaneViaCosts)));
                }
                112 => {
                    return Ok(Some(Token::Kw(Keyword::FortyfiveDegree)));
                }
                113 => {
                    return Ok(Some(Token::Kw(Keyword::WriteResolution)));
                }
                114 => {
                    return Ok(Some(Token::Kw(Keyword::StartRipupCosts)));
                }
                115 => {
                    return Ok(Some(Token::Kw(Keyword::AutorouteSettings)));
                }
                116 => {
                    return Ok(Some(Token::Kw(Keyword::PreferredDirection)));
                }
                117 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::LogicalPartMapping)));
                }
                118 => {
                    return Ok(Some(Token::Kw(Keyword::GeneratedByFreerouting)));
                }
                119 => {
                    return Ok(Some(Token::Kw(Keyword::PreferredDirectionTraceCosts)));
                }
                120 => {
                    return Ok(Some(Token::Kw(
                        Keyword::AgainstPreferredDirectionTraceCosts,
                    )));
                }
                _ => {
                    if zz_input == YYEOF && self.zz_start_read == self.zz_current_pos {
                        return Ok(None);
                    }
                    return Err(DsnError::Scan("Error: could not match input".to_string()));
                }
            }
        }
    }

        pub fn next_string(&mut self) -> String {
        self.next_string_with(false, ' ')
    }

        pub fn next_string_ignoring_newline(&mut self, ignore_newline: bool) -> String {
        self.next_string_with(ignore_newline, ' ')
    }

                                        pub fn next_string_with(&mut self, ignore_newline: bool, leading: char) -> String {
        let leading = u32::from(leading);
        let mut i: isize = 0;

        let skip_leading: &[u16] = if ignore_newline {
            &[8, 10, 13, 32]
        } else {
            &[8, 32]
        };
        let is_skip = |unit: u16| skip_leading.contains(&unit) || u32::from(unit) == leading;

        while let Some(unit) = self.unit_at(self.zz_marked_pos + i) {
            if !is_skip(unit) {
                break;
            }
            i += 1;
        }

        let mut skip_last_char = false;
        let Some(unit) = self.unit_at(self.zz_marked_pos + i) else {
            return String::new();
        };
        let quoted = unit == 34;
        if quoted {
            i += 1;
            skip_last_char = true;
        }

        let stop_at: &[u16] = if quoted {
            &[10, 13, 34]
        } else {
            &[8, 10, 13, 32, 40, 41]
        };
        let is_stop =
            |unit: u16| stop_at.contains(&unit) || (!quoted && u32::from(unit) == leading);

        let mut units: Vec<u16> = Vec::new();
        while let Some(unit) = self.unit_at(self.zz_marked_pos + i) {
            if is_stop(unit) {
                break;
            }
            units.push(unit);
            i += 1;
        }
        let string_buffer = String::from_utf16_lossy(&units);

        if skip_last_char {
            i += 1;
        }

        if i > 0 {
            self.zz_start_read += i - 1;
            self.zz_current_pos += i - 1;
            self.zz_marked_pos += i;
            self.zz_lexical_state = LexicalState::YyInitial;

            if matches!(self.unit_at(self.zz_marked_pos - 1), Some(40) | Some(41)) {
                self.zz_start_read -= 1;
                self.zz_current_pos -= 1;
                self.zz_marked_pos -= 1;
            }
        }

        string_buffer
    }

        pub fn next_string_list(&mut self) -> Vec<String> {
        self.next_string_list_sep(' ')
    }

                    pub fn next_string_list_sep(&mut self, separator: char) -> Vec<String> {
        let mut result = Vec::new();

        let first_string = self.next_string_with(true, separator);
        if !first_string.is_empty() {
            result.push(first_string);
        }

        loop {
            let next_string = self.next_string_with(true, separator);
            if next_string.is_empty() {
                break;
            }
            result.push(next_string);
        }

        self.zz_start_read = self.zz_marked_pos - 1;
        self.zz_current_pos = self.zz_marked_pos - 1;

        result
    }

                                pub fn next_double(&mut self) -> Option<f64> {
        let s = self.next_string();
        java_number_format_parse(&s)
    }

                        pub fn next_closing_bracket(&mut self) -> Result<bool, DsnError> {
        match self.next_token()? {
            None => Ok(false),
            Some(token) => Ok(matches!(token, Token::Close)),
        }
    }
}

pub fn java_number_format_parse(text: &str) -> Option<f64> {
    let chars: Vec<char> = text.chars().collect();
    let mut pos = 0usize;

    if chars.len() >= 3 && chars[0] == 'N' && chars[1] == 'a' && chars[2] == 'N' {
        return Some(f64::NAN);
    }

    let negative = chars.first() == Some(&'-');
    if negative {
        pos = 1;
    }

    if chars.get(pos) == Some(&'\u{221e}') {
        return Some(if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }

    let mut digits = String::new();
    let mut integer_digits = 0usize;
    let mut saw_digit = false;
    let mut saw_decimal = false;
    let mut saw_exponent = false;
    let mut exponent: i64 = 0;

    while pos < chars.len() {
        let ch = chars[pos];
        if ch.is_ascii_digit() {
            digits.push(ch);
            if !saw_decimal {
                integer_digits += 1;
            }
            saw_digit = true;
            pos += 1;
        } else if ch == '.' {
            if saw_decimal || saw_exponent {
                break;
            }
            saw_decimal = true;
            pos += 1;
        } else if ch == ',' {
            if saw_decimal || saw_exponent {
                break;
            }
            pos += 1;
        } else if !saw_exponent && ch == 'E' {
            let mut p = pos + 1;
            let mut negative_exponent = false;
            if chars.get(p) == Some(&'-') {
                p += 1;
                negative_exponent = true;
            }
            if !chars.get(p).is_some_and(char::is_ascii_digit) {
                break;
            }
            let mut value: i64 = 0;
            while let Some(digit) = chars.get(p).and_then(|c| c.to_digit(10)) {
                value = (value * 10 + i64::from(digit)).min(1_000_000_000);
                p += 1;
            }
            exponent = if negative_exponent { -value } else { value };
            saw_exponent = true;
            pos = p;
        } else {
            break;
        }
    }

    if !saw_digit {
        return None;
    }

    let sign = if negative { "-" } else { "" };
    let scale = integer_digits as i64 + exponent;
    format!("{sign}0.{digits}e{scale}").parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

                    #[test]
    fn non_ascii_unicode_digits_are_not_parsed() {
        for text in ["\u{ff11}\u{ff12}", "\u{663}", "\u{661}\u{662}\u{663}"] {
            assert_eq!(java_number_format_parse(text), None, "{text:?}");
        }
        assert_eq!(java_number_format_parse("1\u{ff12}3"), Some(1.0));
        assert_eq!(java_number_format_parse("12"), Some(12.0));
    }

                #[test]
    fn a_token_running_to_the_end_of_the_input_stops_there() {
        let mut scanner = DsnScanner::new(" foo");
        assert_eq!(scanner.next_string(), "foo");
    }
}
