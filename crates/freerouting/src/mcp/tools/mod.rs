//! Spec §13's four tools, their hand-written schemas, and the two things all four share: how a
//! board argument is read, and how a produced file is reported.
//!
//! # Flat arguments (controller ruling AO)
//!
//! Every tool here takes its arguments **flat** — `{"dsn_path": "…"}` — rather than the
//! `{path, query, body}` wrapper the jar's 24 OpenAPI-generated tools use
//! (`api/mcp/OpenApiMcpToolRegistry.java`; 24 wrapped plus 4 flat, measured in
//! `docs/plan-8-prep/evidence/job3-summary.md` §4, delta row 10). The wrapper exists there only
//! because those tools are HTTP calls in disguise, and this server has no HTTP behind it.
//!
//! # The `BoardFilePayload` field names (controller ruling AO)
//!
//! Where a result names the same thing `api/dto/BoardFilePayload.java` names, it uses **Java's
//! spelling**, so an agent written against the jar's REST API is not gratuitously broken:
//! `job_id`, `data` (Base64), and the inherited `size`, `crc32`, `format`, `filename`, `path`
//! (`core/BoardFileDetails.java:31-51`, whose `directoryPath` serialises as `path`). The port's
//! own [`fr_core::BoardFileDetails`] already carries the same names for the same reason.
//!
//! # Which tools report progress, and which observe cancellation
//!
//! `route_board` does both — it is the only one that can run for minutes. `check_drc`,
//! `board_info` and `list_settings` are a load and a walk, so they report nothing and the
//! transport's drop-everything [`ProgressWriter`](super::server::ProgressWriter) means they do
//! not have to say so. **The jar has neither at all** on any of its 28 tools
//! (`McpControllerV1.java:189-198`, and the agent card admits `"streamingToolCalls": false` at
//! `api/AgentCardController.java:128`) — delta rows 6 and 7.

pub mod board_info;
pub mod check_drc;
pub mod list_settings;
pub mod route_board;
pub mod schema;

use super::jsonrpc::RpcError;
use super::server::{State, ToolDef};
use fr_core::{RoutingJob, SessionId, Uuid128};
use serde_json::{Value, json};
use std::path::PathBuf;

/// Registers spec §13's four tools on a fresh [`State`].
///
/// The registry is built **once, here, at start-up**. The jar rebuilds its own by running a fresh
/// Swagger scan of `app.freerouting.api` on every `tools/list` *and* every `tools/call`
/// (`McpControllerV1.java:294-301`, `:315`; the first list in job 3's run A took about two
/// seconds of wall time) — the second half of delta row 10.
pub fn register_all(state: &mut State) {
    state.register_tool(
        ToolDef {
            name: "route_board".into(),
            description: "Auto-route a Specctra DSN (or KiCad board JSON) and return the \
                          Specctra session. Give output_path to have the session written to disk \
                          and get a path back; omit it to get the session as text. Reports \
                          progress and honours cancellation."
                .into(),
            input_schema: schema::route_board_schema(),
        },
        Box::new(route_board::run),
    );
    state.register_tool(
        ToolDef {
            name: "check_drc".into(),
            description: "Run the design-rule check on a board and return the KiCad DRC report \
                          — the same document `freerouting drc` writes. Coordinates are in \
                          millimetres."
                .into(),
            input_schema: schema::check_drc_schema(),
        },
        Box::new(check_drc::run),
    );
    state.register_tool(
        ToolDef {
            name: "board_info".into(),
            description: "Summarise a board: its layers, nets and components by name, the file's \
                          own metadata, and the board statistics."
                .into(),
            input_schema: schema::board_info_schema(),
        },
        Box::new(board_info::run),
    );
    state.register_tool(
        ToolDef {
            name: "list_settings".into(),
            description: "The router settings schema — every field, its type and what it does — \
                          together with the defaults currently in force. What route_board's \
                          `settings` argument accepts."
                .into(),
            input_schema: schema::list_settings_schema(),
        },
        Box::new(list_settings::run),
    );
}

// =================================================================================================
// The board argument
// =================================================================================================

