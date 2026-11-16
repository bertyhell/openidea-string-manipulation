//! Encodings, hashes and timestamps.

use base64::Engine;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig, STANDARD};
use base64::engine::{DecodePaddingMode, general_purpose};
use std::fmt::Write;

type R = Result<String, String>;

pub fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

pub fn md5(s: &str) -> String {
    use md5::Digest;
    hex(&md5::Md5::digest(s.as_bytes()))
}

pub fn sha1(s: &str) -> String {
    use sha1::Digest;
    hex(&sha1::Sha1::digest(s.as_bytes()))
}

pub fn sha256(s: &str) -> String {
    use sha2::Digest;
    hex(&sha2::Sha256::digest(s.as_bytes()))
}

pub fn sha512(s: &str) -> String {
    use sha2::Digest;
    hex(&sha2::Sha512::digest(s.as_bytes()))
}

pub fn sha3_256(s: &str) -> String {
    use sha3::Digest;
    hex(&sha3::Sha3_256::digest(s.as_bytes()))
}

pub fn sha3_512(s: &str) -> String {
    use sha3::Digest;
    hex(&sha3::Sha3_512::digest(s.as_bytes()))
}

pub fn base64_encode(s: &str) -> String {
    STANDARD.encode(s.as_bytes())
}

/// Standard or URL-safe alphabet, with or without padding, ignoring line breaks.
pub fn base64_decode(s: &str) -> R {
    const LENIENT: GeneralPurposeConfig = GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true);
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = GeneralPurpose::new(&base64::alphabet::STANDARD, LENIENT)
        .decode(&compact)
        .or_else(|_| GeneralPurpose::new(&base64::alphabet::URL_SAFE, LENIENT).decode(&compact))
        .or_else(|_| general_purpose::URL_SAFE_NO_PAD.decode(&compact))
        .map_err(|e| format!("not Base64: {e}"))?;
    String::from_utf8(bytes).map_err(|_| "the decoded bytes are not UTF-8 text".into())
}

pub fn hex_encode(s: &str) -> String {
    hex(s.as_bytes())
}

pub fn hex_decode(s: &str) -> R {
    let digits: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let digits = digits.strip_prefix("0x").unwrap_or(&digits);
    if !digits.len().is_multiple_of(2) || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("not hexadecimal bytes".into());
    }
    let bytes: Vec<u8> =
        (0..digits.len()).step_by(2).map(|i| u8::from_str_radix(&digits[i..i + 2], 16).unwrap_or(0)).collect();
    String::from_utf8(bytes).map_err(|_| "the decoded bytes are not UTF-8 text".into())
}

/// `application/x-www-form-urlencoded`, like Java's `URLEncoder`: a space is `+`.
pub fn url_encode(s: &str) -> String {
    percent_encode(s, |b| b.is_ascii_alphanumeric() || b"*-._".contains(&b), true)
}

/// RFC 3986: everything but the unreserved characters, a space is `%20`.
pub fn url_encode_rfc3986(s: &str) -> String {
    percent_encode(s, |b| b.is_ascii_alphanumeric() || b"-._~".contains(&b), false)
}

fn percent_encode(s: &str, keep: impl Fn(u8) -> bool, plus: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if keep(b) {
            out.push(b as char);
        } else if plus && b == b' ' {
            out.push('+');
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

pub fn url_decode(s: &str) -> R {
    percent_decode(s, true)
}

pub fn url_decode_rfc3986(s: &str) -> R {
    percent_decode(s, false)
}

fn percent_decode(s: &str, plus: bool) -> R {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() => {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap_or(0));
                i += 3;
            }
            b'+' if plus => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| "the decoded bytes are not UTF-8 text".into())
}

