use std::collections::BTreeMap;

use crate::{
    MergeError, RouterSettings, SettingsSource, SourceKind, merger::priority, set_field_value,
};

///    path — `COPPERROUTE__ROUTER__OPTIMIZER__MAX_THREADS` → `OPTIMIZER.MAX_THREADS`. Note the
#[derive(Debug, Clone)]
pub struct EnvironmentVariablesSource {
    settings: RouterSettings,
    parsed_variables: BTreeMap<String, String>,
    errors: Vec<MergeError>,
}

const ROUTER_ENV_PREFIX: &str = "COPPERROUTE__ROUTER__";

impl EnvironmentVariablesSource {
    const PRIORITY: i32 = priority::ENVIRONMENT;

    #[must_use]
    pub fn new(environment: &BTreeMap<String, String>) -> Self {
        let mut settings = RouterSettings::new();
        let mut parsed_variables = BTreeMap::new();
        let mut errors = Vec::new();

        for (raw_key, value) in environment {
            let uppercase_key = raw_key.to_uppercase();
            let Some(suffix) = uppercase_key.strip_prefix(ROUTER_ENV_PREFIX) else {
                continue;
            };
            let property_path = suffix.replace("__", ".");

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

    #[must_use]
    pub fn from_process_env() -> Self {
        let environment: BTreeMap<String, String> = std::env::vars_os()
            .filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?)))
            .collect();
        Self::new(&environment)
    }

    #[must_use]
    pub fn get_parsed_variables(&self) -> &BTreeMap<String, String> {
        &self.parsed_variables
    }

    #[must_use]
    pub fn get_parsed_count(&self) -> usize {
        self.parsed_variables.len()
    }

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
