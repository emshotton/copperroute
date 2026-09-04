#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JavaRandom {
    seed: i64,
}

impl JavaRandom {
    const MULTIPLIER: i64 = 0x5DEECE66D_i64;
    const ADDEND: i64 = 0xB;
    const MASK: i64 = (1 << 48) - 1;
    const DOUBLE_UNIT: f64 = 1.0 / ((1_u64 << 53) as f64);

    pub fn new(seed: i64) -> JavaRandom {
        JavaRandom {
            seed: JavaRandom::initial_scramble(seed),
        }
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.seed = JavaRandom::initial_scramble(seed);
    }

    fn initial_scramble(seed: i64) -> i64 {
        (seed ^ JavaRandom::MULTIPLIER) & JavaRandom::MASK
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = self
            .seed
            .wrapping_mul(JavaRandom::MULTIPLIER)
            .wrapping_add(JavaRandom::ADDEND)
            & JavaRandom::MASK;
        (self.seed >> (48 - bits)) as i32
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "bound must be positive");
        let mut r = self.next(31);
        let m = bound - 1;
        if bound & m == 0 {
            r = ((bound as i64 * r as i64) >> 31) as i32;
        } else {
            let mut u = r;
            loop {
                r = u % bound;
                if u.wrapping_sub(r).wrapping_add(m) >= 0 {
                    break;
                }
                u = self.next(31);
            }
        }
        r
    }

    pub fn next_double(&mut self) -> f64 {
        let high = self.next(26) as i64;
        let low = self.next(27) as i64;
        (((high) << 27) + low) as f64 * JavaRandom::DOUBLE_UNIT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scramble_is_the_constructors() {
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
