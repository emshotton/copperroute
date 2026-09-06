#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JavaRandom {
    state: u64,
}

impl JavaRandom {
    const DOUBLE_UNIT: f64 = 1.0 / ((1_u64 << 53) as f64);

    pub fn new(seed: i64) -> JavaRandom {
        JavaRandom { state: seed as u64 }
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.state = seed as u64;
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "bound must be positive");
        let bound = bound as u64;
        let mut product = u128::from(self.next_u64()) * u128::from(bound);
        let mut low = product as u64;
        if low < bound {
            let threshold = bound.wrapping_neg() % bound;
            while low < threshold {
                product = u128::from(self.next_u64()) * u128::from(bound);
                low = product as u64;
            }
        }
        (product >> 64) as i32
    }

    pub fn next_double(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * JavaRandom::DOUBLE_UNIT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_and_set_seed_produce_the_same_stream() {
        let mut a = JavaRandom::new(99);
        let mut b = JavaRandom::new(0);
        b.set_seed(99);
        assert_eq!(a, b);
        assert_eq!(a.next_int(7), b.next_int(7));
    }

    #[test]
    fn next_double_lies_in_the_unit_interval() {
        let mut r = JavaRandom::new(12345);
        for _ in 0..1000 {
            let d = r.next_double();
            assert!((0.0..1.0).contains(&d), "{d}");
        }
    }
}
