//! The Specctra DSN lexer — a port of `io/specctra/parser/SpecctraDsnStreamReader.java` (the
//! JFlex 1.4.1 scanner generated in 2008 from `SpecctraFileDescription.flex`) and of the
//! `io/specctra/parser/IJFlexScanner.java` interface it implements.
//!
//! Two things are worth knowing before reading this file.
//!
//! **The DFA is transcribed, not rewritten** (plan ruling 3). The five packed tables in the
//! Java source are decoded by `scripts/gen-lexer-tables.py` into [`tables`]; only
//! [`DsnScanner::next_token`]'s driver loop and its 120-arm action switch are hand-ported, arm
//! by arm, each carrying the Java line it came from. The scanner is `%ignorecase` and has eight
//! lexical states ([`LexicalState`]); keywords such as `net` or `path` switch state as a side
//! effect of being recognised, which is why a hand-written recogniser was not an option.
//!
//! **There is a second, hand-rolled tokeniser in the same class.**
//! [`DsnScanner::next_string_with`] (`SpecctraDsnStreamReader.java:1739`) walks the buffer
//! directly, bypassing the DFA, and [`DsnScanner::next_double`] parses with Java's *lenient*
//! `NumberFormat`. The two disagree: `1e5` is a float of 100000 through the DFA and a double of
//! 1.0 through `nextDouble`. Both grammars are ported as they are.

pub mod tables;
pub mod token;

pub use token::Token;

use crate::error::DsnError;
use crate::keyword::Keyword;

/// `YYEOF` (`SpecctraDsnStreamReader.java:26`): the input value that denotes end of file.
const YYEOF: i32 = -1;

/// The scanner's eight lexical states (`SpecctraDsnStreamReader.java:29-37`).
///
/// The driver seeds the DFA with `zzState = zzLexicalState` (`:896`), so the discriminants are
/// load-bearing: they are DFA state numbers, not just tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LexicalState {
    /// `YYINITIAL = 0` — the default state.
    YyInitial = 0,
    /// `STRING1 = 1` — inside a `"`-quoted string.
    String1 = 1,
    /// `STRING2 = 2` — inside a `'`-quoted string.
    String2 = 2,
    /// `NAME = 3` — the one token after `net`, `layer`, `padstack`, `via`, `component`,
    /// `image`, `place`, `wire` or `keepout`, where a number lexes as a string instead.
    Name = 3,
    /// `LAYER_NAME = 4` — the one token after `path`, `polygon`, `rect`, `circle` or
    /// `polyline_path`; like `NAME`, but `pcb` and `signal` are still keywords here.
    LayerName = 4,
    /// `COMPONENT_NAME = 5` — reachable only from the DFA tables, never from a `yybegin` in the
    /// action switch.
    ComponentName = 5,
    /// `SPEC_CHAR = 6` — likewise unreachable from the action switch.
    SpecChar = 6,
    /// `IGNORE_QUOTE = 7` — entered by `string_quote`, so the quote character that follows is
    /// not treated as opening a string.
    IgnoreQuote = 7,
}

/// The Specctra scanner: a lexical analyser over an in-memory buffer of UTF-16 code units.
///
/// Java holds `zzBuffer` as a `char[ZZ_BUFFERSIZE]` and refills it from a `Reader`; the port
/// converts the whole input up front instead, because the hand-rolled `nextString` bypass
/// indexes the buffer with no refill and therefore only ever worked for inputs that fit in one
/// bufferful anyway (see [`DsnScanner::new`]).
///
/// The buffer holds **`u16` code units, not `char`s**: `ZZ_CMAP`'s domain is exactly
/// `0..=0xFFFF` (Java builds it as `char[0x10000]`, `SpecctraDsnStreamReader.java:705`), so a
/// supplementary-plane character must be fed to the DFA as the surrogate pair Java sees, one
/// code unit at a time.
#[derive(Debug)]
pub struct DsnScanner {
    /// `zzBuffer`. // Java bug: zzBuffer is a fixed `char[16 * 1024 * 1024]` (`:40`, `:561`)
    /// that `nextString` indexes without ever refilling, so a token straddling a refill is
    /// silently mis-lexed and a file over 16 MiB cannot be scanned at all. The port refuses
    /// such an input with [`DsnError::InputTooLarge`] rather than mis-lexing it.
    buffer: Vec<u16>,
    /// `zzStartRead` — start of `yytext()` in the buffer. Java's positions are `int`s that
    /// `nextString`/`nextStringList` can drive to `-1`, so they are `isize` here.
    zz_start_read: isize,
    /// `zzMarkedPos` — the position after the last accepting match.
    zz_marked_pos: isize,
    /// `zzCurrentPos` — the current scan position.
    zz_current_pos: isize,
    /// `zzState` — the current DFA state.
    zz_state: usize,
    /// `zzLexicalState`.
    zz_lexical_state: LexicalState,
    /// `stringBuffer` — accumulates the body of a quoted string. Kept as UTF-16 code units,
    /// like Java's `StringBuffer`, so that a surrogate pair split across two DFA matches is
    /// reassembled rather than mangled.
    string_buffer: Vec<u16>,
    /// `scopeIdentifier`. // renamed: scopeIdentifier is `public static` in Java
    /// (`SpecctraDsnStreamReader.java:545`); plan ruling 6 makes it instance state.
    scope_identifier: String,
}

