//! `BoardStatistics(byte[], FileFormat)` (core/scoring/BoardStatistics.java:436-552) — the
//! **text-scraping twin** of the computing constructor, and its private helper
//! `countOccurrences` (`:578-586`).
//!
//! # Two constructors, two different objects
//!
//! Plan 7 ported `BoardStatistics(BasicBoard, …)` (`:110-427`), which walks a loaded board and
//! measures it. This one never builds a board: it decodes the file's bytes as UTF-8 and counts
//! **substrings**. Java's own javadoc says so (`:430-431`: *"This method should be used only if
//! the board object is not available, because the board object based method is more detailed."*).
//! Its reader is the result manifest — `RoutingJob.setInput`/`setOutput` build a
//! `BoardStatistics` from the file when no board object exists — which is why the two live in
//! different crates and why nothing in `autoroute/pipeline/**` calls this one.
//!
//! Seven of the fifty fields can ever be written here: `host`, `layers.total_count`,
//! `components.total_count`, `nets.class_count`, `nets.total_count`, `traces.total_count` and
//! `vias.total_count`. Everything else stays at its Java `null`, which is what
//! [`BoardStatistics::default`] is.
//!
//! # The guard at `:437-439` is not expressible here, and it does not need to be
//!
//! Java returns early on `data == null || format == null`, leaving the all-`null` object. The
//! port's signature takes `&[u8]` and a `FileFormat`, neither of which can be null — and the
//! early return produces exactly what a `FileFormat` with no branch produces anyway
//! (`FileFormat::Unknown`, `Frb`, `Rules`, `Scr`, `DrcJson`, `KicadSessionJson`), so the guard
//! is not a distinct behaviour on this side. `P8T2Probe`'s rows 31 and 32 measure both null arms
//! against row 33 and the three answers are the same object.
//!
//! # Quirk #247 is the point of this module
//!
//! [`count_occurrences`] is a **substring** count with no token boundary, so `(layer` also counts
//! `(layer_rule`, `(net` also counts `(network` and `(net_class`, `(via` also counts `(via_rule`
//! and `(class` also counts `(class_class`. The SES layer scrape adds its own off-by-one. All of
//! it is reproduced, none of it is fixed; `crates/fr-core/tests/stats.rs` pins each one against
//! the jar.

use crate::FileFormat;
use fr_router::score::BoardStatistics;

/// The byte-scraping constructor, as an extension trait.
///
/// `fr_router::score::BoardStatistics` is the one home of the type (plan-7 ruling 4) and this
/// crate re-exports rather than redeclares it (plan-8 ruling 1), so the constructor Plan 8 owns
/// arrives as a trait rather than as a second `impl` block on a foreign type.
pub trait BoardStatisticsExt {
    /// Port of `BoardStatistics(byte[], FileFormat)` (BoardStatistics.java:436-552).
    ///
    /// The SES / DSN / KiCad-design-JSON **text scraper**. It never builds a board; see the
    /// module docs for why it is a different object from
    /// [`BoardStatistics::compute`](fr_router::score::BoardStatistics::compute).
    fn from_bytes(data: &[u8], format: FileFormat) -> BoardStatistics;
}

impl BoardStatisticsExt for BoardStatistics {
    fn from_bytes(data: &[u8], format: FileFormat) -> BoardStatistics {
        let mut stats = BoardStatistics::default();

        // `:437-439`. See the module docs: neither null arm is expressible, and both produce
        // this object, which is also what the `match`'s default arm below produces.
        match format {
            // `:441-473`.
            FileFormat::Ses => ses_branch(&decode_utf8(data), &mut stats),
            // `:474-519`.
            FileFormat::Dsn => dsn_branch(&decode_utf8(data), &mut stats),
            // `:520-551`.
            FileFormat::KicadDesignJson => kicad_design_json_branch(&decode_utf8(data), &mut stats),
            // `:441`, `:474`, `:520` are the only three `format ==` tests: every other value
            // falls out of the chain with the object untouched.
            FileFormat::Unknown
            | FileFormat::Frb
            | FileFormat::Rules
            | FileFormat::Scr
            | FileFormat::DrcJson
            | FileFormat::KicadSessionJson => {}
        }

        stats
    }
}