// Civil date ↔ day number (Howard Hinnant's algorithms), proleptic Gregorian.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// `2024-03-05T14:30:00.250+01:00` (also `Z`, a space for `T`, no seconds,
/// no fraction, or only a date; no offset is UTC) → epoch milliseconds.
pub fn iso_to_epoch(s: &str) -> R {
    let bad = || format!("not an ISO-8601 timestamp: {s:?}");
    let t = s.trim();
    let num = |from: usize, n: usize| -> Option<i64> {
        let part = t.get(from..from + n)?;
        if part.bytes().all(|b| b.is_ascii_digit()) { part.parse().ok() } else { None }
    };
    let (y, mo, d) = (num(0, 4).ok_or_else(bad)?, num(5, 2).ok_or_else(bad)?, num(8, 2).ok_or_else(bad)?);
    if t.as_bytes().get(4) != Some(&b'-')
        || t.as_bytes().get(7) != Some(&b'-')
        || !(1..=12).contains(&mo)
        || !(1..=31).contains(&d)
    {
        return Err(bad());
    }
    let mut ms = days_from_civil(y, mo, d) * 86_400_000;
    let mut rest = &t[10..];
    if let Some(time) = rest.strip_prefix(['T', 't', ' ']) {
        let tn = |from: usize| -> Option<i64> {
            let part = time.get(from..from + 2)?;
            if part.bytes().all(|b| b.is_ascii_digit()) { part.parse().ok() } else { None }
        };
        let (h, mi) = (tn(0).ok_or_else(bad)?, tn(3).ok_or_else(bad)?);
        if time.as_bytes().get(2) != Some(&b':') || h > 23 || mi > 59 {
            return Err(bad());
        }
        ms += (h * 3600 + mi * 60) * 1000;
        rest = &time[5..];
        if let Some(sec) = rest.strip_prefix(':') {
            let s2 = sec.get(..2).filter(|p| p.bytes().all(|b| b.is_ascii_digit())).ok_or_else(bad)?;
            ms += s2.parse::<i64>().map_err(|_| bad())? * 1000;
            rest = &sec[2..];
            if let Some(frac) = rest.strip_prefix(['.', ',']) {
                let n = frac.bytes().take_while(u8::is_ascii_digit).count();
                if n == 0 {
                    return Err(bad());
                }
                let digits = format!("{:0<3}", &frac[..n.min(3)]);
                ms += digits.parse::<i64>().map_err(|_| bad())?;
                rest = &frac[n..];
            }
        }
    }
    match rest {
        "" | "Z" | "z" => {}
        off => {
            let sign = match off.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return Err(bad()),
            };
            let digits: String = off[1..].chars().filter(|&c| c != ':').collect();
            if digits.len() != 4 && digits.len() != 2 || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return Err(bad());
            }
            let h: i64 = digits[..2].parse().map_err(|_| bad())?;
            let m: i64 = digits.get(2..).map_or(Ok(0), str::parse).map_err(|_| bad())?;
            ms -= sign * (h * 60 + m) * 60_000;
        }
    }
    Ok(ms.to_string())
}

/// Epoch milliseconds (or seconds, when it has 10 digits or fewer) → `…Z`.
pub fn epoch_to_iso(s: &str) -> R {
    let t = s.trim();
    let n: i64 = t.parse().map_err(|_| format!("not a number of milliseconds: {t:?}"))?;
    let digits = t.trim_start_matches('-').len();
    let ms = if digits <= 10 { n.checked_mul(1000).ok_or("out of range")? } else { n };
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    let (y, m, d) = civil_from_days(days);
    let (h, mi, sec, milli) = (rem / 3_600_000, rem / 60_000 % 60, rem / 1000 % 60, rem % 1000);
    Ok(format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{sec:02}.{milli:03}Z"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes() {
        assert_eq!(md5("abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(sha1("abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(sha256("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert!(sha512("abc").starts_with("ddaf35a193617aba"));
        assert_eq!(sha3_256("abc"), "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532");
        assert!(sha3_512("abc").starts_with("b751850b1a57168a"));
    }

    #[test]
    fn base64_and_hex() {
        assert_eq!(base64_encode("héllo?>"), "aMOpbGxvPz4=");
        assert_eq!(base64_decode("aMOpbGxvPz4=").unwrap(), "héllo?>");
        assert_eq!(base64_decode("aMOpbGxvPz4").unwrap(), "héllo?>");
        assert_eq!(base64_decode("PDw_Pz4-").unwrap(), "<<??>>");
        assert!(base64_decode("not base64!").is_err());
        assert_eq!(hex_encode("hé"), "68c3a9");
        assert_eq!(hex_decode("68 C3 a9").unwrap(), "hé");
        assert!(hex_decode("6").is_err());
    }

    #[test]
    fn urls() {
        assert_eq!(url_encode("a b&c=d/é~"), "a+b%26c%3Dd%2F%C3%A9%7E");
        assert_eq!(url_encode_rfc3986("a b&c=d/é~"), "a%20b%26c%3Dd%2F%C3%A9~");
        assert_eq!(url_decode("a+b%26c%3Dd%2F%C3%A9%7E").unwrap(), "a b&c=d/é~");
        assert_eq!(url_decode_rfc3986("a+b%20c").unwrap(), "a+b c");
        assert_eq!(url_decode("100%").unwrap(), "100%");
    }

    #[test]
    fn timestamps() {
        assert_eq!(iso_to_epoch("1970-01-01T00:00:00Z").unwrap(), "0");
        assert_eq!(iso_to_epoch("2024-03-05T14:30:00.25+01:00").unwrap(), "1709645400250");
        assert_eq!(iso_to_epoch("2024-03-05 13:30").unwrap(), "1709645400000");
        assert_eq!(iso_to_epoch("2024-03-05").unwrap(), "1709596800000");
        assert_eq!(iso_to_epoch("1969-12-31T23:59:59.999Z").unwrap(), "-1");
        assert!(iso_to_epoch("yesterday").is_err());
        assert_eq!(epoch_to_iso("1709645400250").unwrap(), "2024-03-05T13:30:00.250Z");
        assert_eq!(epoch_to_iso("1709645400").unwrap(), "2024-03-05T13:30:00.000Z");
        assert_eq!(epoch_to_iso("-1").unwrap(), "1969-12-31T23:59:59.000Z");
    }
}
