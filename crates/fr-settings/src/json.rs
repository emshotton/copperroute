//! Gson-compatible JSON in and out for [`RouterSettings`] — `util/gson/GsonProvider.java` and
//! `util/gson/RouterSettingsTypeAdapterFactory.java` (freerouting, clone HEAD).
//!
//! `GsonProvider.GSON` (`GsonProvider.java:12-20`) is built with `setPrettyPrinting()`,
//! `disableHtmlEscaping()`, `Strictness.LENIENT` and the `RouterSettingsTypeAdapterFactory`, and
//! with **no** `serializeNulls()` and **no** `serializeSpecialFloatingPointValues()`. That fixes
//! five things this module has to reproduce, all of them JVM-verified by
//! `crates/fr-settings/tests/data/JProbe.java` (transcript in the Task 10 report):
//!
//! 1. **Two-space indent, `": "` after every key, no trailing newline.** `serde_json`'s
//!    [`PrettyFormatter`] is byte-identical to Gson's `JsonWriter` here, so this module wraps it
//!    rather than reimplementing it.
//! 2. **`null` fields are omitted.** Gson's default is `serializeNulls = false`; the port's
//!    equivalent is `#[serde(skip_serializing_if = "Option::is_none")]` on every field of the five
//!    structs. That attribute is load-bearing twice over — see [`JavaNumberFormatter::write_null`].
//! 3. **Key order is `getDeclaredFields()` order**, which is the Rust field order (Task 1 pinned
//!    the two together in `RouterSettings::FIELD_NAMES`).
//! 4. **Numbers are written with `Number.toString()`.** The delegate adapter builds a `JsonElement`
//!    tree of `JsonPrimitive`s and `JsonWriter.value(Number)` appends `value.toString()` — so a
//!    `Double` field prints as `Double.toString` and a `Float` field as `Float.toString`. Both use
//!    scientific notation outside `[1e-3, 1e7)`, which Rust's shortest-round-trip formatter does
//!    not, so [`JavaNumberFormatter`] routes every float through Plan 3's
//!    `java_double_to_string`/`java_float_to_string`.
//! 5. **A non-finite float is refused — in both directions.** Without
//!    `serializeSpecialFloatingPointValues()`, `Gson.toJson` throws `IllegalArgumentException`
//!    ("… is not a valid double value as per JSON specification"), and *reading* one back throws
//!    too: `JsonIOException: MalformedJsonException: JSON forbids NaN and infinities`. The reader
//!    rejects it despite `Strictness.LENIENT` because the factory reads the document in two
//!    passes — `elementAdapter.read(in)` builds a tree with the lenient textual reader
//!    (`RouterSettingsTypeAdapterFactory.java:50`), then `delegate.fromJsonTree(tree)` (`:56`)
//!    re-reads that tree through a **fresh `JsonTreeReader` at default strictness**, which is
//!    where `nextDouble` refuses it. `Strictness.LENIENT` therefore governs only the first,
//!    textual pass. JVM-verified in `JProbe.java` block H, including the quoted spelling
//!    `{"hole_clearance_um": "NaN"}`, which is coerced and then refused the same way.
//!    [`RouterSettings::to_json_string_pretty`] returns an error to match, and `serde_json`
//!    already rejects `NaN`/`Infinity` on the read side, so the two agree in both directions.
//! 6. **`U+2028` and `U+2029` are escaped, `<`, `>`, `&`, `'` are not.** `disableHtmlEscaping()`
//!    turns off the HTML set only; Gson's `JsonWriter` escapes the two line separators
//!    unconditionally, because they are legal in a JSON string but illegal in a JavaScript one.
//!    JVM-verified (`JProbe.java` block J): `"a\u2028b\u2029c"` is written as `a\u2028b\u2029c`
//!    while `<&>'` are written raw. `serde_json` escapes neither, so
//!    [`JavaNumberFormatter::write_string_fragment`] adds the two.  Unreachable for the settings
//!    strings in practice — but `result_json` is a user-supplied path, and the escape is ten
//!    lines, so it is implemented rather than documented as a known-wrong output.
//!
//! The read side is `RouterSettingsTypeAdapterFactory.read` (`:53-70`): the delegate reflective
//! adapter drops every `transient` field, and the factory then re-reads **`layers`** — and only
//! `layers` — from the raw tree (`:59-64`). Task 1's `#[serde(skip)]` / `#[serde(skip_serializing)]`
//! split already encodes that. Two further Gson behaviours are reproduced here:
//!
//! - Gson constructs the target with the declared no-arg constructor before writing any field, so
//!   a JSON object that does not name `fanout`/`optimizer`/`scoring` still comes back with the
//!   three empty objects `RouterSettings()` allocates (`RouterSettings.java:119-124`). The three
//!   fields carry `#[serde(default = "…")]` for exactly that, and an explicit `null` still clears
//!   them.
//! - Unknown keys are ignored at every level (no `deny_unknown_fields`).
//!
//! not ported: Strictness.LENIENT (GsonProvider.java:19) — Gson's *reader* leniency, plus the
//! coercions the reflective adapters do on top of it. The JVM probe shows
//! `GsonProvider.GSON.fromJson` accepting unquoted names, single-quoted names, a leading `//`
//! comment, a quoted scalar coerced to the field's type (`{"max_passes": "42"}` → `42`,
//! `{"strict_drc": "true"}` → `true`) (block D), and — block I — a duplicate key (last wins), a
//! fractional literal truncated into an `Integer` field (`1.9` → `1`), and an empty,
//! whitespace-only or literal-`null` document (a `null` `RouterSettings`, where a *quoted*
//! `"null"` is still a `JsonSyntaxException`). `serde_json` rejects every one of them. **The list
//! is illustrative, not exhaustive** — the rule is "Gson's reader is more permissive", and no
//! test enumerates the boundary. Nothing in this port feeds it non-strict JSON: `JsonFileSettings`
//! — Gson's only reader of this type in headless Java — is out of scope (spec §2), and Plan 8's
//! MCP `settings` input arrives as parsed JSON. The divergence is acceptance-only: where Gson
//! reads a value the port reports an error, never a *different* value. Pinned by
//! `tests/json.rs::{the_lenient_reader_shapes_are_not_ported,
//! the_lenient_reader_coercions_are_not_ported}` and recorded as `docs/java-quirks.md` row 141.

