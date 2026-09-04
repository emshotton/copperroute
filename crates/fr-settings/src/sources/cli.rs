//! | target | `RouterSettings` — reaches the router | the `@Deprecated` bridge — reaches nothing |
use std::collections::BTreeMap;

use crate::field_path::{java_parse_f32, java_split, java_trim};
use crate::{
    BoardUpdateStrategy, ItemSelectionStrategy, MergeError, RouterSettings, SettingsSource,
    SourceKind, merger::priority, set_field_value,
};

#[derive(Debug, Clone)]
pub struct CliSettings {
    settings: RouterSettings,
    parsed_arguments: BTreeMap<String, String>,
    errors: Vec<MergeError>,
}

impl CliSettings {
    const PRIORITY: i32 = priority::CLI;

    #[must_use]
    pub fn new(args: &[String]) -> Self {
        Self::parse(args, false)
    }

    #[must_use]
    pub fn new_with_set_alias(args: &[String]) -> Self {
        Self::parse(args, true)
    }

    fn parse(args: &[String], set_alias: bool) -> Self {
        let mut this = Self {
            settings: RouterSettings::new(),
            parsed_arguments: BTreeMap::new(),
            errors: Vec::new(),
        };

        let mut has_design_input_argument = false;
        let mut has_design_output_argument = false;
        let mut has_explicit_router_enabled_argument = false;

        let mut i = 0;
        while i < args.len() {
            let arg = args[i].as_str();

            if let Some(body) = arg.strip_prefix("--") {
                if set_alias && (body == "set" || body.starts_with("set=")) {
                    let payload = match body.strip_prefix("set=") {
                        Some(rest) => Some(rest.to_string()),
                        None => args.get(i + 1).map(|next| {
                            i += 1;
                            next.clone()
                        }),
                    };
                    if let Some(payload) = payload
                        && let Some((property_name, value)) = payload.split_once('=')
                    {
                        if property_name == "router.enabled" {
                            has_explicit_router_enabled_argument = true;
                        }
                        if property_name.starts_with("router.") {
                            this.apply_router_setting(property_name, value);
                        }
                    }
                } else if body.contains('=') {
                    let (property_name, value) = body.split_once('=').expect("contains checked");

                    if property_name == "router.enabled" {
                        has_explicit_router_enabled_argument = true;
                    }

                    if property_name.starts_with("router.") {
                        this.apply_router_setting(property_name, value);
                    }
                }
            } else if let Some(flag) = arg.strip_prefix('-') {
                let value = match args.get(i + 1) {
                    Some(next) if !next.starts_with('-') => {
                        i += 1;
                        next.clone()
                    }
                    _ => String::new(),
                };

                if flag == "de" {
                    has_design_input_argument = true;
                } else if flag == "do" {
                    has_design_output_argument = true;
                }

                if let Some(property_name) = map_flag_to_property(flag)
                    && property_name.starts_with("router.")
                {
                    this.apply_router_setting(property_name, &value);
                }
            }

            i += 1;
        }

        if has_design_input_argument
            && has_design_output_argument
            && !has_explicit_router_enabled_argument
        {
            this.settings.enabled = Some(true);
        }

        this
    }

    fn apply_router_setting(&mut self, property_name: &str, value: &str) {
        let field_path = property_name
            .strip_prefix("router.")
            .unwrap_or(property_name);

        match set_field_value(&mut self.settings, field_path, value) {
            Ok(()) => {
                self.parsed_arguments
                    .insert(property_name.to_string(), value.to_string());
            }
            Err(error) => self.errors.push(error),
        }
    }

    #[must_use]
    pub fn get_parsed_arguments(&self) -> &BTreeMap<String, String> {
        &self.parsed_arguments
    }

    #[must_use]
    pub fn errors(&self) -> &[MergeError] {
        &self.errors
    }
}

impl SettingsSource for CliSettings {
    fn get_settings(&self) -> Option<&RouterSettings> {
        Some(&self.settings)
    }

    fn get_source_name(&self) -> String {
        "CLI Arguments".to_string()
    }

