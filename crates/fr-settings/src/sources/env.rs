//! `settings/sources/EnvironmentVariablesSource.java` (133): router settings read out of the
//! environment, priority 55.

use std::collections::BTreeMap;

use crate::{
    MergeError, RouterSettings, SettingsSource, SourceKind, merger::priority, set_field_value,
};

/// `settings/sources/EnvironmentVariablesSource.java`: every `FREEROUTING__ROUTER__*` variable,
/// turned into a settings field through [`set_field_value`].
///
/// # The key rules, in Java's own order (`:51-90`)
///
/// 1. **The key is upper-cased first** (`:56`), so variable *names* are case-insensitive:
///    `freerouting__router__max_passes` and `FreeRouting__Router__Max_Passes` both work
///    (`EnvironmentVariablesSourceTest.java:244-257`; JVM-verified, `CProbe A2`/`A2b`).
/// 2. Anything not starting with `FREEROUTING__ROUTER__` is skipped (`:59-61`).
/// 3. The prefix is stripped and `__` becomes `.` (`:64-65`), giving `set_field_value`'s property
///    path — `FREEROUTING__ROUTER__OPTIMIZER__MAX_THREADS` → `OPTIMIZER.MAX_THREADS`. Note the
///    path stays *upper-cased*: field-name resolution is case-insensitive
///    (`ReflectionUtil.getFieldByNameOrSerializedName`), which is why that works at all.
/// 4. Every failure — an unknown field (`:74-80`) or a value that will not convert (`:81-89`) —
///    is warned about and **skipped**. Nothing here is ever fatal, and the *rest* of the
///    environment still lands.
/// 5. Only a variable that actually landed is recorded in [`Self::get_parsed_variables`], and it
///    is keyed by the **raw** key, not the upper-cased one (`:70`).
///
/// The value itself is never touched — `HYBRID_RATIO=1:1` stores the string `"1:1"`, because
/// `setFieldValue` splits only the *path* on `[.:-]` (quirks row 118).
///
/// # One deliberate deviation
///
/// renamed: EnvironmentVariablesSource(Map) -> EnvironmentVariablesSource::new(&BTreeMap) —
/// Java iterates a `HashMap` in unspecified order; this port takes a `BTreeMap` so the parse
/// order is deterministic. It is observable only if two distinct raw keys upper-case to the same
/// property path (`max_passes` and `MAX_PASSES` in one environment), where Java's winner depends
/// on hash order and this port's is the lexicographically last key. No test in
/// `EnvironmentVariablesSourceTest` constructs such an environment, and a real environment cannot
/// (POSIX environments are case-sensitive but shells do not usually export both spellings), so
/// the divergence is unreachable rather than merely untested.
///
/// [`Self::errors`] is this port's stand-in for the two `FRLogger.warn` calls (plan Global
/// Constraints — no `FRLogger`, no `tracing`); nothing in the merge path reads it.
#[derive(Debug, Clone)]
pub struct EnvironmentVariablesSource {
    settings: RouterSettings,
    parsed_variables: BTreeMap<String, String>,
    errors: Vec<MergeError>,
}

/// `EnvironmentVariablesSource.ENV_PREFIX + ROUTER_PREFIX` (`:27-28`).
const ROUTER_ENV_PREFIX: &str = "FREEROUTING__ROUTER__";

impl EnvironmentVariablesSource {
    /// `EnvironmentVariablesSource.PRIORITY` (`:26`).
    const PRIORITY: i32 = priority::ENVIRONMENT;

    /// `EnvironmentVariablesSource(Map<String, String>)` (`:42-45`) plus
    /// `parseEnvironmentVariables` (`:47-99`) — the constructor does all the work, so
    /// [`SettingsSource::get_settings`] is a plain accessor exactly as in Java (`:101-104`).
    #[must_use]
    pub fn new(environment: &BTreeMap<String, String>) -> Self {
        let mut settings = RouterSettings::new();
        let mut parsed_variables = BTreeMap::new();
        let mut errors = Vec::new();

        for (raw_key, value) in environment {
            // :56 — upper-case first, which is what makes the names case-insensitive.
            let uppercase_key = raw_key.to_uppercase();
            // :59-61
            let Some(suffix) = uppercase_key.strip_prefix(ROUTER_ENV_PREFIX) else {
                continue;
            };
            // :64-65
            let property_path = suffix.replace("__", ".");

            // :68-89 — `NoSuchFieldException` and every other exception are warned about and
            // skipped; `parsedVariables.put` (:70) only runs on success.
            match set_field_value(&mut settings, &property_path, value) {
                Ok(()) => {
                    parsed_variables.insert(raw_key.clone(), value.clone());
                }
                Err(error) => errors.push(error),
            }
        }

        Self {
            settings,
            parsed_variables,
            errors,
        }
    }

    /// `EnvironmentVariablesSource()` (`:33-35`): the `System.getenv()` overload.
    ///
    /// The only place in this crate that reads the process environment. Non-UTF-8 keys and values
    /// are skipped rather than lossily converted — Java's `System.getenv()` gives `String`s the
    /// platform decoder produced, and a variable this port cannot decode could not have named a
    /// settings field anyway.
    #[must_use]
    pub fn from_process_env() -> Self {
        let environment: BTreeMap<String, String> = std::env::vars_os()
            .filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?)))
            .collect();
        Self::new(&environment)
    }

    /// `EnvironmentVariablesSource.getParsedVariables` (`:121-123`): every variable that actually
    /// landed, keyed by its raw (un-upper-cased) name.
    ///
    /// renamed: getParsedVariables — Java returns `new HashMap<>(parsedVariables)`, a defensive
    /// copy; a shared borrow gives the same protection in Rust without the allocation.
    #[must_use]
    pub fn get_parsed_variables(&self) -> &BTreeMap<String, String> {
        &self.parsed_variables
    }

    /// `EnvironmentVariablesSource.getParsedCount` (`:130-132`) — the *map's* size, not the local
    /// `parsedCount` the log line at `:92-93` uses. The two always agree, because the input map's
    /// keys are unique and `parsedVariables` is keyed by that same raw key (`:70`).
    #[must_use]
    pub fn get_parsed_count(&self) -> usize {
        self.parsed_variables.len()
    }

    /// Every variable that did **not** land, in parse order — the port's replacement for
    /// `FRLogger.warn` at `:75-88`. Added by this port; Java has no accessor for it.
    // added in Plan 4: (no Java counterpart — the failures only reach FRLogger)
    #[must_use]
    pub fn errors(&self) -> &[MergeError] {
        &self.errors
    }
}

impl SettingsSource for EnvironmentVariablesSource {
    fn get_settings(&self) -> Option<&RouterSettings> {
        Some(&self.settings)
    }

    fn get_source_name(&self) -> String {
        "Environment Variables".to_string()
    }

    fn get_priority(&self) -> i32 {
        Self::PRIORITY
    }

    fn kind(&self) -> SourceKind {
        SourceKind::Environment
    }
}
