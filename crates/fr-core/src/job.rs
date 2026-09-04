use crate::Error;
use crate::file_details::BoardFileDetails;
use crate::manifest::RouterJobResourceUsage;
use fr_settings::{DesignRulesCheckerSettings, RouterSettings};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const DSN_FILE_EXTENSION: &str = "dsn";
pub const BINARY_FILE_EXTENSION: &str = "frb";
pub const RULES_FILE_EXTENSION: &str = "rules";
pub const SES_FILE_EXTENSION: &str = "ses";
pub const EAGLE_SCRIPT_FILE_EXTENSION: &str = "scr";

pub const FILE_SEPARATOR: char = '/';


pub(crate) mod java_path {
            pub(crate) fn of_to_string(s: &str) -> String {
        let absolute = s.starts_with(super::FILE_SEPARATOR);
        let segments: Vec<&str> = s
            .split(super::FILE_SEPARATOR)
            .filter(|seg| !seg.is_empty())
            .collect();
        if segments.is_empty() {
            return if absolute {
                "/".to_string()
            } else {
                String::new()
            };
        }
        let joined = segments.join("/");
        if absolute {
            format!("/{joined}")
        } else {
            joined
        }
    }

        pub(crate) fn parent_of_normalized(normalized: &str) -> Option<String> {
        match normalized.rfind(super::FILE_SEPARATOR) {
            None => None,
            Some(0) if normalized.len() == 1 => None, 
            Some(0) => Some("/".to_string()),
            Some(i) => Some(normalized[..i].to_string()),
        }
    }

            pub(crate) fn file_name_of_normalized(normalized: &str) -> Option<String> {
        if normalized == "/" {
            return None;
        }
        if normalized.is_empty() {
            return Some(String::new());
        }
        Some(
            normalized
                .rsplit(super::FILE_SEPARATOR)
                .next()
                .unwrap_or("")
                .to_string(),
        )
    }

                pub(crate) fn to_absolute_path(s: &str) -> String {
        let normalized = of_to_string(s);
        if normalized.starts_with(super::FILE_SEPARATOR) {
            return normalized;
        }
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        if normalized.is_empty() {
            return cwd;
        }
        join2(&cwd, &normalized)
    }

        pub(crate) fn join2(first: &str, second: &str) -> String {
        if first.is_empty() {
            return of_to_string(second);
        }
        if second.is_empty() {
            return of_to_string(first);
        }
        if first.ends_with(super::FILE_SEPARATOR) {
            of_to_string(&format!("{first}{second}"))
        } else {
            of_to_string(&format!("{first}/{second}"))
        }
    }

