//! `java.util.Random`, reproduced bit for bit.
//!
//! The 48-bit truncated linear congruential generator, `nextInt(bound)`'s power-of-two fast path
//! and rejection loop, and `nextDouble`'s two draws. Ported from the JDK's `java.util.Random`
//! rather than from freerouting, because two freerouting call sites depend on the exact stream:
//!
//! * `PolygonShape.splitToConvexRecu` starts its concavity scan at
//!   `randomGenerator.nextInt(corners.length)` from the fixed seed 99 (PolygonShape.java:17,534),
//!   which decides how a non-convex polygon is cut up — this type lived privately in
//!   [`crate::polygon_shape`] for exactly that reason (`docs/java-quirks.md` #30);
//! * the maze router's ripup resolver draws from a `Random` seeded with `ctrl.ripupCosts`
//!   (`MazeSearchEngine.java:63,79-80`) and multiplies a detour cost by `0.5 + r*r`
//!   (`MazeRipupResolver.java:158-163`), which decides which item gets torn up.
//!
//! Plan-6 ruling 5 promoted it here, public, rather than adding a `rand` dependency: `StdRng`
//! diverges on the first draw.

/// Java's `java.util.Random`: a 48-bit truncated LCG with Java's exact seed scramble.
///
/// Not `Default`: Java's no-argument `new Random()` seeds from a nanosecond clock plus an atomic
/// seed uniquifier, which this port has no business reproducing — every freerouting use site
/// either passes a seed to the constructor or calls [`set_seed`](JavaRandom::set_seed)
/// immediately afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JavaRandom {
    /// The scrambled 48-bit state, i.e. Java's `AtomicLong seed` after `initialScramble`.
    seed: i64,
}

impl JavaRandom {
    /// `java.util.Random.multiplier`.
    const MULTIPLIER: i64 = 0x5DEECE66D_i64;
    /// `java.util.Random.addend`.
    const ADDEND: i64 = 0xB;
    /// `java.util.Random.mask`, i.e. `(1L << 48) - 1`.
    const MASK: i64 = (1 << 48) - 1;
    /// `java.util.Random.DOUBLE_UNIT`, i.e. `0x1.0p-53`. Rust has no hex float literals, and
    /// `2^53` is exactly representable, so the reciprocal is exact.
    const DOUBLE_UNIT: f64 = 1.0 / ((1_u64 << 53) as f64);

    /// `new java.util.Random(long seed)`: `this.seed = initialScramble(seed)`.
    pub fn new(seed: i64) -> JavaRandom {
        JavaRandom {
            seed: JavaRandom::initial_scramble(seed),
        }
    }

    /// `java.util.Random.setSeed(long)` — the same scramble as the constructor.
    ///
    /// not ported: the `haveNextNextGaussian = false` reset that `setSeed` also does; this port
    /// has no `nextGaussian`.
    pub fn set_seed(&mut self, seed: i64) {
        self.seed = JavaRandom::initial_scramble(seed);
    }

    /// `java.util.Random.initialScramble`: `(seed ^ multiplier) & mask`.
    fn initial_scramble(seed: i64) -> i64 {
        (seed ^ JavaRandom::MULTIPLIER) & JavaRandom::MASK
    }

    /// `java.util.Random.next(int bits)`, the LCG step every public draw is built from.
    fn next(&mut self, bits: u32) -> i32 {
        self.seed = self
            .seed
            .wrapping_mul(JavaRandom::MULTIPLIER)
            .wrapping_add(JavaRandom::ADDEND)
            & JavaRandom::MASK;
        (self.seed >> (48 - bits)) as i32
    }

    /// `java.util.Random.nextInt(int bound)`, both branches: the power-of-two fast path and the
    /// rejection loop that keeps the distribution uniform.
    ///
    /// # Panics
    /// For `bound <= 0`, where Java throws `IllegalArgumentException("bound must be positive")`.
    /// `PolygonShape` only ever passes `corners.length`, which is non-zero for every
    /// constructible shape.
    pub fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "bound must be positive");
        let mut r = self.next(31);
        let m = bound - 1;
        if bound & m == 0 {
            // bound is a power of 2
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

    /// `java.util.Random.nextDouble()`: `(((long) next(26) << 27) + next(27)) * 0x1.0p-53`.
    ///
    /// The two draws are sequenced explicitly — Java evaluates `next(26)` before `next(27)`, and
    /// a single expression would leave the order to Rust.
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
