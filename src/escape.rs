//! Escaping and unescaping string literals and markup.

use std::fmt::Write;

type R = Result<String, String>;

fn push_u16(out: &mut String, c: char) {
    let mut buf = [0u16; 2];
    for unit in c.encode_utf16(&mut buf) {
        let _ = write!(out, "\\u{unit:04X}");
    }
}

/// Java (and Kotlin, Groovy) string literal contents.
pub fn escape_java(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if c.is_control() => push_u16(&mut out, c),
            c => out.push(c),
        }
    }
    out
}

pub fn escape_javascript(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\'' => out.push_str("\\'"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\u{b}' => out.push_str("\\v"),
            '\u{2028}' | '\u{2029}' => push_u16(&mut out, c),
            c if c.is_control() => push_u16(&mut out, c),
            c => out.push(c),
        }
    }
    out
}

/// A regular C# string literal (`"…"`).
pub fn escape_csharp(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\0' => out.push_str("\\0"),
            '\u{7}' => out.push_str("\\a"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{b}' => out.push_str("\\v"),
            c if c.is_control() => push_u16(&mut out, c),
            c => out.push(c),
        }
    }
    out
}

/// A double-quoted PHP string.
pub fn escape_php(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '$' => out.push_str("\\$"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{b}' => out.push_str("\\v"),
            '\u{1b}' => out.push_str("\\e"),
            '\u{c}' => out.push_str("\\f"),
            c if c.is_control() => {
                let _ = write!(out, "\\x{:02X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

pub fn escape_json(s: &str) -> String {
    let quoted = serde_json::to_string(s).unwrap_or_default();
    quoted[1..quoted.len() - 1].to_owned()
}

pub fn unescape_json(s: &str) -> R {
    let (open, inner, close) = strip_quotes(s, '"');
    let value: String = serde_json::from_str(&format!("\"{inner}\"")).map_err(|e| format!("not a JSON string: {e}"))?;
    Ok(format!("{open}{value}{close}"))
}

fn strip_quotes(s: &str, q: char) -> (&str, &str, &str) {
    if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
        let n = q.len_utf8();
        (&s[..n], &s[n..s.len() - n], &s[s.len() - n..])
    } else {
        ("", s, "")
    }
}

pub fn escape_sql(s: &str) -> String {
    s.replace('\'', "''")
}

pub fn unescape_sql(s: &str) -> String {
    s.replace("''", "'")
}

#[derive(Clone, Copy, PartialEq)]
pub enum Dialect {
    Java,
    JavaScript,
    CSharp,
    Php,
}

/// Unescapes backslash sequences the way `dialect` reads them. Unknown
/// sequences are kept as written.
pub fn unescape(s: &str, dialect: Dialect) -> R {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut pending_high: Option<u16> = None;
    let hex = |from: usize, n: usize| -> Option<u32> {
        let digits: String = chars.get(from..from + n)?.iter().collect();
        if digits.chars().all(|c| c.is_ascii_hexdigit()) { u32::from_str_radix(&digits, 16).ok() } else { None }
    };
    let push_code = |out: &mut String, pending: &mut Option<u16>, code: u32| {
        if (0xD800..0xDC00).contains(&code) {
            *pending = Some(code as u16);
            return;
        }
        if (0xDC00..0xE000).contains(&code)
            && let Some(high) = pending.take()
        {
            let c = 0x10000 + ((u32::from(high) - 0xD800) << 10) + (code - 0xDC00);
            out.extend(char::from_u32(c));
            return;
        }
        if let Some(high) = pending.take() {
            out.push(char::from_u32(u32::from(high)).unwrap_or('\u{FFFD}'));
        }
        out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
    };
    while i < chars.len() {
        let c = chars[i];
        if c != '\\' || i + 1 == chars.len() {
            if let Some(high) = pending_high.take() {
                out.push(char::from_u32(u32::from(high)).unwrap_or('\u{FFFD}'));
            }
            out.push(c);
            i += 1;
            continue;
        }
        let e = chars[i + 1];
        let simple = match e {
            'n' => Some('\n'),
            'r' => Some('\r'),
            't' => Some('\t'),
            'f' => Some('\u{c}'),
            'b' if dialect != Dialect::Php => Some('\u{8}'),
            'v' if dialect != Dialect::Java => Some('\u{b}'),
            'a' if dialect == Dialect::CSharp => Some('\u{7}'),
            'e' if dialect == Dialect::Php => Some('\u{1b}'),
            '$' if dialect == Dialect::Php => Some('$'),
            '\\' | '"' => Some(e),
            '\'' if dialect != Dialect::Php => Some('\''),
            '/' if dialect == Dialect::JavaScript => Some('/'),
            _ => None,
        };
        if let Some(ch) = simple {
            push_code(&mut out, &mut pending_high, ch as u32);
            i += 2;
            continue;
        }
        // \u{1F600} (JavaScript, PHP)
        if e == 'u'
            && chars.get(i + 2) == Some(&'{')
            && matches!(dialect, Dialect::JavaScript | Dialect::Php)
            && let Some(end) = chars[i + 3..].iter().position(|&c| c == '}')
        {
            let digits: String = chars[i + 3..i + 3 + end].iter().collect();
            if let Ok(code) = u32::from_str_radix(&digits, 16) {
                push_code(&mut out, &mut pending_high, code);
                i += 4 + end;
                continue;
            }
        }
        if e == 'u' && dialect != Dialect::Php {
            // Java allows \uuuu0041.
            let mut j = i + 2;
            while dialect == Dialect::Java && chars.get(j) == Some(&'u') {
                j += 1;
            }
            if let Some(code) = hex(j, 4) {
                push_code(&mut out, &mut pending_high, code);
                i = j + 4;
                continue;
            }
        }
        if e == 'U'
            && dialect == Dialect::CSharp
            && let Some(code) = hex(i + 2, 8)
        {
            push_code(&mut out, &mut pending_high, code);
            i += 10;
            continue;
        }
        if e == 'x' && dialect != Dialect::Java {
            let max = match dialect {
                Dialect::CSharp => 4,
                _ => 2,
            };
            let n = (1..=max)
                .rev()
                .find(|&n| hex(i + 2, n).is_some())
                .filter(|&n| dialect != Dialect::JavaScript || n == 2);
            if let Some(n) = n {
                push_code(&mut out, &mut pending_high, hex(i + 2, n).unwrap_or(0));
                i += 2 + n;
                continue;
            }
        }
        if e.is_digit(8) && dialect != Dialect::CSharp {
            let n = chars[i + 1..].iter().take(3).take_while(|c| c.is_digit(8)).count();
            let digits: String = chars[i + 1..i + 1 + n].iter().collect();
            if let Ok(code) = u32::from_str_radix(&digits, 8)
                && code <= 0o377
            {
                push_code(&mut out, &mut pending_high, code);
                i += 1 + n;
                continue;
            }
        }
        if e == '0' && dialect == Dialect::CSharp {
            push_code(&mut out, &mut pending_high, 0);
            i += 2;
            continue;
        }
        out.push('\\');
        out.push(e);
        i += 2;
    }
    if let Some(high) = pending_high.take() {
        out.push(char::from_u32(u32::from(high)).unwrap_or('\u{FFFD}'));
    }
    Ok(out)
}

pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

pub fn escape_xml(s: &str) -> String {
    escape_html(s).replace("&#39;", "&apos;")
}

/// Named HTML entities beyond Latin-1 that show up in practice.
const ENTITIES: &[(&str, char)] = &[
    ("amp", '&'),
    ("lt", '<'),
    ("gt", '>'),
    ("quot", '"'),
    ("apos", '\''),
    ("OElig", 'Œ'),
    ("oelig", 'œ'),
    ("Scaron", 'Š'),
    ("scaron", 'š'),
    ("Yuml", 'Ÿ'),
    ("fnof", 'ƒ'),
    ("circ", 'ˆ'),
    ("tilde", '˜'),
    ("ensp", '\u{2002}'),
    ("emsp", '\u{2003}'),
    ("thinsp", '\u{2009}'),
    ("zwnj", '\u{200C}'),
    ("zwj", '\u{200D}'),
    ("lrm", '\u{200E}'),
    ("rlm", '\u{200F}'),
    ("ndash", '–'),
    ("mdash", '—'),
    ("lsquo", '‘'),
    ("rsquo", '’'),
    ("sbquo", '‚'),
    ("ldquo", '“'),
    ("rdquo", '”'),
    ("bdquo", '„'),
    ("dagger", '†'),
    ("Dagger", '‡'),
    ("bull", '•'),
    ("hellip", '…'),
    ("permil", '‰'),
    ("prime", '′'),
    ("Prime", '″'),
    ("lsaquo", '‹'),
    ("rsaquo", '›'),
    ("oline", '‾'),
    ("euro", '€'),
    ("trade", '™'),
    ("larr", '←'),
    ("uarr", '↑'),
    ("rarr", '→'),
    ("darr", '↓'),
    ("harr", '↔'),
    ("lArr", '⇐'),
    ("rArr", '⇒'),
    ("hArr", '⇔'),
    ("forall", '∀'),
    ("part", '∂'),
    ("exist", '∃'),
    ("empty", '∅'),
    ("nabla", '∇'),
    ("isin", '∈'),
    ("notin", '∉'),
    ("sum", '∑'),
    ("minus", '−'),
    ("radic", '√'),
    ("infin", '∞'),
    ("and", '∧'),
    ("or", '∨'),
    ("cap", '∩'),
    ("cup", '∪'),
    ("int", '∫'),
    ("asymp", '≈'),
    ("ne", '≠'),
    ("equiv", '≡'),
    ("le", '≤'),
    ("ge", '≥'),
    ("alpha", 'α'),
    ("beta", 'β'),
    ("gamma", 'γ'),
    ("delta", 'δ'),
    ("epsilon", 'ε'),
    ("lambda", 'λ'),
    ("mu", 'μ'),
    ("pi", 'π'),
    ("sigma", 'σ'),
    ("tau", 'τ'),
    ("phi", 'φ'),
    ("omega", 'ω'),
    ("Delta", 'Δ'),
    ("Omega", 'Ω'),
    ("hearts", '♥'),
];

/// Latin-1 entity names, `&nbsp;` (U+00A0) to `&yuml;` (U+00FF), in order.
const LATIN1: [&str; 96] = [
    "nbsp", "iexcl", "cent", "pound", "curren", "yen", "brvbar", "sect", "uml", "copy", "ordf", "laquo", "not", "shy",
    "reg", "macr", "deg", "plusmn", "sup2", "sup3", "acute", "micro", "para", "middot", "cedil", "sup1", "ordm",
    "raquo", "frac14", "frac12", "frac34", "iquest", "Agrave", "Aacute", "Acirc", "Atilde", "Auml", "Aring", "AElig",
    "Ccedil", "Egrave", "Eacute", "Ecirc", "Euml", "Igrave", "Iacute", "Icirc", "Iuml", "ETH", "Ntilde", "Ograve",
    "Oacute", "Ocirc", "Otilde", "Ouml", "times", "Oslash", "Ugrave", "Uacute", "Ucirc", "Uuml", "Yacute", "THORN",
    "szlig", "agrave", "aacute", "acirc", "atilde", "auml", "aring", "aelig", "ccedil", "egrave", "eacute", "ecirc",
    "euml", "igrave", "iacute", "icirc", "iuml", "eth", "ntilde", "ograve", "oacute", "ocirc", "otilde", "ouml",
    "divide", "oslash", "ugrave", "uacute", "ucirc", "uuml", "yacute", "thorn", "yuml",
];

fn entity(name: &str, xml_only: bool) -> Option<char> {
    if let Some(num) = name.strip_prefix('#') {
        let code = match num.strip_prefix(['x', 'X']) {
            Some(h) => u32::from_str_radix(h, 16).ok()?,
            None => num.parse().ok()?,
        };
        return char::from_u32(code);
    }
    if xml_only {
        return ENTITIES[..5].iter().find(|(n, _)| *n == name).map(|&(_, c)| c);
    }
    ENTITIES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|&(_, c)| c)
        .or_else(|| LATIN1.iter().position(|n| *n == name).and_then(|i| char::from_u32(0xA0 + i as u32)))
}

fn unescape_entities(s: &str, xml_only: bool) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        let decoded = after
            .find(';')
            .filter(|&semi| semi > 0 && semi <= 32)
            .and_then(|semi| entity(&after[..semi], xml_only).map(|c| (c, semi)));
        match decoded {
            Some((c, semi)) => {
                out.push(c);
                rest = &after[semi + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn unescape_html(s: &str) -> String {
    unescape_entities(s, false)
}

pub fn unescape_xml(s: &str) -> String {
    unescape_entities(s, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_round_trip() {
        let s = "say \"hi\"\\\n\ttab\u{1}é";
        let e = escape_java(s);
        assert_eq!(e, "say \\\"hi\\\"\\\\\\n\\ttab\\u0001é");
        assert_eq!(unescape(&e, Dialect::Java).unwrap(), s);
        assert_eq!(unescape("\\uD83D\\uDE00 \\101 \\uuu0041", Dialect::Java).unwrap(), "😀 A A");
        assert_eq!(unescape("keep \\q", Dialect::Java).unwrap(), "keep \\q");
    }

    #[test]
    fn javascript_round_trip() {
        let s = "it's \"x\"\n\u{2028}";
        let e = escape_javascript(s);
        assert_eq!(e, "it\\'s \\\"x\\\"\\n\\u2028");
        assert_eq!(unescape(&e, Dialect::JavaScript).unwrap(), s);
        assert_eq!(unescape("\\x41\\u{1F600}\\/", Dialect::JavaScript).unwrap(), "A😀/");
    }

    #[test]
    fn csharp_and_php() {
        assert_eq!(escape_csharp("a\0\"b"), "a\\0\\\"b");
        assert_eq!(unescape("\\x41\\U0001F600\\a", Dialect::CSharp).unwrap(), "A😀\u{7}");
        assert_eq!(escape_php("cost: $5\n"), "cost: \\$5\\n");
        assert_eq!(unescape("cost: \\$5\\n\\x41\\101\\u{e9}", Dialect::Php).unwrap(), "cost: $5\nAAé");
    }

    #[test]
    fn json() {
        assert_eq!(escape_json("a\"b\n\u{1}"), "a\\\"b\\n\\u0001");
        assert_eq!(unescape_json("a\\\"b\\n").unwrap(), "a\"b\n");
        assert_eq!(unescape_json("\"x\\ty\"").unwrap(), "\"x\ty\"");
        assert!(unescape_json("bad \\q").is_err());
    }

    #[test]
    fn markup() {
        assert_eq!(
            escape_html("<a href=\"x\">Tom & 'Jerry'</a>"),
            "&lt;a href=&quot;x&quot;&gt;Tom &amp; &#39;Jerry&#39;&lt;/a&gt;"
        );
        assert_eq!(escape_xml("'"), "&apos;");
        assert_eq!(
            unescape_html("&lt;p&gt; caf&eacute; &copy; &#169; &#xA9; &hellip; &bogus; & x"),
            "<p> café © © © … &bogus; & x"
        );
        assert_eq!(unescape_xml("&lt;&eacute;&apos;"), "<&eacute;'");
    }

    #[test]
    fn sql() {
        assert_eq!(escape_sql("O'Brien"), "O''Brien");
        assert_eq!(unescape_sql("O''Brien"), "O'Brien");
    }
}
