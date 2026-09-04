#[derive(Debug, thiserror::Error)]
pub enum DrcError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