/// `new String(data, StandardCharsets.UTF_8)` (`:443`, `:476`, `:522`).
///
/// Java's UTF-8 decoder is configured with `CodingErrorAction.REPLACE`, and so is
/// [`String::from_utf8_lossy`]: both follow Unicode's maximal-subpart rule, so a malformed
/// sequence becomes the same number of `U+FFFD`s on both sides. `P8T2Probe`'s two `binary junk`
/// rows measure it rather than assuming it.
fn decode_utf8(data: &[u8]) -> String {
    String::from_utf8_lossy(data).into_owned()
}

/// Port of `BoardStatistics.countOccurrences` (BoardStatistics.java:578-586).
///
/// Java bug: BoardStatistics.countOccurrences (core/scoring/BoardStatistics.java:578-586) counts **substrings**, not tokens, so every keyword this constructor looks for also matches its longer relatives: `(layer` counts `(layer_rule`, `(net` counts `(network` and `(net_class`, `(via` counts `(via_rule`, `(class` counts `(class_class`. Quirk #247; reproduced, not fixed.
///
/// Matches do not overlap: the cursor advances by the needle's whole length (`:583`), so
/// `countOccurrences("aaaa", "aa")` is `2`, not `3`.
///
// totalized: `BoardStatistics.countOccurrences` (core/scoring/BoardStatistics.java:581-583) spins for ever on an empty `target` — `indexOf("", i)` answers `i`, `index += 0` never advances, and `count` runs away. Quirk #252: the port answers `0`, because a hang is not a value a parity harness can compare. Unreachable from the constructor, whose four call sites all pass string literals.
pub fn count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    let mut count = 0;
    let mut index = 0;
    // `:581` — `text.indexOf(target, index)`; `:583` — `index += target.length()`.
    while let Some(found) = haystack[index..].find(needle) {
        count += 1;
        index += found + needle.len();
    }
    count
}

/// `java.lang.String.split(String regex)` with the default limit of zero, for a regex that is a
/// **literal** — which is what all three of this constructor's `split` calls are
/// (`"\\(path "` at `:449` and `" "` at `:456`).
///
/// Two behaviours a bare [`str::split`] gets wrong, and this constructor reaches both:
///
/// 1. **No match returns the whole input as one element**, even when the input is empty. So
///    `"".split(" ")` has length **1**, which is why the `words.length >= 2` guard at `:458`
///    rejects a chunk with no space in it rather than indexing out of range.
/// 2. **Trailing empty pieces are dropped.** `"a(path ".split("\\(path ")` is `["a"]`, length 1,
///    so the loop body never runs; and `"(path ".split("\\(path ")` is the **empty** array.
fn java_split_literal<'a>(s: &'a str, separator: &str) -> Vec<&'a str> {
    if !s.contains(separator) {
        return vec![s];
    }
    let mut parts: Vec<&str> = s.split(separator).collect();
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

