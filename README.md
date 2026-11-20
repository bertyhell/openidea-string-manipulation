# String Manipulation for OpenIDEA

An [OpenIDEA](https://github.com/l0ng-ai/openidea) plugin for case switching,
escaping, encoding, hashing, sorting, filtering, aligning and incrementing
text. It is inspired by the
[String Manipulation](https://github.com/krasa/StringManipulation) plugin for
JetBrains IDEs and written from scratch in Rust.

Every action works on all selections and carets at once, and its edits form
one undoable change.

## Using it

- **Alt+M** opens a searchable list of every action (with *Repeat Last* at
  the top). You can rebind it under Settings › Keymap.
- Right-click in the editor and choose **String Manipulation** to open one
  group's list.
- Every action is a command of its own. Find it with Find Action
  (`String Manipulation: …`) or give it a shortcut in the keymap.
- **Repeat Last Action** runs the previous action again, with the same
  answers to its prompts.

### What an action works on

With a selection, an action works on the selected text. With only a caret:

| Actions | Work on |
| --- | --- |
| Case changes, Capitalize, Invert Case, Char ↔ Int, Shuffle Characters | the identifier at the caret (`.` ends an identifier, so select `dot.case` text first) |
| To Capitalized Words, To First word capitalized, and the toggles between camelCase and phrases | the caret's line, without its indentation |
| Increment, Decrement, Create Sequence, Epoch → ISO | the number at the caret |
| Quote/Unquote, Cycle Quotes | the identifier at the caret and its quotes |
| Line actions (sort, filter, trim, align, Increment Duplicates, Create Border) | the whole document, or with several carets, the lines they are on |
| Format, Minify and Sort JSON | the whole document |
| Everything else (escaping, encoding, hashing…) | the caret's line, without its indentation |

## Actions

| Group | Actions |
| --- | --- |
| **Switch Case** | Switch Case (camelCase → snake_case → SCREAMING_SNAKE_CASE → kebab-case → PascalCase) · toggles between camelCase and SCREAMING_SNAKE_CASE, snake_case, kebab-case, dot.case, PascalCase, lower case words, First word capitalized, Capitalized Words · kebab-case ↔ snake_case · To camelCase, PascalCase, snake_case, SCREAMING_SNAKE_CASE, Capitalized_Snake_Case, kebab-case, dot.case, lower case words, Capitalized Words, First word capitalized · Capitalize · lower case · UPPER CASE · Invert Case · To Spring Boot Environment Variable |
| **Escape/Unescape** | Java · JavaScript · C# · HTML · XML · SQL · PHP |
| **JSON** | Format JSON (Pretty-Print) · Minify JSON · Escape JSON · Unescape JSON · Sort JSON Keys |
| **Unicode** | Diacritics to ASCII · Non-ASCII ↔ `\uXXXX` · NFC · NFD · NFKC · NFKD |
| **Encode/Decode** | Base64 · Hex · URL (form and RFC 3986) · MD5, SHA-1, SHA-256, SHA-512, SHA3-256, SHA3-512 hex · ISO timestamp ↔ epoch milliseconds |
| **Increment/Decrement** | Increment · Decrement · Duplicate and Increment/Decrement · Create Sequence · Increment Duplicates · Char ↔ Int |
| **Sort/Shuffle/Reverse** | Sort Lines… (ascending, descending, ignoring case, natural, by length) · quick ascending/descending/natural/length sorts · Sort Tokens… · Sort Lines by Subselection · Reverse/Shuffle Lines · Reverse/Shuffle Characters |
| **Filter/Remove/Trim** | Grep… and Inverted Grep… (text, or `/regex/` and `/regex/i`) · Remove Duplicate Lines · Keep Only Duplicate Lines · Group and Count · Remove Empty / Consecutive Empty Lines · Join Lines · Trim (both ends, leading, trailing) · Collapse Whitespace · Remove All Spaces |
| **Align** | Align Lines to Columns… (any separator, CSV and Markdown tables) · Align Carets/Selections · Align Left/Center/Right |
| **Transform** | Swap Characters/Selections · Swap First and Last Word · Quote/Unquote · Cycle Quotes (`"` → `'` → `` ` ``) · Swap `"` and `'` · Straighten/Educate Quotes · Switch Path Separators · Transform Delimited List… · Create Border… |

Actions whose names end in `…` ask for input first: a separator, a pattern or
a choice. The plugin remembers your last answer and offers it next time.

## Capabilities

| Capability | Required | Why |
| --- | --- | --- |
| `editor.read` | yes | reads the focused editor's text and selections |
| `editor.write` | yes | replaces the selections with the result |
| `storage` | no | remembers the last action and your prompt answers for *Repeat Last Action*. Without it, everything else still works. |

The plugin doesn't use the network, run programs or read project files.

## Install

In OpenIDEA, open Settings › Plugins › Available and search for
*String Manipulation*. It needs OpenIDEA 0.1.39 or later (plugin API 1.3).

To install a downloaded `.oiplugin` package, use Settings › Plugins ›
*Install Plugin from Disk…*.

## Build from source

You need Rust (stable) and the `openidea-plugin` tool, which is attached to
every OpenIDEA release as `openidea-plugin-linux-x86_64.gz`. You can also
build the tool yourself:
`cargo install --locked --git https://github.com/l0ng-ai/openidea ide-host --bin openidea-plugin`.

```sh
cargo test                  # the actions, against the SDK's mock host
openidea-plugin build       # → dist/plugin.wasm, then `openidea-plugin check`
openidea-plugin link        # load this folder in your OpenIDEA, then Settings › Plugins › Reload
```

`plugin.toml` declares one command for each entry of `ACTIONS` in
`src/lib.rs`, and a test keeps the two in sync. When you add an action, run
`cargo test`. If the test fails, copy the generated entries from
`target/commands.toml` into `plugin.toml`.

### Release

Set the new version in `plugin.toml` and `Cargo.toml` (and `change-notes`),
commit and push to `main`, then run:

```sh
scripts/release.sh --dry-run   # test, build and sign only
scripts/release.sh             # also tag v<version> and create the GitHub release
```

It needs the [1Password CLI](https://developer.1password.com/docs/cli/)
(`op`), `gh`, `openidea-plugin`, `jq` and `unzip`. The signing key is read with
`op read` from the reference in `OP_KEY_REF`, in the 1Password account
`OP_ACCOUNT` (the "Our Family" account by default), and goes to
`openidea-plugin pack` through a pipe, never to disk. The script prints the package's SHA-256 and a
`[[versions]]` entry. Add that entry to `plugins/bertyhell.string-manipulation.toml`
in [openidea-plugins](https://github.com/l0ng-ai/openidea-plugins) and open a
pull request.

## License

MIT
