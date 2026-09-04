pub use fr_geometry::java_round;

pub fn java_rint(x: f64) -> f64 {
    x.round_ties_even()
}

pub fn java_round_to_int(x: f64) -> i32 {
    java_round(x) as i32
}

pub fn java_double_to_string(d: f64) -> String {
    if d.is_nan() {
        return "NaN".to_string();
    }
    let negative = d.is_sign_negative();
    if d.is_infinite() {
        return if negative { "-Infinity" } else { "Infinity" }.to_string();
    }
    let magnitude = d.abs();
    if magnitude == 0.0 {
        return if negative { "-0.0" } else { "0.0" }.to_string();
    }
    let (digits, exp10) = shortest_digits_f64(magnitude);
    render(negative, &digits, exp10)
}

pub fn java_float_to_string(f: f32) -> String {
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

pub fn format_placement_rotation(degrees: f64) -> String {
    let rounded = java_rint(degrees * 1000.0) / 1000.0;
    if (rounded - java_rint(rounded)).abs() < 1e-9 {
        return java_format_fixed(rounded, 0);
    }
    let formatted = java_format_fixed(rounded, 3);
    if formatted.contains('.') {
        let trimmed = formatted.trim_end_matches('0');
        return trimmed.strip_suffix('.').unwrap_or(trimmed).to_string();
    }
    formatted
}

pub fn java_format_fixed(x: f64, precision: usize) -> String {
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
        return if precision == 0 {
            format!("{sign}0")
        } else {
            format!("{sign}0.{}", "0".repeat(precision))
        };
    }

    let (digits, exp10) = shortest_digits_f64(magnitude);
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
        ds.resize(keep, b'0');
    } else {
        let round_up = ds[keep] >= b'5';
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

fn round_trip_string(digits: &str, exp10: i32) -> String {
    format!("0.{digits}e{}", exp10 + 1)
}

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
