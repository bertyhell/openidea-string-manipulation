//! Quotes, swaps, lists, borders and JSON.

type R = Result<String, String>;

pub fn straighten_quotes(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '“' | '”' | '„' | '‟' | '″' | '«' | '»' => '"',
            '‘' | '’' | '‚' | '‛' | '′' | '‹' | '›' => '\'',
            c => c,
        })
        .collect()
}

/// Typographic quotes: `"a" 'b' it's` → `“a” ‘b’ it’s`.
pub fn educate_quotes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev: Option<char> = None;
    for c in s.chars() {
        let opening = prev.is_none_or(|p| p.is_whitespace() || "([{<-—–/".contains(p));
        out.push(match (c, opening) {
            ('"', true) => '“',
            ('"', false) => '”',
            ('\'', true) => '‘',
            ('\'', false) => '’',
            (c, _) => c,
        });
        prev = Some(c);
    }
    out
}

const QUOTES: [char; 3] = ['"', '\'', '`'];

fn quoted_with(s: &str) -> Option<char> {
    let first = s.chars().next()?;
    (s.chars().count() >= 2 && QUOTES.contains(&first) && s.ends_with(first)).then_some(first)
}

/// Removes the quotes around `s`, or puts double quotes around it.
pub fn toggle_quotes(s: &str) -> String {
    match quoted_with(s) {
        Some(q) => s[q.len_utf8()..s.len() - q.len_utf8()].to_owned(),
        None => format!("\"{s}\""),
    }
}

/// `"x"` → `'x'` → `` `x` `` → `"x"`; unquoted text gets double quotes.
pub fn cycle_quotes(s: &str) -> String {
    match quoted_with(s) {
        Some(q) => {
            let next = QUOTES[(QUOTES.iter().position(|&c| c == q).unwrap_or(0) + 1) % QUOTES.len()];
            format!("{next}{}{next}", &s[1..s.len() - 1])
        }
        None => format!("\"{s}\""),
    }
}

pub fn swap_quotes(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '"' => '\'',
            '\'' => '"',
            c => c,
        })
        .collect()
}

/// Backslashes become slashes, or (when there are none) slashes backslashes.
pub fn switch_path_separators(s: &str) -> String {
    if s.contains('\\') { s.replace('\\', "/") } else { s.replace('/', "\\") }
}

