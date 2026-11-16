//! Operations on lists of lines.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    Asc,
    Desc,
    InsensitiveAsc,
    InsensitiveDesc,
    NaturalAsc,
    NaturalDesc,
    LengthAsc,
    LengthDesc,
}

impl Sort {
    pub const ALL: [(Sort, &'static str, &'static str); 8] = [
        (Sort::Asc, "asc", "Ascending"),
        (Sort::Desc, "desc", "Descending"),
        (Sort::InsensitiveAsc, "insensitive-asc", "Ascending, ignoring case"),
        (Sort::InsensitiveDesc, "insensitive-desc", "Descending, ignoring case"),
        (Sort::NaturalAsc, "natural-asc", "Natural order (file2 before file10)"),
        (Sort::NaturalDesc, "natural-desc", "Natural order, descending"),
        (Sort::LengthAsc, "length-asc", "By length, shortest first"),
        (Sort::LengthDesc, "length-desc", "By length, longest first"),
    ];

    pub fn from_key(key: &str) -> Option<Sort> {
        Self::ALL.iter().find(|(_, k, _)| *k == key).map(|&(s, _, _)| s)
    }

    /// Compares by the text without its indentation, then as written.
    pub fn cmp(self, a: &str, b: &str) -> Ordering {
        let (ta, tb) = (a.trim_start(), b.trim_start());
        let primary = match self {
            Sort::Asc => ta.cmp(tb),
            Sort::Desc => tb.cmp(ta),
            Sort::InsensitiveAsc => ta.to_lowercase().cmp(&tb.to_lowercase()),
            Sort::InsensitiveDesc => tb.to_lowercase().cmp(&ta.to_lowercase()),
            Sort::NaturalAsc => natural(ta, tb),
            Sort::NaturalDesc => natural(tb, ta),
            Sort::LengthAsc => ta.chars().count().cmp(&tb.chars().count()),
            Sort::LengthDesc => tb.chars().count().cmp(&ta.chars().count()),
        };
        primary.then_with(
            || {
                if matches!(self, Sort::LengthAsc | Sort::LengthDesc) { Ordering::Equal } else { a.cmp(b) }
            },
        )
    }
}

/// Case-insensitive, with runs of digits compared as numbers.
pub fn natural(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut n = String::new();
                    while let Some(&c) = it.peek().filter(|c| c.is_ascii_digit()) {
                        n.push(c);
                        it.next();
                    }
                    n
                };
                let (n, m) = (take(&mut x), take(&mut y));
                let (n0, m0) = (n.trim_start_matches('0'), m.trim_start_matches('0'));
                let ord = n0.len().cmp(&m0.len()).then_with(|| n0.cmp(m0));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(c), Some(d)) => {
                let ord = c.to_lowercase().cmp(d.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                x.next();
                y.next();
            }
        }
    }
}

pub fn sort(lines: &mut [String], how: Sort) {
    lines.sort_by(|a, b| how.cmp(a, b));
}

pub fn unique(lines: &mut Vec<String>) {
    let mut seen = HashSet::new();
    lines.retain(|l| seen.insert(l.clone()));
}

/// Keeps one copy of each line that occurs more than once.
pub fn only_duplicates(lines: &mut Vec<String>) {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for l in lines.iter() {
        *counts.entry(l.clone()).or_default() += 1;
    }
    let mut seen = HashSet::new();
    lines.retain(|l| counts[l] > 1 && seen.insert(l.clone()));
}

pub fn remove_empty(lines: &mut Vec<String>) {
    lines.retain(|l| !l.trim().is_empty());
}

pub fn remove_consecutive_empty(lines: &mut Vec<String>) {
    let mut prev_empty = false;
    lines.retain(|l| {
        let empty = l.trim().is_empty();
        let keep = !(empty && prev_empty);
        prev_empty = empty;
        keep
    });
}

/// `uniq -c` style, most frequent first: `   3 apple`.
pub fn group_and_count(lines: &[String]) -> Vec<String> {
    let mut order: Vec<&String> = Vec::new();
    let mut counts: HashMap<&String, usize> = HashMap::new();
    for l in lines {
        let n = counts.entry(l).or_default();
        if *n == 0 {
            order.push(l);
        }
        *n += 1;
    }
    order.sort_by(|a, b| counts[b].cmp(&counts[a]));
    let width = counts.values().max().map_or(1, |m| m.to_string().len());
    order.into_iter().map(|l| format!("{:>width$} {l}", counts[l])).collect()
}

