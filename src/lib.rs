//! String Manipulation for OpenIDEA: case switching, escaping, encoding,
//! sorting, filtering, aligning and incrementing text, on every selection of
//! the focused editor at once, as one undoable change.
//!
//! What a command works on without a selection depends on the command: case
//! changes take the identifier at the caret, increments the number at the
//! caret, line commands the whole document (or, with several carets, their
//! lines), and the rest the caret's line without its indentation.

mod case;
mod encode;
mod escape;
mod lines;
mod numbers;
mod transform;
mod unicode;

use case::Style;
use escape::Dialect;
use lines::{Align, Pattern, Rng, Sort};
use openidea_plugin::text::{offset, position};
use openidea_plugin::{
    Document, Error, Invocation, PickItem, Plugin, Position, Range, TextEdit, TextEditor, host, register_plugin,
};
use transform::Border;

#[derive(Default)]
pub struct StringManipulation;

impl Plugin for StringManipulation {
    fn run_command(&mut self, id: &str, ctx: &Invocation, arg: Option<&str>) -> Result<Option<String>, Error> {
        let editor = ctx.editor().ok_or("String Manipulation works on the focused editor")?;
        run(&editor, id, arg)?;
        Ok(None)
    }
}

register_plugin!(StringManipulation);

type R = Result<String, String>;

/// What a command takes when a selection is empty.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// The identifier at the caret.
    Word,
    /// The identifier at the caret with the quotes around it.
    Quoted,
    /// The integer at the caret (or the identifier).
    Number,
    /// The caret's line without its indentation and trailing spaces.
    Line,
    /// The whole document.
    Document,
}

enum Op {
    Text(Scope, fn(&str) -> R),
    Lines(fn(Vec<String>) -> Vec<String>),
    /// Asks for what it needs (unless given `arg`) and returns the argument
    /// to replay it with.
    Custom(fn(&TextEditor, Option<&str>) -> Result<Option<String>, Error>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
    Menu,
    Case,
    Escape,
    Unicode,
    Encode,
    Numbers,
    Sort,
    Filter,
    Align,
    Transform,
}

impl Group {
    const LISTED: [Group; 9] = [
        Group::Case,
        Group::Escape,
        Group::Unicode,
        Group::Encode,
        Group::Numbers,
        Group::Sort,
        Group::Filter,
        Group::Align,
        Group::Transform,
    ];

    fn label(self) -> &'static str {
        match self {
            Group::Menu => "Menus",
            Group::Case => "Switch Case",
            Group::Escape => "Escape/Unescape",
            Group::Unicode => "Unicode",
            Group::Encode => "Encode/Decode",
            Group::Numbers => "Increment/Decrement",
            Group::Sort => "Sort/Shuffle/Reverse",
            Group::Filter => "Filter/Remove/Trim",
            Group::Align => "Align",
            Group::Transform => "Transform",
        }
    }
}

pub struct Action {
    pub id: &'static str,
    pub title: &'static str,
    group: Group,
    op: Op,
}

const fn a(id: &'static str, title: &'static str, group: Group, op: Op) -> Action {
    Action { id, title, group, op }
}

use Group as G;
use Op::{Custom, Lines as L, Text as T};
use Scope::{Document as Doc, Line, Number, Quoted, Word};

macro_rules! ok {
    ($f:expr) => {
        |s| Ok($f(s))
    };
}

fn to(style: Style) -> impl Fn(&str) -> String {
    move |s| case::convert(s, style)
}

