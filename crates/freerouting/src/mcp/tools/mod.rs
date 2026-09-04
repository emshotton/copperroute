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
            job.set_input_bytes(Some(text.as_bytes()));
            if let Some(input) = job.input.as_mut() {
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

pub fn file_payload_fields(details: &fr_core::BoardFileDetails) -> Value {
    json!({
        "size": details.size,
        "crc32": details.crc32,
        "format": details.format.java_name(),
        "filename": details.get_filename(),
        "path": details.get_directory_path(),
    })
}

pub fn mint_job_id() -> Uuid128 {
    use std::io::Read;
    let mut bytes = [0u8; 16];
    let Ok(mut device) = std::fs::File::open("/dev/urandom") else {
        return Uuid128::NIL;
    };
    if device.read_exact(&mut bytes).is_err() {
        return Uuid128::NIL;
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid128::from_bytes(bytes)
}

pub fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).copied().map_or(0, u32::from);
        let b2 = chunk.get(2).copied().map_or(0, u32::from);
        let group = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((group >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((group >> 12) & 0x3f) as usize] as char);
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

    #[test]
    fn every_alphabet_index_is_reachable_and_the_length_is_the_rfcs() {
        let all: Vec<u8> = (0..=255u8).collect();
        let encoded = base64_encode(&all);
        assert_eq!(encoded.len(), 344, "4 * ceil(256 / 3)");
        assert!(encoded.ends_with("/P3+/w=="), "{encoded}");
        assert!(encoded.starts_with("AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8g"));
        assert!(
            encoded
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '='),
            "{encoded}"
        );
    }

    #[test]
    fn a_multibyte_document_is_encoded_by_bytes_not_by_characters() {
        let text = "(session \"börd\")";
        assert_eq!(
            base64_encode(text.as_bytes()).len(),
            text.len().div_ceil(3) * 4
        );
    }
}