/// A pattern as typed: `/regex/` (`/regex/i` ignores case), otherwise text.
pub enum Pattern {
    Text(String),
    Regex(regex_lite::Regex),
}

impl Pattern {
    pub fn parse(p: &str) -> Result<Pattern, String> {
        if let Some(body) = p.strip_prefix('/') {
            let (body, flags) = match body.rfind('/') {
                Some(end) => (&body[..end], &body[end + 1..]),
                None => return Ok(Pattern::Text(p.to_owned())),
            };
            if flags.chars().all(|c| c == 'i') {
                let re = regex_lite::RegexBuilder::new(body)
                    .case_insensitive(flags.contains('i'))
                    .build()
                    .map_err(|e| format!("invalid regular expression: {e}"))?;
                return Ok(Pattern::Regex(re));
            }
        }
        Ok(Pattern::Text(p.to_owned()))
    }

    pub fn matches(&self, line: &str) -> bool {
        match self {
            Pattern::Text(t) => line.contains(t.as_str()),
            Pattern::Regex(r) => r.is_match(line),
        }
    }
}

/// Lines padded on the left so all share the same left (`Left`), right or
/// centre edge, measured without their own surrounding spaces.
#[derive(Clone, Copy)]
pub enum Align {
    Left,
    Center,
    Right,
}

pub fn align(lines: &[String], how: Align) -> Vec<String> {
    let trimmed: Vec<&str> = lines.iter().map(|l| l.trim()).collect();
    let width = trimmed.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    trimmed
        .iter()
        .map(|l| {
            let pad = width - l.chars().count();
            match how {
                Align::Left => (*l).to_owned(),
                Align::Right => format!("{}{l}", " ".repeat(pad)),
                Align::Center => format!("{}{l}", " ".repeat(pad / 2)),
            }
        })
        .collect()
}

/// Splits every line on `sep` and pads the cells so the separators line up:
/// `a,bb` / `ccc,d` → `a   , bb` / `ccc , d`; `| a | b |` tables too.
pub fn align_columns(lines: &[String], sep: &str) -> Vec<String> {
    let blank = sep.trim().is_empty();
    let sep = sep.trim();
    let split = |l: &str| -> Vec<String> {
        if blank {
            l.split_whitespace().map(str::to_owned).collect()
        } else {
            l.split(sep).map(|c| c.trim().to_owned()).collect()
        }
    };
    let rows: Vec<Vec<String>> = lines.iter().map(|l| split(l)).collect();
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> =
        (0..cols).map(|i| rows.iter().filter_map(|r| r.get(i)).map(|c| c.chars().count()).max().unwrap_or(0)).collect();
    let joiner = if blank { " ".to_owned() } else { format!(" {sep} ") };
    rows.iter()
        .zip(lines)
        .map(|(r, line)| {
            let indent = &line[..line.len() - line.trim_start().len()];
            if r.len() <= 1 {
                return line.trim_end().to_owned();
            }
            let cells: Vec<String> = r
                .iter()
                .enumerate()
                .map(|(i, c)| if i + 1 == r.len() { c.clone() } else { format!("{c:<w$}", w = widths[i]) })
                .collect();
            format!("{indent}{}", cells.join(&joiner).trim())
        })
        .collect()
}

/// Sorts the `sep`-separated items of `s`, keeping the spacing around the
/// separators where it was: `c, a,  b` → `a, b,  c`.
pub fn sort_tokens(s: &str, sep: &str, how: Sort) -> String {
    if sep.is_empty() || !s.contains(sep) {
        return s.to_owned();
    }
    let parts: Vec<&str> = s.split(sep).collect();
    let slots: Vec<(&str, &str)> = parts
        .iter()
        .map(|p| {
            let t = p.trim();
            let lead = &p[..p.len() - p.trim_start().len()];
            let trail = &p[lead.len() + t.len()..];
            (lead, trail)
        })
        .collect();
    let mut tokens: Vec<String> = parts.iter().map(|p| p.trim().to_owned()).collect();
    // A trailing separator (`a, b,`) leaves an empty last item: keep it last.
    let trailing_empty = tokens.last().is_some_and(String::is_empty);
    let n = tokens.len() - usize::from(trailing_empty);
    tokens[..n].sort_by(|a, b| how.cmp(a, b));
    tokens.iter().zip(slots).map(|(t, (lead, trail))| format!("{lead}{t}{trail}")).collect::<Vec<_>>().join(sep)
}

