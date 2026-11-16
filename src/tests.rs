//! Commands end to end against the SDK's mock host.

use super::*;
use openidea_plugin::mock::MockHost;

const CAPS: &[&str] = &["editor.read", "editor.write", "storage"];

fn host(text: &str, selections: &[(u32, u32, u32, u32)]) -> MockHost {
    let host = MockHost::install().with_capabilities(CAPS);
    host.open_editor("/project/a.txt", "plaintext", text, selections);
    host
}

fn exec(host: &MockHost, id: &str) -> Result<Option<String>, Error> {
    StringManipulation.run_command(id, &host.invocation(), None)
}

fn after(text: &str, selections: &[(u32, u32, u32, u32)], id: &str) -> String {
    let h = host(text, selections);
    exec(&h, id).unwrap();
    h.editor_text()
}

#[test]
fn case_on_the_identifier_at_the_caret_and_on_every_selection() {
    assert_eq!(after("let userName = 1;", &[(0, 6, 0, 6)], "to-snake-case"), "let user_name = 1;");
    assert_eq!(after("fooBar barBaz", &[(0, 0, 0, 6), (0, 7, 0, 13)], "to-kebab-case"), "foo-bar bar-baz");
    assert_eq!(after("fooBar", &[(0, 3, 0, 3)], "switch-case"), "foo_bar");
    assert_eq!(after("  hello big world", &[(0, 4, 0, 4)], "toggle-words-camel"), "  helloBigWorld");
}

#[test]
fn line_commands_without_a_selection_take_the_document() {
    assert_eq!(after("c\nb\na\n", &[(0, 0, 0, 0)], "sort-lines-asc"), "a\nb\nc\n");
    assert_eq!(after("b\r\na\r\n", &[(0, 0, 0, 0)], "sort-lines-asc"), "a\r\nb\r\n");
    assert_eq!(after("x\ny\nx\n", &[(1, 0, 1, 0)], "remove-duplicates"), "x\ny\n");
}

#[test]
fn line_commands_work_on_whole_lines_of_a_selection() {
    // The selection ends at the start of line 3, which stays out.
    assert_eq!(after("z\nc\nb\na\n", &[(1, 1, 3, 0)], "sort-lines-asc"), "z\nb\nc\na\n");
}

#[test]
fn line_commands_with_several_carets_take_their_lines() {
    assert_eq!(after("b\nkeep\na\n", &[(0, 0, 0, 0), (2, 0, 2, 0)], "sort-lines-asc"), "a\nkeep\nb\n");
    let h = host("a\na\nb\n", &[(0, 0, 0, 0), (1, 0, 1, 0)]);
    assert!(exec(&h, "remove-duplicates").is_err());
}

#[test]
fn encode_takes_the_line_without_indentation() {
    assert_eq!(after("    hello  \n", &[(0, 0, 0, 0)], "encode-base64"), "    aGVsbG8=  \n");
    let h = host("  ###", &[(0, 0, 0, 0)]);
    let err = exec(&h, "decode-base64").unwrap_err();
    assert!(matches!(err, Error::Failed(m) if m.starts_with("not Base64")));
    assert_eq!(h.editor_text(), "  ###");
}

#[test]
fn increments_the_number_at_the_caret() {
    assert_eq!(after("a = 9; b = 9", &[(0, 4, 0, 4)], "increment"), "a = 10; b = 9");
    assert_eq!(after("v1 v2", &[(0, 0, 0, 5)], "decrement"), "v0 v1");
    assert_eq!(after("item1\n", &[(0, 2, 0, 2)], "duplicate-and-increment"), "item1\nitem2\n");
    assert_eq!(after("x1 ", &[(0, 0, 0, 2)], "duplicate-and-increment"), "x1x2 ");
}

#[test]
fn creates_a_sequence_over_the_carets() {
    assert_eq!(
        after("id 5\nid 0\nid 0", &[(0, 3, 0, 3), (1, 3, 1, 3), (2, 3, 2, 3)], "create-sequence"),
        "id 5\nid 6\nid 7"
    );
    let h = host("a\nb", &[(0, 1, 0, 1), (1, 1, 1, 1)]);
    h.answer_next_prompt(Some(&["01"]));
    exec(&h, "create-sequence").unwrap();
    assert_eq!(h.editor_text(), "a01\nb02");
}