use std::io;

use fr_dsn::format::double::{java_double_to_string, java_float_to_string};
use serde::Serialize;
use serde_json::ser::{Formatter, PrettyFormatter};

use crate::{FanoutSettings, OptimizerSettings, RouterSettings, ScoringSettings, SettingsError};

// -------------------------------------------------------------------------------------------
// the three `RouterSettings()` allocations, as serde field defaults
// -------------------------------------------------------------------------------------------

/// `RouterSettings.java:122` — `this.fanout = new FanoutSettings()`, run by Gson's
/// `ObjectConstructor` before any field is read.
pub(crate) fn constructed_fanout() -> Option<FanoutSettings> {
    Some(FanoutSettings::default())
}

/// `RouterSettings.java:120` — `this.optimizer = new OptimizerSettings()`.
pub(crate) fn constructed_optimizer() -> Option<OptimizerSettings> {
    Some(OptimizerSettings::default())
}

/// `RouterSettings.java:121` — `this.scoring = new ScoringSettings()`.
pub(crate) fn constructed_scoring() -> Option<ScoringSettings> {
    Some(ScoringSettings::default())
}

// -------------------------------------------------------------------------------------------
// the formatter
// -------------------------------------------------------------------------------------------

/// The message `Gson` puts on the `IllegalArgumentException` it throws for a non-finite float,
/// abbreviated to the part that is not value-dependent.
const NON_FINITE: &str = "not a valid double value as per JSON specification (Gson refuses it: GsonProvider never \
     calls serializeSpecialFloatingPointValues)";

/// [`PrettyFormatter`] with Java's number formatting.
///
/// Only the nine methods `PrettyFormatter` itself overrides are forwarded; every other method of
/// [`Formatter`] keeps its default body, which is what `PrettyFormatter` uses too. The three
/// methods below are the whole of the divergence from `serde_json`'s output.
struct JavaNumberFormatter<'a> {
    inner: PrettyFormatter<'a>,
}

impl JavaNumberFormatter<'_> {
    fn new() -> Self {
        Self {
            inner: PrettyFormatter::new(),
        }
    }
}

impl Formatter for JavaNumberFormatter<'_> {
    /// Unreachable for an actual `null`: every `Option` field of the five settings structs carries
    /// `skip_serializing_if = "Option::is_none"`, and no collection this type serialises has a
    /// nullable element. `serde_json`'s `serialize_f32`/`serialize_f64` route NaN and ±Infinity
    /// here instead of to [`Self::write_f64`], so this is exactly Gson's refusal point.
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

// -------------------------------------------------------------------------------------------
// the two entry points
// -------------------------------------------------------------------------------------------

impl RouterSettings {
    /// `GsonProvider.GSON.fromJson(s, RouterSettings.class)` — a JSON object read as a *settings
    /// source*: only the keys it names are non-`None` (plus the three objects
    /// `RouterSettings()` allocates, see the module docs).
    ///
    /// This is what `JsonFileSettings.loadSettings` (`JsonFileSettings.java:48-54`) calls on
    /// `root["router"]`, and what Plan 8's MCP `settings` input will call. The reader is strict
    /// JSON, not Gson's lenient dialect — see the module docs' `not ported:` note.
    ///
    /// # Errors
    ///
    /// [`SettingsError::Json`] if `s` is not a JSON object whose known keys have the right types.
    /// Java swallows that into an empty `RouterSettings` at `JsonFileSettings.java:60`, one level
    /// up from here; this function reports it and leaves the decision to the caller.
    pub fn from_json_str(s: &str) -> Result<Self, SettingsError> {
        Ok(serde_json::from_str(s)?)
    }

    /// `GsonProvider.GSON.toJson(this)` — the pretty-printed form, byte for byte.
    ///
    /// # Errors
    ///
    /// [`SettingsError::Json`] if any float field is NaN or ±Infinity, which is where `Gson`
    /// throws `IllegalArgumentException` (module docs, point 5).
    pub fn to_json_string_pretty(&self) -> Result<String, SettingsError> {
        let mut buffer = Vec::new();
        let mut serializer =
            serde_json::Serializer::with_formatter(&mut buffer, JavaNumberFormatter::new());
        self.serialize(&mut serializer)?;
        Ok(String::from_utf8(buffer).expect("serde_json writes UTF-8"))
    }
}
