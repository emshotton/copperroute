//! docs for what stays specific to that type (null-field omission via `#[serde(
use std::io;

use serde::Serialize;
use serde_json::ser::{Formatter, PrettyFormatter};

use crate::format::double::{format_double, format_float};

const NON_FINITE: &str = "not a valid double value as per JSON specification (Gson refuses it: GsonProvider never \
     calls serializeSpecialFloatingPointValues)";

pub struct JavaNumberFormatter<'a> {
    inner: PrettyFormatter<'a>,
}

impl JavaNumberFormatter<'_> {
    pub fn new() -> Self {
        Self {
            inner: PrettyFormatter::new(),
        }
    }
}

impl Default for JavaNumberFormatter<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter for JavaNumberFormatter<'_> {
    #[inline]
    fn write_null<W>(&mut self, _writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        Err(io::Error::new(io::ErrorKind::InvalidData, NON_FINITE))
    }

    #[inline]
    fn write_f32<W>(&mut self, writer: &mut W, value: f32) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        writer.write_all(format_float(value).as_bytes())
    }

    #[inline]
    fn write_f64<W>(&mut self, writer: &mut W, value: f64) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        writer.write_all(format_double(value).as_bytes())
    }

    #[inline]
    fn write_string_fragment<W>(&mut self, writer: &mut W, fragment: &str) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let mut rest = fragment;
        while let Some(index) = rest.find(['\u{2028}', '\u{2029}']) {
            writer.write_all(&rest.as_bytes()[..index])?;
            let separator = rest[index..].chars().next().expect("a char boundary");
            writer.write_all(if separator == '\u{2028}' {
                br"\u2028"
            } else {
                br"\u2029"
            })?;
            rest = &rest[index + separator.len_utf8()..];
        }
        writer.write_all(rest.as_bytes())
    }

    #[inline]
    fn begin_array<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.inner.begin_array(writer)
    }

    #[inline]
    fn end_array<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.inner.end_array(writer)
    }

    #[inline]
    fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.inner.begin_array_value(writer, first)
    }

    #[inline]
    fn end_array_value<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.inner.end_array_value(writer)
    }

    #[inline]
    fn begin_object<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.inner.begin_object(writer)
    }

    #[inline]
    fn end_object<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.inner.end_object(writer)
    }

    #[inline]
    fn begin_object_key<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.inner.begin_object_key(writer, first)
    }

    #[inline]
    fn begin_object_value<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.inner.begin_object_value(writer)
    }

    #[inline]
    fn end_object_value<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.inner.end_object_value(writer)
    }
}

pub fn to_gson_string_pretty<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let mut buffer = Vec::new();
    let mut serializer =
        serde_json::Serializer::with_formatter(&mut buffer, JavaNumberFormatter::new());
    value.serialize(&mut serializer)?;
    Ok(String::from_utf8(buffer).expect("serde_json writes UTF-8"))
}