/// `:441-473` — the SES branch.
fn ses_branch(content: &str, stats: &mut BoardStatistics) {
    // `:449`. Split on the literal `"(path "`.
    let lines = java_split_literal(content, "(path ");

    // `:452-466`. An insertion-ordered list of distinct layer names.
    let mut layers: Vec<&str> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let words = java_split_literal(line, " ");
        // Java bug: BoardStatistics.<init> (core/scoring/BoardStatistics.java:458) guards the layer scrape with `i > 0`, so the chunk **before** the first `"(path "` is skipped — correct — but the guard is also the only thing stopping the scrape when there is no `"(path "` at all, and the `words.length >= 2` half then silently drops a real layer whose `(path ` clause is the last thing in the file (`"a(path F.Cu"` scrapes nothing). Quirk #247's second half; `words[0]` of every *other* chunk is taken verbatim, whitespace and all.
        if i > 0 && words.len() >= 2 {
            let layer = words[0];
            // `:462-464`. `List.contains`, so the order is insertion order and only the count
            // is ever read.
            if !layers.contains(&layer) {
                layers.push(layer);
            }
        }
    }

    // `:469-473`.
    stats.layers.total_count = Some(layers.len() as i32);
    stats.components.total_count = Some(count_occurrences(content, "(component") as i32);
    stats.nets.total_count = Some(count_occurrences(content, "(net") as i32);
    stats.traces.total_count = Some(count_occurrences(content, "(wire") as i32);
    stats.vias.total_count = Some(count_occurrences(content, "(via") as i32);
    // Note what the SES branch does NOT set: `nets.class_count` stays `null` here and is set
    // only by the DSN branch (`:516`), which is visible on every committed `.ses`.
}

/// `:474-519` — the DSN branch, whose host scrape can never succeed on a real DSN (quirk #248).
fn dsn_branch(content: &str, stats: &mut BoardStatistics) {
    // `:478-479`.
    let mut host_cad: Option<String> = None;
    let mut host_version: Option<String> = None;

    // `:480`.
    if let Some(parser_index) = content.find("(parser") {
        // Java bug: BoardStatistics.<init> (core/scoring/BoardStatistics.java:482) sets `searchLimit` to the **first `)` after `(parser`**, which in a real Specctra DSN is the end of the first inner clause — `(string_quote ")` — so `parserScope` stops before `(host_cad …)` ever appears and `host` stays null on every board in the corpus. Quirk #248, first of three.
        let mut search_limit = match content[parser_index..].find(')') {
            // `:485-486`.
            Some(offset) => java_min(content.len(), parser_index + offset + 1),
            // `:483-484` — no `)` anywhere after `(parser`.
            None => java_min(content.len(), parser_index + 1000),
        };
        // `content` is indexed in bytes, and the clamp above can land inside a multi-byte
        // character when a `(parser` scope contains one. Java slices UTF-16 code units and
        // cannot land mid-character in the same way; walking forward to the next boundary is the
        // nearest equivalent and is unreachable from any corpus file (the clamp only bites past
        // 1000 bytes with no `)`).
        while !content.is_char_boundary(search_limit) {
            search_limit += 1;
        }
        let parser_scope = &content[parser_index..search_limit];

        // Java bug: BoardStatistics.<init> (core/scoring/BoardStatistics.java:489, :497) searches for the **camelCase** keywords `(hostCad` and `(hostVersion`, which is HEAD's own spelling (`io/specctra/parser/Keyword.java:40-41`, written by `Parser.writeScope` at `:110` and `:117`) but not the Specctra standard's: every CAD-exported DSN — every one of the seven corpus boards — writes `(host_cad` and `(host_version`, so even a `parserScope` wide enough to hold them would not match. Quirk #248, second of three; see the quirk row for the one file shape in this repository where it DOES match.
        if let Some(hc_idx) = parser_scope.find("(hostCad")
            && let Some(hc_end) = parser_scope[hc_idx..].find(')').map(|o| hc_idx + o)
        {
            // Java bug: BoardStatistics.<init> (core/scoring/BoardStatistics.java:493) hard-codes `hcIdx + 9`, i.e. `"(hostCad"` plus **exactly one** character, so `(hostCad  "K")` keeps a leading space (which `trim()` then removes, harmlessly) while `(hostCad"K")` keeps the opening quote — and `removeQuotes` then refuses to strip the closing one. `hostVersion` does the same with `hvIdx + 13`. Quirk #248, third of three.
            let value = slice_totalized(parser_scope, hc_idx + 9, hc_end);
            host_cad = Some(remove_quotes(java_trim(value)).to_string());
        }
        if let Some(hv_idx) = parser_scope.find("(hostVersion")
            && let Some(hv_end) = parser_scope[hv_idx..].find(')').map(|o| hv_idx + o)
        {
            let value = slice_totalized(parser_scope, hv_idx + 13, hv_end);
            host_version = Some(remove_quotes(java_trim(value)).to_string());
        }
    }

    // `:507-511`. Note there is no `hostVersion`-only arm: a file that names only the version
    // leaves `host` null.
    //
    // **This constructor has no `host` fallback at all.** The sibling *computing* constructor's
    // `"Freerouting," + Constants.FREEROUTING_VERSION` at `:118-120` — which would have been
    // [`crate::PARITY_VERSION`]'s third reader — is unreachable there (quirk #249) and absent
    // here, so a failed scrape leaves `host` null rather than naming the port. The
    // `// not reachable:` marker that carries the whole note lives with the Java code it
    // describes, at `crates/fr-router/src/score/statistics.rs`'s `host_of`.
    //
    // The port spells Java's `host == null` as the **empty string**, because
    // `fr_router::score::BoardStatistics.host` is a `String` and plan 8 makes only additive
    // changes to `fr-router`. That conflates it with the one input that scrapes an *empty*
    // `hostCad` — `(parser (hostCad  ))`, where Gson prints `"host": ""` and this port omits the
    // key. Quirk #251 records it; `P8T2Probe`'s `empty hostCad` row is an `XDIFF` carrying both
    // answers. Nothing else can reach it: the computing constructor's `host` is at minimum the
    // six characters `null,null` (quirk #249), and a successful scrape of a non-empty value is
    // non-empty by construction.
    match (host_cad, host_version) {
        (Some(cad), Some(version)) => stats.host = format!("{cad},{version}"),
        (Some(cad), None) => stats.host = cad,
        _ => {}
    }

    // `:514-519`.
    stats.layers.total_count = Some(count_occurrences(content, "(layer") as i32);
    stats.components.total_count = Some(count_occurrences(content, "(component") as i32);
    stats.nets.class_count = Some(count_occurrences(content, "(class") as i32);
    stats.nets.total_count = Some(count_occurrences(content, "(net") as i32);
    stats.traces.total_count = Some(count_occurrences(content, "(wire") as i32);
    stats.vias.total_count = Some(count_occurrences(content, "(via") as i32);
}

