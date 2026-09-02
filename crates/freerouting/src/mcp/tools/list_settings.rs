//! `list_settings` — spec §13's `{} → RouterSettings schema with defaults + descriptions`.
//!
//! The two halves come from two different places on purpose, and the split is what keeps them
//! from drifting:
//!
//! * **the schema** is [`super::schema::router_settings_schema`], hand-written, with a
//!   `description` on every property and no `default` anywhere (see that module for why
//!   `schemars` is refused and what replaces it);
//! * **the defaults** are `DefaultSettings`' own table (`settings/sources/DefaultSettings.java`),
//!   resolved **at call time**, so what a caller reads is what the next `route_board` will
//!   actually start from — including the two fields that depend on the host's processor count
//!   (plan ruling 6, `Runtime.getRuntime().availableProcessors()`).
//!
//! Copying the defaults into the schema literal would make them a second source of truth that a
//! `DefaultSettings` edit could not update, and would make the schema golden machine-dependent.

use super::super::jsonrpc::RpcError;
use super::super::server::{ProgressWriter, State};
use fr_core::CancelToken;
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, SettingsSource};
use serde_json::{Value, json};

/// Answers `{ "schema": …, "defaults": … }`.
///
/// `defaults` is rendered through [`serde_json::to_value`] rather than
/// `RouterSettings::to_json_string_pretty`: this is a *structured* result, not a Gson document,
/// and the MCP's `structuredContent` is a `Value` either way. The values are identical; only the
/// float spelling and the key order differ, and neither is a parity surface here.
///
/// # Errors
///
/// [`RpcError::internal`] if the resolved defaults will not serialize, which cannot happen —
/// `DefaultSettings` writes no non-finite float.
pub fn run(
    _state: &State,
    _args: Value,
    _progress: &ProgressWriter,
    _cancel: &CancelToken,
) -> Result<Value, RpcError> {
    let host = HostEnvironment::detect();
    let defaults = DefaultSettings::new(&host)
        .get_settings()
        .cloned()
        .expect("DefaultSettings always answers a table");
    let defaults = serde_json::to_value(&defaults).map_err(|error| {
        RpcError::internal(format!("the default settings would not serialize: {error}"))
    })?;
    Ok(json!({
        "schema": super::schema::router_settings_schema(),
        "defaults": defaults,
    }))
}