/// Every command, in menu order. `plugin.toml` declares the same ids and titles
/// (a test checks it).
pub static ACTIONS: &[Action] = &[
    a("popup", "String Manipulation…", G::Menu, Custom(|e, _| menu(e, None))),
    a("repeat-last", "Repeat Last Action", G::Menu, Custom(|e, _| repeat_last(e))),
    a("case-menu", "Switch Case…", G::Menu, Custom(|e, _| menu(e, Some(G::Case)))),
    a("escape-menu", "Escape/Unescape…", G::Menu, Custom(|e, _| menu(e, Some(G::Escape)))),
    a("unicode-menu", "Unicode…", G::Menu, Custom(|e, _| menu(e, Some(G::Unicode)))),
    a("encode-menu", "Encode/Decode…", G::Menu, Custom(|e, _| menu(e, Some(G::Encode)))),
    a("increment-menu", "Increment/Decrement…", G::Menu, Custom(|e, _| menu(e, Some(G::Numbers)))),
    a("sort-menu", "Sort/Shuffle/Reverse…", G::Menu, Custom(|e, _| menu(e, Some(G::Sort)))),
    a("filter-menu", "Filter/Remove/Trim…", G::Menu, Custom(|e, _| menu(e, Some(G::Filter)))),
    a("align-menu", "Align…", G::Menu, Custom(|e, _| menu(e, Some(G::Align)))),
    a("transform-menu", "Transform…", G::Menu, Custom(|e, _| menu(e, Some(G::Transform)))),
    // Case
    a(
        "switch-case",
        "Switch Case (camelCase → snake_case → SCREAMING_SNAKE_CASE → kebab-case → PascalCase)",
        G::Case,
        T(Word, ok!(case::switch)),
    ),
    a(
        "toggle-screaming-snake-camel",
        "Toggle SCREAMING_SNAKE_CASE / camelCase",
        G::Case,
        T(Word, |s| Ok(case::toggle(s, Style::ScreamingSnake))),
    ),
    a("toggle-snake-camel", "Toggle snake_case / camelCase", G::Case, T(Word, |s| Ok(case::toggle(s, Style::Snake)))),
    a(
        "toggle-kebab-snake",
        "Toggle kebab-case / snake_case",
        G::Case,
        T(Word, |s| {
            Ok(if case::convert(s, Style::Kebab) == s {
                case::convert(s, Style::Snake)
            } else {
                case::convert(s, Style::Kebab)
            })
        }),
    ),
    a("toggle-kebab-camel", "Toggle kebab-case / camelCase", G::Case, T(Word, |s| Ok(case::toggle(s, Style::Kebab)))),
    a("toggle-dot-camel", "Toggle dot.case / camelCase", G::Case, T(Word, |s| Ok(case::toggle(s, Style::Dot)))),
    a("toggle-pascal-camel", "Toggle PascalCase / camelCase", G::Case, T(Word, |s| Ok(case::toggle(s, Style::Pascal)))),
    a(
        "toggle-words-camel",
        "Toggle lower case words / camelCase",
        G::Case,
        T(Line, |s| Ok(case::toggle(s, Style::LowerWords))),
    ),
    a(
        "toggle-sentence-camel",
        "Toggle First word capitalized / camelCase",
        G::Case,
        T(Line, |s| Ok(case::toggle(s, Style::Sentence))),
    ),
    a(
        "toggle-capitalized-words-camel",
        "Toggle Capitalized Words / camelCase",
        G::Case,
        T(Line, |s| Ok(case::toggle(s, Style::CapitalizedWords))),
    ),
    a("to-camel-case", "To camelCase", G::Case, T(Word, |s| Ok(to(Style::Camel)(s)))),
    a("to-pascal-case", "To PascalCase", G::Case, T(Word, |s| Ok(to(Style::Pascal)(s)))),
    a("to-snake-case", "To snake_case", G::Case, T(Word, |s| Ok(to(Style::Snake)(s)))),
    a("to-screaming-snake-case", "To SCREAMING_SNAKE_CASE", G::Case, T(Word, |s| Ok(to(Style::ScreamingSnake)(s)))),
    a(
        "to-capitalized-snake-case",
        "To Capitalized_Snake_Case",
        G::Case,
        T(Word, |s| Ok(to(Style::CapitalizedSnake)(s))),
    ),
    a("to-kebab-case", "To kebab-case", G::Case, T(Word, |s| Ok(to(Style::Kebab)(s)))),
    a("to-dot-case", "To dot.case", G::Case, T(Word, |s| Ok(to(Style::Dot)(s)))),
    a("to-lower-words", "To lower case words", G::Case, T(Word, |s| Ok(to(Style::LowerWords)(s)))),
    a("to-capitalized-words", "To Capitalized Words", G::Case, T(Line, |s| Ok(to(Style::CapitalizedWords)(s)))),
    a("to-sentence-case", "To First word capitalized", G::Case, T(Line, |s| Ok(to(Style::Sentence)(s)))),
    a("capitalize", "Capitalize", G::Case, T(Word, ok!(case::capitalize))),
    a("to-lower-case", "To lower case", G::Case, T(Word, |s| Ok(s.to_lowercase()))),
    a("to-upper-case", "To UPPER CASE", G::Case, T(Word, |s| Ok(s.to_uppercase()))),
    a("invert-case", "Invert Case", G::Case, T(Word, ok!(case::invert))),
    a("to-spring-env", "To Spring Boot Environment Variable", G::Case, T(Line, ok!(case::spring_env))),
    // Escape
    a("escape-java", "Escape Java", G::Escape, T(Line, ok!(escape::escape_java))),
    a("unescape-java", "Unescape Java", G::Escape, T(Line, |s| escape::unescape(s, Dialect::Java))),
    a("escape-javascript", "Escape JavaScript", G::Escape, T(Line, ok!(escape::escape_javascript))),
    a("unescape-javascript", "Unescape JavaScript", G::Escape, T(Line, |s| escape::unescape(s, Dialect::JavaScript))),
    a("escape-csharp", "Escape C#", G::Escape, T(Line, ok!(escape::escape_csharp))),
    a("unescape-csharp", "Unescape C#", G::Escape, T(Line, |s| escape::unescape(s, Dialect::CSharp))),
    a("escape-json", "Escape JSON", G::Escape, T(Line, ok!(escape::escape_json))),
    a("unescape-json", "Unescape JSON", G::Escape, T(Line, escape::unescape_json)),
    a("escape-html", "Escape HTML", G::Escape, T(Line, ok!(escape::escape_html))),
    a("unescape-html", "Unescape HTML", G::Escape, T(Line, ok!(escape::unescape_html))),
    a("escape-xml", "Escape XML", G::Escape, T(Line, ok!(escape::escape_xml))),
    a("unescape-xml", "Unescape XML", G::Escape, T(Line, ok!(escape::unescape_xml))),
    a("escape-sql", "Escape SQL", G::Escape, T(Line, ok!(escape::escape_sql))),
    a("unescape-sql", "Unescape SQL", G::Escape, T(Line, ok!(escape::unescape_sql))),
    a("escape-php", "Escape PHP", G::Escape, T(Line, ok!(escape::escape_php))),
    a("unescape-php", "Unescape PHP", G::Escape, T(Line, |s| escape::unescape(s, Dialect::Php))),
    // Unicode
    a("to-ascii", "Convert Diacritics (Accents) to ASCII", G::Unicode, T(Line, ok!(unicode::to_ascii))),
    a("escape-unicode", "Escape Non-ASCII to Unicode (\\uXXXX)", G::Unicode, T(Line, ok!(unicode::escape_non_ascii))),
    a("unescape-unicode", "Unescape Unicode (\\uXXXX) to Text", G::Unicode, T(Line, ok!(unicode::unescape_unicode))),
    a("normalize-nfc", "Unicode Normalization C (NFC)", G::Unicode, T(Line, ok!(unicode::nfc))),
    a("normalize-nfd", "Unicode Normalization D (NFD)", G::Unicode, T(Line, ok!(unicode::nfd))),
    a("normalize-nfkc", "Unicode Normalization KC (NFKC)", G::Unicode, T(Line, ok!(unicode::nfkc))),
    a("normalize-nfkd", "Unicode Normalization KD (NFKD)", G::Unicode, T(Line, ok!(unicode::nfkd))),
    // Encode
    a("encode-base64", "Encode to Base64", G::Encode, T(Line, ok!(encode::base64_encode))),
    a("decode-base64", "Decode Base64", G::Encode, T(Line, encode::base64_decode)),
    a("encode-hex", "Encode to Hex", G::Encode, T(Line, ok!(encode::hex_encode))),
    a("decode-hex", "Decode Hex", G::Encode, T(Line, encode::hex_decode)),
    a("encode-url", "Encode URL (form, space as +)", G::Encode, T(Line, ok!(encode::url_encode))),
    a("decode-url", "Decode URL (form, + as space)", G::Encode, T(Line, encode::url_decode)),
    a("encode-url-rfc3986", "Encode URL (RFC 3986)", G::Encode, T(Line, ok!(encode::url_encode_rfc3986))),
    a("decode-url-rfc3986", "Decode URL (RFC 3986)", G::Encode, T(Line, encode::url_decode_rfc3986)),
    a("md5", "Encode to MD5 Hex", G::Encode, T(Line, ok!(encode::md5))),
    a("sha1", "Encode to SHA-1 Hex", G::Encode, T(Line, ok!(encode::sha1))),
    a("sha256", "Encode to SHA-256 Hex", G::Encode, T(Line, ok!(encode::sha256))),
    a("sha512", "Encode to SHA-512 Hex", G::Encode, T(Line, ok!(encode::sha512))),
    a("sha3-256", "Encode to SHA3-256 Hex", G::Encode, T(Line, ok!(encode::sha3_256))),
    a("sha3-512", "Encode to SHA3-512 Hex", G::Encode, T(Line, ok!(encode::sha3_512))),
    a("iso-to-epoch", "ISO Timestamp to Epoch Milliseconds", G::Encode, T(Line, encode::iso_to_epoch)),
    a("epoch-to-iso", "Epoch Milliseconds to ISO Timestamp", G::Encode, T(Number, encode::epoch_to_iso)),
    // Increment / decrement
    a("increment", "Increment", G::Numbers, T(Number, |s| Ok(numbers::step_all(s, 1)))),
    a("decrement", "Decrement", G::Numbers, T(Number, |s| Ok(numbers::step_all(s, -1)))),
    a("duplicate-and-increment", "Duplicate and Increment", G::Numbers, Custom(|e, _| duplicate_and_step(e, 1))),
    a("duplicate-and-decrement", "Duplicate and Decrement", G::Numbers, Custom(|e, _| duplicate_and_step(e, -1))),
    a("create-sequence", "Create Sequence", G::Numbers, Custom(create_sequence)),
    a("increment-duplicates", "Increment Duplicates", G::Numbers, L(increment_duplicates)),
    a("char-int", "Convert Between Char and Int", G::Numbers, T(Word, numbers::char_int)),
    // Sort / shuffle / reverse
    a("sort-lines", "Sort Lines…", G::Sort, Custom(sort_lines)),
    a(
        "sort-lines-asc",
        "Sort Lines Ascending",
        G::Sort,
        L(|mut l| {
            lines::sort(&mut l, Sort::Asc);
            l
        }),
    ),
    a(
        "sort-lines-desc",
        "Sort Lines Descending",
        G::Sort,
        L(|mut l| {
            lines::sort(&mut l, Sort::Desc);
            l
        }),
    ),
    a(
        "sort-lines-natural",
        "Sort Lines Naturally (file2 before file10)",
        G::Sort,
        L(|mut l| {
            lines::sort(&mut l, Sort::NaturalAsc);
            l
        }),
    ),
    a(
        "sort-lines-length",
        "Sort Lines by Length",
        G::Sort,
        L(|mut l| {
            lines::sort(&mut l, Sort::LengthAsc);
            l
        }),
    ),
    a("sort-tokens", "Sort Tokens…", G::Sort, Custom(sort_tokens)),
    a("sort-by-subselection", "Sort Lines by Subselection", G::Sort, Custom(|e, _| sort_by_subselection(e))),
    a(
        "reverse-lines",
        "Reverse Lines",
        G::Sort,
        L(|mut l| {
            l.reverse();
            l
        }),
    ),
    a(
        "shuffle-lines",
        "Shuffle Lines",
        G::Sort,
        L(|mut l| {
            Rng::new().shuffle(&mut l);
            l
        }),
    ),
    a("reverse-chars", "Reverse Characters", G::Sort, T(Line, ok!(transform::reverse_chars))),
    a(
        "shuffle-chars",
        "Shuffle Characters",
        G::Sort,
        T(Word, |s| {
            let mut c: Vec<char> = s.chars().collect();
            Rng::new().shuffle(&mut c);
            Ok(c.into_iter().collect())
        }),
    ),
    a("json-sort", "Sort JSON Keys", G::Sort, T(Doc, transform::json_sort)),
    // Filter / remove / trim
    a("grep", "Grep (Keep Matching Lines)…", G::Filter, Custom(|e, arg| grep(e, arg, false))),
    a("inverted-grep", "Inverted Grep (Remove Matching Lines)…", G::Filter, Custom(|e, arg| grep(e, arg, true))),
    a(
        "remove-duplicates",
        "Remove Duplicate Lines",
        G::Filter,
        L(|mut l| {
            lines::unique(&mut l);
            l
        }),
    ),
    a(
        "keep-duplicates",
        "Keep Only Duplicate Lines",
        G::Filter,
        L(|mut l| {
            lines::only_duplicates(&mut l);
            l
        }),
    ),
    a("group-and-count", "Group and Count Lines", G::Filter, L(|l| lines::group_and_count(&l))),
    a(
        "remove-empty-lines",
        "Remove Empty Lines",
        G::Filter,
        L(|mut l| {
            lines::remove_empty(&mut l);
            l
        }),
    ),
    a(
        "remove-consecutive-empty-lines",
        "Remove Consecutive Empty Lines",
        G::Filter,
        L(|mut l| {
            lines::remove_consecutive_empty(&mut l);
            l
        }),
    ),
    a(
        "join-lines",
        "Remove All Newlines (Join Lines)",
        G::Filter,
        L(|l| vec![l.iter().map(|x| x.trim()).filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" ")]),
    ),
    a("trim", "Trim", G::Filter, L(|l| l.iter().map(|x| x.trim().to_owned()).collect())),
    a(
        "trim-leading",
        "Trim Leading Whitespace",
        G::Filter,
        L(|l| l.iter().map(|x| x.trim_start().to_owned()).collect()),
    ),
    a(
        "trim-trailing",
        "Trim Trailing Whitespace",
        G::Filter,
        L(|l| l.iter().map(|x| x.trim_end().to_owned()).collect()),
    ),
    a(
        "collapse-whitespace",
        "Replace Whitespace with a Single Space",
        G::Filter,
        L(|l| {
            l.iter()
                .map(|x| {
                    let indent = &x[..x.len() - x.trim_start().len()];
                    format!("{indent}{}", x.split_whitespace().collect::<Vec<_>>().join(" "))
                })
                .collect()
        }),
    ),
    a(
        "remove-all-spaces",
        "Remove All Spaces",
        G::Filter,
        T(Line, |s| Ok(s.chars().filter(|c| !c.is_whitespace() || *c == '\n').collect())),
    ),
    a("json-minify", "Minify JSON", G::Filter, T(Doc, transform::json_minify)),
    a("json-pretty", "Pretty-Print JSON", G::Filter, T(Doc, transform::json_pretty)),
    // Align
    a("align-columns", "Align Lines to Columns…", G::Align, Custom(align_columns)),
    a("align-carets", "Align Carets/Selections", G::Align, Custom(|e, _| align_carets(e))),
    a("align-left", "Align Left", G::Align, L(|l| lines::align(&l, Align::Left))),
    a("align-center", "Align Center", G::Align, L(|l| lines::align(&l, Align::Center))),
    a("align-right", "Align Right", G::Align, L(|l| lines::align(&l, Align::Right))),
    // Transform
    a("swap", "Swap Characters/Selections", G::Transform, Custom(|e, _| swap(e))),
    a("swap-words", "Swap First and Last Word", G::Transform, T(Line, ok!(transform::swap_words))),
    a("toggle-quotes", "Quote/Unquote", G::Transform, T(Quoted, ok!(transform::toggle_quotes))),
    a("cycle-quotes", "Cycle Quotes (\" → ' → `)", G::Transform, T(Quoted, ok!(transform::cycle_quotes))),
    a("swap-quotes", "Swap \" and '", G::Transform, T(Line, ok!(transform::swap_quotes))),
    a("straighten-quotes", "Straighten Quotes (“” → \"\")", G::Transform, T(Line, ok!(transform::straighten_quotes))),
    a("educate-quotes", "Educate Quotes (\"\" → “”)", G::Transform, T(Line, ok!(transform::educate_quotes))),
    a(
        "switch-path-separators",
        "Switch File Path Separators (\\ ↔ /)",
        G::Transform,
        T(Line, ok!(transform::switch_path_separators)),
    ),
    a("transform-list", "Transform Delimited List…", G::Transform, Custom(transform_list)),
    a("create-border", "Create Border…", G::Transform, Custom(create_border)),
];