#[test]
fn increments_duplicate_lines() {
    assert_eq!(after("a1\na1\na1\nb\nb", &[(0, 0, 0, 0)], "increment-duplicates"), "a1\na2\na3\nb\nb");
}

#[test]
fn sort_lines_asks_how_and_repeats_without_asking() {
    let h = host("b10\na\nb2", &[(0, 0, 0, 0)]);
    h.answer_next_prompt(Some(&["natural-desc"]));
    exec(&h, "sort-lines").unwrap();
    assert_eq!(h.editor_text(), "b10\nb2\na");
    assert!(h.storage()["last-action"].contains("natural-desc"));
    // A dismissed prompt changes nothing.
    let h2 = host("b\na", &[(0, 0, 0, 0)]);
    assert!(matches!(exec(&h2, "sort-lines"), Err(Error::Cancelled)));
    assert_eq!(h2.editor_text(), "b\na");
}

#[test]
fn repeat_last_replays_the_last_action_and_its_answer() {
    let h = host("keep 1\ndrop\nkeep 2", &[(0, 0, 0, 0)]);
    h.answer_next_prompt(Some(&["keep"]));
    exec(&h, "grep").unwrap();
    assert_eq!(h.editor_text(), "keep 1\nkeep 2");
    h.type_in_editor("\ndrop");
    exec(&h, "repeat-last").unwrap();
    assert_eq!(h.editor_text(), "keep 1\nkeep 2");
}

#[test]
fn the_popup_runs_the_chosen_action() {
    let h = host("hello", &[(0, 0, 0, 0)]);
    h.answer_next_prompt(Some(&["to-upper-case"]));
    exec(&h, "popup").unwrap();
    assert_eq!(h.editor_text(), "HELLO");
    // Remembered for Repeat Last.
    assert!(h.storage()["last-action"].contains("to-upper-case"));
}

#[test]
fn works_without_storage() {
    let h = MockHost::install().with_capabilities(&["editor.read", "editor.write"]);
    h.open_editor("/a.txt", "plaintext", "abc", &[(0, 0, 0, 0)]);
    StringManipulation.run_command("to-upper-case", &h.invocation(), None).unwrap();
    assert_eq!(h.editor_text(), "ABC");
    assert!(StringManipulation.run_command("repeat-last", &h.invocation(), None).is_err());
}

#[test]
fn grep_with_a_regex() {
    let h = host("a1\nb\na22", &[(0, 0, 0, 0)]);
    h.answer_next_prompt(Some(&["/^a\\d+$/"]));
    exec(&h, "inverted-grep").unwrap();
    assert_eq!(h.editor_text(), "b");
}

#[test]
fn aligns() {
    let h = host("a,bb\nccc,d", &[(0, 0, 0, 0)]);
    h.answer_next_prompt(Some(&[","]));
    exec(&h, "align-columns").unwrap();
    assert_eq!(h.editor_text(), "a   , bb\nccc , d");
    assert_eq!(after("ab = 1\nabcd = 2", &[(0, 3, 0, 3), (1, 5, 1, 5)], "align-carets"), "ab   = 1\nabcd = 2");
}

#[test]
fn swaps() {
    assert_eq!(after("one two", &[(0, 0, 0, 3), (0, 4, 0, 7)], "swap"), "two one");
    assert_eq!(after("ab", &[(0, 1, 0, 1)], "swap"), "ba");
    assert_eq!(after("first, second", &[(0, 0, 0, 13)], "swap"), "second, first");
}

#[test]
fn sorts_lines_by_subselection() {
    let text = "x = c\ny = a\nz = b";
    assert_eq!(after(text, &[(0, 4, 0, 5), (1, 4, 1, 5), (2, 4, 2, 5)], "sort-by-subselection"), "y = a\nz = b\nx = c");
}