/// `:520-551` — the KiCad design-JSON branch.
///
/// not ported: Gson's `Strictness.LENIENT` dialect (`util/gson/GsonProvider.java:20`) on the read side — unquoted keys, single-quoted strings, `NaN`, trailing commas. `serde_json` is strict, and `crates/fr-settings/src/json.rs` made the same call for `RouterSettings.fromJson` (quirk #141). A file this port rejects and Java accepts leaves every field null on this side and populates them on Java's; no corpus file reaches it, and the two rows `P8T2Probe` does measure (`kicad malformed`, `kicad array`) agree because both readers refuse them.
fn kicad_design_json_branch(content: &str, stats: &mut BoardStatistics) {
    // `:523-525` plus `:548-550`'s `catch (Exception)`: a parse failure, a top-level value that
    // is not an object, and a wrongly-typed member all land in the same place — the fields
    // written *before* the failure are kept, which is why this is a sequence of `?`-less steps
    // with an early return rather than one fallible expression.
    let Ok(value) = serde_json::from_str::<serde_json::Value>(content) else {
        return;
    };
    let Some(object) = value.as_object() else {
        // `Gson.fromJson(content, JsonObject.class)` throws `JsonSyntaxException` ("Expected a
        // com.google.gson.JsonObject but was …") for an array, a bare `null` or a primitive, and
        // `:548` catches it.
        return;
    };

    // `:526-543`, in Java's order. `getAsJsonArray` throws `IllegalStateException` on a member
    // that is not an array, and `:548` catches it — so the first wrongly-typed member stops the
    // walk and everything before it survives.
    for (key, field) in [
        ("layers", ArrayField::Layers),
        ("components", ArrayField::Components),
        ("netClasses", ArrayField::NetClasses),
        ("nets", ArrayField::Nets),
        ("traces", ArrayField::Traces),
        ("vias", ArrayField::Vias),
    ] {
        if let Some(member) = object.get(key) {
            let Some(array) = member.as_array() else {
                return;
            };
            let count = Some(array.len() as i32);
            match field {
                ArrayField::Layers => stats.layers.total_count = count,
                ArrayField::Components => stats.components.total_count = count,
                ArrayField::NetClasses => stats.nets.class_count = count,
                ArrayField::Nets => stats.nets.total_count = count,
                ArrayField::Traces => stats.traces.total_count = count,
                ArrayField::Vias => stats.vias.total_count = count,
            }
        }
    }

    // `:544-546`. `json.has("designName")` is this `contains_key`; the value itself is read from
    // the **raw source text** rather than from the parsed `Value` — [`get_as_string`] gives the
    // two reasons.
    if object.contains_key("designName") {
        let Some(raw) = raw_member(content, "designName") else {
            // Unreachable: `serde_json` accepted the document above, so a member it reports is a
            // member [`raw_member`] can find. Returning rather than panicking keeps the branch's
            // "a failure leaves the earlier fields set" shape either way.
            return;
        };
        let Some(text) = get_as_string(raw) else {
            return;
        };
        stats.host = format!("KiCad JSON,{text}");
    }
}

