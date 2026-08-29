//! Java's number-to-text primitives, reproduced exactly.
//!
//! Every DSN/SES writer eventually funnels a coordinate, a width or a rotation through one of
//! these. Java reaches them as `String.valueOf(double)` (io/specctra/parser/Wiring.java:97,
//! PolygonPath.java:27,31,33, Rectangle.java:76-82, Circle.java, Polygon.java),
//! `String.valueOf((float) …)` (io/specctra/parser/AutorouteSettings.java:228,234) and
//! `SesWriter.formatPlacementRotation` (io/specctra/SesWriter.java:259-269). Rust's own `{}` and
//! `{:.N}` agree with none of them, so the port cannot use them anywhere on a write path.
//!
//! The three algorithms and where they differ from Rust:
//!
//! * **`Double.toString` / `Float.toString`** (JDK 19+ spec). Both pick the shortest decimal that
//!   round-trips, *except* that when the shortest has a single significant digit the candidate set
//!   widens to lengths 1 **and** 2 and the decimal closest to the actual value wins. That rule is
//!   invisible for normal doubles (the closest two-digit decimal is then just the one-digit one
//!   with a trailing zero) and decisive in the subnormal tail, where `Double.MIN_VALUE` prints
//!   `4.9E-324` rather than Rust's shortest `5e-324`. The plain-vs-scientific switch is Java's,
//!   not Rust's: `10^-3 <= |d| < 10^7` prints plain, everything else prints `D.DDDEnn`.
//! * **`Math.rint`** — ties to even. `f64::round_ties_even` matches; [`java_rint`] exists so the
//!   Java name is greppable.
//! * **`String.format(Locale.ENGLISH, "%.Nf", d)`** — `java.util.Formatter` takes the *shortest
//!   round-trip digits* (the very digits `Double.toString` prints) and rounds **those** `HALF_UP`.
//!   It never looks at the double's exact binary expansion. Rust's `{:.N}` differs on both counts:
//!   it rounds the exact expansion, half-to-even. See [`java_format_fixed`].

pub use fr_geometry::java_round; // Math.round

/// Java `Math.rint(double)`: the integral value nearest `x`, ties to even, sign of zero preserved.
///
/// `2.5 -> 2.0`, `-2.5 -> -2.0`, `0.5 -> 0.0`, `-0.5 -> -0.0`, `1.5 -> 2.0`. Rust's
/// `f64::round_ties_even` is the same IEEE 754 `roundToIntegralTiesToEven` operation; this wrapper
/// exists so that the Java call site is greppable and so no writer reaches for `f64::round`
/// (half away from zero) by mistake.
pub fn java_rint(x: f64) -> f64 {
    // Math.rint
    x.round_ties_even()
}

/// Java's `(int) Math.round(double)`.
///
/// `Math.round` returns a `long`; the cast to `int` narrows it by **truncating the high 32 bits**,
/// not by saturating. `(int) Math.round(1e18)` is therefore `-1486618624`, and Rust's `as` on
/// `i64 -> i32` wraps identically. Java bug: nothing here, but the surprise is worth naming.
pub fn java_round_to_int(x: f64) -> i32 {
    // (int) Math.round
    java_round(x) as i32
}

/// Java `Double.toString(double)`.
pub fn java_double_to_string(d: f64) -> String {
    // Double.toString
    if d.is_nan() {
        return "NaN".to_string();
    }
    let negative = d.is_sign_negative();
    if d.is_infinite() {
        return if negative { "-Infinity" } else { "Infinity" }.to_string();
    }
    let magnitude = d.abs();
    if magnitude == 0.0 {
        // The sign comes from the sign *bit*, so `-0.0` prints `-0.0`.
        return if negative { "-0.0" } else { "0.0" }.to_string();
    }
    let (digits, exp10) = shortest_digits_f64(magnitude);
    render(negative, &digits, exp10)
}

/// Java `Float.toString(float)`.
///
/// Deliberately **not** `java_double_to_string(f as f64)`: the shortest round-trip search runs
/// against the float's own 24-bit significand and yields a different, shorter digit string
/// (`0.1f` prints `0.1`, while the same value widened to a double prints `0.10000000149011612`).
pub fn java_float_to_string(f: f32) -> String {
    // Float.toString
    if f.is_nan() {
        return "NaN".to_string();
    }
    let negative = f.is_sign_negative();
    if f.is_infinite() {
        return if negative { "-Infinity" } else { "Infinity" }.to_string();
    }
    let magnitude = f.abs();
    if magnitude == 0.0 {
        return if negative { "-0.0" } else { "0.0" }.to_string();
    }
    let (digits, exp10) = shortest_digits_f32(magnitude);
    render(negative, &digits, exp10)
}