fn find(id: &str) -> Option<&'static Action> {
    ACTIONS.iter().find(|a| a.id == id)
}

/// Runs `id` and remembers it for Repeat Last Action.
fn run(editor: &TextEditor, id: &str, arg: Option<&str>) -> Result<(), Error> {
    let action = find(id).ok_or_else(|| Error::NotFound(format!("command {id}")))?;
    let replay = match &action.op {
        Op::Text(scope, f) => edit_text(editor, *scope, f).map(|()| None)?,
        Op::Lines(f) => edit_lines(editor, &|l| Ok(f(l))).map(|()| None)?,
        Op::Custom(f) => f(editor, arg)?,
    };
    if action.group != Group::Menu {
        let last = serde_json::json!({ "id": id, "arg": replay });
        // Without the optional `storage` capability there is nothing to repeat.
        let _ = host::storage::set("last-action", &last.to_string());
    }
    Ok(())
}

fn repeat_last(editor: &TextEditor) -> Result<Option<String>, Error> {
    let last = last_action().ok_or("no action to repeat yet")?;
    run(editor, last.0.id, last.1.as_deref())?;
    Ok(None)
}

fn last_action() -> Option<(&'static Action, Option<String>)> {
    let json = host::storage::get("last-action").ok().flatten()?;
    let v: serde_json::Value = serde_json::from_str(&json).ok()?;
    let action = find(v["id"].as_str()?)?;
    Some((action, v["arg"].as_str().map(str::to_owned)))
}

