//! The token type the Specctra scanner produces.

use crate::keyword::Keyword;

/// One token of a Specctra DSN/SES/rules file.
///
/// Java's `SpecctraDsnStreamReader.nextToken` returns a bare `Object`: a `Keyword` (compared
/// with `==` — that identity *is* the parser's dispatch), a `String`, an `Integer`, a `Double`,
/// or `null` at end of file. The port makes the union explicit; `null` becomes `Ok(None)` from
/// [`crate::lexer::DsnScanner::next_token`].
///
/// `Open`/`Close` are Java's `Keyword.OPEN_BRACKET`/`Keyword.CLOSED_BRACKET`; splitting them out
/// of `Kw` turns every `nextToken() == Keyword.OPEN_BRACKET` test into a `matches!`.
/// `// renamed: OPEN_BRACKET`, `// renamed: CLOSED_BRACKET`.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// `(` — Java's `Keyword.OPEN_BRACKET`.
    Open,
    /// `)` — Java's `Keyword.CLOSED_BRACKET`.
    Close,
    /// A keyword the DFA recognised (Java returns the `Keyword` singleton itself).
    Kw(Keyword),
    /// An identifier or quoted string (Java returns a `String`).
    Str(String),
    /// A `DecIntegerLiteral` (Java returns an `Integer` via `Integer.valueOf`).
    Int(i64),
    /// A `DecFloatLiteral` (Java returns a `Double` via `Double.valueOf`).
    Float(f64),
}
