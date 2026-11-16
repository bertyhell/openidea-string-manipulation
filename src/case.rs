//! Identifier case styles: splitting into words and joining them again.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Camel,
    Pascal,
    Snake,
    ScreamingSnake,
    CapitalizedSnake,
    Kebab,
    Dot,
    LowerWords,
    CapitalizedWords,
    Sentence,
}

/// Splits on separators and case changes: `fooBar_baz-qux` → foo, Bar, baz,
/// qux; `HTTPServer2Go` → HTTP, Server2, Go.
pub fn words(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        if let Some(&prev) = i.checked_sub(1).map(|p| &chars[p]) {
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            let boundary = c.is_uppercase()
                && !cur.is_empty()
                && (prev.is_lowercase() || prev.is_ascii_digit() || (prev.is_uppercase() && next_lower));
            if boundary {
                out.push(std::mem::take(&mut cur));
            }
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn capitalize_word(w: &str) -> String {
    let mut chars = w.chars();
    match chars.next() {
        Some(f) => f.to_uppercase().chain(chars.as_str().to_lowercase().chars()).collect(),
        None => String::new(),
    }
}

/// Joins the words of `s` in `style`. Text that has several identifiers
/// (`a_b c_d`, several lines) is converted one identifier at a time, keeping
/// what is between them, except for the styles that put spaces in.
pub fn convert(s: &str, style: Style) -> String {
    // A phrase on one line (`hello big world`) is one identifier.
    let t = s.trim();
    if t.contains(' ') && t.chars().all(|c| c.is_alphanumeric() || " _-.".contains(c)) {
        let lead = s.len() - s.trim_start().len();
        return format!("{}{}{}", &s[..lead], join(&words(t), style), &s[lead + t.len()..]);
    }
    if matches!(style, Style::LowerWords | Style::CapitalizedWords | Style::Sentence) {
        return map_runs(s, |run| join(&words(run), style), |c| c.is_alphanumeric() || "_-.$ ".contains(c));
    }
    map_runs(s, |run| join(&words(run), style), |c| c.is_alphanumeric() || "_-.$".contains(c))
}

/// Applies `f` to every maximal run of chars matching `part`, leaving the rest.
fn map_runs(s: &str, f: impl Fn(&str) -> String, part: impl Fn(char) -> bool) -> String {
    let mut out = String::with_capacity(s.len());
    let mut start = None;
    for (i, c) in s.char_indices() {
        match (part(c), start) {
            (true, None) => start = Some(i),
            (false, Some(st)) => {
                push_run(&mut out, &s[st..i], &f);
                start = None;
                out.push(c);
            }
            (false, None) => out.push(c),
            (true, Some(_)) => {}
        }
    }
    if let Some(st) = start {
        push_run(&mut out, &s[st..], &f);
    }
    out
}

/// Keeps a run's leading and trailing spaces out of the conversion.
fn push_run(out: &mut String, run: &str, f: &impl Fn(&str) -> String) {
    let trimmed = run.trim_matches(' ');
    let lead = run.len() - run.trim_start_matches(' ').len();
    out.push_str(&run[..lead]);
    if !trimmed.is_empty() {
        out.push_str(&f(trimmed));
    }
    out.push_str(&run[lead + trimmed.len()..]);
}

pub fn join(words: &[String], style: Style) -> String {
    let lower = || words.iter().map(|w| w.to_lowercase());
    match style {
        Style::Camel => {
            words.iter().enumerate().map(|(i, w)| if i == 0 { w.to_lowercase() } else { capitalize_word(w) }).collect()
        }
        Style::Pascal => words.iter().map(|w| capitalize_word(w)).collect(),
        Style::Snake => lower().collect::<Vec<_>>().join("_"),
        Style::ScreamingSnake => words.iter().map(|w| w.to_uppercase()).collect::<Vec<_>>().join("_"),
        Style::CapitalizedSnake => words.iter().map(|w| capitalize_word(w)).collect::<Vec<_>>().join("_"),
        Style::Kebab => lower().collect::<Vec<_>>().join("-"),
        Style::Dot => lower().collect::<Vec<_>>().join("."),
        Style::LowerWords => lower().collect::<Vec<_>>().join(" "),
        Style::CapitalizedWords => words.iter().map(|w| capitalize_word(w)).collect::<Vec<_>>().join(" "),
        Style::Sentence => words
            .iter()
            .enumerate()
            .map(|(i, w)| if i == 0 { capitalize_word(w) } else { w.to_lowercase() })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// The style `s` is written in, as far as it can tell.
pub fn detect(s: &str) -> Option<Style> {
    let s = s.trim();
    let has = |c: char| s.contains(c);
    let any_lower = s.chars().any(char::is_lowercase);
    let any_upper = s.chars().any(char::is_uppercase);
    let first_upper = s.chars().next().is_some_and(char::is_uppercase);
    Some(if has(' ') {
        let ws = words(s);
        if ws.len() > 1 && ws.iter().all(|w| w.chars().next().is_some_and(char::is_uppercase)) {
            Style::CapitalizedWords
        } else if first_upper {
            Style::Sentence
        } else {
            Style::LowerWords
        }
    } else if has('_') {
        if !any_lower {
            Style::ScreamingSnake
        } else if first_upper {
            Style::CapitalizedSnake
        } else {
            Style::Snake
        }
    } else if has('-') {
        Style::Kebab
    } else if has('.') {
        Style::Dot
    } else if first_upper && any_lower {
        Style::Pascal
    } else if any_lower && any_upper {
        Style::Camel
    } else {
        return None;
    })
}

const CYCLE: [Style; 5] = [Style::Camel, Style::Snake, Style::ScreamingSnake, Style::Kebab, Style::Pascal];

/// "Switch Case": camelCase → snake_case → SCREAMING_SNAKE_CASE → kebab-case →
/// PascalCase → camelCase, skipping styles that would not change the text.
pub fn switch(s: &str) -> String {
    let start = match detect(s) {
        Some(style) => CYCLE.iter().position(|&c| c == style).unwrap_or(CYCLE.len() - 1),
        // One lowercase word reads as camelCase (and snake_case, kebab-case…).
        None if s.chars().any(char::is_lowercase) => 0,
        None => 3,
    };
    for step in 1..=CYCLE.len() {
        let next = convert(s, CYCLE[(start + step) % CYCLE.len()]);
        if next != s {
            return next;
        }
    }
    s.to_owned()
}

/// "X / camelCase": text in `style` becomes camelCase, anything else `style`.
pub fn toggle(s: &str, style: Style) -> String {
    let converted = convert(s, style);
    if converted == s { convert(s, Style::Camel) } else { converted }
}

pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(f) => f.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

pub fn invert(s: &str) -> String {
    s.chars()
        .flat_map(|c| -> Box<dyn Iterator<Item = char>> {
            if c.is_uppercase() {
                Box::new(c.to_lowercase())
            } else if c.is_lowercase() {
                Box::new(c.to_uppercase())
            } else {
                Box::new(std::iter::once(c))
            }
        })
        .collect()
}

/// A Spring Boot property name as an environment variable (relaxed binding):
/// `spring.main.log-startup-info` → `SPRING_MAIN_LOGSTARTUPINFO`,
/// `my.list[0].name` → `MY_LIST_0_NAME`.
pub fn spring_env(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.trim().chars() {
        match c {
            '.' | '[' => out.push('_'),
            '-' | ']' => {}
            c => out.extend(c.to_uppercase()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_words() {
        assert_eq!(words("fooBar_baz-qux"), ["foo", "Bar", "baz", "qux"]);
        assert_eq!(words("HTTPServer2Go"), ["HTTP", "Server2", "Go"]);
        assert_eq!(words("parseURL"), ["parse", "URL"]);
        assert_eq!(words("v2Api"), ["v2", "Api"]);
    }

    #[test]
    fn converts_styles() {
        let s = "user_account-id";
        assert_eq!(convert(s, Style::Camel), "userAccountId");
        assert_eq!(convert(s, Style::Pascal), "UserAccountId");
        assert_eq!(convert("userAccountId", Style::Snake), "user_account_id");
        assert_eq!(convert("userAccountId", Style::ScreamingSnake), "USER_ACCOUNT_ID");
        assert_eq!(convert("userAccountId", Style::CapitalizedSnake), "User_Account_Id");
        assert_eq!(convert("userAccountId", Style::Kebab), "user-account-id");
        assert_eq!(convert("userAccountId", Style::Dot), "user.account.id");
        assert_eq!(convert("userAccountId", Style::LowerWords), "user account id");
        assert_eq!(convert("userAccountId", Style::CapitalizedWords), "User Account Id");
        assert_eq!(convert("userAccountId", Style::Sentence), "User account id");
        assert_eq!(convert("hello world", Style::Camel), "helloWorld");
        assert_eq!(convert("  Hello big World ", Style::Snake), "  hello_big_world ");
        assert_eq!(convert("hello world", Style::CapitalizedWords), "Hello World");
    }

    #[test]
    fn converts_each_identifier_and_keeps_the_rest() {
        assert_eq!(convert("let fooBar = bazQux;", Style::Snake), "let foo_bar = baz_qux;");
        assert_eq!(convert("a_b\n  c_d", Style::Camel), "aB\n  cD");
        assert_eq!(convert("(some words) here", Style::CapitalizedWords), "(Some Words) Here");
    }

    #[test]
    fn switches_through_the_cycle() {
        assert_eq!(switch("fooBar"), "foo_bar");
        assert_eq!(switch("foo_bar"), "FOO_BAR");
        assert_eq!(switch("FOO_BAR"), "foo-bar");
        assert_eq!(switch("foo-bar"), "FooBar");
        assert_eq!(switch("FooBar"), "fooBar");
        assert_eq!(switch("foo"), "FOO");
        assert_eq!(switch("FOO"), "Foo");
        assert_eq!(switch("Foo"), "foo");
    }

    #[test]
    fn toggles_with_camel() {
        assert_eq!(toggle("fooBar", Style::Snake), "foo_bar");
        assert_eq!(toggle("foo_bar", Style::Snake), "fooBar");
        assert_eq!(toggle("foo bar", Style::LowerWords), "fooBar");
    }

    #[test]
    fn other_case_changes() {
        assert_eq!(capitalize("élan vital"), "Élan vital");
        assert_eq!(invert("Hello World"), "hELLO wORLD");
        assert_eq!(spring_env("spring.main.log-startup-info"), "SPRING_MAIN_LOGSTARTUPINFO");
        assert_eq!(spring_env("my.list[0].name"), "MY_LIST_0_NAME");
    }
}
