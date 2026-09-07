#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Signum {
    Positive,
    Negative,
    Zero,
}

impl Signum {
    pub fn of_i64(value: i64) -> Signum {
        match value.signum() {
            1 => Signum::Positive,
            -1 => Signum::Negative,
            _ => Signum::Zero,
        }
    }

    pub fn of_f64(value: f64) -> Signum {
        if value > 0.0 {
            Signum::Positive
        } else if value < 0.0 {
            Signum::Negative
        } else {
            Signum::Zero
        }
    }

    pub fn as_int_i64(value: i64) -> i32 {
        value.signum() as i32
    }

    pub fn as_int_f64(value: f64) -> i32 {
        if value > 0.0 {
            1
        } else if value < 0.0 {
            -1
        } else {
            0
        }
    }

    pub fn negate(self) -> Signum {
        match self {
            Signum::Positive => Signum::Negative,
            Signum::Negative => Signum::Positive,
            Signum::Zero => Signum::Zero,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn of_and_as_int() {
        assert_eq!(Signum::of_i64(3), Signum::Positive);
        assert_eq!(Signum::of_f64(-0.5), Signum::Negative);
        assert_eq!(Signum::as_int_f64(0.0), 0);
        assert_eq!(Signum::as_int_i64(-9), -1);
        assert_eq!(Signum::Positive.negate(), Signum::Negative);
    }
}