    fn get_priority(&self) -> i32 {
        Self::PRIORITY
    }

    fn kind(&self) -> SourceKind {
        SourceKind::Cli
    }
}

fn map_flag_to_property(flag: &str) -> Option<&'static str> {
    match flag {
        "mp" => Some("router.max_passes"),
        "mt" => Some("router.max_threads"),
        _ => None,
    }
}

/// What `GlobalSettings.applyCommandLineArguments` writes into the `@Deprecated public final
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LegacyBridge {
    pub max_passes: Option<i32>,

    pub optimizer_max_threads: Option<i32>,

    pub optimization_improvement_threshold: Option<f32>,

    pub board_update_strategy: Option<BoardUpdateStrategy>,

    pub item_selection_strategy: Option<ItemSelectionStrategy>,

    pub hybrid_ratio: Option<String>,

    pub ignore_net_classes: Option<Vec<String>>,

    pub router_enabled: Option<bool>,

    pub drc_enabled: Option<bool>,
}

impl LegacyBridge {
    fn absorb_router_settings(&mut self, scratch: &RouterSettings) {
        if scratch.enabled.is_some() {
            self.router_enabled = scratch.enabled;
        }
        if scratch.max_passes.is_some() {
            self.max_passes = scratch.max_passes;
        }
        if scratch.ignore_net_classes.is_some() {
            self.ignore_net_classes = scratch.ignore_net_classes.clone();
        }
        let Some(optimizer) = scratch.optimizer.as_ref() else {
            return;
        };
        if optimizer.max_threads.is_some() {
            self.optimizer_max_threads = optimizer.max_threads;
        }
        if optimizer.optimization_improvement_threshold.is_some() {
            self.optimization_improvement_threshold = optimizer.optimization_improvement_threshold;
        }
        if optimizer.board_update_strategy.is_some() {
            self.board_update_strategy = optimizer.board_update_strategy;
        }
        if optimizer.item_selection_strategy.is_some() {
            self.item_selection_strategy = optimizer.item_selection_strategy;
        }
        if optimizer.hybrid_ratio.is_some() {
            self.hybrid_ratio = optimizer.hybrid_ratio.clone();
        }
    }
}

