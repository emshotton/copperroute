#![forbid(unsafe_code)]

pub mod cancel;
pub mod ctx;
pub mod file_details;
pub mod job;
pub mod load;
pub mod manifest;
pub mod pipeline;
pub mod progress;
pub mod save;
pub mod stats_from_bytes;
pub mod stats_json;
pub mod summary;
pub mod timespan;

pub use cancel::{CancelToken, Deadline, JobStopReason};
pub use ctx::{Ctx, RoutingResult};
pub use file_details::BoardFileDetails;
pub use job::{
    BINARY_FILE_EXTENSION, DSN_FILE_EXTENSION, EAGLE_SCRIPT_FILE_EXTENSION, FILE_SEPARATOR,
    FileFormat, JobId, RULES_FILE_EXTENSION, RoutingJob, RoutingJobState, RoutingStage,
    SES_FILE_EXTENSION, SessionId, Uuid128, validate_session_host,
};
pub use load::{
    LoadedBoard, ParsedBoard, apply_immediate_post_load_processing, apply_parsed_board_result,
    apply_router_settings_for_loaded_board, load_board_if_needed, load_from_kicad_json,
    load_from_specctra_dsn, parse_board_if_needed, parse_board_result, parse_from_specctra_dsn,
};
pub use manifest::{
    FixtureInfo, PhaseDetail, PhaseMetrics, RouterJobResourceUsage, RoutingResultManifest,
    SCHEMA_VERSION, format_utc_iso8601, now_utc_iso8601, resolve_git_sha, sha256_hex,
};
pub use pipeline::RoutingPipeline;
pub use progress::{SyncProgressSink, SyncProgressSinkView};
pub use save::{calculate_crc32_for_board, save_as_specctra_session_ses};
pub use stats_from_bytes::{BoardStatisticsExt, count_occurrences};
pub use stats_json::{GsonBoardStatistics, to_gson_json, to_gson_string};
pub use summary::{
    BoardSummary, ComponentSummary, LayerSummary, NetSummary, SummaryMetadata, summarise,
};
pub use timespan::{
    MAX_TIMEOUT_SECONDS, TimespanError, convert_from_timespan_to_duration_format,
    job_timeout_deadline, job_timeout_deadline_from, parse_timespan, parse_timespan_seconds,
    parse_timespan_seconds_java,
};

pub use fr_router::pipeline::{
    NamedAlgorithmType, NoopProgressSink, PassRecord, PipelineResult, ProgressSink, RouterBudget,
    RouterCounters, RouterStop, RoutingEvent, StopRequestState, TaskState, build_unrouted_report,
    prepare_board, run_pipeline,
};
pub use fr_router::score::BoardStatistics;

pub const PARITY_VERSION: &str = "2.3.1-SNAPSHOT";

pub const PARITY_BUILD_DATE: &str = "2026-09-01";

pub const PARITY_JAR_REVISION: &str = "278fe14123c49376667239659c98d41a597acce9";

pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Router(#[from] fr_router::RouterError),

    #[error(transparent)]
    Board(#[from] fr_board::BoardError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Load(String),

    #[error("{0}")]
    Session(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parity_version_is_the_head_jars() {
        assert_eq!(PARITY_VERSION, "2.3.1-SNAPSHOT");
        assert_eq!(PARITY_BUILD_DATE, "2026-09-01");
        assert_eq!(PARITY_JAR_REVISION.len(), 40);
    }

    #[test]
    fn the_server_version_is_the_crates_own_and_not_the_jars() {
        assert_eq!(SERVER_VERSION, env!("CARGO_PKG_VERSION"));
        assert_ne!(SERVER_VERSION, PARITY_VERSION);
    }
}
