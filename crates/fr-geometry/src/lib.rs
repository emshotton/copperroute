//! Planar geometry for the freerouting port. Faithful port of
//! `app.freerouting.geometry.planar` — exact integer/rational arithmetic.

pub mod bigint_aux;
pub mod limits;
pub mod side;
pub mod signum;

pub use limits::{CRIT_INT, java_round};
pub use side::Side;
pub use signum::Signum;
