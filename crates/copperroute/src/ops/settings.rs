use std::path::PathBuf;

use copper_board::Board;
use copper_settings::sources::{CliSettings, JsonFileSettings};
use copper_settings::{HostEnvironment, RouterSettings, SettingsInputs, SettingsSource};

use super::OpError;

#[derive(Debug, Clone, Default)]
pub struct SettingsOverrides {
    pub settings_file: Option<PathBuf>,
    pub set: Vec<String>,
    pub sparse: Option<RouterSettings>,
}

pub fn parse_set(payload: &str) -> Result<(String, String), OpError> {
    let Some((name, value)) = payload.split_once('=') else {
        return Err(OpError::Settings(format!(
            "--set {payload}: expected <section>.<field>=<value>"
        )));
    };
    if !name.starts_with("router.") {
        return Err(OpError::Settings(format!(
            "--set {payload}: the field must start with `router.` (see `copperroute route --help`)"
        )));
    }
    Ok((name.to_string(), value.to_string()))
}

pub fn resolve(
    overrides: &SettingsOverrides,
    dsn: Option<&RouterSettings>,
    explicit_rules: Option<&[u8]>,
    scheduler_rules: Option<&[u8]>,
    board: Option<&Board>,
    host: &HostEnvironment,
) -> Result<RouterSettings, OpError> {
    let json_file = match overrides.settings_file.as_deref() {
        Some(path) => {
            if let Err(error) = std::fs::metadata(path) {
                return Err(OpError::Settings(format!(
                    "--settings {}: {error}",
                    path.display()
                )));
            }
            let source = JsonFileSettings::new(path);
            if let Some(error) = source.errors().first() {
                return Err(OpError::Settings(format!(
                    "--settings {}: {error}",
                    path.display()
                )));
            }
            Some(source)
        }
        None => None,
    };

    let mut argv = Vec::with_capacity(overrides.set.len() * 2);
    for payload in &overrides.set {
        parse_set(payload)?;
        argv.push("--set".to_string());
        argv.push(payload.clone());
    }
    let cli = CliSettings::new_with_set_alias(&argv);
    if let Some(error) = cli.errors().first() {
        return Err(OpError::Settings(format!("--set: {error}")));
    }

    let inputs = SettingsInputs {
        json_file: json_file.as_ref().and_then(SettingsSource::get_settings),
        dsn,
        cli_rules: explicit_rules,
        scheduler_rules,
        env: None,
        cli: cli.get_settings(),
    };
    let mut settings = copper_settings::resolve_headless(&inputs, board, host);
    if let Some(sparse) = overrides.sparse.as_ref() {
        settings.apply_new_values_from(sparse);
        if let Some(board) = board {
            settings.apply_board_specific_optimizations(board);
        }
    }
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use copper_settings::HostEnvironment;

    #[test]
    fn parse_set_splits_at_the_first_equals_and_requires_the_router_prefix() {
        assert_eq!(
            parse_set("router.scoring.via_costs=77").unwrap(),
            ("router.scoring.via_costs".to_string(), "77".to_string())
        );
        assert_eq!(
            parse_set("router.optimizer.hybrid_ratio=1:2=x").unwrap(),
            (
                "router.optimizer.hybrid_ratio".to_string(),
                "1:2=x".to_string()
            )
        );
        assert!(parse_set("via_costs=77").is_err(), "no router. prefix");
        assert!(parse_set("router.scoring.via_costs").is_err(), "no =");
    }

    #[test]
    fn set_reaches_priority_sixty_and_an_unknown_field_is_refused() {
        let host = HostEnvironment::with_processors(4);
        let overrides = SettingsOverrides {
            settings_file: None,
            set: vec!["router.scoring.via_costs=77".to_string()],
            sparse: None,
        };
        let settings = resolve(&overrides, None, None, None, None, &host).unwrap();
        assert_eq!(settings.scoring.as_ref().unwrap().via_costs, Some(77));

        let bad = SettingsOverrides {
            settings_file: None,
            set: vec!["router.scoring.no_such_field=1".to_string()],
            sparse: None,
        };
        let error = resolve(&bad, None, None, None, None, &host).unwrap_err();
        assert!(matches!(error, OpError::Settings(_)), "{error}");
    }

    #[test]
    fn the_sparse_payload_outranks_set() {
        let host = HostEnvironment::with_processors(4);
        let mut sparse = copper_settings::RouterSettings::new();
        copper_settings::set_field_value(&mut sparse, "scoring.via_costs", "5").unwrap();
        let overrides = SettingsOverrides {
            settings_file: None,
            set: vec!["router.scoring.via_costs=77".to_string()],
            sparse: Some(sparse),
        };
        let settings = resolve(&overrides, None, None, None, None, &host).unwrap();
        assert_eq!(settings.scoring.as_ref().unwrap().via_costs, Some(5));
    }

    #[test]
    fn a_settings_file_sits_below_set() {
        let dir = std::env::temp_dir().join("fr-ops-settings");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("s.json");
        std::fs::write(
            &file,
            r#"{"router": {"scoring": {"via_costs": 33, "plane_via_costs": 9}}}"#,
        )
        .unwrap();
        let host = HostEnvironment::with_processors(4);
        let overrides = SettingsOverrides {
            settings_file: Some(file),
            set: vec!["router.scoring.via_costs=77".to_string()],
            sparse: None,
        };
        let settings = resolve(&overrides, None, None, None, None, &host).unwrap();
        let scoring = settings.scoring.as_ref().unwrap();
        assert_eq!(scoring.via_costs, Some(77));
        assert_eq!(scoring.plane_via_costs, Some(9));
    }

    #[test]
    fn a_missing_settings_file_is_refused() {
        let host = HostEnvironment::with_processors(4);
        let overrides = SettingsOverrides {
            settings_file: Some(std::path::PathBuf::from("/nonexistent/s.json")),
            set: Vec::new(),
            sparse: None,
        };
        assert!(matches!(
            resolve(&overrides, None, None, None, None, &host),
            Err(OpError::Settings(_))
        ));
    }
}
