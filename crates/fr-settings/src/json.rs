//! Gson-compatible JSON in and out for [`RouterSettings`] — `util/gson/GsonProvider.java` and
//! `util/gson/RouterSettingsTypeAdapterFactory.java` (freerouting, clone HEAD).
//!
//! `GsonProvider.GSON` (`GsonProvider.java:12-20`) is built with `setPrettyPrinting()`,
//! `disableHtmlEscaping()`, `Strictness.LENIENT` and the `RouterSettingsTypeAdapterFactory`, and
//! with **no** `serializeNulls()` and **no** `serializeSpecialFloatingPointValues()`. The write
//! half of that configuration — two-space pretty printing, `Number.toString()` number rendering,
//! the non-finite-float refusal and the `U+2028`/`U+2029` string escapes — is
//! `fr_dsn::format::json::JavaNumberFormatter` and `fr_dsn::format::json::to_gson_string_pretty`,
//! moved there in Plan 5 (`docs/superpowers/plans/2026-08-29-plan-5-drc.md` ruling 7): `fr-drc`'s
//! KiCad DRC report needs the identical formatter and must not depend on this crate.
//! [`RouterSettings::to_json_string_pretty`] is a thin wrapper over `to_gson_string_pretty`; see
//! `fr_dsn::format::json`'s module docs for that half's four JVM-verified points. Two things
//! about *this* type still have to be reproduced here, not there, both JVM-verified by
//! `crates/fr-settings/tests/data/JProbe.java` (transcript in the Task 10 report):
//!
//! 1. **`null` fields are omitted.** Gson's default is `serializeNulls = false`; the port's
//!    equivalent is `#[serde(skip_serializing_if = "Option::is_none")]` on every field of the five
//!    structs. That attribute is load-bearing twice over — see
//!    `fr_dsn::format::json::JavaNumberFormatter::write_null`'s docs.
//! 2. **Key order is `getDeclaredFields()` order**, which is the Rust field order (Task 1 pinned
//!    the two together in `RouterSettings::FIELD_NAMES`).
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
//! test enumerates the boundary. The port's readers are strict: the MCP's `route_board
//! { settings? }` argument arrives as JSON the transport has already parsed (so a `NaN` literal
//! never reaches this reader — the whole line is a `-32700` first), and
//! [`crate::sources::JsonFileSettings`] — Gson's only reader of this type in headless Java —
//! reads a real `freerouting.json` through `from_json_str`. *(This
//! sentence read ~~"Nothing in this port feeds it non-strict JSON: `JsonFileSettings` … is out of
//! scope (spec §2)"~~ until Plan 8 Task 5, when scan ruling R7 ruled that source back in.)* The
//! divergence stays acceptance-only, for a reason that survives the change:
//! `JsonFileSettings.loadSettings` swallows every exception into an **empty** `RouterSettings`
//! (`JsonFileSettings.java:55-62`), so where Gson coerces a value and the port errors, the port
//! contributes nothing at priority 10 — never a *different* value. Pinned by
//! `tests/json.rs::{the_lenient_reader_shapes_are_not_ported,
//! the_lenient_reader_coercions_are_not_ported}` and recorded as `docs/java-quirks.md` row 141.

use fr_dsn::format::json::to_gson_string_pretty;

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
// the two entry points
// -------------------------------------------------------------------------------------------

impl RouterSettings {
    /// `GsonProvider.GSON.fromJson(s, RouterSettings.class)` — a JSON object read as a *settings
    /// source*: only the keys it names are non-`None` (plus the three objects
    /// `RouterSettings()` allocates, see the module docs).
    ///
    /// This is what `JsonFileSettings.loadSettings` (`JsonFileSettings.java:48-54`) calls on
    /// `root["router"]`, and — since Plan 8 Task 12 — what the MCP's `route_board { settings? }`
    /// argument calls, from
    /// `crates/freerouting/src/mcp/tools/route_board.rs::settings_payload`. That call site
    /// re-serialises the already-parsed argument to text and comes back through here rather than
    /// reaching for `serde_json::from_value`, so this stays the **one** reader of this type and
    /// quirk #141's divergence is recorded in one place. The reader is strict JSON, not Gson's
    /// lenient dialect — see the module docs' `not ported:` note.
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
    /// throws `IllegalArgumentException` (`fr_dsn::format::json`'s module docs, point 3).
    pub fn to_json_string_pretty(&self) -> Result<String, SettingsError> {
        Ok(to_gson_string_pretty(self)?)
    }
}
