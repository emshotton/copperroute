//! `serde_json`'s [`PrettyFormatter`] with Java's number formatting and Gson's two extra string
//! escapes — the write half of `util/gson/GsonProvider.java:12-20`'s `GsonProvider.GSON`
//! (`new GsonBuilder().setPrettyPrinting().disableHtmlEscaping()…`, with **no**
//! `serializeNulls()` and **no** `serializeSpecialFloatingPointValues()`).
//!
//! Moved here from `fr-settings` in Plan 5 (`docs/superpowers/plans/2026-08-29-plan-5-drc.md`
//! ruling 7): `fr-drc`'s KiCad DRC report needs the identical formatter and must not depend on
//! `fr-settings`. `fr-settings` keeps its two `RouterSettings` JSON entry points
//! (`RouterSettings::to_json_string_pretty` / `from_json_str`), calling through to
//! [`to_gson_string_pretty`] for the write side; see `crates/fr-settings/src/json.rs`'s module
//! docs for what stays specific to that type (null-field omission via `#[serde(
//! skip_serializing_if)]`, and `getDeclaredFields()` key order).
//!
//! Four things this reproduces, all JVM-verified by `crates/fr-settings/tests/data/JProbe.java`
//! (transcript in `.superpowers/sdd/2026-08-28-plan-4-settings/task-10-report.md`) against
//! `RouterSettings` — the behaviour itself belongs to `GsonProvider.GSON`, not to that type:
//!
//! 1. **Two-space indent, `": "` after every key, no trailing newline.** [`PrettyFormatter`] is
//!    byte-identical to Gson's `JsonWriter` here, so this module wraps it rather than
//!    reimplementing it.
//! 2. **Numbers are written with `Number.toString()`.** The delegate adapter builds a
//!    `JsonElement` tree of `JsonPrimitive`s and `JsonWriter.value(Number)` appends
//!    `value.toString()` — so a `Double` field prints as `Double.toString` and a `Float` field as
//!    `Float.toString`. Both use scientific notation outside `[1e-3, 1e7)`, which Rust's
//!    shortest-round-trip formatter does not, so [`JavaNumberFormatter`] routes every float
//!    through `java_double_to_string`/`java_float_to_string`.
//! 3. **A non-finite float is refused — in both directions.** Without
//!    `serializeSpecialFloatingPointValues()`, `Gson.toJson` throws `IllegalArgumentException`
//!    ("… is not a valid double value as per JSON specification"), and *reading* one back throws
//!    too. `serde_json` already rejects `NaN`/`Infinity` on the read side, so the two agree in
//!    both directions; [`to_gson_string_pretty`] returns an error on the write side to match.
//! 4. **`U+2028` and `U+2029` are escaped, `<`, `>`, `&`, `'` are not.** `disableHtmlEscaping()`
//!    turns off the HTML set only; Gson's `JsonWriter` escapes the two line separators
//!    unconditionally, because they are legal in a JSON string but illegal in a JavaScript one.
//!    JVM-verified (`JProbe.java` block J): `"a\u2028b\u2029c"` is written as `a\u2028b\u2029c`
//!    while `<&>'` are written raw. `serde_json` escapes neither, so
//!    [`JavaNumberFormatter::write_string_fragment`] adds the two.

use std::io;

use serde::Serialize;
use serde_json::ser::{Formatter, PrettyFormatter};

use crate::format::double::{java_double_to_string, java_float_to_string};

/// The message `Gson` puts on the `IllegalArgumentException` it throws for a non-finite float,
/// abbreviated to the part that is not value-dependent.
const NON_FINITE: &str = "not a valid double value as per JSON specification (Gson refuses it: GsonProvider never \
     calls serializeSpecialFloatingPointValues)";

/// [`PrettyFormatter`] with Java's number formatting.
///
/// Only the nine methods `PrettyFormatter` itself overrides are forwarded; every other method of
/// [`Formatter`] keeps its default body, which is what `PrettyFormatter` uses too. The three
/// methods below are the whole of the divergence from `serde_json`'s output.
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
    /// Unreachable for an actual `null` from any of `fr-settings`' five settings structs: every
    /// `Option` field carries `skip_serializing_if = "Option::is_none"`, and no collection those
    /// types serialise has a nullable element. `serde_json`'s `serialize_f32`/`serialize_f64`
    /// route NaN and ±Infinity here instead of to [`Self::write_f64`], so this is exactly Gson's
    /// refusal point.
    #[inline]
    fn write_null<W>(&mut self, _writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        Err(io::Error::new(io::ErrorKind::InvalidData, NON_FINITE))
    }

    /// `Float.toString` — `JsonWriter.value(Number)` on a `JsonPrimitive` holding a `Float`.
    #[inline]
    fn write_f32<W>(&mut self, writer: &mut W, value: f32) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        writer.write_all(java_float_to_string(value).as_bytes())
    }

    /// `Double.toString` — the same, for a `Double`.
    #[inline]
    fn write_f64<W>(&mut self, writer: &mut W, value: f64) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        writer.write_all(java_double_to_string(value).as_bytes())
    }

    /// Gson's `JsonWriter` escapes `U+2028`/`U+2029` unconditionally — `disableHtmlEscaping()`
    /// only turns off the `<`, `>`, `&`, `'`, `=` set (`JProbe.java` block J). `serde_json`
    /// escapes neither, and it never splits a fragment inside a character, so a plain scan of the
    /// fragment is enough.
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

    // --- the nine `PrettyFormatter` overrides, forwarded verbatim ---------------------------

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

/// `GsonProvider.GSON.toJson(value)` for any [`Serialize`]: pretty, two-space, Java number text,
/// non-finite floats refused exactly where Gson throws.
///
/// # Errors
///
/// Any `serde_json::Error` `value`'s `Serialize` impl produces, plus [`JavaNumberFormatter`]'s
/// own refusal (module docs, point 3) surfaced as an [`io::Error`] wrapped by `serde_json` into
/// its `Io` variant.
pub fn to_gson_string_pretty<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let mut buffer = Vec::new();
    let mut serializer =
        serde_json::Serializer::with_formatter(&mut buffer, JavaNumberFormatter::new());
    value.serialize(&mut serializer)?;
    Ok(String::from_utf8(buffer).expect("serde_json writes UTF-8"))
}