/// The pick list of one group, or of every action (`None`).
fn menu(editor: &TextEditor, group: Option<Group>) -> Result<Option<String>, Error> {
    let mut items = Vec::new();
    if group.is_none()
        && let Some((last, _)) = last_action()
    {
        items.push(PickItem::new("repeat-last", &format!("Repeat Last: {}", last.title)).description("Repeat"));
    }
    for g in Group::LISTED.iter().filter(|g| group.is_none_or(|x| x == **g)) {
        for a in ACTIONS.iter().filter(|a| a.group == *g) {
            items.push(PickItem::new(a.id, a.title).description(g.label()));
        }
    }
    let title = group.map_or("String Manipulation", Group::label);
    let Some(id) = host::pick(title, &items)? else {
        return Err(Error::Cancelled);
    };
    run(editor, &id, None)?;
    Ok(None)
}

/// A remembered prompt answer, or `default`.
fn remembered(key: &str, default: &str) -> String {
    host::storage::get(&format!("input.{key}")).ok().flatten().unwrap_or_else(|| default.to_owned())
}

fn ask(key: &str, prompt: &str, default: &str) -> Result<String, Error> {
    let value = host::input(prompt, Some(&remembered(key, default)))?.ok_or(Error::Cancelled)?;
    let _ = host::storage::set(&format!("input.{key}"), &value);
    Ok(value)
}

