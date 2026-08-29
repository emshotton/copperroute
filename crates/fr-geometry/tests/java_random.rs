//! `JavaRandom` against a real JVM.
//!
//! Every literal below was produced by JDK 25 and pasted in verbatim. The command was
//!
//! ```text
//! /opt/homebrew/opt/openjdk@25/bin/jshell -q -R-Djava.awt.headless=true <<'EOF'
//! var r = new java.util.Random(42L);
//! for (int i=0;i<5;i++) { double d = r.nextDouble(); System.out.println(Double.toHexString(d) + " " + d); }
//! var r2 = new java.util.Random(42L);
//! for (int i=0;i<5;i++) System.out.println(r2.nextInt(10));
//! var r2b = new java.util.Random(42L);
//! for (int i=0;i<5;i++) System.out.println(r2b.nextInt(8));
//! var r3 = new java.util.Random(1000L);
//! for (int i=0;i<5;i++) { double d = r3.nextDouble(); System.out.println(Double.toHexString(d) + " " + d); }
//! var r4 = new java.util.Random(); r4.setSeed(1000L);
//! for (int i=0;i<5;i++) System.out.println(Double.toHexString(r4.nextDouble()));
//! /exit
//! EOF
//! ```
//!
//! `JavaRandom` was private inside `polygon_shape.rs` until plan-6 Task 1 promoted it (plan-6
//! ruling 5); `polygon_shape.rs`'s own tests are the guard that the promotion was verbatim and
//! are deliberately not touched here.

use fr_geometry::JavaRandom;

/// `new java.util.Random(42L).nextDouble()` ×5.
const SEED_42_DOUBLES: [f64; 5] = [
    0.7275636800328681,  // 0x1.74833a06ff457p-1
    0.6832234717598454,  // 0x1.5dcf778622e01p-1
    0.30871945533265976, // 0x1.3c20f3f12bbb4p-2
    0.27707849007413665, // 0x1.1bba76b52c856p-2
    0.6655489517945736,  // 0x1.54c2d50bb0864p-1
];

/// `new java.util.Random(1000L).nextDouble()` ×5.
const SEED_1000_DOUBLES: [f64; 5] = [
    0.7101849056320707,   // 0x1.6b9d5b1f9aed1p-1
    0.574836350385667,    // 0x1.2650f33aeab84p-1
    0.9464192094792073,   // 0x1.e4910f0209eabp-1
    0.039405954311386604, // 0x1.42d046a11f57p-5
    0.4864098780914311,   // 0x1.f2156e5b6a8a6p-2
];

/// `new java.util.Random(42L).nextInt(10)` ×5.
const SEED_42_INTS: [i32; 5] = [0, 3, 8, 4, 0];

#[test]
fn next_double_matches_the_jvm_bit_for_bit() {
    let mut r = JavaRandom::new(42);
    for (i, expected) in SEED_42_DOUBLES.iter().enumerate() {
        let got = r.next_double();
        assert_eq!(
            got.to_bits(),
            expected.to_bits(),
            "draw {i}: got {got}, JVM said {expected}"
        );
    }

    let mut r = JavaRandom::new(1000);
    for (i, expected) in SEED_1000_DOUBLES.iter().enumerate() {
        let got = r.next_double();
        assert_eq!(
            got.to_bits(),
            expected.to_bits(),
            "draw {i}: got {got}, JVM said {expected}"
        );
    }
}

#[test]
fn next_int_matches_the_jvm() {
    let mut r = JavaRandom::new(42);
    let got: Vec<i32> = (0..5).map(|_| r.next_int(10)).collect();
    assert_eq!(got, SEED_42_INTS);
}

#[test]
fn set_seed_scrambles_exactly_like_the_constructor() {
    // `new Random(); setSeed(1000L)` is `MazeSearchEngine.java:63,79-80`'s shape, and the JVM
    // gives it the same stream as `new Random(1000L)`.
    let mut constructed = JavaRandom::new(1000);
    let mut reseeded = JavaRandom::new(7);
    reseeded.set_seed(1000);
    for (i, expected) in SEED_1000_DOUBLES.iter().enumerate() {
        assert_eq!(reseeded.next_double().to_bits(), expected.to_bits(), "{i}");
        assert_eq!(
            constructed.next_double().to_bits(),
            expected.to_bits(),
            "{i}"
        );
    }
}

#[test]
fn set_seed_resets_a_partly_drawn_generator() {
    let mut r = JavaRandom::new(1000);
    r.next_double();
    r.next_int(10);
    r.set_seed(1000);
    assert_eq!(r.next_double().to_bits(), SEED_1000_DOUBLES[0].to_bits());
}

#[test]
fn next_int_takes_the_power_of_two_fast_path() {
    // `nextInt` has two branches (`bound & (bound - 1) == 0` and the rejection loop); the seed-42
    // ints above exercise the loop, this one the fast path. Values from the same JVM session.
    let mut r = JavaRandom::new(42);
    let got: Vec<i32> = (0..5).map(|_| r.next_int(8)).collect();
    // `new java.util.Random(42L).nextInt(8)` x5
    assert_eq!(got, vec![5, 0, 5, 0, 2]);
}

#[test]
#[should_panic(expected = "bound must be positive")]
fn next_int_rejects_a_non_positive_bound_where_java_throws() {
    JavaRandom::new(1).next_int(0);
}
