use crate::format::indent_writer::IndentFileWriter;
use std::io::Write;

pub const DSN_RESERVED: [&str; 6] = ["(", ")", " ", ";", "-", "_"];

pub const SES_RESERVED: [&str; 10] = ["(", ")", " ", ";", "-", "_", "/", "~", "{", "}"];

pub struct IdentifierType {
    string_quote: String,
    reserved_chars: Vec<String>,
}

impl IdentifierType {
        pub fn new(reserved_chars: Vec<String>, string_quote: String) -> Self {
        Self {
            string_quote,
            reserved_chars,
        }
    }

            pub fn write<W: Write>(&self, name: &str, out: &mut IndentFileWriter<W>) {
        let mut name = name.to_string();

        while name.chars().count() > 2 && name.starts_with('"') && name.ends_with('"') {
            let n = name.chars().count();
            name = name.chars().skip(1).take(n - 3).collect();
        }

        name = name.replace(&self.string_quote, "");

        let mut need_quotes = self
            .reserved_chars
            .iter()
            .any(|reserved| name.contains(reserved.as_str()));

        if !need_quotes {
            need_quotes = name.as_bytes().iter().any(|&b| (b as i8) <= 0);
        }

        if !need_quotes && starts_like_a_number(&name) {
            need_quotes = true;
        }

        if need_quotes {
            name = format!("{}{name}{}", self.string_quote, self.string_quote);
        }

        out.write(&name);
    }

                    #[allow(dead_code)]
    pub(crate) fn is_legal(&self, string: &str) -> bool {
        self.reserved_chars
            .iter()
            .all(|reserved| !string.contains(reserved.as_str()))
    }
}

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

            #[test]
    fn is_legal_rejects_reserved_characters() {
        let id = dsn_id();
        assert!(id.is_legal("F.Cu"));
        assert!(!id.is_legal("D-"));
        assert!(!id.is_legal("a b"));
    }
}
