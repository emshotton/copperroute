#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    OnTheLeft,
    OnTheRight,
    Collinear,
}

impl Side {
    pub fn of_i64(value: i64) -> Side {
        match value.signum() {
            1 => Side::OnTheLeft,
            -1 => Side::OnTheRight,
            _ => Side::Collinear,
        }
    }

    pub fn of_f64(value: f64) -> Side {
        if value > 0.0 {
            Side::OnTheLeft
        } else if value < 0.0 {
            Side::OnTheRight
        } else {
            Side::Collinear
        }
    }

    pub fn negate(self) -> Side {
        match self {
            Side::OnTheLeft => Side::OnTheRight,
            Side::OnTheRight => Side::OnTheLeft,
            Side::Collinear => Side::Collinear,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn of_and_negate() {
        assert_eq!(Side::of_i64(5), Side::OnTheLeft);
        assert_eq!(Side::of_i64(-5), Side::OnTheRight);
        assert_eq!(Side::of_i64(0), Side::Collinear);
        assert_eq!(Side::OnTheLeft.negate(), Side::OnTheRight);
        assert_eq!(Side::Collinear.negate(), Side::Collinear);
        assert_eq!(Side::of_f64(-0.0), Side::Collinear);
    }
}