/// The six `json.has(...)` members of `:526-543` that are read as arrays.
#[derive(Clone, Copy)]
enum ArrayField {
    Layers,
    Components,
    NetClasses,
    Nets,
    Traces,
    Vias,
}

/// `com.google.gson.JsonElement.getAsString()` as `:545` reaches it, over the **raw source
/// slice** of the value rather than over a parsed `serde_json::Value`.
///
/// Two behaviours make the raw slice necessary, both measured against the HEAD jar:
///
/// 1. **A number answers its own source text.** `JsonPrimitive.getAsString()` on a number
///    produced by Gson's lazy reader returns `LazilyParsedNumber.toString()`, which is the
///    characters that were in the file: `1e5` stays `1e5`, `1.50` stays `1.50`, and a
///    thirty-digit integer keeps all thirty digits. `serde_json` without the
///    `arbitrary_precision` feature parses into `i64`/`u64`/`f64` and **re-renders** on
///    `Display` — `1e5` would come back as `100000.0` and `1.50` as `1.5`. Enabling that feature
///    is a `Cargo.toml` edit plan 8 does not permit without a ruling, and reading the token out
///    of the text `serde_json` has already validated costs less than one.
/// 2. **A one-element array delegates to its element.** `JsonArray.getAsString()` is
///    *overridden*: `size() == 1` forwards to `get(0).getAsString()`, recursively, so `["foo"]`
///    answers `foo`, `[["deep"]]` answers `deep` and `[1e5]` answers `1e5`. Any other size
///    throws `IllegalStateException`, which `:548` catches.
///
/// An object and `null` throw (`UnsupportedOperationException`) and are `None` here; a boolean
/// answers `true`/`false`. All of it is pinned by `P8T2Probe`'s twelve `kicad designName …` rows.
fn get_as_string(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let first = *raw.as_bytes().first()?;
    match first {
        // A JSON string: `serde_json` is the unescaper, and its answer is the parsed value
        // `JsonPrimitive.getAsString()` also returns for a string.
        b'"' => serde_json::from_str::<String>(raw).ok(),
        // `JsonArray.getAsString()`'s `size() == 1` delegation.
        b'[' => match raw_array_elements(raw)?.as_slice() {
            [only] => get_as_string(only),
            _ => None,
        },
        // `JsonObject` does not override `getAsString()`, so `JsonElement`'s throws.
        b'{' => None,
        // `JsonNull.getAsString()` throws `UnsupportedOperationException`, caught at `:548`.
        _ if raw == "null" => None,
        // A number, `true` or `false` — all three answer the source token verbatim, which for
        // the two literals is also what `Boolean.toString` produces.
        _ => Some(raw.to_string()),
    }
}

