use std::fmt;

const MAX_DEPTH: usize = 100;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Atom(String),
    Node(Node),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub values: Vec<Value>,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SexprError(&'static str);

impl fmt::Display for SexprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for SexprError {}

impl Node {
    #[must_use]
    pub fn name(&self) -> &str {
        self.atom(0).unwrap_or_default()
    }

    #[must_use]
    pub fn atom(&self, index: usize) -> Option<&str> {
        match self.values.get(index) {
            Some(Value::Atom(text)) => Some(text.as_str()),
            _ => None,
        }
    }

    pub fn children<'a>(&'a self, key: &str) -> impl Iterator<Item = &'a Node> {
        self.values.iter().filter_map(move |value| match value {
            Value::Node(node) if node.name() == key => Some(node),
            _ => None,
        })
    }

    #[must_use]
    pub fn child(&self, key: &str) -> Option<&Node> {
        self.children(key).next()
    }

    #[must_use]
    pub fn value(&self, key: &str) -> Option<&str> {
        self.child(key)?.atom(1)
    }

    #[must_use]
    pub fn number(&self, key: &str) -> Option<f64> {
        self.value(key)?.parse().ok()
    }

    #[must_use]
    pub fn point(&self, key: &str) -> Option<(f64, f64)> {
        let node = self.child(key)?;
        Some((node.atom(1)?.parse().ok()?, node.atom(2)?.parse().ok()?))
    }
}

pub fn parse(text: &str) -> Result<Node, SexprError> {
    let bytes = text.as_bytes();
    let mut cursor = 0usize;
    let node = read_node(text, bytes, &mut cursor, 0)?;
    skip_space(bytes, &mut cursor);
    if cursor != bytes.len() {
        return Err(SexprError("Expected one kicad_pcb board."));
    }
    Ok(node)
}

fn skip_space(bytes: &[u8], cursor: &mut usize) {
    while *cursor < bytes.len() && bytes[*cursor].is_ascii_whitespace() {
        *cursor += 1;
    }
}

fn read_node(
    text: &str,
    bytes: &[u8],
    cursor: &mut usize,
    depth: usize,
) -> Result<Node, SexprError> {
    if depth > MAX_DEPTH {
        return Err(SexprError("File nesting is too deep."));
    }
    skip_space(bytes, cursor);
    let start = *cursor;
    if bytes.get(start) != Some(&b'(') {
        return Err(SexprError("Expected a KiCad S-expression."));
    }
    *cursor += 1;
    let mut values = Vec::new();
    loop {
        skip_space(bytes, cursor);
        match bytes.get(*cursor) {
            None => return Err(SexprError("Unclosed expression.")),
            Some(b')') => {
                *cursor += 1;
                return Ok(Node {
                    values,
                    start,
                    end: *cursor,
                });
            }
            Some(b'(') => values.push(Value::Node(read_node(text, bytes, cursor, depth + 1)?)),
            Some(b'"') => values.push(Value::Atom(read_quoted(text, bytes, cursor)?)),
            Some(_) => values.push(Value::Atom(read_bare(text, bytes, cursor))),
        }
    }
}

fn read_quoted(text: &str, bytes: &[u8], cursor: &mut usize) -> Result<String, SexprError> {
    *cursor += 1;
    let mut out = String::new();
    let mut closed = false;
    while *cursor < bytes.len() {
        let start = *cursor;
        match bytes[start] {
            b'"' => {
                *cursor += 1;
                closed = true;
                break;
            }
            b'\\' => {
                *cursor += 1;
                if *cursor < bytes.len() {
                    let escaped = next_char(text, *cursor);
                    out.push_str(escaped);
                    *cursor += escaped.len();
                }
            }
            _ => {
                let ch = next_char(text, start);
                out.push_str(ch);
                *cursor += ch.len();
            }
        }
    }
    if !closed {
        return Err(SexprError("Unclosed string."));
    }
    Ok(out)
}

fn read_bare(text: &str, bytes: &[u8], cursor: &mut usize) -> String {
    let start = *cursor;
    while *cursor < bytes.len() {
        let byte = bytes[*cursor];
        if byte.is_ascii_whitespace() || byte == b'(' || byte == b')' {
            break;
        }
        *cursor += 1;
    }
    text[start..*cursor].to_string()
}

fn next_char(text: &str, index: usize) -> &str {
    let rest = &text[index..];
    let width = rest.chars().next().map_or(0, char::len_utf8);
    &rest[..width]
}
