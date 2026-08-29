//! Error types for `fr-settings`: the crate-level error and the merge engine's per-field error
//! (`util/ReflectionUtil.java`'s swallowed exceptions, per plan ruling 4).

/// Errors this crate's I/O-facing operations can produce.
///
/// The merge engine itself (Task 2) does not return this type for a single field mismatch — see
/// [`MergeError`] — this is for operations that can fail outright (reading a source file,
/// (de)serialising JSON).
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    /// A DSN/`.rules`/SES read failed (`fr_dsn::dsn_reader::read_metadata`,
    /// `fr_dsn::rules_reader::read_router_settings`).
    #[error(transparent)]
    Dsn(#[from] fr_dsn::DsnError),

    /// An I/O error opening or reading a settings source file.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// A `freerouting.json`-shaped file did not parse.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// A single field the merge engine (Task 2's `copy_fields`) could not copy.
///
/// `ReflectionUtil.copyFields` (ReflectionUtil.java:215-344) swallows every exception it hits per
/// field (`:338-340`, `catch (Exception e) { FRLogger.error(...); }`) and moves on to the next
/// field rather than aborting the whole merge. This crate has no `FRLogger` (Global
/// Constraints — no `tracing`), so each swallowed exception becomes a `MergeError` pushed onto
/// [`MergeReport::errors`] instead of a log line.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MergeError {
    /// The merge engine tried to copy into a field name absent from the target struct's field
    /// table (would be a Java `NoSuchFieldException`).
    #[error("no such field: {path}")]
    NoSuchField {
        /// Dotted path to the field, e.g. `"scoring.via_costs"`.
        path: String,
    },

    /// A numeric-looking source value did not parse as the target field's number type (would be
    /// a Java `NumberFormatException` from `ReflectionUtil.setFieldValue`).
    #[error("field {path}: {value:?} is not a valid number")]
    NumberFormat {
        /// Dotted path to the field.
        path: String,
        /// The value that failed to parse.
        value: String,
    },

    /// A string-looking source value did not match any of the target enum field's constant names
    /// (would be a Java `IllegalArgumentException` from `Enum.valueOf`).
    #[error("field {path}: {value:?} is not a valid enum constant")]
    EnumName {
        /// Dotted path to the field.
        path: String,
        /// The value that failed to match a constant.
        value: String,
    },

    /// A property path asked for something the field's type cannot do: assigning a scalar string
    /// to a struct- or array-typed field (Java: `convertValue` returns the raw `String` and
    /// `field.set` throws `IllegalArgumentException`, `ReflectionUtil.java:203-204` + `:41`), or
    /// navigating *through* a scalar field (Java: `field.getType().getDeclaredConstructor()`
    /// throws `NoSuchMethodException`, `:77`). Added by Task 3 alongside
    /// [`crate::field_path::set_field_value`]; `copy_fields` never produces it, because its
    /// source and target fields always have the same Rust type.
    #[error("field {path}: {value:?} cannot be assigned to a field of this type")]
    TypeMismatch {
        /// Dotted path to the field.
        path: String,
        /// The value that could not be assigned.
        value: String,
    },
}

/// Outcome of one `copy_fields` pass: how many fields were actually written, and every per-field
/// error encountered along the way (never aborts the merge — see [`MergeError`]'s doc comment).
///
/// `fields_changed` counts one per field the port actually writes (plan ruling 2): it does not
/// reproduce Java's boxed-`Integer`-identity `!=` comparison quirk, because no caller of
/// `SettingsMerger.merge` (`SettingsMerger.java:171`) or `RulesReader.read`
/// (`RulesReader.java:156`) reads the count for anything besides a log line this port drops.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergeReport {
    /// Number of fields this pass actually wrote a new value into.
    pub fields_changed: usize,
    /// Every field this pass could not copy, in the order encountered.
    pub errors: Vec<MergeError>,
}