/// Port of `SesWriter.formatPlacementRotation` (SesWriter.java:259-269): whole degrees as bare
/// integers (`0`, `339`), fractional degrees to at most three decimals with no trailing zeros
/// (`338.5`, never `338.500`).
pub fn format_placement_rotation(degrees: f64) -> String {
    // formatPlacementRotation
    let rounded = java_rint(degrees * 1000.0) / 1000.0;
    if (rounded - java_rint(rounded)).abs() < 1e-9 {
        return java_format_fixed(rounded, 0);
    }
    let formatted = java_format_fixed(rounded, 3);
    if formatted.contains('.') {
        // Java: `.replaceAll("0+$", "").replaceAll("\\.$", "")`.
        let trimmed = formatted.trim_end_matches('0');
        return trimmed.strip_suffix('.').unwrap_or(trimmed).to_string();
    }
    formatted
}

/// Java `String.format(Locale.ENGLISH, "%.<precision>f", x)`.
///
/// `java.util.Formatter` does **not** round the double's exact binary value. It asks
/// `FloatingDecimal` for the shortest round-trip digit string — the same digits `Double.toString`
/// prints — pads it with zeros when more precision is requested than those digits carry, and
/// rounds it `HALF_UP` (half away from zero) when less is requested. Two consequences that Rust's
/// `{:.N}` gets differently:
///
/// * `%.2f` of `8.475` is `8.48`, because the digits are `8475` and `HALF_UP` rounds the `5` up —
///   even though the double is really `8.47499999999999964…`, which rounds to `8.47` exactly.
/// * `%.3f` of `0.0625` is `0.063`, because `HALF_UP` breaks the exact tie away from zero where
///   Rust's half-to-even breaks it towards `0.062`.
///
/// Verified on JDK 25 over two million random doubles: the Formatter's digit string and
/// `Double.toString`'s digit string agree on every one, so a single shortest-digits routine
/// serves both.
pub fn java_format_fixed(x: f64, precision: usize) -> String {
    // String.format("%.Nf", …)
    if x.is_nan() {
        return "NaN".to_string();
    }
    let negative = x.is_sign_negative();
    let sign = if negative { "-" } else { "" };
    if x.is_infinite() {
        return format!("{sign}Infinity");
    }
    let magnitude = x.abs();
    if magnitude == 0.0 {
        // `%.0f` of `-0.0` is `-0`, and `%.3f` of it is `-0.000`.
        return if precision == 0 {
            format!("{sign}0")
        } else {
            format!("{sign}0.{}", "0".repeat(precision))
        };
    }

    let (digits, exp10) = shortest_digits_f64(magnitude);
    // Lay the digits out against a decimal point sitting `point` places from the left, padding
    // with leading zeros when the value is below 1 so that `point` is never negative.
    let mut ds: Vec<u8> = Vec::with_capacity(digits.len() + precision + 2);
    let mut point = exp10 + 1;
    if point < 0 {
        ds.extend(std::iter::repeat_n(b'0', (-point) as usize));
        point = 0;
    }
    ds.extend_from_slice(digits.as_bytes());
    let mut point_index = point as usize;

    let keep = point_index + precision;
    if keep >= ds.len() {
        // More precision than the shortest digits carry: Java pads with zeros rather than
        // continuing the exact binary expansion.
        ds.resize(keep, b'0');
    } else {
        let round_up = ds[keep] >= b'5'; // HALF_UP: the dropped tail's leading digit decides.
        ds.truncate(keep);
        if round_up {
            let mut i = keep;
            loop {
                if i == 0 {
                    ds.insert(0, b'1');
                    point_index += 1;
                    break;
                }
                i -= 1;
                if ds[i] == b'9' {
                    ds[i] = b'0';
                } else {
                    ds[i] += 1;
                    break;
                }
            }
        }
    }

    let integer_part = if point_index == 0 {
        "0".to_string()
    } else {
        String::from_utf8_lossy(&ds[..point_index]).into_owned()
    };
    if precision == 0 {
        format!("{sign}{integer_part}")
    } else {
        let fraction = String::from_utf8_lossy(&ds[point_index..]).into_owned();
        format!("{sign}{integer_part}.{fraction}")
    }
}

/// Lays out a sign, a significant-digit string and the decimal exponent of its leading digit the
/// way `Double.toString`/`Float.toString` do: plain decimal for `10^-3 <= m < 10^7` (equivalently
/// `-3 <= exp10 <= 6`, because `10^exp10 <= m < 10^(exp10+1)`), computerized scientific notation
/// otherwise — uppercase `E`, no `+` on a positive exponent — and **always at least one digit on
/// each side of the point**.
fn render(negative: bool, digits: &str, exp10: i32) -> String {
    let sign = if negative { "-" } else { "" };
    if (-3..=6).contains(&exp10) {
        if exp10 >= 0 {
            let point = exp10 as usize + 1;
            if digits.len() <= point {
                let zeros = point - digits.len();
                format!("{sign}{digits}{}.0", "0".repeat(zeros))
            } else {
                format!("{sign}{}.{}", &digits[..point], &digits[point..])
            }
        } else {
            let zeros = (-exp10 - 1) as usize;
            format!("{sign}0.{}{digits}", "0".repeat(zeros))
        }
    } else {
        let (first, rest) = digits.split_at(1);
        let rest = if rest.is_empty() { "0" } else { rest };
        format!("{sign}{first}.{rest}E{exp10}")
    }
}

