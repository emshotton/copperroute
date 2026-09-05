use fr_geometry::JavaRandom;

const SEED_42_DOUBLES: [f64; 5] = [
    0.7275636800328681,
    0.6832234717598454,
    0.30871945533265976,
    0.27707849007413665,
    0.6655489517945736,
];

const SEED_1000_DOUBLES: [f64; 5] = [
    0.7101849056320707,
    0.574836350385667,
    0.9464192094792073,
    0.039405954311386604,
    0.4864098780914311,
];

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
    let mut r = JavaRandom::new(42);
    let got: Vec<i32> = (0..5).map(|_| r.next_int(8)).collect();
    assert_eq!(got, vec![5, 0, 5, 0, 2]);
}

#[test]
#[should_panic(expected = "bound must be positive")]
fn next_int_rejects_a_non_positive_bound_where_java_throws() {
    JavaRandom::new(1).next_int(0);
}
