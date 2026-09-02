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
//! | `io/kicad/KiCadJsonReader.importSession` (`:757-855`) | [`reader::import_session`] — Plan 8 Task 10 |
//! | `io/kicad/KiCadJsonWriter.java` (227) | [`writer`] — Plan 8 Task 10 |
//!
//! The audit for `io/kicad` runs **twice**, once per crate that holds part of the package. Plan 5
//! put the first against `crates/fr-drc/src` with `scripts/audit-map/fr-drc.map`, so the
//! `renamed:` markers that close `readBoard`, `addPoint`, `boundingBox` and
//! `KiCadBoardJson.Point2D` are in `crates/fr-drc/src/lib.rs` and point here. Plan 8 Task 14
//! added the second, against **this** crate with `scripts/audit-map/fr-dsn.map`, which is why the
//! two `renamed:` lines below exist: they are the mirror image of `fr-drc`'s, naming where the
//! *DRC* half of `io/kicad` went so that neither invocation can pass by falling back to the
//! crate-wide search.
//!
// renamed: KiCadDrcReport.addViolation (io/kicad/KiCadDrcReport.java:77-79) -> `fr_drc::report::KiCadDrcReport::add_violation`. Plan-5 ruling 13 puts the four `KiCadDrc*` report DTOs in `fr-drc`, beside the checker that fills them; only the board/session JSON codec is `fr-dsn`'s. `crates/fr-drc/src/report/mod.rs:91-94` is the port.
// renamed: KiCadDrcReport.addUnconnectedItem (io/kicad/KiCadDrcReport.java:82-84) -> `fr_drc::report::KiCadDrcReport::add_unconnected_item`, same split, `crates/fr-drc/src/report/mod.rs:96-99`. `KiCadDrcViolation`, `KiCadDrcViolationItem` and `KiCadDrcPosition` have no public method at all — fields and a constructor — so a map row is all they need.
//!
//! Since Task 9 the reader is live on the end-to-end load path: `fr_core::load::kicad_read_board`
//! calls [`reader::read_board`], so `-de <board>.json` loads a real board and `-do out.ses` writes
//! the SES the jar writes, byte for byte.

pub mod dto;
pub mod reader;
pub mod writer;

pub use dto::{
    ComponentJson, ConductionAreaJson, CustomClearanceRuleJson, KiCadBoardJson, LayerJson,
    NetClassJson, NetJson, OutlineJson, PadJson, Point2D, TraceJson, UnitJson, ViaJson,
};
pub use reader::{import_session, read_board};
pub use writer::{DEFAULT_DESIGN_NAME, write};
