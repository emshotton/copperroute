//! Rust twin of `scripts/differential/java/P3T2.java` (Plan 3 Task 2).
//!
//! Walks the same seeded pseudo-random doubles through `copper_dsn::format::double`'s
//! `format_double` / `format_float` / `format_placement_rotation` that the Java
//! driver walks through `Double.toString` / `Float.toString` /
//! `SesWriter.formatPlacementRotation`. See `scripts/differential/README.md`.

use std::io::{BufWriter, Write};

use copper_dsn::format::double::{
    format_placement_rotation, format_double, format_float,
};

/// `java.util.Random`, duplicated here (the copies inside `copper-geometry`/`copper-board` are private)
/// exactly as `p2t13.rs` duplicates it.
struct JavaRandom {
    seed: i64,
}

impl JavaRandom {
    const MULTIPLIER: i64 = 0x5DEECE66D_i64;
    const ADDEND: i64 = 0xB;
    const MASK: i64 = (1 << 48) - 1;

    fn new(seed: i64) -> JavaRandom {
        JavaRandom {
            seed: (seed ^ JavaRandom::MULTIPLIER) & JavaRandom::MASK,
        }
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = self
            .seed
            .wrapping_mul(JavaRandom::MULTIPLIER)
            .wrapping_add(JavaRandom::ADDEND)
            & JavaRandom::MASK;
        (self.seed >> (48 - bits)) as i32
    }

    /// `nextLong()`: two 32-bit draws, the second one *sign-extended* before the addition.
    fn next_long(&mut self) -> i64 {
        let high = i64::from(self.next(32)) << 32;
        high.wrapping_add(i64::from(self.next(32)))
    }

    fn next_int(&mut self, bound: i32) -> i32 {
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
}

/// `10^k` for `k` in 0..6, as literals — matching `P3T2.POW10`.
const POW10: [f64; 7] = [1.0, 10.0, 100.0, 1000.0, 10000.0, 100000.0, 1000000.0];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("usage: p3t2 <count> <seed> <mode>");
        std::process::exit(1);
    }
    let count: u64 = args[1].parse().expect("count");
    let seed: i64 = args[2].parse().expect("seed");
    let mode: i32 = args[3].parse().expect("mode");

    let mut random = JavaRandom::new(seed);
    let stdout = std::io::stdout();
    let mut out = BufWriter::with_capacity(1 << 20, stdout.lock());

    let mut emitted = 0u64;
    while emitted < count {
        let value = match mode {
            0 => {
                let candidate = f64::from_bits(random.next_long() as u64);
                if candidate.is_nan() || candidate.is_infinite() {
                    continue;
                }
                candidate
            }
            1 => {
                // Java evaluates the left operand first: `nextInt(2000001)` then `nextInt(7)`.
                let numerator = random.next_int(2000001) - 1000000;
                f64::from(numerator) / POW10[random.next_int(7) as usize]
            }
            2 => f64::from(random.next_int(20000001) - 10000000),
            3 => f64::from(random.next_int(360000)) / 1000.0,
            other => panic!("unknown mode {other}"),
        };
        let bits = value.to_bits();
        if mode == 0 {
            writeln!(
                out,
                "{:x} {} {}",
                bits,
                format_double(value),
                format_float(value as f32)
            )
            .expect("write");
        } else {
            writeln!(
                out,
                "{:x} {} {} {}",
                bits,
                format_double(value),
                format_float(value as f32),
                format_placement_rotation(value)
            )
            .expect("write");
        }
        emitted += 1;
    }
    out.flush().expect("flush");
}