/// Reads spec §13's `dsn_path | dsn_text` pair into a [`RoutingJob`].
///
/// "Exactly one" is enforced here as well as in the schema, because the jar's own measurement is
/// that a published schema is **advisory**: job 3 §4 records `additionalProperties:false` and
/// `required` being ignored at call time by the jar's `invokeTool`. A tool that trusts its schema
/// trusts the client.
///
/// # The two variants are not symmetric, and the asymmetry is Java's
///
/// `dsn_path` goes through [`RoutingJob::set_input`], which is `setInputFromFile`
/// (`core/RoutingJob.java:425-461`): the format comes from the **bytes** first and the extension
/// only as a fallback, the default output name is derived, and `job.name` becomes the input's
/// base name — which is the design name that ends up in the SES `(session "…")` header.
///
/// `dsn_text` has no file, so it goes through `setInput(byte[])` (`:270-275`), which sniffs the
/// format and stops. That leaves `job.name` as the id-derived `J-XXXXXX`, and since [`mint_job_id`]
/// draws a **real** id, an anonymous upload's session header would then change on every call. The
/// port therefore names the synthetic input `board.dsn` and the design `board`, which is the one
/// place this module departs from Java's API path and is safe to depart in: the jar has no
/// `route_board` tool, so there is nothing here to be bug-compatible with, and a deterministic
/// document is worth more than a faithful reproduction of a random one.
///
/// The path itself is **not** kept: everything downstream that needs it reads it back off
/// `job.input`'s own filename and directory, which is where `RoutingJobScheduler.java:131-152`
/// reads it too (the adjacent-`<design>.rules` probe).
///
/// # Errors
///
/// [`RpcError::invalid_params`] when neither argument is present, when both are, when the path
/// cannot be read, or when the bytes are not a format the loader accepts.
pub fn board_input(args: &Value) -> Result<RoutingJob, RpcError> {
    let path = optional_string(args, "dsn_path")?;
    let text = optional_string(args, "dsn_text")?;
    let mut job = RoutingJob::with_id(SessionId::NIL, mint_job_id());
    match (path, text) {
        (Some(_), Some(_)) => Err(RpcError::invalid_params(
            "give exactly one of dsn_path and dsn_text, not both",
        )),
        (None, None) => Err(RpcError::invalid_params(
            "one of dsn_path and dsn_text is required",
        )),
        (Some(path), None) => {
            let path = PathBuf::from(path);
            job.set_input(&path).map_err(|error| {
                RpcError::invalid_params(format!(
                    "Couldn't load the input file '{}': {error}",
                    path.display()
                ))
            })?;
            Ok(job)
        }
        (None, Some(text)) => {
            // `setInput(byte[])` (`:270-275`) — the format comes from the **bytes**, so a
            // `dsn_text` carrying a KiCad board JSON is recognised as one.
            job.set_input_bytes(Some(text.as_bytes()));
            if let Some(input) = job.input.as_mut() {
                // The synthetic name follows the format the bytes announced, so `filename` does
                // not claim `.dsn` for a JSON document. `set_filename` only *derives* a format
                // when the current one is `UNKNOWN` (`BoardFileDetails.java:174-177`), so this
                // cannot overwrite what the sniff decided; the `"dsn"` fallback is for bytes the
                // sniff did not recognise, which the loader refuses either way with Java's own
                // message.
                let extension = match input.format.default_extension() {
                    "" => "dsn",
                    extension => extension,
                };
                input.set_filename(Some(&format!("board.{extension}")));
            }
            job.name = "board".to_string();
            Ok(job)
        }
    }
}

/// One optional string argument. A present-but-wrong-typed argument is a refusal rather than a
/// silent `None`: an agent that sends `{"dsn_path": 3}` has a bug, and answering "one of dsn_path
/// and dsn_text is required" would hide it.
///
/// # Errors
///
/// [`RpcError::invalid_params`] when the key is present and is not a string.
pub fn optional_string(args: &Value, key: &str) -> Result<Option<String>, RpcError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(other) => Err(RpcError::invalid_params(format!(
            "{key} must be a string, not {}",
            type_name_of(other)
        ))),
    }
}

/// The JSON type name, for a refusal message a caller can act on.
fn type_name_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// The `BoardFilePayload` members a produced file contributes to a tool result — Java's
/// spellings, from Java's own field set.
///
/// `statistics` is deliberately **not** among them even though `BoardFileDetails` carries one:
/// `route_board`'s result already has spec §13's `stats`, and two statistics objects under two
/// names in one result would be a place for a reader to pick the wrong one.
pub fn file_payload_fields(details: &fr_core::BoardFileDetails) -> Value {
    json!({
        "size": details.size,
        "crc32": details.crc32,
        "format": details.format.java_name(),
        "filename": details.get_filename(),
        "path": details.get_directory_path(),
    })
}

// =================================================================================================
// The job id (controller ruling BC)
// =================================================================================================

