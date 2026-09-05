use super::super::jsonrpc::RpcError;
use super::super::server::{ProgressWriter, State};
use fr_core::CancelToken;
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, SettingsSource};
use serde_json::{Value, json};

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