/// The significant digits and decimal exponent Java's `Double.toString` would use for a finite,
/// strictly positive `m`: `m ≈ 0.d1d2… × 10^(exp10+1)`, i.e. `exp10` is the exponent of the
/// leading digit, and the digit string has no leading and no trailing zeros.
///
/// Rust's `{:e}` already yields the shortest round-trip form, which is Java's answer whenever it
/// has two or more digits *and* is not an exact tie. When it has exactly one digit, Java's spec
/// widens the candidate set to decimals of length 1 *or* 2 and takes the one closest to `m` (ties
/// to an even last digit) — which is precisely `{:.1e}`, two significant digits correctly rounded.
/// Trailing zeros are stripped again afterwards so that `0.001` does not come back as `0.0010`.
///
/// Unlike the tie-break below, this branch does no round-trip re-check. The argument for that is
/// a heuristic, not a proof: the two-significant-digit grid refines the one-digit grid, so the
/// nearest two-digit decimal is at most as far from `m` as the one-digit decimal Rust just
/// produced, and that one round-trips by construction. Distance alone does not settle it at a
/// binade boundary, where the rounding interval is lopsided — which is exactly why the tie-break
/// below *does* re-check. What actually backs this branch is the `p3t2` differential:
/// **26 M values across four modes and two seeds, zero diff lines**
/// (`scripts/differential/README.md`), mode 0 of which walks random `f64` bit patterns and so
/// reaches one-digit shortest forms at every exponent.
fn shortest_digits_f64(m: f64) -> (String, i32) {
    let (digits, exp10) = split_exp_form(&format!("{m:e}"));
    if digits.len() == 1 {
        return split_exp_form(&format!("{m:.1e}"));
    }
    let exact = odd_mantissa_f64(m)
        .and_then(|(mantissa, twos)| exact_significant_digits(mantissa, twos, digits.len() + 1));
    break_tie_to_even(digits, exp10, exact, |candidate, exponent| {
        round_trip_string(candidate, exponent).parse::<f64>() == Ok(m)
    })
}

/// [`shortest_digits_f64`] on an `f32`. The shortest round-trip search must run on the float
/// itself; widening first would ask for enough digits to round-trip a double.
fn shortest_digits_f32(m: f32) -> (String, i32) {
    let (digits, exp10) = split_exp_form(&format!("{m:e}"));
    if digits.len() == 1 {
        return split_exp_form(&format!("{m:.1e}"));
    }
    let exact = odd_mantissa_f32(m)
        .and_then(|(mantissa, twos)| exact_significant_digits(mantissa, twos, digits.len() + 1));
    break_tie_to_even(digits, exp10, exact, |candidate, exponent| {
        round_trip_string(candidate, exponent).parse::<f32>() == Ok(m)
    })
}

/// Java's tie rule, which Rust's shortest formatter does not implement.
///
/// When the value sits *exactly* halfway between the two nearest decimals of the shortest length,
/// both round-trip, so both are candidates and Java takes "the one whose least significant digit
/// is even" (the `Double.toString` spec). Rust picks the other one often enough to matter:
/// `1055987014896502.25` is `1.0559870148965022E15` in Java and `1.0559870148965023e15` in Rust,
/// and `169903.625f` is `169903.62` in Java and `169903.63` in Rust. Both were found by the
/// `p3t2` differential within its first 15k values.
///
/// A tie is exactly the case "the value's *exact* significant digits are the shortest digits plus
/// one more, and that extra digit is a `5`" — hence `exact`, which is `None` whenever the exact
/// expansion is too long for a tie to be possible. The chosen candidate is checked for round-trip
/// before it is used: at a binary-exponent boundary the rounding interval is lopsided, so the
/// nearer-looking candidate may not represent the value at all, and the same check rejects a
/// carried candidate (`999 -> 1000`) whose shorter form would have been found already.
fn break_tie_to_even(
    digits: String,
    exp10: i32,
    exact: Option<String>,
    round_trips: impl Fn(&str, i32) -> bool,
) -> (String, i32) {
    let Some(exact) = exact else {
        return (digits, exp10);
    };
    let length = digits.len();
    if exact.len() != length + 1 || !exact.ends_with('5') {
        return (digits, exp10);
    }
    let lower = &exact[..length];
    let (chosen, chosen_exp10) = if lower.as_bytes()[length - 1] % 2 == 0 {
        (strip_trailing_zeros(lower), exp10)
    } else {
        increment_digits(lower, exp10)
    };
    if round_trips(&chosen, chosen_exp10) {
        (chosen, chosen_exp10)
    } else {
        (digits, exp10)
    }
}

