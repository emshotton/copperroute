//! `settings/sources/RulesFileSettings.java` (109): a `.rules` file's `(autoroute_settings …)`
//! block, priority 40.

use std::io::Read;
use std::path::Path;

use fr_board::Board;
use fr_dsn::keyword::Keyword;
use fr_dsn::lexer::{DsnScanner, LexicalState, Token};
use fr_dsn::parser::autoroute_settings::read_autoroute_settings_scope;
use fr_dsn::parser::geometry::DsnLayerStructure;
use fr_dsn::parser::scope_parameter::skip_scope;

use crate::{RouterSettings, SettingsSource, SourceKind, merger::priority};

/// `settings/sources/RulesFileSettings.java`: the explicit routing-rule overrides.
///
/// This is the tier that carries per-layer directions and per-layer trace costs into the merge,
/// and — because of quirk Q18 (`docs/java-quirks.md` #128) — the tier whose trace costs the DSN
/// source at priority 20 has usually already blocked. Its per-layer `preferredDirectionHorizontal`
/// still lands, because
/// `layers` is an *object* array and merges element-wise (plan ruling 1's Q1 channel).
///
/// renamed: RulesFileSettings -> RulesFileSettings::new / RulesFileSettings::from_path.
/// Java has four constructors (`:28-69`) — `InputStream`, `File`, `Path`, `String` — that differ
/// only in how they reach the bytes and what they call the source. They collapse into two:
/// [`Self::new`] for an already-open reader and [`Self::from_path`] for the three
/// filesystem-facing ones, which share Java's "file missing → blank settings" arm (`:40`,
/// `:57-69`).
///
/// **The two naming rules Java's overloads disagree on are preserved**: the `File`/`Path`
/// constructors name the source with `file.getName()` — the *basename* (`:39`) — while the
/// `String` constructor keeps the string verbatim (`:58`). [`Self::from_path`] takes the
/// basename, [`Self::new`] takes whatever the caller passes. JVM-verified: `SProbe
/// D.rulesMissing.getSourceName = RULES file: dummy.rules` (String ctor) and `E.hw48na` =
/// `RULES file: Issue029-hw48na_valid.rules` for a path with directories in it (File ctor).
#[derive(Debug, Clone)]
pub struct RulesFileSettings {
    settings: RouterSettings,
    file_name: String,
}

impl RulesFileSettings {
    /// `RulesFileSettings.PRIORITY` (`:18`).
    const PRIORITY: i32 = priority::RULES_FILE;

    /// `RulesFileSettings(InputStream, String)` + `loadSettings` (`:28-31`, `:81-93`).
    ///
    /// `RulesReader.readRouterSettings` returning `null` — an empty stream, a bad header, or no
    /// `(autoroute_settings …)` scope — and **any** exception both give a blank
    /// `RouterSettings`; this never fails (`:84-92`). The two `FRLogger` calls are dropped
    /// (plan Global Constraints); a `MergeReport` entry would have no merge to attach to,
    /// because the failure happens before any field is copied.
    #[must_use]
    pub fn new(rules: impl Read, file_name: &str) -> Self {
        let settings = match fr_dsn::rules_reader::read_router_settings(rules) {
            Ok(Some(extracted)) => RouterSettings::from(extracted),
            Ok(None) | Err(_) => RouterSettings::new(),
        };
        Self {
            settings,
            file_name: file_name.to_string(),
        }
    }

    /// `RulesFileSettings(File)` / `(Path)` / `(String)` (`:38-69`, `loadFromFile` `:71-79`).
    ///
    /// A path that does not exist, or that cannot be opened, gives a blank `RouterSettings`
    /// rather than an error (`:40`, `:63-65`, `:76-78`) — `SProbe D.rulesMissing`. The source
    /// name is the path's **basename**, matching the `File` constructor (`:39`); a path with no
    /// final component at all falls back to its full display form, where Java's
    /// `file.getName()` would answer the empty string.
    #[must_use]
    pub fn from_path(path: &Path) -> Self {
        let file_name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        match std::fs::File::open(path) {
            Ok(file) => Self::new(file, &file_name),
            Err(_) => Self {
                settings: RouterSettings::new(),
                file_name,
            },
        }
    }
}

impl SettingsSource for RulesFileSettings {
    fn get_settings(&self) -> Option<&RouterSettings> {
        Some(&self.settings)
    }

    fn get_source_name(&self) -> String {
        format!("RULES file: {}", self.file_name)
    }

    fn get_priority(&self) -> i32 {
        Self::PRIORITY
    }

    fn kind(&self) -> SourceKind {
        SourceKind::RulesFile
    }
}

