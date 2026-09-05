#[derive(Debug, thiserror::Error)]
pub enum DrcError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("KiCad project: {0}")]
    Project(String),
}
