//! `io/kicad/**`'s board-JSON codec: the DTO tree and the reader over it.
//!
//! Plan 5 ported the four `io/kicad/KiCadDrc*.java` report DTOs into `fr-drc`; this module is the
//! **board** JSON — the `.json` branch of a `-de`/`-di` load — which plan-5 ruling 13 deferred to
//! Plan 8. It lives in `fr-dsn` rather than beside the DRC DTOs because it is a board reader:
//! everything it produces (`fr_board::Board`, [`crate::BoardReadResult`],
//! [`crate::CoordinateTransform`]) is this crate's, and `fr-core`'s load path then takes DSN and
//! KiCad JSON through one signature.
//!
//! | Java | here |
//! |---|---|
//! | `io/kicad/KiCadBoardJson.java` (142) | [`dto`] |
//! | `io/kicad/KiCadJsonReader.readBoard` (`:61-755`) | [`reader::read_board`] — Plan 8 Task 8 landed sections 1-8 and the four private helpers they call, Task 9 sections 9-11 and the two only those call (`getDescriptivePadstackName`, `arePackagePinsIdentical`); **complete** |
//! | `io/kicad/KiCadJsonReader.importSession` (`:757-855`) | Plan 8 Task 10 |
//! | `io/kicad/KiCadJsonWriter.java` (227) | Plan 8 Task 10 |
//!
//! The audit for `io/kicad` runs against `crates/fr-drc/src` with
//! `scripts/audit-map/fr-drc.map` (that is where plan 5 put it), so the `renamed:` markers that
//! close `readBoard`, `addPoint`, `boundingBox` and `KiCadBoardJson.Point2D` are in
//! `crates/fr-drc/src/lib.rs` and point here. `scripts/audit-map/fr-dsn.map` records the same
//! homes so the map does not have to be re-derived when Plan 8 Task 14 re-runs the sweep.
//!
//! Since Task 9 the reader is live on the end-to-end load path: `fr_core::load::kicad_read_board`
//! calls [`reader::read_board`], so `-de <board>.json` loads a real board and `-do out.ses` writes
//! the SES the jar writes, byte for byte.

pub mod dto;
pub mod reader;

pub use dto::{
    ComponentJson, ConductionAreaJson, CustomClearanceRuleJson, KiCadBoardJson, LayerJson,
    NetClassJson, NetJson, OutlineJson, PadJson, Point2D, TraceJson, UnitJson, ViaJson,
};
pub use reader::read_board;