/// Swaps the first and last word, keeping everything between them.
pub fn swap_words(s: &str) -> String {
    let spans: Vec<(usize, usize)> = {
        let mut v = Vec::new();
        let mut start = None;
        for (i, c) in s.char_indices() {
            let w = c.is_alphanumeric() || c == '_';
            match (w, start) {
                (true, None) => start = Some(i),
                (false, Some(st)) => {
                    v.push((st, i));
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(st) = start {
            v.push((st, s.len()));
        }
        v
    };
    if spans.len() < 2 {
        return s.to_owned();
    }
    let (a, b) = (spans[0], spans[spans.len() - 1]);
    format!("{}{}{}{}{}", &s[..a.0], &s[b.0..b.1], &s[a.1..b.0], &s[a.0..a.1], &s[b.1..])
}

pub fn reverse_chars(s: &str) -> String {
    s.chars().rev().collect()
}

/// Reads `\n`, `\t` and `\\` in a delimiter typed into a prompt.
pub fn unescape_delimiter(d: &str) -> String {
    d.replace("\\\\", "\u{0}").replace("\\n", "\n").replace("\\t", "\t").replace('\u{0}', "\\")
}

/// Splits on `from`, trims and drops empty items, wraps each in `quote` and
/// joins with `to`: `a, b, c` → `'a'\n'b'\n'c'`.
pub fn transform_list(s: &str, from: &str, to: &str, quote: &str) -> String {
    let items: Vec<&str> = if from.trim().is_empty() && !from.contains('\n') {
        s.split_whitespace().collect()
    } else {
        s.split(from).map(str::trim).filter(|t| !t.is_empty()).collect()
    };
    let close: String = quote
        .chars()
        .rev()
        .map(|c| match c {
            '(' => ')',
            '[' => ']',
            '{' => '}',
            '<' => '>',
            c => c,
        })
        .collect();
    items.iter().map(|i| format!("{quote}{i}{close}")).collect::<Vec<_>>().join(to)
}

#[derive(Clone, Copy)]
pub enum Border {
    Ascii,
    Single,
    Double,
    Rounded,
    Comment,
}

impl Border {
    pub const ALL: [(Border, &'static str, &'static str); 5] = [
        (Border::Single, "single", "┌─┐ single line"),
        (Border::Double, "double", "╔═╗ double line"),
        (Border::Rounded, "rounded", "╭─╮ rounded"),
        (Border::Ascii, "ascii", "+-+ ASCII"),
        (Border::Comment, "comment", "/** */ block comment"),
    ];

    pub fn from_key(key: &str) -> Option<Border> {
        Self::ALL.iter().find(|(_, k, _)| *k == key).map(|&(b, _, _)| b)
    }
}

/// Draws a box around the lines of `s`.
pub fn border(s: &str, style: Border) -> String {
    let lines: Vec<&str> = s.lines().collect();
    let indent =
        lines.iter().filter(|l| !l.trim().is_empty()).map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
    let body: Vec<&str> = lines.iter().map(|l| l.get(indent..).unwrap_or("").trim_end()).collect();
    let pad = " ".repeat(indent);
    let width = body.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let (tl, h, tr, v, bl, br) = match style {
        Border::Ascii => ("+", "-", "+", "|", "+", "+"),
        Border::Single => ("┌", "─", "┐", "│", "└", "┘"),
        Border::Double => ("╔", "═", "╗", "║", "╚", "╝"),
        Border::Rounded => ("╭", "─", "╮", "│", "╰", "╯"),
        Border::Comment => {
            let mut out = vec![format!("{pad}/*")];
            out.extend(body.iter().map(|l| if l.is_empty() { format!("{pad} *") } else { format!("{pad} * {l}") }));
            out.push(format!("{pad} */"));
            return out.join("\n");
        }
    };
    let mut out = vec![format!("{pad}{tl}{}{tr}", h.repeat(width + 2))];
    out.extend(body.iter().map(|l| format!("{pad}{v} {l}{} {v}", " ".repeat(width - l.chars().count()))));
    out.push(format!("{pad}{bl}{}{br}", h.repeat(width + 2)));
    out.join("\n")
}

fn parse_json(s: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(s).map_err(|e| format!("not valid JSON: {e}"))
}

fn sort_keys(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            map.sort_keys();
            map.values_mut().for_each(sort_keys);
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(sort_keys),
        _ => {}
    }
}

/// Indentation of the first indented line, two spaces if there is none.
fn json_indent(s: &str) -> String {
    s.lines().skip(1).map(|l| &l[..l.len() - l.trim_start().len()]).find(|i| !i.is_empty()).unwrap_or("  ").to_owned()
}

fn write_json(v: &serde_json::Value, like: &str) -> R {
    if !like.trim().contains('\n') {
        return serde_json::to_string(v).map_err(|e| e.to_string());
    }
    let indent = json_indent(like);
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    serde::Serialize::serialize(v, &mut ser).map_err(|e| e.to_string())?;
    String::from_utf8(buf).map_err(|e| e.to_string())
}

/// Object keys sorted at every level, in the layout it had (one line or
/// pretty-printed with its indentation).
pub fn json_sort(s: &str) -> R {
    let mut v = parse_json(s)?;
    sort_keys(&mut v);
    write_json(&v, s)
}

pub fn json_minify(s: &str) -> R {
    serde_json::to_string(&parse_json(s)?).map_err(|e| e.to_string())
}

pub fn json_pretty(s: &str) -> R {
    let v = parse_json(s)?;
    write_json(&v, "{\n  }")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes() {
        assert_eq!(straighten_quotes("“a” ‘b’ it’s"), "\"a\" 'b' it's");
        assert_eq!(educate_quotes("\"a\" ('b') it's"), "“a” (‘b’) it’s");
        assert_eq!(toggle_quotes("'x'"), "x");
        assert_eq!(toggle_quotes("x"), "\"x\"");
        assert_eq!(cycle_quotes("\"x\""), "'x'");
        assert_eq!(cycle_quotes("'x'"), "`x`");
        assert_eq!(cycle_quotes("`x`"), "\"x\"");
        assert_eq!(swap_quotes("say \"it's\""), "say 'it\"s'");
    }

    #[test]
    fn swaps_and_paths() {
        assert_eq!(swap_words("first, second"), "second, first");
        assert_eq!(swap_words("a b c"), "c b a");
        assert_eq!(swap_words("one"), "one");
        assert_eq!(switch_path_separators("C:\\a\\b"), "C:/a/b");
        assert_eq!(switch_path_separators("a/b"), "a\\b");
    }

    #[test]
    fn lists() {
        assert_eq!(transform_list("a, b,,c", ",", "\n", "'"), "'a'\n'b'\n'c'");
        assert_eq!(transform_list("a\nb\n", "\n", ", ", "\""), "\"a\", \"b\"");
        assert_eq!(transform_list("x y", " ", "|", "("), "(x)|(y)");
        assert_eq!(unescape_delimiter("\\n\\t\\\\n"), "\n\t\\n");
    }

    #[test]
    fn borders() {
        assert_eq!(border("  ab\n  c", Border::Ascii), "  +----+\n  | ab |\n  | c  |\n  +----+");
        assert_eq!(border("hi", Border::Comment), "/*\n * hi\n */");
    }

    #[test]
    fn json() {
        assert_eq!(
            json_sort("{\"b\":1,\"a\":{\"d\":[{\"z\":1,\"y\":2}],\"c\":0}}").unwrap(),
            "{\"a\":{\"c\":0,\"d\":[{\"y\":2,\"z\":1}]},\"b\":1}"
        );
        assert_eq!(json_sort("{\n    \"b\": 1,\n    \"a\": 2\n}").unwrap(), "{\n    \"a\": 2,\n    \"b\": 1\n}");
        assert_eq!(json_minify("{ \"b\" : [1, 2], \"a\": null }").unwrap(), "{\"b\":[1,2],\"a\":null}");
        assert_eq!(json_pretty("{\"a\":[1]}").unwrap(), "{\n  \"a\": [\n    1\n  ]\n}");
        assert!(json_minify("{").is_err());
    }
}
