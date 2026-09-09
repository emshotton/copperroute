use copper_core::BoardSummary;

use super::OpError;
use super::load::{LoadRequest, load};

pub struct InfoRequest {
    pub load: LoadRequest,
}

pub struct InfoOutcome {
    pub summary: BoardSummary,
    pub warnings: Vec<String>,
}

pub fn info(request: &InfoRequest) -> Result<InfoOutcome, OpError> {
    let mut loaded = load(&request.load)?;
    let summary = copper_core::summarise(&mut loaded.board, loaded.metadata.as_ref());
    Ok(InfoOutcome {
        summary,
        warnings: loaded.warnings,
    })
}