/// A real, per-call job id, from **std-only entropy**.
///
/// Java mints `UUID.randomUUID()` (`core/RoutingJob.java:39`). Plan 6 ruling 5 forbids a `rand`
/// dependency and the Global Constraints forbid static mutable state, so [`Uuid128::NIL`] is what
/// `RoutingJob::new` uses everywhere else in this port — and controller ruling BC says the id a
/// caller can *see* must nevertheless be a real one, drawn at this layer and passed through
/// [`RoutingJob::with_id`].
///
/// The source is `/dev/urandom` read with [`std::fs`], which is the only std-only source of real
/// entropy on this port's platforms. **When it cannot be read the answer is [`Uuid128::NIL`]**,
/// not a panic and not a clock-derived substitute: a tool that refuses to route because a device
/// node is missing would be worse than one that hands back the nil id, and a *fake* id that looks
/// random would be worse than both. Every parity comparison that sees a job id normalises it
/// (ruling BC), so neither answer can move a compared byte.
///
/// The two version/variant nibbles are set the way `UUID.randomUUID()` sets them (RFC 4122 §4.4),
/// so the string is a well-formed v4 UUID rather than 16 arbitrary bytes wearing a UUID's shape.
pub fn mint_job_id() -> Uuid128 {
    use std::io::Read;
    // `File::open` + `read_exact`, never `fs::read`: `/dev/urandom` is an endless stream, so
    // reading it to the end never returns.
    let mut bytes = [0u8; 16];
    let Ok(mut device) = std::fs::File::open("/dev/urandom") else {
        return Uuid128::NIL;
    };
    if device.read_exact(&mut bytes).is_err() {
        return Uuid128::NIL;
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // IETF variant
    Uuid128::from_bytes(bytes)
}

// =================================================================================================
// Base64 (scan ruling R12)
// =================================================================================================

/// RFC 4648 §4 standard-alphabet Base64, **with** padding — the encoding `java.util.Base64`
/// produces for `api/dto/BoardFilePayload.dataBase64` (`:19-28`,
/// `@SerializedName("data") public String dataBase64`).
///
/// # Why this is not a dependency
///
/// Scan ruling R12: `grep base64 Cargo.toml Cargo.lock crates/*/Cargo.toml` answers **nothing**,
/// and the plan's Global Constraint forbids adding a crate. Forty lines of table lookup with the
/// RFC's own test vectors is a smaller risk than a new supply-chain edge in the last plan of the
/// port. It is the visible pair of Task 4's hand-written SHA-256
/// (`fr_core::manifest::sha256_hex`), and Task 14's hand-off lists both under "deliberate
/// divergences".
///
/// Pinned by [`tests::rfc_4648_section_10_vectors`], which is the RFC's own table verbatim.
pub fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        // The three input bytes as one 24-bit big-endian group, zero-padded when the chunk is
        // short — which is what makes the last group's low bits zero, as the RFC requires.
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).copied().map_or(0, u32::from);
        let b2 = chunk.get(2).copied().map_or(0, u32::from);
        let group = (b0 << 16) | (b1 << 8) | b2;
        // Four 6-bit indices, most significant first.
        out.push(ALPHABET[((group >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((group >> 12) & 0x3f) as usize] as char);
        // `=` for each byte the chunk did not have — one for a two-byte tail, two for a one-byte
        // tail, none otherwise (RFC 4648 §4's padding rule).
        if chunk.len() > 1 {
            out.push(ALPHABET[((group >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(group & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 4648 §10's test vectors, verbatim — the whole table.
    #[test]
    fn rfc_4648_section_10_vectors() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64_encode(input.as_bytes()), expected, "input {input:?}");
        }
    }

    /// The alphabet's two awkward characters (`+` and `/`) and the whole 0..=255 byte range, so
    /// the table lookup is exercised at every index rather than only at the seven the RFC names.
    #[test]
    fn every_alphabet_index_is_reachable_and_the_length_is_the_rfcs() {
        let all: Vec<u8> = (0..=255u8).collect();
        let encoded = base64_encode(&all);
        assert_eq!(encoded.len(), 344, "4 * ceil(256 / 3)");
        assert!(encoded.ends_with("/P3+/w=="), "{encoded}");
        assert!(encoded.starts_with("AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8g"));
        // Every character is in the standard alphabet; no URL-safe `-`/`_` anywhere.
        assert!(
            encoded
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '='),
            "{encoded}"
        );
    }

    /// A UTF-8 document round-trips through the encoder at the same length a Java
    /// `Base64.getEncoder().encodeToString(s.getBytes(UTF_8))` would produce.
    #[test]
    fn a_multibyte_document_is_encoded_by_bytes_not_by_characters() {
        let text = "(session \"börd\")";
        assert_eq!(
            base64_encode(text.as_bytes()).len(),
            text.len().div_ceil(3) * 4
        );
    }
}
