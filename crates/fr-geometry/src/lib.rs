//! Planar geometry for the freerouting port. Faithful port of
//! `app.freerouting.geometry.planar` — exact integer/rational arithmetic.

pub mod bigint_aux;
pub mod int_direction;
pub mod int_vector;
pub mod limits;
pub mod side;
pub mod signum;

pub use int_direction::IntDirection;
pub use int_vector::IntVector;
pub use limits::{CRIT_INT, java_round};
pub use side::Side;
pub use signum::Signum;