/// The **second** parse of the same `.rules` file: `RulesReader.read`'s `(autoroute_settings …)`
/// arm, resolved against the **board's** layer structure (`RulesReader.java:112`), applied onto
/// an already-merged [`RouterSettings`] (`:153-157`).
///
/// # Why a `.rules` file is parsed twice, with two different layer structures
///
/// The headless path reads the scheduler's `.rules` file twice, and the two reads disagree about
/// what a `layer_rule` name means:
///
/// | where | entry point | layer structure |
/// |---|---|---|
/// | priority 40, both merges | [`RulesFileSettings`] → `RulesReader.readRouterSettings` (`:180-236`) | **discovered from the file** — a `LinkedHashSet` of its own `(layer_rule …)` names (`:198`, `:238-274`) |
/// | after merge #2 | `RulesReader.read(…, board, job.routerSettings)` (`RoutingJobScheduler.java:173-184`) | **the board's** (`RulesReader.java:112`) |
///
/// For a file whose `layer_rule`s name every layer of the board in board order the two agree. For
/// any other file they do not: a two-`layer_rule` file (`F.Cu`, `B.Cu`) read against a four-layer
/// board puts `B.Cu` at index **3**, while the file-discovered parse puts it at index **1**. Both
/// results reach the answer — the first through the merge, the second through the post-merge
/// re-apply — so the port has to perform both parses too. `resolve_headless` does exactly that,
/// which is why it takes the file's **bytes** rather than one parsed `RouterSettings`; the
/// difference was measured on 13 of Task 9's 84 differential rows before this function existed.
/// `docs/java-quirks.md` #142.
///
/// # What this ports, and what it leaves out
///
/// Java's `RulesReader.read` also writes clearances, net classes, padstacks, via rules and the
/// snap angle into the **board**. [`fr_dsn::rules_reader::read`] is the port of that whole
/// method, and it is what a host that owns a `&mut Board` should call. This function is the
/// settings-only half — every other scope is skipped — because `resolve_headless` resolves
/// *settings* against a board it does not modify, and because `fr_dsn`'s `target_settings`
/// parameter is a `DsnRouterSettings` (`fr-dsn` cannot see this crate), which cannot express the
/// full `applyNewValuesFrom` Java performs here.
///
/// Every `(autoroute_settings …)` scope in the file is applied, in order, exactly as Java's loop
/// does (`:116-162`) — not just the first.
///
/// # Returns
///
/// Java's `boolean`: `true` when the `(rules …)` scope closed cleanly, `false` for a bad header,
/// an unexpected end of file, or a scanner error (`:121-133`, `:164-166`). No caller in the
/// headless path reads it — `RoutingJobScheduler.java:173-184` discards it inside a `try` — and
/// nothing inside the scope can make it `false`.
///
// renamed: RulesReader.read -> apply_rules_file_against_board, restricted to the
// `(autoroute_settings …)` arm and taking `&Board` instead of `&mut BasicBoard`; the full method
// is `fr_dsn::rules_reader::read`.
pub fn apply_rules_file_against_board(
    bytes: &[u8],
    board: &Board,
    target: &mut RouterSettings,
) -> bool {
    // `new InputStreamReader(in)` (`RulesReader.java:78`), as `fr_dsn`'s own reader decodes it.
    let text = String::from_utf8_lossy(bytes).into_owned();
    let layer_structure = DsnLayerStructure::from_board(board.layer_structure());
    // fixed: T4 (#86) — was a fallible constructor with a 16 MiB ceiling; now infallible.
    let mut scanner = DsnScanner::new(&text);

    // The `(rules PCB <name>` header (`:80-110`). The name is consumed and never validated: a
    // mismatch is logged, not fatal.
    for expected in [
        Token::Open,
        Token::Kw(Keyword::Rules),
        Token::Kw(Keyword::PcbScope),
    ] {
        match scanner.next_token() {
            Ok(Some(token)) if token == expected => {}
            _ => return false,
        }
    }
    scanner.yybegin(LexicalState::Name);
    if scanner.next_token().is_err() {
        return false;
    }

    // `:116-162` — the top-level scope loop, keeping only the `AUTOROUTE_SETTINGS` arm.
    let mut prev_was_open = false;
    loop {
        let next_token = match scanner.next_token() {
            Ok(Some(token)) => token,
            // "unexpected end of file" (`:125-129`), and the scanner error Java's
            // `catch (IOException)` does not catch either (`:164-166` returns `false` for the
            // `IOException` half).
            Ok(None) | Err(_) => return false,
        };
        if next_token == Token::Close {
            // End of the `(rules …)` scope — Java's `true` (`:130-133`).
            return true;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            if next_token == Token::Kw(Keyword::AutorouteSettings) {
                // `:152-157`.
                match read_autoroute_settings_scope(&mut scanner, &layer_structure) {
                    Ok(Some(parsed)) => {
                        target.apply_new_values_from(&RouterSettings::from(&parsed));
                    }
                    Ok(None) => {}
                    Err(_) => return false,
                }
            } else if skip_scope(&mut scanner).is_err() {
                return false;
            }
        }
        prev_was_open = is_open;
    }
}
