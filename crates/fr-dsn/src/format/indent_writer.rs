use std::io::{self, Write};

const INDENT_STRING: &str = "  ";
const BEGIN_SCOPE: &str = "(";
const END_SCOPE: &str = ")";

pub struct IndentFileWriter<W: Write> {
    out: W,
    indent_level: i32,
    first_error: Option<io::Error>,
}

impl<W: Write> IndentFileWriter<W> {
        pub fn new(out: W) -> Self {
        Self {
            out,
            indent_level: 0,
            first_error: None,
        }
    }

        pub fn start_scope(&mut self, new_line: bool) {
        if new_line {
            self.new_line();
        }
        self.write_raw(BEGIN_SCOPE);
        self.indent_level += 1;
    }

            pub fn start_scope_nl(&mut self) {
        self.start_scope(true);
    }

            pub fn end_scope(&mut self) {
        self.indent_level -= 1;
        self.new_line();
        self.write_raw(END_SCOPE);
    }

                    pub fn new_line(&mut self) {
        self.write_raw("\n");
        for _ in 0..self.indent_level.max(0) {
            self.write_raw(INDENT_STRING);
        }
    }

            pub fn write(&mut self, s: &str) {
        self.write_raw(s);
    }

                    pub fn flush(&mut self) -> io::Result<()> {
        let flush_result = self.out.flush();
        match self.first_error.take() {
            Some(e) => Err(e),
            None => flush_result,
        }
    }

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

            #[test]
    fn start_scope_false_write_end_scope() {
        let mut w = IndentFileWriter::new(Vec::new());
        w.start_scope(false);
        w.write("pcb x");
        w.end_scope();
        w.flush().unwrap();
        assert_eq!(output(w), "(pcb x\n)");
    }

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