impl DsnScanner {
    /// Creates a scanner over `input` (Java: `SpecctraDsnStreamReader(InputStream)`, which
    /// wraps the stream in an `InputStreamReader` and so decodes it as UTF-8).
    ///
    /// Fails with [`DsnError::InputTooLarge`] when the input does not fit Java's fixed 16 MiB
    /// `zzBuffer`; see the field's note.
    pub fn new(input: &str) -> Result<Self, DsnError> {
        let buffer: Vec<u16> = input.encode_utf16().collect();
        if buffer.len() > tables::ZZ_BUFFERSIZE {
            return Err(DsnError::InputTooLarge {
                units: buffer.len(),
                limit: tables::ZZ_BUFFERSIZE,
            });
        }
        Ok(Self {
            buffer,
            zz_start_read: 0,
            zz_marked_pos: 0,
            zz_current_pos: 0,
            zz_state: 0,
            zz_lexical_state: LexicalState::YyInitial,
            string_buffer: Vec::new(),
            scope_identifier: String::new(),
        })
    }

    /// `yybegin` (`:795`) — enters a new lexical state.
    pub fn yybegin(&mut self, new_state: LexicalState) {
        self.zz_lexical_state = new_state;
    }

    /// `yystate` (`:786`) — the current lexical state.
    pub fn yystate(&self) -> LexicalState {
        self.zz_lexical_state
    }

    /// `getScopeIdentifier` (`:861`). // renamed: getScopeIdentifier
    pub fn scope_identifier(&self) -> &str {
        &self.scope_identifier
    }

    /// `setScopeIdentifier` (`:866`).
    pub fn set_scope_identifier(&mut self, identifier: &str) {
        self.scope_identifier = identifier.to_string();
    }

    /// `yytext` (`:800`) — the text matched by the current regular expression.
    ///
    /// `from_utf16_lossy` where Java's `new String(char[], int, int)` keeps unpaired surrogates:
    /// only reachable if a match splits a surrogate pair, which the character classes do not do
    /// (both halves are in the same class).
    pub fn yytext(&self) -> String {
        String::from_utf16_lossy(self.matched_units())
    }

    /// The matched region as raw code units.
    fn matched_units(&self) -> &[u16] {
        let start = self.zz_start_read.max(0) as usize;
        let end = self.zz_marked_pos.max(0) as usize;
        &self.buffer[start.min(self.buffer.len())..end.min(self.buffer.len())]
    }

    /// The code unit at `pos`, or `None` when `pos` is outside the buffer (where Java would
    /// throw `ArrayIndexOutOfBoundsException`, or read the unwritten tail of `zzBuffer`).
    fn unit_at(&self, pos: isize) -> Option<u16> {
        if pos < 0 {
            return None;
        }
        self.buffer.get(pos as usize).copied()
    }

    // not ported: yyclose — the port owns no `Reader` to close.
    // not ported: yyreset — the port constructs a new scanner per input instead.
    // not ported: yypushback — no ported caller uses it (the action switch never does).
    // not ported: yycharat — no ported caller uses it.
    // not ported: yylength — no ported caller uses it.
    // not ported: zzRefill — the whole input is converted up front by `new`, so the buffer is
    // never refilled and never shifted. Java's shifting changes the numeric value of the
    // positions but not the text they point at, and the only place a raw position is observable
    // is the dropped `FRLogger.warn` of action 2.