/// The raw source slice of top-level member `key`'s value, or `None` if there is none.
///
/// `content` has already been accepted by `serde_json`, so this walks a **valid** JSON object and
/// never has to diagnose anything. A duplicated key answers the **last** occurrence, which is
/// what `LinkedTreeMap.put` (Gson) and `serde_json::Map::insert` both do.
fn raw_member<'a>(content: &'a str, key: &str) -> Option<&'a str> {
    let bytes = content.as_bytes();
    let mut i = skip_whitespace(bytes, 0);
    if bytes.get(i) != Some(&b'{') {
        return None;
    }
    i += 1;
    let mut found = None;
    loop {
        i = skip_whitespace(bytes, i);
        match bytes.get(i) {
            Some(b',') => {
                i += 1;
                continue;
            }
            Some(b'"') => {}
            // `}`, or anything else a valid object cannot hold here.
            _ => return found,
        }
        let key_end = scan_value_end(bytes, i)?;
        let name = serde_json::from_str::<String>(&content[i..key_end]).ok()?;
        i = skip_whitespace(bytes, key_end);
        if bytes.get(i) != Some(&b':') {
            return found;
        }
        i = skip_whitespace(bytes, i + 1);
        let value_end = scan_value_end(bytes, i)?;
        if name == key {
            found = Some(&content[i..value_end]);
        }
        i = value_end;
    }
}

/// The raw source slices of the elements of the array `raw` starts with.
fn raw_array_elements(raw: &str) -> Option<Vec<&str>> {
    let bytes = raw.as_bytes();
    let mut i = skip_whitespace(bytes, 0);
    if bytes.get(i) != Some(&b'[') {
        return None;
    }
    i += 1;
    let mut elements = Vec::new();
    loop {
        i = skip_whitespace(bytes, i);
        match bytes.get(i) {
            Some(b',') => {
                i += 1;
                continue;
            }
            // `]`, or the end of the slice.
            Some(b']') | None => return Some(elements),
            Some(_) => {}
        }
        let end = scan_value_end(bytes, i)?;
        elements.push(&raw[i..end]);
        i = end;
    }
}

/// The index just past the JSON value starting at `start`, for a document `serde_json` has
/// already accepted: a string (honouring `\` escapes), a bracketed array or object (by depth,
/// ignoring brackets inside strings), or a bare literal or number token.
fn scan_value_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    match *bytes.get(i)? {
        b'"' => {
            i += 1;
            while let Some(&b) = bytes.get(i) {
                match b {
                    b'\\' => i += 2,
                    b'"' => return Some(i + 1),
                    _ => i += 1,
                }
            }
            None
        }
        open @ (b'[' | b'{') => {
            let close = if open == b'[' { b']' } else { b'}' };
            let mut depth = 0_usize;
            while let Some(&b) = bytes.get(i) {
                if b == b'"' {
                    i = scan_value_end(bytes, i)?;
                } else if b == open {
                    depth += 1;
                    i += 1;
                } else if b == close {
                    depth -= 1;
                    i += 1;
                    if depth == 0 {
                        return Some(i);
                    }
                } else {
                    i += 1;
                }
            }
            None
        }
        // A number, `true`, `false` or `null`: everything up to the next structural character.
        _ => {
            while let Some(&b) = bytes.get(i) {
                if b.is_ascii_whitespace() || matches!(b, b',' | b']' | b'}' | b':') {
                    break;
                }
                i += 1;
            }
            (i > start).then_some(i)
        }
    }
}

