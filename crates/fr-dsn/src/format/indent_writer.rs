//! `datastructures/IndentFileWriter.java`: an indentation-tracking wrapper around a byte sink,
//! used while writing Specctra DSN/SES text.

use std::io::{self, Write};

const INDENT_STRING: &str = "  ";
const BEGIN_SCOPE: &str = "(";
const END_SCOPE: &str = ")";

/// Indentation-tracking writer for Specctra scopes (`IndentFileWriter.java`).
///
/// Java extends `OutputStreamWriter` and swallows every `IOException` into an
/// `FRLogger.error`/`FRLogger.warn` call — `start_scope`, `end_scope` and `new_line` each wrap
/// their `write` in their own try/catch. The port keeps writing best-effort the same way, but
/// records the *first* I/O error into `first_error` rather than dropping it —
/// `// totalized:` in reverse: Java totalizes a write failure into "nothing happened"; this
/// port un-totalizes it just enough that [`IndentFileWriter::flush`] can surface a full disk
/// instead of silently truncating output.
///
/// Newline is always `"\n"` (IndentFileWriter.java:57 — never `\r\n`), output is UTF-8 with no
/// BOM (`IndentFileWriter(OutputStream)` constructs its `OutputStreamWriter` superclass with
/// `StandardCharsets.UTF_8`, IndentFileWriter.java:19).
pub struct IndentFileWriter<W: Write> {
    out: W,
    indent_level: i32,
    first_error: Option<io::Error>,
}

impl<W: Write> IndentFileWriter<W> {
    /// `IndentFileWriter(OutputStream)` (IndentFileWriter.java:18-20).
    pub fn new(out: W) -> Self {
        Self {
            out,
            indent_level: 0,
            first_error: None,
        }
    }

    /// `startScope(boolean)` (IndentFileWriter.java:23-31).
    pub fn start_scope(&mut self, new_line: bool) {
        if new_line {
            self.new_line();
        }
        self.write_raw(BEGIN_SCOPE);
        self.indent_level += 1;
    }

    /// `startScope()` (IndentFileWriter.java:36-38), Java's no-arg overload — begins a new scope
    /// on a new line.
    // renamed: startScope()
    pub fn start_scope_nl(&mut self) {
        self.start_scope(true);
    }

    /// `endScope()` (IndentFileWriter.java:41-49). Decrements the indent level *before* emitting
    /// the newline + indent + closing paren.
    pub fn end_scope(&mut self) {
        self.indent_level -= 1;
        self.new_line();
        self.write_raw(END_SCOPE);
    }

    /// `newLine()` (IndentFileWriter.java:52-61). Java's `for (i = 0; i < currentIndentLevel;
    /// i++)` never runs when the level is negative (an `end_scope` with no matching
    /// `start_scope`), so a negative `indent_level` here just emits a bare newline rather than
    /// panicking.
    pub fn new_line(&mut self) {
        self.write_raw("\n");
        for _ in 0..self.indent_level.max(0) {
            self.write_raw(INDENT_STRING);
        }
    }

    /// Plain, non-indenting write — Java's inherited `OutputStreamWriter.write(String)`, which
    /// `IndentFileWriter` does not override.
    pub fn write(&mut self, s: &str) {
        self.write_raw(s);
    }

    /// Flushes the underlying sink and surfaces the first I/O error recorded by any prior
    /// `write`/`start_scope`/`end_scope`/`new_line` call, or a fresh flush error if none was
    /// recorded — see the type-level doc for why this differs from Java, which has no `flush`
    /// override and swallows every write failure.
    pub fn flush(&mut self) -> io::Result<()> {
        let flush_result = self.out.flush();
        match self.first_error.take() {
            Some(e) => Err(e),
            None => flush_result,
        }
    }

    /// Test/introspection helper with no Java counterpart: recovers the wrapped sink.
    pub fn into_inner(self) -> W {
        self.out
    }

    fn write_raw(&mut self, s: &str) {
        if let Err(e) = self.out.write_all(s.as_bytes())
            && self.first_error.is_none()
        {
            self.first_error = Some(e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(w: IndentFileWriter<Vec<u8>>) -> String {
        String::from_utf8(w.into_inner()).expect("output must be valid UTF-8")
    }

    /// `start_scope(false)` then `write("pcb x")` then `end_scope()` — JVM-verified against
    /// `tools/freerouting-2.3.0.jar`.
    #[test]
    fn start_scope_false_write_end_scope() {
        let mut w = IndentFileWriter::new(Vec::new());
        w.start_scope(false);
        w.write("pcb x");
        w.end_scope();
        w.flush().unwrap();
        assert_eq!(output(w), "(pcb x\n)");
    }

    /// Two nested `start_scope_nl()` (Java's no-arg `startScope()`) followed by two
    /// `end_scope()`. JVM-verified against `tools/freerouting-2.3.0.jar`: the brief's expected
    /// string `(\n  (\n  )\n)` omits the leading `\n` that Java's no-arg overload
    /// unconditionally emits (`startScope()` always calls `newLine()` first, even at indent
    /// level 0) — corrected here per "Java wins over plan text".
    #[test]
    fn two_nested_start_scope_nl() {
        let mut w = IndentFileWriter::new(Vec::new());
        w.start_scope_nl();
        w.start_scope_nl();
        w.end_scope();
        w.end_scope();
        w.flush().unwrap();
        assert_eq!(output(w), "\n(\n  (\n  )\n)");
    }

    /// `new_line()` at indent level 3 emits exactly `"\n      "` (newline + three
    /// `INDENT_STRING`s). JVM-verified against `tools/freerouting-2.3.0.jar`.
    #[test]
    fn new_line_at_level_three() {
        let mut w = IndentFileWriter::new(Vec::new());
        w.start_scope_nl();
        w.start_scope_nl();
        w.start_scope_nl();
        let before = w.out.len();
        w.new_line();
        w.flush().unwrap();
        let out = output(w);
        assert_eq!(&out[before..], "\n      ");
    }

    #[test]
    fn negative_indent_level_does_not_panic() {
        let mut w = IndentFileWriter::new(Vec::new());
        w.end_scope();
        w.flush().unwrap();
        assert_eq!(output(w), "\n)");
    }

    #[test]
    fn flush_surfaces_first_recorded_write_error() {
        struct AlwaysFails;
        impl Write for AlwaysFails {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("disk full"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let mut w = IndentFileWriter::new(AlwaysFails);
        w.write("first");
        w.write("second");
        let err = w
            .flush()
            .expect_err("first recorded write error must surface");
        assert_eq!(err.to_string(), "disk full");
    }
}