fn pick_key(title: &str, items: &[(&str, &str)]) -> Result<String, Error> {
    let items: Vec<PickItem> = items.iter().map(|(k, l)| PickItem::new(k, l)).collect();
    host::pick(title, &items)?.ok_or(Error::Cancelled)
}

// ---------------------------------------------------------------------------
// Editing

/// Computes edits on the current document and applies them as one change,
/// once more if the user typed in between. Overlapping edits apply once.
fn edit_with(editor: &TextEditor, compute: &Compute) -> Result<(), Error> {
    let selections = editor.selections()?;
    for _ in 0..2 {
        let doc = editor.document()?;
        let mut edits = compute(&doc, &selections)?;
        edits.retain(|e| doc.text_in(e.range) != Some(e.text.as_str()));
        edits.sort_by_key(|e| e.range);
        edits.dedup_by(|b, a| b.range.start < a.range.end || b.range == a.range);
        if edits.is_empty() {
            return Ok(());
        }
        match editor.apply_edits(doc.version, &edits) {
            Err(Error::Stale) => continue,
            other => return other.map(|_| ()),
        }
    }
    Err(Error::Stale)
}

fn edit_text(editor: &TextEditor, scope: Scope, f: &dyn Fn(&str) -> R) -> Result<(), Error> {
    edit_with(editor, &|doc, selections| {
        let mut edits = Vec::new();
        for sel in selections {
            let range = target(&doc.text, *sel, scope);
            let Some(old) = doc.text_in(range) else {
                continue;
            };
            if old.is_empty() && scope != Scope::Number {
                continue;
            }
            edits.push(TextEdit { range, text: f(old).map_err(Error::Failed)? });
        }
        Ok(edits)
    })
}

