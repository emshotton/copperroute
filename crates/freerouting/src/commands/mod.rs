//! The four subcommand runners.
//!
//! Each takes its parsed arguments **and the raw argv**, because the settings ladder is built
//! from the raw argv rather than the rewritten one (scan ruling R19 — see `crate::run`).

pub mod drc;
pub mod info;
pub mod route;
