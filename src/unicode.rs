//! Unicode normalization, diacritics and `\uXXXX` escapes.

use std::fmt::Write;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

pub fn nfc(s: &str) -> String {
    s.nfc().collect()
}

pub fn nfd(s: &str) -> String {
    s.nfd().collect()
}

pub fn nfkc(s: &str) -> String {
    s.nfkc().collect()
}

pub fn nfkd(s: &str) -> String {
    s.nfkd().collect()
}

/// Drops accents (`Crème brûlée` → `Creme brulee`) and spells out letters
/// that have no decomposition (`ß` → `ss`, `Ø` → `O`).
pub fn to_ascii(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.nfkd() {
        if is_combining_mark(c) {
            continue;
        }
        match c {
            'ß' => out.push_str("ss"),
            'ẞ' => out.push_str("SS"),
            'æ' => out.push_str("ae"),
            'Æ' => out.push_str("AE"),
            'œ' => out.push_str("oe"),
            'Œ' => out.push_str("OE"),
            'ø' => out.push('o'),
            'Ø' => out.push('O'),
            'đ' | 'ð' => out.push('d'),
            'Đ' | 'Ð' => out.push('D'),
            'ł' => out.push('l'),
            'Ł' => out.push('L'),
            'ħ' => out.push('h'),
            'Ħ' => out.push('H'),
            'ı' => out.push('i'),
            'þ' => out.push_str("th"),
            'Þ' => out.push_str("TH"),
            c => out.push(c),
        }
    }
    out
}

/// Every character outside ASCII as `\uXXXX` (surrogate pairs above U+FFFF).
pub fn escape_non_ascii(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut buf = [0u16; 2];
    for c in s.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            for unit in c.encode_utf16(&mut buf) {
                let _ = write!(out, "\\u{unit:04x}");
            }
        }
    }
    out
}

/// `\uXXXX` (with surrogate pairs) and `\u{X…}` back to characters; every
/// other backslash stays.
pub fn unescape_unicode(s: &str) -> String {
    let mut units: Vec<u16> = Vec::new();
    let mut out = String::with_capacity(s.len());
    let flush = |units: &mut Vec<u16>, out: &mut String| {
        out.extend(char::decode_utf16(units.drain(..)).map(|r| r.unwrap_or('\u{FFFD}')));
    };
    let mut rest = s;
    while let Some(at) = rest.find("\\u") {
        let before = &rest[..at];
        if !before.is_empty() {
            flush(&mut units, &mut out);
            out.push_str(before);
        }
        let after = &rest[at + 2..];
        if let Some(braced) = after.strip_prefix('{')
            && let Some(end) = braced.find('}')
            && let Some(c) = u32::from_str_radix(&braced[..end], 16).ok().and_then(char::from_u32)
        {
            flush(&mut units, &mut out);
            out.push(c);
            rest = &braced[end + 1..];
            continue;
        }
        let digits = after.get(..4).filter(|d| d.bytes().all(|b| b.is_ascii_hexdigit()));
        match digits.and_then(|d| u16::from_str_radix(d, 16).ok()) {
            Some(unit) => {
                units.push(unit);
                rest = &after[4..];
            }
            None => {
                flush(&mut units, &mut out);
                out.push_str("\\u");
                rest = after;
            }
        }
    }
    flush(&mut units, &mut out);
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_diacritics() {
        assert_eq!(to_ascii("Crème brûlée, Øresund, Straße, Łódź, ﬁ"), "Creme brulee, Oresund, Strasse, Lodz, fi");
    }

    #[test]
    fn normalizes() {
        assert_eq!(nfd("é").chars().count(), 2);
        assert_eq!(nfc("e\u{301}"), "é");
        assert_eq!(nfkc("ﬁ²"), "fi2");
        assert_eq!(nfkd("ﬁ").chars().count(), 2);
    }

    #[test]
    fn unicode_escapes() {
        let s = "café 😀 \\n";
        let e = escape_non_ascii(s);
        assert_eq!(e, "caf\\u00e9 \\ud83d\\ude00 \\n");
        assert_eq!(unescape_unicode(&e), s);
        assert_eq!(unescape_unicode("\\u{1F600}\\u00E9\\uZZ"), "😀é\\uZZ");
    }
}
