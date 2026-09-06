pub mod drc;
pub mod info;
pub mod load;
pub mod route;
pub mod settings;

pub use drc::{DrcOutcome, DrcRequest, drc};
pub use info::{InfoRequest, info};
pub use load::{BoardSource, LoadRequest, Loaded, load};
pub use route::{OutputFormat, OutputTarget, RouteOutcome, RouteRequest, route};
pub use settings::SettingsOverrides;

#[derive(Debug, thiserror::Error)]
pub enum OpError {
    #[error("{0}")]
    Input(String),
    #[error("{0}")]
    Load(String),
    #[error("{0}")]
    Settings(String),
    #[error(transparent)]
    Router(#[from] fr_router::RouterError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<fr_core::Error> for OpError {
    fn from(error: fr_core::Error) -> Self {
        match error {
            fr_core::Error::Router(inner) => OpError::Router(inner),
            fr_core::Error::Io(inner) => OpError::Io(inner),
            other => OpError::Load(other.to_string()),
        }
    }
}
