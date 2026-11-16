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

/// `n` + `delta`, keeping zero padding (`007` → `008`, `-01` → `00`).
pub fn step(n: &str, delta: i128) -> String {
    let negative = n.starts_with('-');
    let digits = n.trim_start_matches('-');
    let Ok(v) = digits.parse::<i128>() else {
        return n.to_owned();
    };
    let v = if negative { -v } else { v };
    let r = v.saturating_add(delta);
    let width = if digits.len() > 1 && digits.starts_with('0') { digits.len() } else { 0 };
    if r < 0 { format!("-{:0width$}", -r) } else { format!("{r:0width$}") }
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
