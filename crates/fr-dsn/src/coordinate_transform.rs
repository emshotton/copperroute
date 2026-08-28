//! `io/CoordinateTransform.java` — board-coordinate ⇄ external-coordinate (DSN/KiCad JSON)
//! conversion.
//!
//! Placeholder: [`crate::error::BoardReadResult`]'s doc comment already promises this type
//! arrives in Plan 3 Task 5 ("this variant does not yet carry a `CoordinateTransform` field —
//! that type does not exist until Plan 3 Task 5"); [`ReadScopeParameter`](
//! crate::parser::scope_parameter::ReadScopeParameter)'s `coordinate_transform` field and
//! [`WriteScopeParameter`](crate::parser::scope_parameter::WriteScopeParameter)'s need something
//! to compile against before then. Body (`scaleFactor`, `baseX`, `baseY`,
//! `boardToDsn`/`dsnToBoard`, CoordinateTransform.java:20-100+) added in Plan 3 Task 5.
// added in Plan 3 Task 5: CoordinateTransform (boardToDsn, dsnToBoard, and the rest)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoordinateTransform;
