//! `datastructures/IdentifierType.java`: legal-identifier quoting for Specctra DSN/SES output.

use crate::format::indent_writer::IndentFileWriter;
use std::io::Write;

/// Reserved characters for DSN identifiers (`io/specctra/parser/WriteScopeParameter.java:33`).
pub const DSN_RESERVED: [&str; 6] = ["(", ")", " ", ";", "-", "_"];

/// Reserved characters for SES (session) identifiers (`io/specctra/SesWriter.java:63`).
pub const SES_RESERVED: [&str; 10] = ["(", ")", " ", ";", "-", "_", "/", "~", "{", "}"];

/// Describes legal identifiers together with the character used for string quotes
/// (`IdentifierType.java`).
pub struct IdentifierType {
    string_quote: String,
    reserved_chars: Vec<String>,
}

impl IdentifierType {
    /// `IdentifierType(String[], String)` (IdentifierType.java:18-21).
    pub fn new(reserved_chars: Vec<String>, string_quote: String) -> Self {
        Self {
            string_quote,
            reserved_chars,
        }
    }

    /// Writes `name` after putting it into quotes, if it contains reserved characters or blanks
    /// (`write`, IdentifierType.java:24-67).
    pub fn write<W: Write>(&self, name: &str, out: &mut IndentFileWriter<W>) {
        let mut name = name.to_string();

        // (1) Strip a wrapping pair of quote characters — with Java's off-by-one bug:
        // `substring(1, length() - 2)` drops the first character *and the last two*, instead
        // of the correct `substring(1, length() - 1)` (IdentifierType.java:25-30). Java indexes
        // `charAt` on UTF-16 code units; this loop's boundary check only ever inspects the
        // first/last character against the ASCII `"` quote, so Rust `chars()` (Unicode scalar
        // value) indexing here is behaviorally equivalent to Java's for every string reachable
        // in practice.
        while name.chars().count() > 2 && name.starts_with('"') && name.ends_with('"') {
            let n = name.chars().count();
            name = name.chars().skip(1).take(n - 3).collect();
        }

        // (2) If the name contains our quote character, remove it (IdentifierType.java:33-35).
        name = name.replace(&self.string_quote, "");

        // (3) If the name contains a reserved character, it must be quoted
        // (IdentifierType.java:38-44).
        let mut need_quotes = self
            .reserved_chars
            .iter()
            .any(|reserved| name.contains(reserved.as_str()));

        // (4) If the name contains a non-ASCII character — any UTF-8 byte `<= 0` as a *signed*
        // byte, i.e. any byte `>= 0x80` or a NUL — it must be quoted (IdentifierType.java:47-52).
        if !need_quotes {
            need_quotes = name.as_bytes().iter().any(|&b| (b as i8) <= 0);
        }

        // (5) Otherwise, if the name looks like a number (`^-?\d.*`, hand-rolled — no `regex`
        // crate) it must be quoted (IdentifierType.java:55-58).
        if !need_quotes && starts_like_a_number(&name) {
            need_quotes = true;
        }

        // (6) Quote by wrapping in `string_quote` (IdentifierType.java:60-61).
        if need_quotes {
            name = format!("{}{name}{}", self.string_quote, self.string_quote);
        }

        out.write(&name);
    }

    /// Looks, if string does not contain reserved characters or blanks (`isLegal`,
    /// IdentifierType.java:70-81). Private in Java, and Java never calls it — kept for fidelity
    /// with the Java class, which has no caller for it either; this crate's unit test below is
    /// its only caller.
    #[allow(dead_code)]
    pub(crate) fn is_legal(&self, string: &str) -> bool {
        self.reserved_chars
            .iter()
            .all(|reserved| !string.contains(reserved.as_str()))
    }
}

/// `^-?\d.*` without the `regex` crate: an optional leading `-`, then an ASCII digit (the rest
/// of the string is unconstrained, matching Java's `.*`).
fn starts_like_a_number(s: &str) -> bool {
    let mut chars = s.chars();
    let mut c = chars.next();
    if c == Some('-') {
        c = chars.next();
    }
    matches!(c, Some(d) if d.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dsn_id() -> IdentifierType {
        IdentifierType::new(
            DSN_RESERVED.iter().map(|s| s.to_string()).collect(),
            "\"".to_string(),
        )
    }

    /// `isLegal` is private in Java and has no caller there; ported for fidelity only. This is
    /// its only caller in the port too.
    #[test]
    fn is_legal_rejects_reserved_characters() {
        let id = dsn_id();
        assert!(id.is_legal("F.Cu"));
        assert!(!id.is_legal("D-"));
        assert!(!id.is_legal("a b"));
    }
}