/// The range a command works on for `sel`.
fn target(text: &str, sel: Range, scope: Scope) -> Range {
    if !sel.is_empty() {
        return sel;
    }
    let Some(at) = offset(text, sel.start) else {
        return sel;
    };
    let range = |a: usize, b: usize| Range::new(position(text, a), position(text, b));
    let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[at..].find('\n').map_or(text.len(), |i| at + i);
    match scope {
        Scope::Document => range(0, text.len()),
        Scope::Line => {
            let line = &text[line_start..line_end];
            let lead = line.len() - line.trim_start().len();
            let content = line.trim().len();
            range(line_start + lead, line_start + lead + content)
        }
        Scope::Number => match numbers::at(&text[line_start..line_end], at - line_start) {
            Some((a, b)) => range(line_start + a, line_start + b),
            None => {
                let (a, b) = word_at(text, at);
                range(a, b)
            }
        },
        Scope::Word => {
            let (a, b) = word_at(text, at);
            range(a, b)
        }
        Scope::Quoted => {
            let (mut a, mut b) = word_at(text, at);
            let before = text[..a].chars().next_back();
            let after = text[b..].chars().next();
            if let (Some(q), Some(q2)) = (before, after)
                && q == q2
                && matches!(q, '"' | '\'' | '`')
            {
                a -= q.len_utf8();
                b += q.len_utf8();
            }
            range(a, b)
        }
    }
}

fn word_at(text: &str, at: usize) -> (usize, usize) {
    let word = |c: char| c.is_alphanumeric() || c == '_' || c == '-' || c == '$';
    let start = text[..at].char_indices().rev().take_while(|(_, c)| word(*c)).last().map_or(at, |(i, _)| i);
    let end = text[at..].char_indices().find(|(_, c)| !word(*c)).map_or(text.len(), |(i, _)| at + i);
    (start, end)
}

/// The lines of `text` (without their `\r`), and the line break to rejoin them.
fn split_lines(text: &str) -> (Vec<String>, &'static str) {
    let crlf = text.contains("\r\n");
    let lines = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l).to_owned()).collect();
    (lines, if crlf { "\r\n" } else { "\n" })
}

fn line_len(text: &str, line: u32) -> u32 {
    text.split('\n').nth(line as usize).map_or(0, |l| l.strip_suffix('\r').unwrap_or(l).encode_utf16().count() as u32)
}

/// The whole lines of a selection; a selection ending at the start of a line
/// leaves that line out.
fn whole_lines(text: &str, sel: Range) -> Range {
    let end_line =
        if sel.end.character == 0 && sel.end.line > sel.start.line { sel.end.line - 1 } else { sel.end.line };
    Range::new(Position::new(sel.start.line, 0), Position::new(end_line, line_len(text, end_line)))
}

/// Applies `f` to the lines of each selection; with only carets, to the
/// whole document (one caret) or the carets' lines (several, which `f` must
/// keep as many).
fn edit_lines(editor: &TextEditor, f: &dyn Fn(Vec<String>) -> R2) -> Result<(), Error> {
    edit_with(editor, &|doc, selections| {
        let text = &doc.text;
        let (all, nl) = split_lines(text);
        if selections.iter().any(|s| !s.is_empty()) {
            let mut edits = Vec::new();
            for sel in selections.iter().filter(|s| !s.is_empty()) {
                let range = whole_lines(text, *sel);
                let block = all[range.start.line as usize..=range.end.line as usize].to_vec();
                let out = f(block).map_err(Error::Failed)?;
                edits.push(TextEdit { range, text: out.join(nl) });
            }
            return Ok(edits);
        }
        let mut caret_lines: Vec<u32> = selections.iter().map(|s| s.start.line).collect();
        caret_lines.sort_unstable();
        caret_lines.dedup();
        if caret_lines.len() <= 1 {
            // The whole document, leaving its final line break alone.
            let trailing = all.last().is_some_and(String::is_empty) && all.len() > 1;
            let body = all[..all.len() - usize::from(trailing)].to_vec();
            let last = (body.len() - 1) as u32;
            let range = Range::new(Position::new(0, 0), Position::new(last, line_len(text, last)));
            let out = f(body).map_err(Error::Failed)?;
            return Ok(vec![TextEdit { range, text: out.join(nl) }]);
        }
        let block: Vec<String> = caret_lines.iter().map(|&l| all[l as usize].clone()).collect();
        let out = f(block).map_err(Error::Failed)?;
        if out.len() != caret_lines.len() {
            return Err(Error::Failed("with several carets this command needs a selection".into()));
        }
        Ok(caret_lines
            .iter()
            .zip(out)
            .map(|(&l, text_out)| TextEdit {
                range: Range::new(Position::new(l, 0), Position::new(l, line_len(text, l))),
                text: text_out,
            })
            .collect())
    })
}

type R2 = Result<Vec<String>, String>;

/// Edits for a document and the selections the command ran with.
type Compute<'a> = dyn Fn(&Document, &[Range]) -> Result<Vec<TextEdit>, Error> + 'a;

