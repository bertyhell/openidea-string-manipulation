//! Finding and stepping integers in text.

/// Byte ranges of the integers in `s`. A `-` is a sign only where it can't be
/// a minus between operands or a separator (`a-1`, `2024-01`).
pub fn find(s: &str) -> Vec<(usize, usize)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let mut start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if start > 0 && b[start - 1] == b'-' {
            let before = start.checked_sub(2).map(|p| b[p]);
            if before.is_none_or(|c| !(c.is_ascii_alphanumeric() || c == b'_' || c == b')' || c == b']')) {
                start -= 1;
            }
        }
        out.push((start, i));
    }
    out
}

/// `n` + `delta`, keeping zero padding (`007` → `008`, `-01` → `00`). Works on
/// the decimal digits, so numbers of any length step without overflowing.
pub fn step(n: &str, delta: i128) -> String {
    let negative = n.starts_with('-');
    let digits = n.trim_start_matches('-');
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return n.to_owned();
    }
    let a = digits.trim_start_matches('0');
    let d = delta.unsigned_abs().to_string();
    let (negative_result, magnitude) = if negative == (delta < 0) || delta == 0 {
        (negative, add_digits(a, &d))
    } else if longer_or_equal(a, &d) {
        (negative, sub_digits(a, &d))
    } else {
        (!negative, sub_digits(&d, a))
    };
    let magnitude = magnitude.trim_start_matches('0');
    let magnitude = if magnitude.is_empty() { "0" } else { magnitude };
    let width = if digits.len() > 1 && digits.starts_with('0') { digits.len() } else { 0 };
    let sign = if negative_result && magnitude != "0" { "-" } else { "" };
    format!("{sign}{magnitude:0>width$}")
}

/// Whether the digit string `a` is at least `b` (neither has leading zeros).
fn longer_or_equal(a: &str, b: &str) -> bool {
    (a.len(), a) >= (b.len(), b)
}

fn add_digits(a: &str, b: &str) -> String {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut out = Vec::with_capacity(a.len().max(b.len()) + 1);
    let mut carry = 0;
    for i in 0..a.len().max(b.len()) {
        let x = a.len().checked_sub(i + 1).map_or(0, |j| a[j] - b'0');
        let y = b.len().checked_sub(i + 1).map_or(0, |j| b[j] - b'0');
        let s = x + y + carry;
        out.push(b'0' + s % 10);
        carry = s / 10;
    }
    if carry > 0 {
        out.push(b'0' + carry);
    }
    out.iter().rev().map(|&c| c as char).collect()
}

/// `a` - `b` for `a` ≥ `b`.
fn sub_digits(a: &str, b: &str) -> String {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut out = Vec::with_capacity(a.len());
    let mut borrow = 0;
    for i in 0..a.len() {
        let x = (a[a.len() - 1 - i] - b'0') as i8;
        let y = b.len().checked_sub(i + 1).map_or(0, |j| (b[j] - b'0') as i8);
        let mut s = x - y - borrow;
        borrow = 0;
        if s < 0 {
            s += 10;
            borrow = 1;
        }
        out.push(b'0' + s as u8);
    }
    out.iter().rev().map(|&c| c as char).collect()
}

/// Every integer in `s` stepped by `delta`.
pub fn step_all(s: &str, delta: i128) -> String {
    replace(s, |n, _| step(n, delta))
}

/// Replaces each integer of `s` with `f(text, index)`.
pub fn replace(s: &str, mut f: impl FnMut(&str, usize) -> String) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last = 0;
    for (i, (a, b)) in find(s).into_iter().enumerate() {
        out.push_str(&s[last..a]);
        out.push_str(&f(&s[a..b], i));
        last = b;
    }
    out.push_str(&s[last..]);
    out
}

/// The integer touching byte offset `at`, if any.
pub fn at(s: &str, at: usize) -> Option<(usize, usize)> {
    find(s).into_iter().find(|&(a, b)| a <= at && at <= b)
}

pub fn first(s: &str) -> Option<i128> {
    find(s).first().and_then(|&(a, b)| s[a..b].parse().ok())
}

/// Code points ↔ characters: `A` → `65`, `72 105` → `Hi`.
pub fn char_int(s: &str) -> Result<String, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(s.to_owned());
    }
    let parts: Vec<&str> = t.split(|c: char| c.is_whitespace() || c == ',').filter(|p| !p.is_empty()).collect();
    if parts.iter().all(|p| p.bytes().all(|b| b.is_ascii_digit())) {
        return parts
            .iter()
            .map(|p| p.parse::<u32>().ok().and_then(char::from_u32).ok_or_else(|| format!("{p} is not a character")))
            .collect();
    }
    Ok(t.chars().map(|c| (c as u32).to_string()).collect::<Vec<_>>().join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_numbers_and_signs() {
        assert_eq!(find("x = -5, y-3, 2024-01-02"), [(4, 6), (10, 11), (13, 17), (18, 20), (21, 23)]);
        assert_eq!(find("(-1)"), [(1, 3)]);
    }

    #[test]
    fn steps() {
        assert_eq!(step_all("item9 v007 -1 a-1", 1), "item10 v008 0 a-2");
        assert_eq!(step_all("x0 -01", -1), "x-1 -02");
        assert_eq!(step("99", 1), "100");
        assert_eq!(step("-1", 1), "0");
        assert_eq!(step("-5", 3), "-2");
        assert_eq!(step("2", -5), "-3");
        assert_eq!(step("0", 0), "0");
        // Beyond i128 and u128: no overflow, no saturation.
        assert_eq!(step("170141183460469231731687303715884105727", 1), "170141183460469231731687303715884105728");
        assert_eq!(step("9999999999999999999999999999999999999999", 1), "10000000000000000000000000000000000000000");
        assert_eq!(step("-10000000000000000000000000000000000000000", 1), "-9999999999999999999999999999999999999999");
        assert_eq!(step("5", i128::MIN), "-170141183460469231731687303715884105723");
        assert_eq!(at("abc 123 def", 7), Some((4, 7)));
        assert_eq!(at("abc 123 def", 2), None);
        assert_eq!(first("id: 42, 7"), Some(42));
    }

    #[test]
    fn chars_and_ints() {
        assert_eq!(char_int("A").unwrap(), "65");
        assert_eq!(char_int("Hi").unwrap(), "72 105");
        assert_eq!(char_int("72 105").unwrap(), "Hi");
        assert_eq!(char_int("128512").unwrap(), "😀");
    }
}