/// A small, unpredictable seed (the platform's random source through
/// `RandomState`), then xorshift.
pub struct Rng(u64);

impl Rng {
    pub fn new() -> Rng {
        use std::hash::{BuildHasher, Hasher};
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u8(0);
        Rng(h.finish() | 1)
    }

    pub fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn sorts() {
        let mut l = v(&["b10", "B2", "a", "  c", "b2"]);
        sort(&mut l, Sort::Asc);
        assert_eq!(l, ["B2", "a", "b10", "b2", "  c"]);
        sort(&mut l, Sort::InsensitiveAsc);
        assert_eq!(l, ["a", "b10", "B2", "b2", "  c"]);
        sort(&mut l, Sort::NaturalAsc);
        assert_eq!(l, ["a", "B2", "b2", "b10", "  c"]);
        sort(&mut l, Sort::LengthDesc);
        assert_eq!(l[0], "b10");
        assert_eq!(natural("file02", "file2"), "file02".cmp("file2"));
    }

    #[test]
    fn filters() {
        let mut l = v(&["a", "b", "a", "", "", "c", "b", "a"]);
        let mut u = l.clone();
        unique(&mut u);
        assert_eq!(u, ["a", "b", "", "c"]);
        let mut d = l.clone();
        only_duplicates(&mut d);
        assert_eq!(d, ["a", "b", ""]);
        let mut c = l.clone();
        remove_consecutive_empty(&mut c);
        assert_eq!(c, ["a", "b", "a", "", "c", "b", "a"]);
        remove_empty(&mut l);
        assert_eq!(l, ["a", "b", "a", "c", "b", "a"]);
        assert_eq!(group_and_count(&l), ["3 a", "2 b", "1 c"]);
    }

    #[test]
    fn patterns() {
        assert!(Pattern::parse("foo").unwrap().matches("a foo b"));
        assert!(Pattern::parse("/^fo+$/").unwrap().matches("fooo"));
        assert!(Pattern::parse("/FOO/i").unwrap().matches("foo"));
        assert!(Pattern::parse("/a/b").unwrap().matches("x/a/b"));
        assert!(Pattern::parse("/(/").is_err());
    }

    #[test]
    fn aligns() {
        let l = v(&["  ab", "abcd ", " c"]);
        assert_eq!(align(&l, Align::Left), ["ab", "abcd", "c"]);
        assert_eq!(align(&l, Align::Right), ["  ab", "abcd", "   c"]);
        assert_eq!(align(&l, Align::Center), [" ab", "abcd", " c"]);
    }

    #[test]
    fn aligns_columns() {
        let l = v(&["a,bb,c", "ccc, d ,e", "  x"]);
        assert_eq!(align_columns(&l, ","), ["a   , bb , c", "ccc , d  , e", "  x"]);
        assert_eq!(align_columns(&v(&["a  1", "bbb 22"]), " "), ["a   1", "bbb 22"]);
        let t = v(&["| a | long |", "| bbb | c |"]);
        assert_eq!(align_columns(&t, "|"), ["| a   | long |", "| bbb | c    |"]);
    }

    #[test]
    fn sorts_tokens() {
        assert_eq!(sort_tokens("c, a,  b", ",", Sort::Asc), "a, b,  c");
        assert_eq!(sort_tokens("z,y,", ",", Sort::Asc), "y,z,");
        assert_eq!(sort_tokens("x", ",", Sort::Asc), "x");
    }

    #[test]
    fn shuffles_everything() {
        let mut items: Vec<u32> = (0..50).collect();
        Rng::new().shuffle(&mut items);
        let mut sorted = items.clone();
        sorted.sort();
        assert_eq!(sorted, (0..50).collect::<Vec<_>>());
    }
}