// ---------------------------------------------------------------------------
// Commands that need more than one selection's text

fn increment_duplicates(lines: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    lines
        .into_iter()
        .map(|mut l| {
            if !l.trim().is_empty() {
                while seen.contains(&l) && !numbers::find(&l).is_empty() {
                    l = numbers::step_all(&l, 1);
                }
                seen.insert(l.clone());
            }
            l
        })
        .collect()
}

/// Inserts a copy of each selection (a caret: its line) after it, with its
/// numbers stepped by `delta`.
fn duplicate_and_step(editor: &TextEditor, delta: i128) -> Result<Option<String>, Error> {
    edit_with(editor, &|doc, selections| {
        let mut edits = Vec::new();
        for sel in selections {
            if sel.is_empty() {
                let l = sel.start.line;
                let (all, nl) = split_lines(&doc.text);
                let end = Position::new(l, line_len(&doc.text, l));
                edits.push(TextEdit {
                    range: Range::new(end, end),
                    text: format!("{nl}{}", numbers::step_all(&all[l as usize], delta)),
                });
            } else if let Some(old) = doc.text_in(*sel) {
                edits.push(TextEdit { range: Range::new(sel.end, sel.end), text: numbers::step_all(old, delta) });
            }
        }
        // Two carets on one line duplicate it once.
        edits.dedup_by(|b, a| b.range == a.range);
        Ok(edits)
    })?;
    Ok(None)
}

/// Selections in document order.
fn ordered(selections: &[Range]) -> Vec<Range> {
    let mut s = selections.to_vec();
    s.sort();
    s
}

/// Numbers the selections: the first keeps its number (or asks where to
/// start), the next ones count up from it.
fn create_sequence(editor: &TextEditor, arg: Option<&str>) -> Result<Option<String>, Error> {
    let selections = ordered(&editor.selections()?);
    let doc = editor.document()?;
    let first = selections.first().map(|s| target(&doc.text, *s, Scope::Number));
    let first_number =
        first.and_then(|r| doc.text_in(r)).and_then(|t| numbers::find(t).first().map(|&(a, b)| t[a..b].to_owned()));
    let (start, asked) = match first_number {
        Some(n) => (n, None),
        None => {
            let s = match arg {
                Some(a) => a.to_owned(),
                None => ask("sequence-start", "Start the sequence at", "1")?,
            };
            if numbers::first(&s).is_none() {
                return Err(Error::Failed(format!("{s:?} is not a number")));
            }
            (s.trim().to_owned(), Some(s))
        }
    };
    edit_with(editor, &|doc, sels| {
        let mut edits = Vec::new();
        for (i, sel) in ordered(sels).into_iter().enumerate() {
            let value = numbers::step(&start, i as i128);
            let range = target(&doc.text, sel, Scope::Number);
            let old = doc.text_in(range).unwrap_or("");
            let text = match numbers::find(old).first() {
                Some(&(a, b)) => format!("{}{value}{}", &old[..a], &old[b..]),
                None if sel.is_empty() => {
                    edits.push(TextEdit { range: sel, text: value });
                    continue;
                }
                None => value,
            };
            edits.push(TextEdit { range, text });
        }
        Ok(edits)
    })?;
    Ok(asked)
}

fn sort_lines(editor: &TextEditor, arg: Option<&str>) -> Result<Option<String>, Error> {
    let key = match arg {
        Some(a) => a.to_owned(),
        None => {
            let items: Vec<(&str, &str)> = Sort::ALL.iter().map(|&(_, k, l)| (k, l)).collect();
            pick_key("Sort Lines", &items)?
        }
    };
    let how = Sort::from_key(&key).ok_or_else(|| Error::Invalid(format!("unknown sort order {key}")))?;
    edit_lines(editor, &|mut l| {
        lines::sort(&mut l, how);
        Ok(l)
    })?;
    Ok(Some(key))
}

fn sort_tokens(editor: &TextEditor, arg: Option<&str>) -> Result<Option<String>, Error> {
    let sep = match arg {
        Some(a) => a.to_owned(),
        None => ask("token-separator", "Sort the items separated by", ",")?,
    };
    let delimiter = transform::unescape_delimiter(&sep);
    edit_text(editor, Scope::Line, &|s| Ok(lines::sort_tokens(s, &delimiter, Sort::NaturalAsc)))?;
    Ok(Some(sep))
}

/// Several selections on different lines: sorts those lines by the selected
/// text, each line moving as a whole into the slots of the others.
fn sort_by_subselection(editor: &TextEditor) -> Result<Option<String>, Error> {
    edit_with(editor, &|doc, selections| {
        let sels = ordered(selections);
        let mut lines_seen: Vec<u32> = sels.iter().map(|s| s.start.line).collect();
        lines_seen.dedup();
        if sels.len() < 2 || lines_seen.len() != sels.len() || sels.iter().any(|s| s.start.line != s.end.line) {
            return Err(Error::Failed("select a part of each line to sort by, one selection per line".into()));
        }
        let (all, _) = split_lines(&doc.text);
        let mut keyed: Vec<(String, String)> = sels
            .iter()
            .map(|s| (doc.text_in(*s).unwrap_or("").to_owned(), all[s.start.line as usize].clone()))
            .collect();
        keyed.sort_by(|a, b| lines::natural(&a.0, &b.0));
        Ok(sels
            .iter()
            .zip(keyed)
            .map(|(s, (_, line))| {
                let l = s.start.line;
                TextEdit {
                    range: Range::new(Position::new(l, 0), Position::new(l, line_len(&doc.text, l))),
                    text: line,
                }
            })
            .collect())
    })?;
    Ok(None)
}

