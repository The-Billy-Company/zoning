//! The three adapters, held to the one server.
//!
//! `lsp.rs` proves the server answers. This file proves the editors ask — that each adapter
//! in `editors/` launches the same executable with the same argv the protocol suite proves,
//! claims the same files, and teaches its highlighter the same vocabulary the server
//! completes. None of that is checkable from inside an editor on a CI runner: VS Code, Zed,
//! and Neovim are three different runtimes, and two of them will not install headless. What
//! *is* checkable is that the words in the adapter and the words in the server are the same
//! words, which is where every drift in this tree has actually come from — a keyword added
//! to the completion list and forgotten in three grammars.
//!
//! So the vocabulary is never written down here. It is asked of a running server and then
//! looked for in the adapters, which means adding a keyword to `KEYWORDS` fails this suite
//! until all three highlighters learn it.

#![allow(clippy::expect_used, reason = "a test that cannot read an adapter has failed")]

mod editor;

use std::collections::BTreeSet;

use editor::{EXECUTABLE, Editor, SERVE, adapter};
use serde_json::json;

/// Every keyword the server offers an editor's completion menu, as single words. The two
/// two-word keywords (`forbid cycles`, `limit reach`) are highlighted a word at a time,
/// because that is what a highlighter matches on.
fn vocabulary() -> BTreeSet<String> {
    let mut editor = Editor::attached();
    editor.hello();
    let uri = "file:///vocabulary.zone";
    editor.open(uri, "package vocabulary\n\nzones {\n    only src/**\n}\n");
    let offered = editor
        .ask(
            "textDocument/completion",
            json!({"textDocument": {"uri": uri}, "position": {"line": 4, "character": 1}}),
        )
        .expect("the server must offer completions");
    let laws: BTreeSet<&str> = zoning::ordinance::Law::NAMES.into_iter().collect();
    let words = offered["items"]
        .as_array()
        .expect("completion items are a list")
        .iter()
        .filter_map(|item| item["label"].as_str())
        // A law name is a variance subject, not a keyword; the adapters highlight those
        // separately, and `highlight_the_same_laws` covers them.
        .filter(|label| !laws.contains(label))
        .flat_map(str::split_whitespace)
        .map(str::to_owned)
        .collect();
    editor.goodbye();
    words
}

/// A word a highlighter must be able to see: present, and not glued to a longer word.
fn teaches(source: &str, word: &str) -> bool {
    source.match_indices(word).any(|(at, _)| {
        let before = source[..at].chars().next_back();
        let after = source[at + word.len()..].chars().next();
        let loose = |edge: Option<char>| !edge.is_some_and(|c| c.is_alphanumeric() || c == '_');
        loose(before) && loose(after)
    })
}

/// Each adapter's highlighter, and the files that carry the words it can see. Vim and VS
/// Code match on literal word lists, so one file holds everything. Zed asks its grammar:
/// the keyword list is in the query, but the law names are a grammar rule the query paints
/// wholesale via `(law) @type`, so the grammar is the second half of Zed's vocabulary.
const HIGHLIGHTERS: [(&str, &[&str]); 3] = [
    ("Vim", &["vim/syntax/zoning.vim"]),
    ("VS Code", &["vscode/syntaxes/zoning.tmLanguage.json"]),
    ("Zed", &["zed/languages/zoning/highlights.scm", "zed/grammar/grammar.js"]),
];

/// Every word the named adapter's highlighter can see, joined into one haystack.
fn seen_by(files: &[&str]) -> String {
    files.iter().map(|path| adapter(path)).collect::<Vec<_>>().join("\n")
}

/// Each adapter's language-server wiring, and the file that spells the launch.
const LAUNCHERS: [(&str, &str); 4] = [
    ("Vim (vim-lsp, LanguageClient, coc, ALE)", "vim/autoload/zoning/lsp.vim"),
    ("Neovim (native)", "vim/lsp/zoning.lua"),
    ("VS Code", "vscode/src/extension.ts"),
    ("Zed", "zed/src/lib.rs"),
];

