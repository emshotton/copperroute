use std::io::Write;

use fr_board::Board;
use fr_dsn::CoordinateTransform;

use crate::{BoardFileDetails, Error};

pub fn save_as_specctra_session_ses(
    board: &Board,
    ct: &CoordinateTransform,
    design_name: &str,
    out: &mut impl Write,
) -> Result<(), Error> {
    fr_dsn::ses_writer::write(board, ct, out, design_name)?;
    Ok(())
}

pub fn calculate_crc32_for_board(board: &Board, ct: &CoordinateTransform) -> u32 {
    let mut memory_stream: Vec<u8> = Vec::new();
    fr_dsn::dsn_writer::write(board, ct, &mut memory_stream, "N/A", false)
        .expect("save::calculate_crc32_for_board: writing to a Vec<u8> cannot fail");
    BoardFileDetails::calculate_crc32(&memory_stream)
}