#[must_use]
pub fn apply_command_line_arguments(args: &[String]) -> LegacyBridge {
    let mut bridge = LegacyBridge::default();

    let value_of = |i: usize| -> Option<&str> {
        match args.get(i + 1) {
            Some(next) if !next.starts_with('-') => Some(next.as_str()),
            _ => None,
        }
    };

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();

        if arg.eq_ignore_ascii_case("-help")
            || arg.eq_ignore_ascii_case("--help")
            || arg.eq_ignore_ascii_case("-h")
        {
            i += 1;
            continue;
        }

        if let Some(body) = arg.strip_prefix("--") {
            if let Some((property_name, value)) = body.split_once('=')
                && property_name != "user_data_path"
                && let Some(field_path) = property_name.strip_prefix("router.")
            {
                let mut scratch = RouterSettings::new();
                if set_field_value(&mut scratch, field_path, value).is_ok() {
                    bridge.absorb_router_settings(&scratch);
                }
            }
            i += 1;
            continue;
        }

        if arg.starts_with("-de") {
            if value_of(i).is_some() {
                let mut j = i + 1;
                while j < args.len() && !args[j].starts_with('-') {
                    j += 1;
                }
                i = j;
                continue;
            }
        } else if arg.starts_with("-di") || arg.starts_with("-do") {
            if value_of(i).is_some() {
                i += 1;
            }
        } else if arg.starts_with("-drc") {
            bridge.router_enabled = Some(false);
            bridge.drc_enabled = Some(true);
            if value_of(i).is_some() {
                i += 1;
            }
        } else if arg.starts_with("-dr") {
            if value_of(i).is_some() {
                i += 1;
            }
        } else if arg.starts_with("-mp") {
            if let Some(value) = value_of(i)
                && let Some(decoded) = java_integer_decode(value)
            {
                bridge.max_passes = Some(if decoded < 0 { 1 } else { decoded.min(9999) });
                i += 1;
            }
        } else if arg.starts_with("-mt") {
            if let Some(value) = value_of(i)
                && let Some(decoded) = java_integer_decode(value)
            {
                bridge.optimizer_max_threads = Some(decoded.clamp(0, 1024));
                i += 1;
            }
        } else if arg.starts_with("-oit") {
            if let Some(value) = value_of(i)
                && let Ok(percent) =
                    java_parse_f32(value, "optimizer.optimization_improvement_threshold")
            {
                let threshold = percent / 100.0f32;
                bridge.optimization_improvement_threshold =
                    Some(if threshold <= 0.0 { 0.0f32 } else { threshold });
                i += 1;
            }
        } else if arg.starts_with("-us") {
            if let Some(value) = value_of(i) {
                let op = java_trim(&value.to_lowercase()).to_string();
                bridge.board_update_strategy = Some(match op.as_str() {
                    "global" => BoardUpdateStrategy::GlobalOptimal,
                    "hybrid" => BoardUpdateStrategy::Hybrid,
                    _ => BoardUpdateStrategy::Greedy,
                });
                i += 1;
            }
        } else if arg.starts_with("-is") {
            if let Some(value) = value_of(i) {
                let op = java_trim(&value.to_lowercase()).to_string();
                bridge.item_selection_strategy = Some(if op.starts_with("seq") {
                    ItemSelectionStrategy::Sequential
                } else if op.starts_with("rand") {
                    ItemSelectionStrategy::Random
                } else {
                    ItemSelectionStrategy::Prioritized
                });
                i += 1;
            }
        } else if arg.starts_with("-hr") {
            if let Some(value) = value_of(i) {
                bridge.hybrid_ratio = Some(java_trim(value).to_string());
                i += 1;
            }
        } else if arg == "-l" {
            if value_of(i).is_some() {
                i += 1;
            }
        } else if arg.starts_with("-dl") || arg.starts_with("-da") || arg.starts_with("-help") {
        } else if arg.starts_with("-host") {
            if value_of(i).is_some() {
                i += 1;
            }
        } else if arg.starts_with("-inc") {
            if let Some(value) = value_of(i) {
                bridge.ignore_net_classes = Some(
                    java_split(value, |c| c == ',')
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                );
                i += 1;
            }
        } else if arg.starts_with("-dct") || arg.starts_with("-ll") {
            if value_of(i).is_some() {
                i += 1;
            }
        }

        i += 1;
    }

    bridge
}

#[must_use]
pub fn legacy_flag_value_is_consumed(arg: &str, value: &str) -> bool {
    if arg.starts_with("-de")
        || arg.starts_with("-di")
        || arg.starts_with("-do")
        || arg.starts_with("-drc")
        || arg.starts_with("-dr")
    {
        true
    } else if arg.starts_with("-mp") || arg.starts_with("-mt") {
        java_integer_decode(value).is_some()
    } else if arg.starts_with("-oit") {
        java_parse_f32(value, "optimizer.optimization_improvement_threshold").is_ok()
    } else if arg.starts_with("-dct") {
        crate::field_path::java_parse_i32(value, "gui.dialog_confirmation_timeout").is_ok()
    } else {
        true
    }
}