#[test]
fn every_adapter_launches_the_command_the_protocol_suite_proves() {
    for (editor, path) in LAUNCHERS {
        let source = adapter(path);
        assert!(
            teaches(&source, EXECUTABLE),
            "{editor} must launch `{EXECUTABLE}`, the name the crate installs (editors/{path})"
        );
        for word in SERVE {
            assert!(
                source.contains(word),
                "{editor} must pass `{word}`: the server only speaks over the argv \
                 `{EXECUTABLE} {}`, which is the argv the spawned-door test proves \
                 (editors/{path})",
                SERVE.join(" ")
            );
        }
    }
}

#[test]
fn every_adapter_claims_the_zone_extension() {
    for (editor, path, claim) in [
        ("Vim", "vim/ftdetect/zoning.vim", "*.zone"),
        ("Neovim", "vim/plugin/zoning.lua", "zone ="),
        ("VS Code", "vscode/package.json", ".zone"),
        ("Zed", "zed/languages/zoning/config.toml", "\"zone\""),
    ] {
        let source = adapter(path);
        assert!(
            source.contains(claim),
            "{editor} must claim `.zone` files by naming {claim} (editors/{path})"
        );
    }
}

#[test]
fn every_highlighter_learns_every_keyword_the_server_completes() {
    let vocabulary = vocabulary();
    assert!(vocabulary.len() > 10, "the server offered {vocabulary:?}, which is too few to be it");

    for (editor, files) in HIGHLIGHTERS {
        let source = seen_by(files);
        let missing: Vec<&String> =
            vocabulary.iter().filter(|word| !teaches(&source, word)).collect();
        assert!(
            missing.is_empty(),
            "{editor} does not highlight {missing:?}, which the server completes — \
             a keyword the menu offers and the buffer leaves grey ({files:?})"
        );
    }
}

#[test]
fn every_highlighter_learns_every_law_a_variance_can_name() {
    for (editor, files) in HIGHLIGHTERS {
        let source = seen_by(files);
        let missing: Vec<&&str> =
            zoning::ordinance::Law::NAMES.iter().filter(|law| !teaches(&source, law)).collect();
        assert!(
            missing.is_empty(),
            "{editor} does not highlight the {missing:?} law(s), so a `variance` naming one \
             reads as prose ({files:?})"
        );
    }
}

#[test]
fn zed_paints_the_law_rule_its_grammar_defines() {
    // Zed passes the law vocabulary through its grammar rather than a word list, which is
    // the better arrangement and also a second place for it to go wrong: a grammar that
    // parses `(law)` and a query that never captures it leaves every law unpainted.
    let query = adapter("zed/languages/zoning/highlights.scm");
    assert!(
        query.contains("(law)"),
        "zed/languages/zoning/highlights.scm must capture `(law)`, because Zed's law \
         vocabulary lives in the grammar rule and not in this query's keyword list"
    );
}

#[test]
fn every_adapter_agrees_on_the_comment_marker_and_the_folded_reason() {
    // `//` and the `\\` reason continuation are the two pieces of surface syntax an editor
    // gets wrong in a way the user feels immediately: a wrong comment marker breaks the
    // comment keybinding, and an unrecognized continuation breaks a multi-line reason.
    for (editor, path, marker) in [
        ("Vim syntax", "vim/syntax/zoning.vim", "//"),
        ("Vim ftplugin", "vim/ftplugin/zoning.vim", "commentstring=//"),
        ("VS Code", "vscode/language-configuration.json", "\"lineComment\": \"//\""),
        ("Zed", "zed/languages/zoning/config.toml", "line_comments = [ \"// \" ]"),
    ] {
        assert!(
            adapter(path).contains(marker),
            "{editor} must know the comment marker is `{marker}` (editors/{path})"
        );
    }

    for (editor, path) in
        [("Vim", "vim/syntax/zoning.vim"), ("VS Code", "vscode/syntaxes/zoning.tmLanguage.json")]
    {
        let source = adapter(path);
        assert!(
            source.contains("\\\\"),
            "{editor} must recognize the `\\\\` folded-reason continuation (editors/{path})"
        );
    }
}
