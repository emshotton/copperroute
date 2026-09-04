use crate::keyword::Keyword;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
        Open,
        Close,
        Kw(Keyword),
        Str(String),
        Int(i64),
        Float(f64),
}
