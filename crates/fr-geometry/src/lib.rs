//! Planar geometry for the freerouting port. Faithful port of
//! `app.freerouting.geometry.planar` — exact integer/rational arithmetic.

pub mod bigint_aux;
pub mod bigint_direction;
pub mod direction;
pub mod float_line;
pub mod float_point;
pub mod int_box;
pub mod int_direction;
pub mod int_octagon;
pub mod int_point;
pub mod int_vector;
pub mod limits;
pub mod line;
pub mod point;
pub mod rational_point;
pub mod rational_vector;
pub mod side;
pub mod signum;
pub mod simplex;
pub mod vector;

pub use bigint_direction::BigIntDirection;
pub use direction::Direction;
pub use float_line::FloatLine;
pub use float_point::FloatPoint;
pub use int_box::IntBox;
pub use int_direction::IntDirection;
pub use int_octagon::IntOctagon;
pub use int_point::IntPoint;
pub use int_vector::IntVector;
pub use limits::{CRIT_INT, java_round};
pub use line::Line;
pub use point::Point;
pub use rational_point::RationalPoint;
pub use rational_vector::RationalVector;
pub use side::Side;
pub use signum::Signum;
pub use simplex::Simplex;
pub use vector::Vector;