    /// `nextToken` (`:877`) — resumes scanning until the next regular expression matches, the
    /// end of input is reached (`Ok(None)`, Java's `null`) or the input cannot be matched.
    ///
    /// The body is the JFlex driver loop (`:877-940`) followed by the action switch
    /// (`:940-1728`), transcribed arm by arm.
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
            // `zzForAction`.
            loop {
                // The `>= 0` half has no Java counterpart (Java would throw); positions only go
                // negative through `next_string_list`, which never leaves `zz_marked_pos`
                // negative, and the driver seeds this loop from `zz_marked_pos`.
                if zz_current_pos_l >= 0 && zz_current_pos_l < zz_end_read_l {
                    zz_input = self.buffer[zz_current_pos_l as usize] as i32;
                    zz_current_pos_l += 1;
                } else {
                    // Java calls `zzRefill()` here first; with the whole input already in the
                    // buffer it can only report EOF.
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
                // SpecctraDsnStreamReader.java:1401
                1 => {
                    self.yybegin(LexicalState::YyInitial);
                    return Ok(Some(Token::Str(self.yytext())));
                }
                // SpecctraDsnStreamReader.java:1650 — `FRLogger.warn("Non-ansi character ...")`,
                // dropped per the plan's Global Constraints (no `FRLogger` in `fr-dsn`).
                2 => {}
                // SpecctraDsnStreamReader.java:1603 — whitespace and comments: `/* ignore */`.
                3 => {}
                // SpecctraDsnStreamReader.java:1270
                4 => {
                    return Ok(Some(Token::Str(self.yytext())));
                }
                // SpecctraDsnStreamReader.java:1688 — `Integer.valueOf(yytext())`, which throws
                // `NumberFormatException` when the literal does not fit a Java `int`.
                5 => {
                    let text = self.yytext();
                    let value: i32 = text.parse().map_err(|_| {
                        DsnError::Scan(format!("For input string: \"{text}\" (Integer.valueOf)"))
                    })?;
                    return Ok(Some(Token::Int(i64::from(value))));
                }
                // SpecctraDsnStreamReader.java:1341
                6 => {
                    self.string_buffer.clear();
                    self.yybegin(LexicalState::String1);
                }
                // SpecctraDsnStreamReader.java:1306
                7 => {
                    self.string_buffer.clear();
                    self.yybegin(LexicalState::String2);
                }
                // SpecctraDsnStreamReader.java:1642
                10 => {
                    let start = self.zz_start_read.max(0) as usize;
                    let end = self.zz_marked_pos.max(0) as usize;
                    let (start, end) = (start.min(self.buffer.len()), end.min(self.buffer.len()));
                    self.string_buffer
                        .extend_from_slice(&self.buffer[start..end]);
                }
                // SpecctraDsnStreamReader.java:1570
                11 => {
                    self.yybegin(LexicalState::YyInitial);
                    return Ok(Some(Token::Str(String::from_utf16_lossy(
                        &self.string_buffer,
                    ))));
                }
                // SpecctraDsnStreamReader.java:1477 — a backslash inside a quoted string is
                // appended literally; there is no escape processing anywhere in this scanner.
                12 => {
                    self.string_buffer.push(u16::from(b'\\'));
                }
                // SpecctraDsnStreamReader.java:1484 — `Double.valueOf(yytext())`.
                15 => {
                    let text = self.yytext();
                    let value: f64 = text.parse().map_err(|_| {
                        DsnError::Scan(format!("For input string: \"{text}\" (Double.valueOf)"))
                    })?;
                    return Ok(Some(Token::Float(value)));
                }
                // -- the 109 keyword arms, in case order -------------------------------------
                // SpecctraDsnStreamReader.java:1150
                8 => {
                    return Ok(Some(Token::Open));
                }
                // SpecctraDsnStreamReader.java:1182
                9 => {
                    return Ok(Some(Token::Close));
                }
                // SpecctraDsnStreamReader.java:1019
                13 => {
                    self.yybegin(LexicalState::YyInitial);
                    return Ok(Some(Token::Open));
                }
                // SpecctraDsnStreamReader.java:1175
                14 => {
                    self.yybegin(LexicalState::YyInitial);
                    return Ok(Some(Token::Close));
                }
                // SpecctraDsnStreamReader.java:1114
                16 => {
                    return Ok(Some(Token::Kw(Keyword::On)));
                }
                // SpecctraDsnStreamReader.java:1363
                17 => {
                    return Ok(Some(Token::Kw(Keyword::Off)));
                }
                // SpecctraDsnStreamReader.java:948
                18 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::Via)));
                }
                // SpecctraDsnStreamReader.java:1577
                19 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::Net)));
                }
                // SpecctraDsnStreamReader.java:1084
                20 => {
                    return Ok(Some(Token::Kw(Keyword::PcbScope)));
                }
                // SpecctraDsnStreamReader.java:1096
                21 => {
                    return Ok(Some(Token::Kw(Keyword::Pin)));
                }
                // SpecctraDsnStreamReader.java:1156
                22 => {
                    return Ok(Some(Token::Kw(Keyword::Fix)));
                }
                // SpecctraDsnStreamReader.java:1032
                23 => {
                    self.yybegin(LexicalState::YyInitial);
                    return Ok(Some(Token::Kw(Keyword::PcbScope)));
                }
                // SpecctraDsnStreamReader.java:955
                24 => {
                    return Ok(Some(Token::Kw(Keyword::Back)));
                }
                // SpecctraDsnStreamReader.java:1694
                25 => {
                    return Ok(Some(Token::Kw(Keyword::Side)));
                }
                // SpecctraDsnStreamReader.java:1552
                26 => {
                    return Ok(Some(Token::Kw(Keyword::Type)));
                }
                // SpecctraDsnStreamReader.java:1052
                27 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::ComponentScope)));
                }
                // SpecctraDsnStreamReader.java:1194
                28 => {
                    self.yybegin(LexicalState::LayerName);
                    return Ok(Some(Token::Kw(Keyword::Circle)));
                }
                // SpecctraDsnStreamReader.java:1108
                29 => {
                    return Ok(Some(Token::Kw(Keyword::Vias)));
                }
                // SpecctraDsnStreamReader.java:1504
                30 => {
                    return Ok(Some(Token::Kw(Keyword::None)));
                }
                // SpecctraDsnStreamReader.java:961
                31 => {
                    self.yybegin(LexicalState::LayerName);
                    return Ok(Some(Token::Kw(Keyword::PolygonPath)));
                }
                // SpecctraDsnStreamReader.java:992
                32 => {
                    self.yybegin(LexicalState::LayerName);
                    return Ok(Some(Token::Kw(Keyword::Polygon)));
                }
                // SpecctraDsnStreamReader.java:1676
                33 => {
                    return Ok(Some(Token::Kw(Keyword::Pins)));
                }
                // SpecctraDsnStreamReader.java:1039
                34 => {
                    self.yybegin(LexicalState::LayerName);
                    return Ok(Some(Token::Kw(Keyword::Rectangle)));
                }
                // SpecctraDsnStreamReader.java:1102
                35 => {
                    return Ok(Some(Token::Kw(Keyword::Rule)));
                }
                // SpecctraDsnStreamReader.java:1231
                36 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::Wire)));
                }
                // SpecctraDsnStreamReader.java:1090
                37 => {
                    return Ok(Some(Token::Kw(Keyword::Spare)));
                }
                // SpecctraDsnStreamReader.java:1528
                38 => {
                    return Ok(Some(Token::Kw(Keyword::Shape)));
                }
                // SpecctraDsnStreamReader.java:1357
                39 => {
                    return Ok(Some(Token::Kw(Keyword::Order)));
                }
                // SpecctraDsnStreamReader.java:1244
                40 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::Layer)));
                }
                // SpecctraDsnStreamReader.java:1700
                41 => {
                    return Ok(Some(Token::Kw(Keyword::Clearance)));
                }
                // SpecctraDsnStreamReader.java:1491
                42 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::Class)));
                }
                // SpecctraDsnStreamReader.java:1059
                43 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::Image)));
                }
                // SpecctraDsnStreamReader.java:1540
                44 => {
                    return Ok(Some(Token::Kw(Keyword::Power)));
                }
                // SpecctraDsnStreamReader.java:1334
                45 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::Place)));
                }
                // SpecctraDsnStreamReader.java:1394
                46 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::PlaneScope)));
                }
                // SpecctraDsnStreamReader.java:1294
                47 => {
                    return Ok(Some(Token::Kw(Keyword::Rules)));
                }
                // SpecctraDsnStreamReader.java:1564
                48 => {
                    return Ok(Some(Token::Kw(Keyword::Front)));
                }
                // SpecctraDsnStreamReader.java:1428
                49 => {
                    return Ok(Some(Token::Kw(Keyword::Width)));
                }
                // SpecctraDsnStreamReader.java:1201
                50 => {
                    return Ok(Some(Token::Kw(Keyword::Attach)));
                }
                // SpecctraDsnStreamReader.java:1558
                51 => {
                    return Ok(Some(Token::Kw(Keyword::Active)));
                }
                // SpecctraDsnStreamReader.java:1375
                52 => {
                    return Ok(Some(Token::Kw(Keyword::Signal)));
                }
                // SpecctraDsnStreamReader.java:1522
                53 => {
                    return Ok(Some(Token::Kw(Keyword::Length)));
                }
                // SpecctraDsnStreamReader.java:1144
                54 => {
                    return Ok(Some(Token::Kw(Keyword::Normal)));
                }
                // SpecctraDsnStreamReader.java:1636
                55 => {
                    return Ok(Some(Token::Kw(Keyword::ParserScope)));
                }
                // SpecctraDsnStreamReader.java:974
                56 => {
                    return Ok(Some(Token::Kw(Keyword::Routes)));
                }
                // SpecctraDsnStreamReader.java:1300
                57 => {
                    return Ok(Some(Token::Kw(Keyword::Rotate)));
                }
                // SpecctraDsnStreamReader.java:1282
                58 => {
                    return Ok(Some(Token::Kw(Keyword::Fanout)));
                }
                // SpecctraDsnStreamReader.java:1534
                59 => {
                    return Ok(Some(Token::Kw(Keyword::Fromto)));
                }
                // SpecctraDsnStreamReader.java:1072
                60 => {
                    return Ok(Some(Token::Kw(Keyword::Window)));
                }
                // SpecctraDsnStreamReader.java:1257
                61 => {
                    return Ok(Some(Token::Kw(Keyword::WiringScope)));
                }
                // SpecctraDsnStreamReader.java:1713
                62 => {
                    self.yybegin(LexicalState::YyInitial);
                    return Ok(Some(Token::Kw(Keyword::Signal)));
                }
                // SpecctraDsnStreamReader.java:1120
                63 => {
                    return Ok(Some(Token::Kw(Keyword::Session)));
                }
                // SpecctraDsnStreamReader.java:1328
                64 => {
                    return Ok(Some(Token::Kw(Keyword::Outline)));
                }
                // SpecctraDsnStreamReader.java:1597
                65 => {
                    return Ok(Some(Token::Kw(Keyword::LibraryScope)));
                }
                // SpecctraDsnStreamReader.java:1590
                66 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::UseVia)));
                }
                // SpecctraDsnStreamReader.java:1440
                67 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::UseNet)));
                }
                // SpecctraDsnStreamReader.java:1388
                68 => {
                    return Ok(Some(Token::Kw(Keyword::Control)));
                }
                // SpecctraDsnStreamReader.java:1251
                69 => {
                    return Ok(Some(Token::Kw(Keyword::Classes)));
                }
                // SpecctraDsnStreamReader.java:1219
                70 => {
                    return Ok(Some(Token::Kw(Keyword::Circuit)));
                }
                // SpecctraDsnStreamReader.java:968
                71 => {
                    return Ok(Some(Token::Kw(Keyword::NetworkScope)));
                }
                // SpecctraDsnStreamReader.java:1623
                72 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::Keepout)));
                }
                // SpecctraDsnStreamReader.java:1276
                73 => {
                    return Ok(Some(Token::Kw(Keyword::Absolute)));
                }
                // SpecctraDsnStreamReader.java:1126
                74 => {
                    return Ok(Some(Token::Kw(Keyword::Boundary)));
                }
                // SpecctraDsnStreamReader.java:1471
                75 => {
                    return Ok(Some(Token::Kw(Keyword::Constant)));
                }
                // SpecctraDsnStreamReader.java:1078
                76 => {
                    return Ok(Some(Token::Kw(Keyword::Vertical)));
                }
                // SpecctraDsnStreamReader.java:1213
                77 => {
                    return Ok(Some(Token::Kw(Keyword::ViaRule)));
                }
                // SpecctraDsnStreamReader.java:999
                78 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::Padstack)));
                }
                // SpecctraDsnStreamReader.java:1288
                79 => {
                    return Ok(Some(Token::Kw(Keyword::Position)));
                }
                // SpecctraDsnStreamReader.java:1321
                80 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::HostCad)));
                }
                // SpecctraDsnStreamReader.java:1369
                81 => {
                    return Ok(Some(Token::Kw(Keyword::Autoroute)));
                }
                // SpecctraDsnStreamReader.java:1046
                82 => {
                    return Ok(Some(Token::Kw(Keyword::StructureScope)));
                }
                // SpecctraDsnStreamReader.java:1415
                83 => {
                    return Ok(Some(Token::Kw(Keyword::LockType)));
                }
                // SpecctraDsnStreamReader.java:1314
                84 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::UseLayer)));
                }
                // SpecctraDsnStreamReader.java:1459
                85 => {
                    return Ok(Some(Token::Kw(Keyword::ViaCosts)));
                }
                // SpecctraDsnStreamReader.java:1169
                86 => {
                    return Ok(Some(Token::Kw(Keyword::Postroute)));
                }
                // SpecctraDsnStreamReader.java:1225
                87 => {
                    return Ok(Some(Token::Kw(Keyword::PlacementScope)));
                }
                // SpecctraDsnStreamReader.java:1132
                88 => {
                    return Ok(Some(Token::Kw(Keyword::SnapAngle)));
                }
                // SpecctraDsnStreamReader.java:1162
                89 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::LayerRule)));
                }
                // SpecctraDsnStreamReader.java:1188
                90 => {
                    return Ok(Some(Token::Kw(Keyword::ViaAtSmd)));
                }
                // SpecctraDsnStreamReader.java:1498
                91 => {
                    return Ok(Some(Token::Kw(Keyword::PullTight)));
                }
                // SpecctraDsnStreamReader.java:1207
                92 => {
                    return Ok(Some(Token::Kw(Keyword::ResolutionScope)));
                }
                // SpecctraDsnStreamReader.java:980
                93 => {
                    return Ok(Some(Token::Kw(Keyword::FlipStyle)));
                }
                // SpecctraDsnStreamReader.java:1546
                94 => {
                    return Ok(Some(Token::Kw(Keyword::Horizontal)));
                }
                // SpecctraDsnStreamReader.java:1664
                95 => {
                    return Ok(Some(Token::Kw(Keyword::ShoveFixed)));
                }
                // SpecctraDsnStreamReader.java:1584
                96 => {
                    return Ok(Some(Token::Kw(Keyword::ClassClass)));
                }
                // SpecctraDsnStreamReader.java:1706
                97 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::ViaKeepout)));
                }
                // SpecctraDsnStreamReader.java:1630
                98 => {
                    return Ok(Some(Token::Kw(Keyword::NetworkOut)));
                }
                // SpecctraDsnStreamReader.java:1350
                99 => {
                    self.yybegin(LexicalState::IgnoreQuote);
                    return Ok(Some(Token::Kw(Keyword::StringQuote)));
                }
                // SpecctraDsnStreamReader.java:1408
                100 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::LogicalPart)));
                }
                // SpecctraDsnStreamReader.java:1453
                101 => {
                    return Ok(Some(Token::Kw(Keyword::PartLibraryScope)));
                }
                // SpecctraDsnStreamReader.java:1465
                102 => {
                    return Ok(Some(Token::Kw(Keyword::RotateFirst)));
                }
                // SpecctraDsnStreamReader.java:1263
                103 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::HostVersion)));
                }
                // SpecctraDsnStreamReader.java:1670
                104 => {
                    return Ok(Some(Token::Kw(Keyword::Keepout)));
                }
                // SpecctraDsnStreamReader.java:1026
                105 => {
                    return Ok(Some(Token::Kw(Keyword::StartPassNo)));
                }
                // SpecctraDsnStreamReader.java:1434
                106 => {
                    return Ok(Some(Token::Kw(Keyword::NinetyDegree)));
                }
                // SpecctraDsnStreamReader.java:1381
                107 => {
                    self.yybegin(LexicalState::LayerName);
                    return Ok(Some(Token::Kw(Keyword::PolylinePath)));
                }
                // SpecctraDsnStreamReader.java:986
                108 => {
                    return Ok(Some(Token::Kw(Keyword::PlaceControl)));
                }
                // SpecctraDsnStreamReader.java:1421
                109 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::PlaceKeepout)));
                }
                // SpecctraDsnStreamReader.java:1006
                110 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::ClearanceClass)));
                }
                // SpecctraDsnStreamReader.java:1617
                111 => {
                    return Ok(Some(Token::Kw(Keyword::PlaneViaCosts)));
                }
                // SpecctraDsnStreamReader.java:1066
                112 => {
                    return Ok(Some(Token::Kw(Keyword::FortyfiveDegree)));
                }
                // SpecctraDsnStreamReader.java:1138
                113 => {
                    return Ok(Some(Token::Kw(Keyword::WriteResolution)));
                }
                // SpecctraDsnStreamReader.java:1516
                114 => {
                    return Ok(Some(Token::Kw(Keyword::StartRipupCosts)));
                }
                // SpecctraDsnStreamReader.java:1013
                115 => {
                    return Ok(Some(Token::Kw(Keyword::AutorouteSettings)));
                }
                // SpecctraDsnStreamReader.java:1238
                116 => {
                    return Ok(Some(Token::Kw(Keyword::PreferredDirection)));
                }
                // SpecctraDsnStreamReader.java:1610
                117 => {
                    self.yybegin(LexicalState::Name);
                    return Ok(Some(Token::Kw(Keyword::LogicalPartMapping)));
                }
                // SpecctraDsnStreamReader.java:1447
                118 => {
                    return Ok(Some(Token::Kw(Keyword::GeneratedByFreerouting)));
                }
                // SpecctraDsnStreamReader.java:1682
                119 => {
                    return Ok(Some(Token::Kw(Keyword::PreferredDirectionTraceCosts)));
                }
                // SpecctraDsnStreamReader.java:1510
                120 => {
                    return Ok(Some(Token::Kw(
                        Keyword::AgainstPreferredDirectionTraceCosts,
                    )));
                }
                _ => {
                    // SpecctraDsnStreamReader.java:1720. `zzStartRead == zzCurrentPos` always
                    // holds: both are assigned `zzMarkedPosL` at the head of the loop and only
                    // `zzRefill` (which shifts both by the same amount) touches them since.
                    if zz_input == YYEOF && self.zz_start_read == self.zz_current_pos {
                        // not ported: zzAtEOF — Java sets it here and only `zzRefill`/`yyclose`
                        // ever read it, and neither is ported; scanning past the end returns
                        // `Ok(None)` again anyway.
                        return Ok(None);
                    }
                    // `zzScanError(ZZ_NO_MATCH)`, which throws `Error` in Java (`:833`).
                    return Err(DsnError::Scan("Error: could not match input".to_string()));
                }
            }
        }
    }

    /// `nextString()` (`:1731`) — `nextString(false, ' ')`.
    pub fn next_string(&mut self) -> String {
        self.next_string_with(false, ' ')
    }

    /// `nextString(boolean)` (`:1735`) — `nextString(ignoreNewline, ' ')`.
    pub fn next_string_ignoring_newline(&mut self, ignore_newline: bool) -> String {
        self.next_string_with(ignore_newline, ' ')
    }

    /// `nextString(boolean, char)` (`:1739`) — the hand-rolled bypass that does **not** use the
    /// DFA: it walks `zzBuffer` from `zzMarkedPos`, skipping a leading run of skip characters
    /// and then reading up to the first stop character.
    ///
    /// The four character sets are Java's, verbatim (`:593-596`):
    /// `stringSkipTrailing = {8, 32}`, `stringSkipTrailingNewLines = {8, 10, 13, 32}`,
    /// `stringStopAt = {8, 10, 13, 32, 40, 41}`, `stringStopAtQuotes = {10, 13, 34}`.
    /// // Java bug: nextString's sets contain 8 (backspace) and not 9 (tab), though the
    /// comments say "spaces, tabs" — a tab is part of a string here, a backspace is not.
    pub fn next_string_with(&mut self, ignore_newline: bool, leading: char) -> String {
        let leading = u32::from(leading);
        // Java reuses the DFA's `stringBuffer` field here (`stringBuffer.setLength(0)` first); a
        // local is equivalent, because every reader of that field (actions 10 and 11) is
        // preceded by the action 6/7 that clears it.
        let mut string_buffer = String::new();
        let mut i: isize = 0;

        // `skipLeading`
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
        // Java indexes `zzBuffer[zzMarkedPos + i]` here with no bounds check.
        // totalized: nextString returns the empty string where Java reads past the end of the
        // input (`ArrayIndexOutOfBoundsException` on a buffer sized to the file, or a run of
        // unwritten `\0`s in the real 16 MiB one). No reachable caller observes it: every DSN
        // scope this scanner is used inside ends with `)`.
        let Some(unit) = self.unit_at(self.zz_marked_pos + i) else {
            return string_buffer;
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

        // Read the actual string until a stop character.
        let mut units: Vec<u16> = Vec::new();
        while let Some(unit) = self.unit_at(self.zz_marked_pos + i) {
            if is_stop(unit) {
                break;
            }
            units.push(unit);
            i += 1;
        }
        string_buffer.push_str(&String::from_utf16_lossy(&units));

        if skip_last_char {
            i += 1;
        }

        if i > 0 {
            self.zz_start_read += i - 1;
            self.zz_current_pos += i - 1;
            self.zz_marked_pos += i;
            self.zz_lexical_state = LexicalState::YyInitial;

            if matches!(self.unit_at(self.zz_marked_pos - 1), Some(40) | Some(41)) {
                // The string ended on a bracket, which has to stay available as the next token.
                self.zz_start_read -= 1;
                self.zz_current_pos -= 1;
                self.zz_marked_pos -= 1;
            }
        }

        string_buffer
    }

    /// `nextStringList()` (`:1798`) — `nextStringList(' ')`.
    pub fn next_string_list(&mut self) -> Vec<String> {
        self.next_string_list_sep(' ')
    }

    /// `nextStringList(char)` (`:1802`) — reads strings until an empty one.
    ///
    /// The first string is allowed to be empty and is then dropped: a workaround for the KiCad 8
    /// bug that starts a net list with a `""` entry (Java's comment at `:1801-1804`).
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

    /// `nextDouble()` (`:1829`) — `nextString()` parsed by Java's **lenient**
    /// `NumberFormat.getInstance(Locale.US)`; `None` where Java catches `ParseException` and
    /// returns `null`.
    ///
    /// // renamed: nf — Java caches the `NumberFormat` in a `public static` field (`:546`);
    /// plan ruling 6 forbids static mutable state, and the parser it configures is stateless,
    /// so the port is the free function [`java_number_format_parse`].
    pub fn next_double(&mut self) -> Option<f64> {
        let s = self.next_string();
        java_number_format_parse(&s)
    }

    /// `nextClosingBracket()` (`:1842`).
    ///
    /// Java returns `false` (after an `FRLogger.warn`, dropped here) at end of file and for any
    /// token that is not `)`, and lets a scan `Error` propagate — hence `Result<bool, _>` rather
    /// than the plain `bool` of the Java signature.
    pub fn next_closing_bracket(&mut self) -> Result<bool, DsnError> {
        match self.next_token()? {
            // "Network.read_net_pins: unexpected end of file"
            None => Ok(false),
            // "Network.read_net_pins: expected closed bracket is missing"
            Some(token) => Ok(matches!(token, Token::Close)),
        }
    }
}