/// Adds one to a digit string, returning it trailing-zero-stripped together with the (possibly
/// bumped) exponent of its leading digit: `("199", 2) -> ("2", 2)`, `("999", 2) -> ("1", 3)`.
fn increment_digits(digits: &str, exp10: i32) -> (String, i32) {
    let mut bytes = digits.as_bytes().to_vec();
    let mut index = bytes.len();
    let mut exponent = exp10;
    loop {
        if index == 0 {
            bytes.insert(0, b'1');
            bytes.pop();
            exponent += 1;
            break;
        }
        index -= 1;
        if bytes[index] == b'9' {
            bytes[index] = b'0';
        } else {
            bytes[index] += 1;
            break;
        }
    }
    (
        strip_trailing_zeros(&String::from_utf8_lossy(&bytes)),
        exponent,
    )
}

fn strip_trailing_zeros(digits: &str) -> String {
    let trimmed = digits.trim_end_matches('0');
    if trimmed.is_empty() { "0" } else { trimmed }.to_string()
}

/// A parseable spelling of `0.<digits> × 10^(exp10 + 1)`, used only to re-check that a candidate
/// really does round-trip to the value being printed.
fn round_trip_string(digits: &str, exp10: i32) -> String {
    format!("0.{digits}e{}", exp10 + 1)
}

/// The odd mantissa `m` and the power `j` for which `value == m / 2^j`, or `None` when the value
/// is an integer (`j <= 0`) — an integral double's exact expansion cannot end in a `5` unless it
/// is small enough to be printed exactly, so no tie is possible there.
fn odd_mantissa_f64(value: f64) -> Option<(u64, u32)> {
    let bits = value.to_bits();
    let biased_exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1u64 << 52) - 1);
    let (mut mantissa, mut exponent) = if biased_exponent == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1u64 << 52), biased_exponent - 1075)
    };
    normalise_odd(&mut mantissa, &mut exponent)
}

/// [`odd_mantissa_f64`] for an `f32`: 23 fraction bits, exponent bias 150.
fn odd_mantissa_f32(value: f32) -> Option<(u64, u32)> {
    let bits = value.to_bits();
    let biased_exponent = ((bits >> 23) & 0xff) as i32;
    let fraction = u64::from(bits & ((1u32 << 23) - 1));
    let (mut mantissa, mut exponent) = if biased_exponent == 0 {
        (fraction, -149)
    } else {
        (fraction | (1u64 << 23), biased_exponent - 150)
    };
    normalise_odd(&mut mantissa, &mut exponent)
}

fn normalise_odd(mantissa: &mut u64, exponent: &mut i32) -> Option<(u64, u32)> {
    while *exponent < 0 && *mantissa & 1 == 0 && *mantissa != 0 {
        *mantissa >>= 1;
        *exponent += 1;
    }
    if *exponent < 0 {
        Some((*mantissa, (-*exponent) as u32))
    } else {
        None
    }
}

/// The exact significant decimal digits of `mantissa / 2^twos` (an odd mantissa, so the product
/// below has no trailing zeros), or `None` when there are more than `max_length` of them.
///
/// `value * 10^twos == mantissa * 5^twos`, which is exact in a `u128` for every `twos` that could
/// possibly yield few enough digits: `5^27` already has 19 digits on its own, one more than the
/// longest `Double.toString` digit string plus the tie digit.
fn exact_significant_digits(mantissa: u64, twos: u32, max_length: usize) -> Option<String> {
    if twos > 27 {
        return None;
    }
    let scaled = u128::from(mantissa).checked_mul(5u128.checked_pow(twos)?)?;
    let digits = scaled.to_string();
    if digits.len() > max_length {
        None
    } else {
        Some(digits)
    }
}

/// Splits Rust's `LowerExp` output — `D[.DDD]e<exp>`, never a `+`, never a leading zero on the
/// exponent — into trailing-zero-stripped significant digits and the exponent of the leading
/// digit. The digit string is never empty: the callers exclude zero.
fn split_exp_form(formatted: &str) -> (String, i32) {
    let (mantissa, exponent) = formatted
        .split_once('e')
        .expect("Rust's LowerExp always emits an `e`");
    let digits = mantissa.replace('.', "");
    let exp10: i32 = exponent
        .parse()
        .expect("Rust's LowerExp always emits a decimal exponent");
    (strip_trailing_zeros(&digits), exp10)
}
