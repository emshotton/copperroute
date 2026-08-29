//! `settings/SettingsSource.java` (42) and `settings/SettingsMerger.java` (199): the settings
//! source contract, the priority ladder, and the generic merger that walks it.
//!
//! # Why this type exists even though `resolve_headless` is the entry point
//!
//! Plan ruling 9: `SettingsMerger` is a Java class in scope for the audit, `SettingsMergerTest`
//! is one of the ported tests, and Task 8 runs it in Java's two-merge shape over the same matrix
//! as the linear `resolve_headless` pass to prove ruling 1's equivalence a second time. It stays
//! a thin generic merger over `Vec<Box<dyn SettingsSource>>` with no knowledge of any concrete
//! source.

use crate::{HostEnvironment, RouterSettings};

/// The priority ladder (`SettingsSource.java:35-40`). Lower numbers are applied first; higher
/// numbers override what came before.
///
/// Two of these have no source in this crate: `JSON_FILE` (spec §2 — no persistent config file)
/// and `GUI` (no GUI in this port). They are kept so the ladder reads the way Java documents it
/// and so nobody reuses one of the numbers.
pub mod priority {
    /// `sources/DefaultSettings.java:83`.
    pub const DEFAULT: i32 = 0;
    /// `sources/JsonFileSettings.java` — not ported (spec §2), the number is reserved.
    pub const JSON_FILE: i32 = 10;
    /// `sources/DsnFileSettings.java:16`.
    pub const DSN_FILE: i32 = 20;
    /// `sources/SesFileSettings.java:13`.
    pub const SES_FILE: i32 = 30;
    /// `sources/RulesFileSettings.java:18`.
    pub const RULES_FILE: i32 = 40;
    /// `sources/GuiSettingsSource.java` — not ported (no GUI), the number is reserved.
    pub const GUI: i32 = 50;
    /// `sources/EnvironmentVariablesSource.java` (Task 7).
    pub const ENVIRONMENT: i32 = 55;
    /// `sources/CliSettings.java` (Task 7).
    pub const CLI: i32 = 60;
    /// `sources/ApiSettings.java:12`.
    pub const API: i32 = 70;
}

/// Which *kind* of source an implementation is — this port's stand-in for Java's
/// `existingSource.getClass().equals(newSource.getClass())` identity test in
/// [`SettingsMerger::add_or_replace_sources`] (`SettingsMerger.java:116-118`).
///
/// A `Box<dyn SettingsSource>` has no observable class, so the identity has to be declared. Each
/// ported source returns its own constant; anything else names itself through
/// [`SourceKind::Custom`], which gives two independently written sources distinct identities in
/// the same way two anonymous Java classes have distinct `Class` objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceKind {
    /// `sources/DefaultSettings.java`.
    Default,
    /// `sources/JsonFileSettings.java` — reserved; not ported (spec §2).
    JsonFile,
    /// `sources/DsnFileSettings.java`.
    DsnFile,
    /// `sources/SesFileSettings.java`.
    SesFile,
    /// `sources/RulesFileSettings.java`.
    RulesFile,
    /// `sources/GuiSettingsSource.java` — reserved; not ported (no GUI).
    Gui,
    /// `sources/EnvironmentVariablesSource.java` (Task 7).
    Environment,
    /// `sources/CliSettings.java` (Task 7).
    Cli,
    /// `sources/ApiSettings.java`.
    Api,
    /// Any source declared outside this crate; the name is its identity.
    Custom(&'static str),
}

/// `settings/SettingsSource.java`: one origin of router settings.
///
/// A source populates **only** the fields it actually provides and leaves the rest `None` — that
/// nullability is the merge protocol itself (`SettingsMerger.java:22-31`, plan ruling 4).
pub trait SettingsSource {
    /// `SettingsSource.getSettings` (:22). `None` is Java's `null`, which
    /// [`SettingsMerger::merge`] skips entirely (`SettingsMerger.java:150-158`).
    ///
    /// Java returns a fresh object on every call for some sources (`DefaultSettings.getSettings`
    /// builds one — JVM-verified, `SProbe A.sameInstance = false`); this port hands back a
    /// borrow of a value the source computed once, because a merge never mutates a source's
    /// settings (it clones the base and applies the rest on top).
    fn get_settings(&self) -> Option<&RouterSettings>;