/// Java's lenient `NumberFormat.getInstance(Locale.US).parse(...)`, as `nextDouble` uses it.
///
/// This is a *second, different* number grammar from the DFA's `DecFloatLiteral`, and the
/// differences are all observable through `nextDouble`:
///
/// - it parses a **prefix** and ignores the rest, so `"1abc"` is 1.0 and `"1.2.3"` is 1.2;
/// - `,` is a grouping separator and is simply skipped before the decimal point (`"1,2"` is 12,
///   `",5"` is 5), but ends the number after it;
/// - the exponent separator is an **uppercase `E` only**, and its sign may only be `-`: `"1E5"`
///   is 100000, but `"1e5"` is 1.0 and `"1E+3"` is 1.0;
/// - a leading `+` is not accepted, and neither is leading whitespace: `"+1"` and `" 1"` fail
///   where the DFA happily lexes `+5` as an integer;
/// - `"NaN"` (before any sign) and `"∞"` are accepted; `"Infinity"` is not;
/// - it fails (Java: `ParseException`, here `None`) only when no digit was consumed.
///
/// Verified against JDK 25's `NumberFormat` over the cases above; see the Task 3 report.
pub fn java_number_format_parse(text: &str) -> Option<f64> {
    let chars: Vec<char> = text.chars().collect();
    let mut pos = 0usize;

    // `DecimalFormat.subparse` tests the NaN symbol before the sign prefix, so `-NaN` fails.
    if chars.len() >= 3 && chars[0] == 'N' && chars[1] == 'a' && chars[2] == 'N' {
        return Some(f64::NAN);
    }

    // The positive prefix is empty and the negative prefix is `-`; the longer match wins.
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
            // Grouping is used by default; a separator after the decimal point ends the number.
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
                // Saturate rather than overflow; such an exponent is Infinity or 0 either way.
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

    // Java assembles the retained digits and their decimal position into a `double` through
    // `DigitList.getDouble`, which is `Double.parseDouble` of `0.<digits>E<decimalAt>` — the
    // same correctly-rounded conversion Rust's `str::parse::<f64>` performs.
    let sign = if negative { "-" } else { "" };
    let scale = integer_digits as i64 + exponent;
    format!("{sign}0.{digits}e{scale}").parse().ok()
}
