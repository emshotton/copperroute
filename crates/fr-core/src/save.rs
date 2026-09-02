//! The board **save** half of `HeadlessBoardManager` — `saveAsSpecctraSessionSes` (:862-877) and
//! the CRC32 pair `calculateCrc32` (:603-605) / `calculateCrc32ForBoard` (:607-618).
//!
//! Both take the [`CoordinateTransform`] the read produced, which is why
//! [`crate::load::LoadedBoard`] holds one (Plan 3 ruling A). Java's writers reach the transform
//! through the board; the port's take it explicitly, and only the transform
//! `Structure.createBoard` built round-trips a file's coordinates unchanged.

use std::io::Write;

use fr_board::Board;
use fr_dsn::CoordinateTransform;

use crate::{BoardFileDetails, Error};

/// Port of `HeadlessBoardManager.saveAsSpecctraSessionSes`
/// (HeadlessBoardManager.java:862-877).
///
/// Java answers `boolean` — `true` on success, `false` after logging the `IOException`
/// (`:868-872`) — and, when the write succeeded, refreshes `originalBoardChecksum` with
/// [`calculate_crc32_for_board`] (`:874-876`). The port answers `Result<(), Error>` instead: the
/// bool is the only thing a Java caller can act on and it discards the cause, while the checksum
/// refresh writes a **field of the manager object the port does not have**, so it is the caller's
/// (Task 6's `-do` path recomputes it for the result manifest).
///
/// Java does not close the stream and says so (`:857-858`); `&mut impl Write` keeps that.
//
// not ported: `originalBoardChecksum` (:155, :874-876) — a manager field whose only readers are
// `HeadlessBoardManager`'s own change detection, itself unreachable headless. The value it holds
// is [`calculate_crc32_for_board`]'s, which the caller can take when it wants it.
pub fn save_as_specctra_session_ses(
    board: &Board,
    ct: &CoordinateTransform,
    design_name: &str,
    out: &mut impl Write,
) -> Result<(), Error> {
    // :865 — `SesWriter.write(this.getRoutingBoard(), outputStream, designName)`.
    fr_dsn::ses_writer::write(board, ct, out, design_name)?;
    Ok(())
}

/// Port of `HeadlessBoardManager.calculateCrc32ForBoard`
/// (HeadlessBoardManager.java:607-618): serialise the board to DSN in memory with the design name
/// `"N/A"` and `compatMode == false`, then CRC32 the bytes.
///
/// The `"N/A"` and the `false` are Java's literals at `:610` and are load-bearing — the design
/// name reaches the `(pcb …)` header, so a different one is a different checksum.
///
/// Java's `IOException` arm (`:611-616`) logs and throws `IllegalStateException`. The port writes
/// into a `Vec<u8>`, whose `Write` impl cannot fail, so there is no arm to reproduce: the
/// `expect` below is the same "cannot happen" Java's `throw` documents.
// renamed: `HeadlessBoardManager.calculateCrc32` (:603-605) -> `calculate_crc32_for_board` — the
// public no-argument method is `calculateCrc32ForBoard(this.getRoutingBoard())` and the port has
// no `this.board`, so the two Java methods collapse into the one that takes the board.
pub fn calculate_crc32_for_board(board: &Board, ct: &CoordinateTransform) -> u32 {
    // :608-610.
    let mut memory_stream: Vec<u8> = Vec::new();
    fr_dsn::dsn_writer::write(board, ct, &mut memory_stream, "N/A", false)
        .expect("save::calculate_crc32_for_board: writing to a Vec<u8> cannot fail");
    // :617.
    BoardFileDetails::calculate_crc32(&memory_stream)
}