    /// `SettingsSource.getSourceName` (:29).
    fn get_source_name(&self) -> String;

    /// `SettingsSource.getPriority` (:41) — see [`priority`].
    fn get_priority(&self) -> i32;

    /// This source's identity for [`SettingsMerger::add_or_replace_sources`] — see
    /// [`SourceKind`]. Added by the port; Java uses `getClass()`.
    fn kind(&self) -> SourceKind;
}

/// `settings/SettingsMerger.java`: merges router settings from several sources into one resolved
/// [`RouterSettings`].
pub struct SettingsMerger {
    sources: Vec<Box<dyn SettingsSource>>,
}

impl SettingsMerger {
    /// `SettingsMerger(SettingsSource...)` / `SettingsMerger(List<SettingsSource>)`
    /// (`SettingsMerger.java:60-77`) — both constructors delegate to `addOrReplaceSources`, so
    /// duplicates in the argument list already collapse here.
    ///
    /// renamed: SettingsMerger -> SettingsMerger::new (Java's two overloads collapse into one
    /// `Vec` constructor; Rust has no varargs).
    #[must_use]
    pub fn new(sources: Vec<Box<dyn SettingsSource>>) -> Self {
        let mut merger = Self {
            sources: Vec::new(),
        };
        merger.add_or_replace_sources(sources);
        merger
    }

    /// `SettingsMerger.addOrReplaceSources` (:107-126): a new source whose [`SourceKind`] is
    /// already registered replaces that entry **in place** (`sources.set(i, …)`, :121); anything
    /// else is appended. JVM-verified — `SProbe C` = 1, 1, 2, 2.
    ///
    /// not ported: isAssignableFrom (`SettingsMerger.java:116-118`) — Java's second replacement
    /// arm also fires when the *existing* source's class is a **supertype** of the new one, so a
    /// `WorkspaceSettings` can replace a `GuiSettingsSource` placeholder. Rust has no supertype
    /// relation between two concrete types, and both classes involved are GUI-only and out of
    /// scope. It is a latent trap in Java rather than live behaviour — registering a base class
    /// after a subclass silently replaces the subclass, and the check is not symmetric — so it
    /// is recorded as quirk Q13 and left unported.
    pub fn add_or_replace_sources(&mut self, new_sources: Vec<Box<dyn SettingsSource>>) {
        for new_source in new_sources {
            match self
                .sources
                .iter()
                .position(|existing| existing.kind() == new_source.kind())
            {
                Some(i) => self.sources[i] = new_source,
                None => self.sources.push(new_source),
            }
        }
    }

    /// The registered sources, in registration order. Java's field is `private` and read only by
    /// `merge`/`clone`; this borrow exists so Task 8's two-merge equivalence harness and the
    /// ported `addOrReplaceSources` test can see what a merger holds.
    // added in Plan 4: (no Java counterpart — `SettingsMerger.sources` is a private field)
    #[must_use]
    pub fn sources(&self) -> &[Box<dyn SettingsSource>] {
        &self.sources
    }