/// JSON's four insignificant characters (RFC 8259 §2) — the same four `serde_json` skips.
fn skip_whitespace(bytes: &[u8], mut i: usize) -> usize {
    while matches!(bytes.get(i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
        i += 1;
    }
    i
}

/// Port of `TextManager.removeQuotes` (util/TextManager.java:142-149).
///
/// Note the `length() < 2` guard: a lone `"` is returned unchanged, and so is anything that does
/// not carry a quote at **both** ends — which is what makes quirk #248's `(hostCad"K")` come out
/// as `K"` rather than `K`.
fn remove_quotes(text: &str) -> &str {
    // `String.length()` counts UTF-16 units and `str::len` counts bytes, so the port's guard is
    // the weaker of the two — but the difference is unobservable: a string that reaches `< 2`
    // bytes and a string that reaches `< 2` units both fail the two-quote test below, `"` being
    // one byte and one unit.
    if text.len() < 2 {
        return text;
    }
    if text.starts_with('"') && text.ends_with('"') {
        &text[1..text.len() - 1]
    } else {
        text
    }
}

/// `String.substring(begin, end)` for the two host slices, with the one range Java refuses.
///
// totalized: `BoardStatistics.<init>` (core/scoring/BoardStatistics.java:493, :501) computes `parserScope.substring(hcIdx + 9, hcEnd)` without checking that `hcIdx + 9 <= hcEnd`, so `(parser (hostCad))` — where the closing bracket sits one character after the keyword — throws `StringIndexOutOfBoundsException` out of the constructor and the caller gets no statistics at all. Quirk #250: the port clamps the start to the end, which yields the empty string the slice would have held had the input carried the one space Java's `+ 9` assumes.
///
/// `begin` is a byte offset where Java's is a UTF-16 offset. The two agree for every input the
/// keywords can precede — `(hostCad` and `(hostVersion` are ASCII, and the `+ 9` / `+ 13` step
/// over exactly one character — except for a supplementary-plane character immediately after the
/// keyword, where Java would slice a lone surrogate and this walks to the character's end.
fn slice_totalized(text: &str, begin: usize, end: usize) -> &str {
    let mut begin = java_min(begin, end);
    while !text.is_char_boundary(begin) {
        begin += 1;
    }
    &text[begin..end]
}

/// `Math.min(int, int)`.
fn java_min(a: usize, b: usize) -> usize {
    if a < b { a } else { b }
}

/// `java.lang.String.trim()`, which is **not** [`str::trim`].
///
/// Java strips every code unit `<= U+0020`; Rust strips Unicode `White_Space`. The two sets
/// differ in *both* directions, and `:493`/`:501` are the only `trim()` calls this constructor
/// makes. Measured against the HEAD jar:
///
/// | input | Java `trim()` | Rust `str::trim()` |
/// |---|---|---|
/// | `"X\u{a0}"` (NBSP) | **kept** — length stays 2 | stripped |
/// | `"X\u{1}"` (SOH) | **stripped** — length 1 | kept |
/// | `"X\u{b}"` (VT) | stripped | stripped |
/// | `"X\u{2028}"`, `"X\u{3000}"`, `"X\u{85}"` | **kept** | stripped |
///
/// `P8T2Probe`'s `trim keeps NBSP` and `trim drops the control char` rows pin the first two.
/// Java counts UTF-16 units where this counts chars, which cannot differ: no character above
/// `U+FFFF` has a code unit `<= U+0020`.
///
/// **This crate has a second copy**, [`crate::manifest`]'s, differing only in its return type
/// (`String`, because its caller stores rather than slices). See that one for why the duplication
/// is left as it is and for the third, cross-crate copy in `fr-settings`.
fn java_trim(text: &str) -> &str {
    text.trim_matches(|c: char| c <= '\u{20}')
}