#[test]
fn quotes_at_the_caret() {
    assert_eq!(after("f(\"name\")", &[(0, 4, 0, 4)], "toggle-quotes"), "f(name)");
    assert_eq!(after("f(\"name\")", &[(0, 4, 0, 4)], "cycle-quotes"), "f('name')");
    assert_eq!(after("f(name)", &[(0, 3, 0, 3)], "toggle-quotes"), "f(\"name\")");
}

#[test]
fn transforms_a_list() {
    let h = host("a, b, c", &[(0, 0, 0, 0)]);
    h.answer_next_prompt(Some(&[","]));
    h.answer_next_prompt(Some(&["\\n"]));
    h.answer_next_prompt(Some(&["'"]));
    exec(&h, "transform-list").unwrap();
    assert_eq!(h.editor_text(), "'a'\n'b'\n'c'");
}

#[test]
fn json_on_the_document() {
    assert_eq!(after("{\"b\": 1, \"a\": 2}", &[(0, 0, 0, 0)], "json-sort"), "{\"a\":2,\"b\":1}");
    assert_eq!(after("{\"a\": [1, 2]}", &[(0, 3, 0, 3)], "json-minify"), "{\"a\":[1,2]}");
}

#[test]
fn borders_and_unicode() {
    let h = host("hi", &[(0, 0, 0, 0)]);
    h.answer_next_prompt(Some(&["ascii"]));
    exec(&h, "create-border").unwrap();
    assert_eq!(h.editor_text(), "+----+\n| hi |\n+----+");
    assert_eq!(after("Crème", &[(0, 0, 0, 0)], "to-ascii"), "Creme");
    // UTF-16 columns: the emoji is two units wide.
    assert_eq!(after("😀 fooBar", &[(0, 3, 0, 3)], "to-snake-case"), "😀 foo_bar");
}

#[test]
fn edits_are_one_change_and_retry_once_when_stale() {
    let h = host("aa bb", &[(0, 0, 0, 2), (0, 3, 0, 5)]);
    let v = h.editor_version();
    exec(&h, "to-upper-case").unwrap();
    assert_eq!(h.editor_text(), "AA BB");
    assert_eq!(h.editor_version(), v + 1);
}

#[test]
fn unknown_command() {
    let h = host("", &[(0, 0, 0, 0)]);
    assert!(matches!(exec(&h, "nope"), Err(Error::NotFound(_))));
}

/// `plugin.toml` declares every action (same id, `String Manipulation: <title>`)
/// and nothing else, so Find Action lists them before the code loads.
#[test]
fn manifest_declares_every_action() {
    let manifest: toml::Value = toml::from_str(include_str!("../plugin.toml")).unwrap();
    let empty = Vec::new();
    let commands = manifest["contributes"].get("commands").and_then(|c| c.as_array()).unwrap_or(&empty);
    let declared: Vec<(String, String)> = commands
        .iter()
        .map(|c| (c["id"].as_str().unwrap().to_owned(), c["title"].as_str().unwrap().to_owned()))
        .collect();
    let expected: Vec<(String, String)> =
        ACTIONS.iter().map(|a| (a.id.to_owned(), format!("String Manipulation: {}", a.title))).collect();
    if declared != expected {
        let mut toml = String::new();
        for a in ACTIONS {
            let category = if a.group == Group::Menu {
                "String Manipulation".to_owned()
            } else {
                format!("String Manipulation › {}", a.group.label())
            };
            toml.push_str(&format!(
                "[[contributes.commands]]\nid = {:?}\ntitle = {:?}\ncategory = {:?}\nwhen = \"editorFocus\"\n\n",
                a.id,
                format!("String Manipulation: {}", a.title),
                category
            ));
        }
        std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/target/commands.toml"), toml).unwrap();
        panic!("plugin.toml is out of date: see target/commands.toml");
    }
    for m in manifest["contributes"]["menus"].as_array().unwrap() {
        assert!(find(m["command"].as_str().unwrap()).is_some(), "menu entry for an unknown command");
    }
}

#[test]
fn ids_are_unique() {
    let mut ids: Vec<&str> = ACTIONS.iter().map(|a| a.id).collect();
    ids.sort_unstable();
    let n = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), n);
}