fn grep(editor: &TextEditor, arg: Option<&str>, invert: bool) -> Result<Option<String>, Error> {
    let pattern = match arg {
        Some(a) => a.to_owned(),
        None => {
            let prompt = if invert {
                "Remove the lines containing (text or /regex/i)"
            } else {
                "Keep the lines containing (text or /regex/i)"
            };
            ask("grep", prompt, "")?
        }
    };
    let p = Pattern::parse(&pattern).map_err(Error::Failed)?;
    edit_lines(editor, &|l| Ok(l.into_iter().filter(|x| p.matches(x) != invert).collect()))?;
    Ok(Some(pattern))
}

fn align_columns(editor: &TextEditor, arg: Option<&str>) -> Result<Option<String>, Error> {
    let sep = match arg {
        Some(a) => a.to_owned(),
        None => ask("column-separator", "Align the columns separated by", ",")?,
    };
    let s = transform::unescape_delimiter(&sep);
    edit_lines(editor, &|l| Ok(lines::align_columns(&l, &s)))?;
    Ok(Some(sep))
}

/// Pads before each caret (or selection) so they all reach the rightmost one.
fn align_carets(editor: &TextEditor) -> Result<Option<String>, Error> {
    edit_with(editor, &|_, selections| {
        let max = selections.iter().map(|s| s.start.character).max().unwrap_or(0);
        Ok(selections
            .iter()
            .filter(|s| s.start.character < max)
            .map(|s| TextEdit {
                range: Range::new(s.start, s.start),
                text: " ".repeat((max - s.start.character) as usize),
            })
            .collect())
    })?;
    Ok(None)
}

/// Several selections: each takes the text of the one before it (the first
/// the last's). One caret: swaps the characters around it.
fn swap(editor: &TextEditor) -> Result<Option<String>, Error> {
    edit_with(editor, &|doc, selections| {
        let sels = ordered(selections);
        if sels.len() >= 2 {
            let texts: Vec<String> = sels.iter().map(|s| doc.text_in(*s).unwrap_or("").to_owned()).collect();
            return Ok(sels
                .iter()
                .enumerate()
                .map(|(i, s)| TextEdit { range: *s, text: texts[(i + texts.len() - 1) % texts.len()].clone() })
                .collect());
        }
        let Some(sel) = sels.first() else {
            return Ok(Vec::new());
        };
        let text = &doc.text;
        if !sel.is_empty() {
            return Ok(vec![TextEdit { range: *sel, text: transform::swap_words(doc.text_in(*sel).unwrap_or("")) }]);
        }
        let at = offset(text, sel.start).unwrap_or(0);
        let prev = text[..at].chars().next_back();
        let next = text[at..].chars().next();
        match (prev, next) {
            (Some(p), Some(n)) if p != '\n' && n != '\n' => {
                let range = Range::new(position(text, at - p.len_utf8()), position(text, at + n.len_utf8()));
                Ok(vec![TextEdit { range, text: format!("{n}{p}") }])
            }
            _ => Ok(Vec::new()),
        }
    })?;
    Ok(None)
}

fn transform_list(editor: &TextEditor, arg: Option<&str>) -> Result<Option<String>, Error> {
    let (from, to, quote) = match arg.and_then(|a| serde_json::from_str::<[String; 3]>(a).ok()) {
        Some([f, t, q]) => (f, t, q),
        None => (
            ask("list-from", "Split the list on (\\n for line breaks)", ",")?,
            ask("list-to", "Join the items with (\\n for line breaks)", ", ")?,
            ask("list-quote", "Wrap each item in (empty for none)", "")?,
        ),
    };
    let (f, t) = (transform::unescape_delimiter(&from), transform::unescape_delimiter(&to));
    edit_text(editor, Scope::Line, &|s| Ok(transform::transform_list(s, &f, &t, &quote)))?;
    Ok(Some(serde_json::json!([from, to, quote]).to_string()))
}

fn create_border(editor: &TextEditor, arg: Option<&str>) -> Result<Option<String>, Error> {
    let key = match arg {
        Some(a) => a.to_owned(),
        None => {
            let items: Vec<(&str, &str)> = Border::ALL.iter().map(|&(_, k, l)| (k, l)).collect();
            pick_key("Create Border", &items)?
        }
    };
    let style = Border::from_key(&key).ok_or_else(|| Error::Invalid(format!("unknown border {key}")))?;
    edit_lines(editor, &|l| Ok(transform::border(&l.join("\n"), style).split('\n').map(str::to_owned).collect()))?;
    Ok(Some(key))
}

#[cfg(test)]
mod tests;