                                pub(crate) fn split_on_dot(s: &str) -> Vec<&str> {
        if !s.contains('.') {
            return vec![s];
        }
        let mut parts: Vec<&str> = s.split('.').collect();
        while parts.last().is_some_and(|p| p.is_empty()) {
            parts.pop();
        }
        parts
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FileFormat {
        #[default]
    Unknown,
        Dsn,
        Frb,
        Ses,
        Rules,
        Scr,
        DrcJson,
        KicadDesignJson,
        KicadSessionJson,
}

const SHIFT_LOOP_BOUND: usize = 5;

impl FileFormat {
        pub fn java_name(self) -> &'static str {
        match self {
            FileFormat::Unknown => "UNKNOWN",
            FileFormat::Dsn => "DSN",
            FileFormat::Frb => "FRB",
            FileFormat::Ses => "SES",
            FileFormat::Rules => "RULES",
            FileFormat::Scr => "SCR",
            FileFormat::DrcJson => "DRC_JSON",
            FileFormat::KicadDesignJson => "KICAD_DESIGN_JSON",
            FileFormat::KicadSessionJson => "KICAD_SESSION_JSON",
        }
    }

        pub fn from_java_name(name: &str) -> Option<FileFormat> {
        Some(match name {
            "UNKNOWN" => FileFormat::Unknown,
            "DSN" => FileFormat::Dsn,
            "FRB" => FileFormat::Frb,
            "SES" => FileFormat::Ses,
            "RULES" => FileFormat::Rules,
            "SCR" => FileFormat::Scr,
            "DRC_JSON" => FileFormat::DrcJson,
            "KICAD_DESIGN_JSON" => FileFormat::KicadDesignJson,
            "KICAD_SESSION_JSON" => FileFormat::KicadSessionJson,
            _ => return None,
        })
    }

                                                                                                    pub fn sniff_bytes(content: &[u8]) -> FileFormat {
        FileFormat::sniff_bytes_inner(content).0
    }

                                            pub fn sniff_bytes_opt(content: Option<&[u8]>) -> FileFormat {
        match content {
            None => FileFormat::Unknown, 
            Some(content) => FileFormat::sniff_bytes(content),
        }
    }

                                            pub fn java_shift_loop_hangs(content: &[u8]) -> bool {
        FileFormat::sniff_bytes_inner(content).1
    }

        fn sniff_bytes_inner(content: &[u8]) -> (FileFormat, bool) {

        for &b in content {
            if b == b' ' || b == b'\t' || b == b'\r' || b == b'\n' {
                continue;
            }
            if b == b'{' {
                return (FileFormat::KicadDesignJson, false);
            }
            break;
        }

        if content.len() < 6 {
            return (FileFormat::Unknown, false);
        }
        let mut buffer = [
            content[0], content[1], content[2], content[3], content[4], content[5],
        ];

        if buffer[0] == 0xAC && buffer[1] == 0xED && buffer[2] == 0x00 && buffer[3] == 0x05 {
            return (FileFormat::Frb, false);
        }

        let mut shifts = 0usize;
        while (buffer[0] == 0x0A || buffer[0] == 0x0D) && shifts < SHIFT_LOOP_BOUND {
            buffer[0] = buffer[1];
            buffer[1] = buffer[2];
            buffer[2] = buffer[3];
            buffer[3] = buffer[4];
            buffer[4] = buffer[5];
            shifts += 1;
        }
        let hangs = shifts == SHIFT_LOOP_BOUND && (buffer[0] == 0x0A || buffer[0] == 0x0D);

        if (buffer[0] == 0x28 && buffer[1] == 0x70 && buffer[2] == 0x63 && buffer[3] == 0x62)
            || (buffer[0] == 0x28 && buffer[1] == 0x50 && buffer[2] == 0x43 && buffer[3] == 0x42)
        {
            return (FileFormat::Dsn, hangs);
        }

        if (buffer[0] == 0x28 && buffer[1] == 0x73 && buffer[2] == 0x65 && buffer[3] == 0x73)
            || (buffer[0] == 0x28 && buffer[1] == 0x53 && buffer[2] == 0x45 && buffer[3] == 0x53)
        {
            return (FileFormat::Ses, hangs);
        }

        if buffer[0] == 0x28
            && (buffer[1] == 0x72 || buffer[1] == 0x52)
            && (buffer[2] == 0x75 || buffer[2] == 0x55)
            && (buffer[3] == 0x6C || buffer[3] == 0x4C)
        {
            return (FileFormat::Rules, hangs);
        }

        (FileFormat::Unknown, hangs)
    }

                                                pub fn from_path(path: &Path) -> FileFormat {
        let filename = java_path::of_to_string(&path.to_string_lossy()).to_lowercase();
        let parts = java_path::split_on_dot(&filename);
        if parts.len() > 1 {
            let extension = parts[parts.len() - 1].to_lowercase();
            return match extension.as_str() {
                DSN_FILE_EXTENSION => FileFormat::Dsn,
                BINARY_FILE_EXTENSION => FileFormat::Frb,
                "ses" => FileFormat::Ses,
                RULES_FILE_EXTENSION => FileFormat::Rules,
                "scr" => FileFormat::Scr,
                "json" => FileFormat::KicadDesignJson,
                _ => FileFormat::Unknown,
            };
        }
        FileFormat::Unknown
    }

                        pub fn default_extension(self) -> &'static str {
        match self {
            FileFormat::Ses => "ses",
            FileFormat::Dsn => "dsn",
            FileFormat::Frb => "frb",
            FileFormat::Rules => "rules",
            FileFormat::Scr => "scr",
            _ => "",
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RoutingJobState {
        #[default]
    Invalid,
        Queued,
        ReadyToStart,
        Running,
        Paused,
        Completed,
        TimedOut,
        Stopping,
        Cancelled,
        Terminated,
}

impl RoutingJobState {
            pub fn java_name(self) -> &'static str {
        match self {
            RoutingJobState::Invalid => "INVALID",
            RoutingJobState::Queued => "QUEUED",
            RoutingJobState::ReadyToStart => "READY_TO_START",
            RoutingJobState::Running => "RUNNING",
            RoutingJobState::Paused => "PAUSED",
            RoutingJobState::Completed => "COMPLETED",
            RoutingJobState::TimedOut => "TIMED_OUT",
            RoutingJobState::Stopping => "STOPPING",
            RoutingJobState::Cancelled => "CANCELLED",
            RoutingJobState::Terminated => "TERMINATED",
        }
    }

                                                        pub fn is_cli_terminal(self) -> bool {
        matches!(
            self,
            RoutingJobState::Completed
                | RoutingJobState::Terminated
                | RoutingJobState::TimedOut
                | RoutingJobState::Cancelled
                | RoutingJobState::Invalid
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RoutingStage {
        #[default]
    Idle,
        Routing,
        Optimization,
}

impl RoutingStage {
        pub fn java_name(self) -> &'static str {
        match self {
            RoutingStage::Idle => "IDLE",
            RoutingStage::Routing => "ROUTING",
            RoutingStage::Optimization => "OPTIMIZATION",
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Uuid128([u8; 16]);

pub type JobId = Uuid128;

pub type SessionId = Uuid128;

impl Uuid128 {
        pub const NIL: Uuid128 = Uuid128([0u8; 16]);

        pub fn from_bytes(bytes: [u8; 16]) -> Uuid128 {
        Uuid128(bytes)
    }

        pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

        pub fn to_java_string(self) -> String {
        let h: Vec<String> = self.0.iter().map(|b| format!("{b:02x}")).collect();
        format!(
            "{}{}{}{}-{}{}-{}{}-{}{}-{}{}{}{}{}{}",
            h[0],
            h[1],
            h[2],
            h[3],
            h[4],
            h[5],
            h[6],
            h[7],
            h[8],
            h[9],
            h[10],
            h[11],
            h[12],
            h[13],
            h[14],
            h[15]
        )
    }

            pub fn short_upper6(self) -> String {
        self.to_java_string()
            .chars()
            .take(6)
            .collect::<String>()
            .to_uppercase()
    }
}

pub fn validate_session_host(host: Option<&str>) -> Result<String, Error> {
    let host = match host {
        Some(h) if !h.trim().is_empty() => h,
        _ => "Unknown/0.0",
    };
    let parts = if host.contains('/') {
        let mut v: Vec<&str> = host.split('/').collect();
        while v.last().is_some_and(|p| p.is_empty()) {
            v.pop();
        }
        v
    } else {
        vec![host]
    };
    if parts.len() != 2 {
        return Err(Error::Session(format!(
            "Invalid host value: '{host}'. It must contain the host name and version separated by '/'."
        )));
    }
    Ok(host.to_string())
}


#[derive(Debug, Clone)]
pub struct RoutingJob {
        pub id: JobId,
                pub created_at: Instant,
        pub short_name: String,
        pub name: String,
        pub started_at: Option<Instant>,
        pub finished_at: Option<Instant>,
        pub state: RoutingJobState,
            pub stage: RoutingStage,
        pub session_id: Option<SessionId>,
        pub input: Option<BoardFileDetails>,
        pub output: Option<BoardFileDetails>,
        pub rules: Option<BoardFileDetails>,
        pub drc: Option<BoardFileDetails>,
            pub router_settings: RouterSettings,
        pub drc_settings: DesignRulesCheckerSettings,
                        pub resource_usage: RouterJobResourceUsage,
            current_pass: i32,
                                    optimizer_pass: i32,
        is_cancelled_by_user: bool,
}

impl Default for RoutingJob {
        fn default() -> RoutingJob {
        let id = JobId::NIL;
        let short6 = id.short_upper6();
        RoutingJob {
            id,
            created_at: Instant::now(),
            short_name: short6.clone(),
            name: format!("J-{short6}"),
            started_at: None,
            finished_at: None,
            state: RoutingJobState::Invalid,
            stage: RoutingStage::Idle,
            session_id: None,
            input: None,
            output: None,
            rules: None,
            drc: None,
            router_settings: RouterSettings::new(),
            drc_settings: DesignRulesCheckerSettings::default(),
            resource_usage: RouterJobResourceUsage::default(),
            current_pass: 0,
            optimizer_pass: 0,
            is_cancelled_by_user: false,
        }
    }
}

impl RoutingJob {
                    pub fn new(session_id: SessionId) -> RoutingJob {
        RoutingJob::with_id(session_id, JobId::NIL)
    }

            pub fn with_id(session_id: SessionId, id: JobId) -> RoutingJob {
        let mut job = RoutingJob {
            id,
            ..RoutingJob::default()
        };
        let short6 = id.short_upper6();
        job.name = format!("J-{short6}");
        job.session_id = Some(session_id);
        job.short_name = format!("{}\\{}", session_id.short_upper6(), short6);
        job
    }


            pub fn get_current_pass(&self) -> i32 {
        self.current_pass
    }

            pub fn set_current_pass(&mut self, current_pass: i32) {
        self.current_pass = current_pass;
    }

            pub fn get_optimizer_pass(&self) -> i32 {
        self.optimizer_pass
    }

            pub fn set_optimizer_pass(&mut self, optimizer_pass: i32) {
        self.optimizer_pass = optimizer_pass;
    }

        pub fn is_cancelled_by_user(&self) -> bool {
        self.is_cancelled_by_user
    }

        pub fn set_cancelled_by_user(&mut self, cancelled: bool) {
        self.is_cancelled_by_user = cancelled;
    }

        pub fn get_input(&self) -> Option<&BoardFileDetails> {
        self.input.as_ref()
    }

            pub fn get_duration(&self) -> Option<Duration> {
        let started = self.started_at?;
        match self.finished_at {
            Some(finished) => Some(finished.saturating_duration_since(started)),
            None => Some(Instant::now().saturating_duration_since(started)),
        }
    }

                pub fn log_prefix(&self) -> String {
        format!("[{}] ", self.short_name)
    }


                                                pub fn set_input_bytes(&mut self, content: Option<&[u8]>) -> bool {
        self.input = Some(BoardFileDetails::default());
        self.try_to_set_input(content)
    }

        fn try_to_set_input(&mut self, content: Option<&[u8]>) -> bool {
        let Some(content) = content else {
            return false; 
        };
        let input = self
            .input
            .as_mut()
            .expect("set_input_bytes assigns `input` before calling this");
        input.format = FileFormat::sniff_bytes(content);
        if input.format != FileFormat::Unknown {
            let format = input.format;
            input.set_data(content.to_vec(), format);
            return true;
        }
        false
    }

                                                                                pub fn set_input(&mut self, input_file: &Path) -> Result<(), Error> {
        let content = std::fs::read(input_file)?;

        self.set_input_bytes(Some(&content)); 
        let absolute = java_path::to_absolute_path(&input_file.to_string_lossy());
        {
            let input = self.input.as_mut().expect("set_input_bytes assigns it");
            input.set_filename(Some(&absolute)); 
            if input.format == FileFormat::Unknown {
                input.format = FileFormat::from_path(Path::new(&input.get_absolute_path()));
            }
        }

        let input_format = self.input.as_ref().map(|i| i.format);
        let input_absolute = self
            .input
            .as_ref()
            .map(|i| i.get_absolute_path())
            .unwrap_or_default();

        let derived = match input_format {
            Some(FileFormat::Frb) => Some(BINARY_FILE_EXTENSION),
            Some(FileFormat::Dsn) => Some(SES_FILE_EXTENSION),
            Some(FileFormat::KicadDesignJson) => Some("json"),
            _ => None,
        };
        if let Some(extension) = derived {
            let mut output = BoardFileDetails::default();
            output.set_filename(Some(&RoutingJob::change_file_extension(
                &input_absolute,
                extension,
            )));
            self.output = Some(output);
        }

        if input_format != Some(FileFormat::Unknown)
            && let Some(input) = self.input.as_ref()
        {
            self.name = input.get_filename_without_extension();
        }
        Ok(())
    }


                                pub fn set_rules_bytes(&mut self, content: &[u8]) -> bool {
        let mut rules = BoardFileDetails::default();
        rules.format = FileFormat::Rules; 
        let format = FileFormat::sniff_bytes(content);
        rules.set_data(content.to_vec(), format);
        self.rules = Some(rules);
        true
    }

                                                        pub fn set_rules(&mut self, rules_file: &Path) -> Result<(), Error> {
        if !rules_file.exists() {
            return Ok(()); 
        }
        let content = std::fs::read(rules_file)?;
        let mut rules = BoardFileDetails::default();
        rules.format = FileFormat::Rules; 
        let name = rules_file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        rules.set_filename(Some(&name));
        let format = FileFormat::sniff_bytes(&content);
        rules.set_data(content, format);
        self.rules = Some(rules);
        Ok(())
    }


                                                                                                                pub fn try_to_set_output_file(&mut self, output_file: Option<&Path>) -> bool {
        let Some(output_file) = output_file else {
            return false; 
        };
        let ff = FileFormat::from_path(output_file); 
        if !matches!(
            ff,
            FileFormat::Dsn
                | FileFormat::Frb
                | FileFormat::Ses
                | FileFormat::Scr
                | FileFormat::KicadDesignJson
        ) {
            return false; 
        }
        let mut output = BoardFileDetails::from_file(output_file); 
        output.format = if ff == FileFormat::KicadDesignJson {
            FileFormat::KicadSessionJson 
        } else {
            ff
        };
        self.output = Some(output);
        true
    }

            pub fn get_rules_file(&self) -> Option<PathBuf> {
        let output = self.output.as_ref()?;
        Some(PathBuf::from(RoutingJob::change_file_extension(
            &output.get_absolute_path(),
            RULES_FILE_EXTENSION,
        )))
    }

        pub fn get_eagle_script_file(&self) -> Option<PathBuf> {
        let output = self.output.as_ref()?;
        Some(PathBuf::from(RoutingJob::change_file_extension(
            &output.get_absolute_path(),
            EAGLE_SCRIPT_FILE_EXTENSION,
        )))
    }

                            pub fn set_dummy_input_file(&mut self, filename: Option<&str>) {
        self.input = Some(BoardFileDetails::default());
        self.output = Some(BoardFileDetails::default());
        if let Some(filename) = filename
            && filename.to_lowercase().ends_with(DSN_FILE_EXTENSION)
        {
            let input = self.input.as_mut().expect("just assigned");
            input.format = FileFormat::Dsn;
            input.set_filename(Some(filename));
        }
    }


                                                                                                    pub fn change_file_extension(filename: &str, new_file_extension: &str) -> String {
        let normalized = java_path::of_to_string(filename); 

        let original_full_path_without_filename = match java_path::parent_of_normalized(&normalized)
        {
            Some(parent) => java_path::to_absolute_path(&parent),
            None => String::new(),
        };
        let original_filename = java_path::file_name_of_normalized(&normalized).unwrap_or_default();

        let name_parts = java_path::split_on_dot(&original_filename); 
        if name_parts.len() > 1 {
            let extension = name_parts[name_parts.len() - 1].to_lowercase(); 
            if extension == new_file_extension {
                return normalized; 
            }
            let keep = original_filename.len() - extension.len() - 1;
            let new_filename = format!("{}.{new_file_extension}", &original_filename[..keep]);
            return java_path::join2(&original_full_path_without_filename, &new_filename); 
        }

        java_path::join2(
            &original_full_path_without_filename,
            &format!("{original_filename}.{new_file_extension}"),
        )
    }
}

