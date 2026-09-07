use copper_core::BoardSummary;

use super::OpError;
use super::load::{LoadRequest, load};

pub struct InfoRequest {
    pub load: LoadRequest,
}

pub fn info(request: &InfoRequest) -> Result<BoardSummary, OpError> {
    let mut loaded = load(&request.load)?;
    Ok(copper_core::summarise(
        &mut loaded.board,
        loaded.metadata.as_ref(),
    ))
}