fn java_integer_decode(nm: &str) -> Option<i32> {
    if nm.is_empty() {
        return None;
    }

    let bytes = nm.as_bytes();
    let mut index = 0usize;
    let mut negative = false;
    if bytes[0] == b'-' {
        negative = true;
        index = 1;
    } else if bytes[0] == b'+' {
        index = 1;
    }

    let rest = nm.get(index..)?;
    let radix = if rest.starts_with("0x") || rest.starts_with("0X") {
        index += 2;
        16
    } else if rest.starts_with('#') {
        index += 1;
        16
    } else if rest.starts_with('0') && nm.len() > index + 1 {
        index += 1;
        8
    } else {
        10
    };

    let digits = nm.get(index..)?;
    if digits.starts_with('-') || digits.starts_with('+') {
        return None;
    }

    match i32::from_str_radix(digits, radix) {
        Ok(value) => Some(if negative { -value } else { value }),
        Err(_) if negative => i32::from_str_radix(&format!("-{digits}"), radix).ok(),
        Err(_) => None,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeSlots {
    pub initial_input_file: Option<String>,
    pub design_session_filename: Option<String>,
    pub initial_rules_file: Option<String>,
}

#[must_use]
pub fn classify_de_arguments(args: &[String]) -> DeSlots {
    classify_de_arguments_reporting(args).0
}

#[must_use]
pub fn classify_de_arguments_reporting(args: &[String]) -> (DeSlots, Vec<String>) {
    let mut slots = DeSlots::default();
    let mut warnings: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg.starts_with("--") || !arg.starts_with("-de") {
            i += 1;
            continue;
        }

        if args.get(i + 1).is_none_or(|next| next.starts_with('-')) {
            i += 1;
            continue;
        }

        let mut files: Vec<String> = Vec::new();
        let mut j = i + 1;
        while j < args.len() && !args[j].starts_with('-') {
            let raw_arg = java_trim(&args[j]).to_string();
            if std::path::Path::new(&raw_arg).exists() {
                files.push(raw_arg);
            } else if raw_arg.contains('+') {
                for part in java_split(&raw_arg, |c| c == '+') {
                    let part = java_trim(part);
                    if !part.is_empty() {
                        files.push(part.to_string());
                    }
                }
            } else {
                files.push(raw_arg);
            }
            j += 1;
        }

        let mut has_dsn = false;
        let mut has_ses = false;
        let mut has_rules = false;
        for file in files {
            let file = java_trim(&file).to_string();
            if file.is_empty() {
                continue;
            }
            let lower = file.to_lowercase();
            if lower.ends_with(".dsn") {
                if has_dsn {
                    warnings.push(MULTIPLE_DSN_FILES.to_string());
                }
                slots.initial_input_file = Some(file);
                has_dsn = true;
            } else if lower.ends_with(".json") {
                if has_dsn {
                    if has_ses {
                        warnings.push(MULTIPLE_SESSION_FILES.to_string());
                    }
                    slots.design_session_filename = Some(file);
                    has_ses = true;
                } else {
                    slots.initial_input_file = Some(file);
                    has_dsn = true;
                }
            } else if lower.ends_with(".ses") {
                if has_ses {
                    warnings.push(MULTIPLE_SES_FILES.to_string());
                }
                slots.design_session_filename = Some(file);
                has_ses = true;
            } else if lower.ends_with(".rules") {
                if has_rules {
                    warnings.push(MULTIPLE_RULES_FILES.to_string());
                }
                slots.initial_rules_file = Some(file);
                has_rules = true;
            } else {
                warnings.push(format!(
                    "{UNKNOWN_FILE_TYPE_PREFIX}{file}{UNKNOWN_FILE_TYPE_SUFFIX}"
                ));
            }
        }

        i = j;
    }

    (slots, warnings)
}

pub const MULTIPLE_DSN_FILES: &str =
    "Multiple DSN files provided in -de argument. Only the last one will be used.";
pub const MULTIPLE_SESSION_FILES: &str =
    "Multiple session files (SES/JSON) provided in -de argument. Only the last one will be used.";
pub const MULTIPLE_SES_FILES: &str =
    "Multiple SES files provided in -de argument. Only the last one will be used.";
pub const MULTIPLE_RULES_FILES: &str =
    "Multiple RULES files provided in -de argument. Only the last one will be used.";
pub const UNKNOWN_FILE_TYPE_PREFIX: &str = "Unknown file type in -de argument: ";
pub const UNKNOWN_FILE_TYPE_SUFFIX: &str = ". Expected .dsn, .json, .ses, or .rules";
pub const UNKNOWN_COMMAND_LINE_ARGUMENT_PREFIX: &str = "Unknown command line argument: ";
pub const UNKNOWN_SETTINGS_PROPERTY_PREFIX: &str = "Unknown settings property: ";

// `@Deprecated public final RouterSettings routerSettings` / `guiSettings` that no routing path