    /// `SettingsMerger.merge` (:133-193).
    ///
    /// 1. **No sources at all → `new RouterSettings()`, returned immediately without
    ///    `validate()`** (:134-137). That early return is the only reason
    ///    `SettingsMergerTest.emptySourcesList` does not hit quirk Q5's `NullPointerException`.
    /// 2. Sort ascending by priority with a **stable** sort (:143 — Java's `List.sort` is
    ///    stable, and two sources may share a priority; JVM-verified, `SProbe
    ///    B.stableSortTie.maxPasses = 42`).
    /// 3. The first source that returns settings provides the base, via `clone()`
    ///    (:160-168) — [`RouterSettings::java_clone`], **not** the derived [`Clone`]: the two
    ///    differ in `result_json_path` and in how a null nested object comes back (quirk #114).
    /// 4. Every later one goes through [`RouterSettings::apply_new_values_from`] (:171).
    /// 5. `validate()` at the end (:189).
    ///
    /// `Runtime.getRuntime().availableProcessors()` reaches `validate` as `host` (plan ruling 6);
    /// Java's `merge()` takes no argument.
    ///
    /// # Panics
    ///
    /// When every registered source returns `None`, Java falls through to `new RouterSettings()`
    /// and *still* calls `validate()` (:184-189), which dereferences the null `maxPasses`. JVM
    /// row `SProbe B.onlyNullSource = java.lang.NullPointerException`; reproduced as
    /// [`RouterSettings::validate`]'s documented panic (quirk #125). The same panic is reachable
    /// for a merger whose lowest-priority source is not `DefaultSettings`.
    #[must_use]
    pub fn merge(&self, host: &HostEnvironment) -> RouterSettings {
        // :134-137 — `FRLogger.warn("No settings sources provided, using defaults")` is dropped
        // (plan Global Constraints: no `FRLogger`, and `fr-settings` must not depend on
        // `tracing`).
        if self.sources.is_empty() {
            return RouterSettings::new();
        }

        let mut sorted: Vec<&Box<dyn SettingsSource>> = self.sources.iter().collect();
        sorted.sort_by_key(|source| source.get_priority());

        let mut merged: Option<RouterSettings> = None;
        for source in sorted {
            let Some(settings) = source.get_settings() else {
                continue;
            };
            match merged.as_mut() {
                None => merged = Some(settings.java_clone()),
                // Java logs the change count (:171); plan ruling 2 — no caller reads it.
                Some(target) => {
                    let _report = target.apply_new_values_from(settings);
                }
            }
        }

        // `unwrap_or_default()` — which clippy suggests here — would be a *bug*:
        // `RouterSettings::default()` leaves `fanout`/`optimizer`/`scoring` `None`, while Java's
        // `new RouterSettings()` allocates all three (:184-186, `RouterSettings.java:119-124`).
        // `tests/struct_shape.rs` pins the two apart.
        #[allow(clippy::unwrap_or_default)]
        let mut merged = merged.unwrap_or_else(RouterSettings::new);
        merged.validate(host);
        merged
    }

    // not ported: clone (`SettingsMerger.java:195-198`) — `new SettingsMerger(this.sources)` is a
    // shallow copy that keeps the *same* `SettingsSource` instances, which a
    // `Vec<Box<dyn SettingsSource>>` cannot duplicate without an `Rc`. Java clones the prototype
    // merger so it can add file-specific sources twice without disturbing the original
    // (`Freerouting.java:125-146`, `RoutingJobScheduler.java:103-170`); the port's sources are
    // values a caller can rebuild, so Task 8 constructs a second merger from the same inputs
    // instead. Behaviour-identical, because `merge` never mutates a source.
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Stub(i32, SourceKind);

    impl SettingsSource for Stub {
        fn get_settings(&self) -> Option<&RouterSettings> {
            None
        }

        fn get_source_name(&self) -> String {
            format!("stub {}", self.0)
        }

        fn get_priority(&self) -> i32 {
            self.0
        }

        fn kind(&self) -> SourceKind {
            self.1
        }
    }

    #[test]
    fn constructor_collapses_duplicate_kinds_like_add_or_replace() {
        // `SettingsMerger(SettingsSource...)` delegates to `addOrReplaceSources` (:65), so two
        // sources of one kind never both survive the constructor.
        let merger = SettingsMerger::new(vec![
            Box::new(Stub(0, SourceKind::Default)),
            Box::new(Stub(1, SourceKind::Default)),
            Box::new(Stub(2, SourceKind::Api)),
        ]);
        assert_eq!(merger.sources().len(), 2);
        assert_eq!(merger.sources()[0].get_priority(), 1);
        assert_eq!(merger.sources()[1].get_priority(), 2);
    }

    #[test]
    fn empty_merger_returns_a_blank_without_validating() {
        // The early return at :134-137: no `validate()`, so no panic on the null `maxPasses`.
        let merged = SettingsMerger::new(Vec::new()).merge(&HostEnvironment::with_processors(4));
        assert_eq!(merged.max_passes, None);
    }
}
