//! Every capability the server advertises, exercised the way an editor exercises it.
//!
//! A language server's contract is its `initialize` response: each `…Provider` key is a
//! promise that a request will be answered, and an editor wires its UI to those promises
//! before it ever sends one. So the suite is anchored by
//! [`every_advertised_capability_is_answered_and_covered`], which reads the promises off
//! the handshake rather than off a list written here — a capability added to
//! `language::capabilities` with no handler fails, and one added with a handler but no
//! test below fails too. The rest of the file is one test per promise, asserting *what*
//! came back and not merely that something did: a protocol-shaped empty answer is how a
//! feature silently stops working in every editor at once.
//!
//! Positions are UTF-16 code units, because that is what the protocol says and what
//! editors send. `café` and an astral-plane emoji appear on purpose — an off-by-one there
//! is invisible in ASCII and puts every squiggle in the wrong column in real files.

#![allow(clippy::expect_used, reason = "a test that cannot reach the server has failed")]

mod editor;

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use editor::Editor;
use serde_json::{Value, json};

/// The `…Provider` keys, and the request each one promises to answer.
///
/// Adding a capability means adding its row here, which is what makes the parity gate
/// below fail until the feature has a test of its own.
const PROMISES: &[(&str, &str)] = &[
    ("completionProvider", "textDocument/completion"),
    ("hoverProvider", "textDocument/hover"),
    ("definitionProvider", "textDocument/definition"),
    ("referencesProvider", "textDocument/references"),
    ("documentSymbolProvider", "textDocument/documentSymbol"),
    ("workspaceSymbolProvider", "workspace/symbol"),
    ("foldingRangeProvider", "textDocument/foldingRange"),
    ("documentFormattingProvider", "textDocument/formatting"),
    ("renameProvider", "textDocument/rename"),
    ("codeActionProvider", "textDocument/codeAction"),
    ("semanticTokensProvider", "textDocument/semanticTokens/full"),
];

/// A contract that parses clean, so a test about one feature is never really about faults.
///
/// Line numbers are load-bearing below: 0 `package`, 5 `zones {`, 6 `floor`, 7 `engine`,
/// 8 `face`, 11 `seal`, 12 `limit`.
const CONTRACT: &str = "package layered {
    root src
    language zig
}

zones {
    floor floor.zig
    engine engine/**
    face face.zig
}

seal engine through engine.zig open to face.zig
limit reach to 2 hops
";

/// The package the sample buffers claim to govern, laid down once per run.
///
/// A `.zone` buffer is not a function of its text alone: `root`, and every seal's
/// directory and entry file, are resolved against the disk on each keystroke. So the suite
/// builds one real package under Cargo's own scratch directory and never writes the
/// contract itself — an editor's buffer runs ahead of the file, and that is the state worth
/// covering.
static PACKAGE: LazyLock<PathBuf> = LazyLock::new(|| {
    let home = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("authoring");
    for file in ["src/floor.zig", "src/face.zig", "src/engine/engine.zig"] {
        let path = home.join(file);
        std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))
            .expect("the sample package's directories must be creatable");
        std::fs::write(&path, "").expect("the sample package's files must be writable");
    }
    home
});

/// The URI of an unsaved buffer in the sample package.
fn buffer(name: &str) -> String {
    format!("file://{}/contract/{name}.zone", PACKAGE.display())
}

/// A session with `CONTRACT` already open.
fn authoring() -> Editor {
    let mut editor = Editor::attached();
    editor.hello();
    let faults = editor.open(&buffer("layered"), CONTRACT);
    assert!(faults.is_empty(), "the sample contract must parse clean, and reported {faults:?}");
    editor
}

/// Ask about a position in the open contract.
fn at(line: usize, character: usize) -> Value {
    json!({
        "textDocument": {"uri": buffer("layered")},
        "position": {"line": line, "character": character}
    })
}

/// Just the document, for the requests that take no position.
fn whole() -> Value {
    json!({"textDocument": {"uri": buffer("layered")}})
}

#[test]
fn every_advertised_capability_is_answered_and_covered() {
    let mut editor = Editor::attached();
    let capabilities = editor.hello()["capabilities"].clone();
    let advertised = capabilities.as_object().expect("capabilities must be an object");
    editor.open(&buffer("layered"), CONTRACT);

    for key in advertised.keys().filter(|key| key.ends_with("Provider")) {
        assert!(
            PROMISES.iter().any(|(promise, _)| promise == key),
            "`{key}` is advertised to editors but has no row in PROMISES, so nothing below \
             proves it works — add the row and the test it forces"
        );
    }

    // `prepareRename` is promised by a flag inside `renameProvider` rather than by a key of
    // its own, so it is the one promise the key sweep above cannot see.
    assert_eq!(capabilities["renameProvider"]["prepareProvider"], json!(true));
    let promised = PROMISES
        .iter()
        .map(|(_, method)| *method)
        .chain(std::iter::once("textDocument/prepareRename"));

    for method in promised {
        let refused = editor.ask(method, at(5, 1)).err().map(|error| error.code);
        assert_ne!(
            refused,
            Some(-32601),
            "{method} is advertised in the handshake and refused as method-not-found"
        );
    }

    assert_eq!(capabilities["textDocumentSync"]["openClose"], json!(true));
    editor.goodbye();
}

#[test]
fn completion_offers_every_keyword_and_every_law() {
    let mut editor = authoring();
    let items = editor.ask("textDocument/completion", at(5, 1)).expect("completion answers");
    let labels: Vec<&str> = items["items"]
        .as_array()
        .expect("items")
        .iter()
        .filter_map(|i| i["label"].as_str())
        .collect();

    for keyword in ["package", "workspace", "zones", "seal", "keep", "use", "variance"] {
        assert!(labels.contains(&keyword), "completion is missing the `{keyword}` keyword");
    }
    for law in zoning::ordinance::Law::NAMES {
        assert!(labels.contains(&law), "completion is missing the `{law}` law");
    }
    assert!(
        items["items"].as_array().is_some_and(|items| items
            .iter()
            .all(|item| { item["detail"].as_str().is_some_and(|detail| !detail.is_empty()) })),
        "an item with no detail is a menu entry that explains nothing"
    );
    editor.goodbye();
}

#[test]
fn hover_explains_a_keyword_and_stays_honest_about_a_name() {
    let mut editor = authoring();

    let keyword = editor.ask("textDocument/hover", at(5, 2)).expect("hover answers");
    let explained = keyword["contents"]["value"].as_str().expect("markdown hover");
    assert!(explained.contains("zones"), "hover must name what it is hovering: {explained}");
    assert!(
        explained.contains("ordered architectural layers"),
        "hover over a keyword must carry that keyword's own description: {explained}"
    );

    let name = editor.ask("textDocument/hover", at(7, 5)).expect("hover answers");
    let described = name["contents"]["value"].as_str().expect("markdown hover");
    assert!(described.contains("engine"), "hover over a zone name must name the zone");
    assert!(
        described.contains("A name in this zoning contract"),
        "a zone name is not a keyword and must not borrow a keyword's description"
    );
    editor.goodbye();
}

#[test]
fn definition_lands_on_the_first_mention_and_references_find_them_all() {
    let mut editor = authoring();

    let definition = editor.ask("textDocument/definition", at(7, 5)).expect("definition answers");
    assert_eq!(definition["uri"], buffer("layered"));
    assert_eq!(definition["range"]["start"], json!({"line": 7, "character": 4}));
    assert_eq!(definition["range"]["end"], json!({"line": 7, "character": 10}));

    let references = editor.ask("textDocument/references", at(7, 5)).expect("references answer");
    let lines: Vec<u64> = references
        .as_array()
        .expect("references are a list")
        .iter()
        .filter_map(|found| found["range"]["start"]["line"].as_u64())
        .collect();
    assert_eq!(
        lines,
        vec![7, 11],
        "`engine` is declared in the stack and sealed below it; `engine.zig` and `engine/**` \
         are different words and must not be counted"
    );
    editor.goodbye();
}

#[test]
fn document_symbols_carry_the_declarations_and_workspace_symbols_filter() {
    let mut editor = authoring();

    let symbols = editor.ask("textDocument/documentSymbol", whole()).expect("symbols answer");
    let named: Vec<&str> = symbols
        .as_array()
        .expect("symbols are a list")
        .iter()
        .filter_map(|symbol| symbol["name"].as_str())
        .collect();
    assert_eq!(named, vec!["layered", "engine"], "the package and its seal are the declarations");
    for symbol in symbols.as_array().expect("symbols are a list") {
        assert!(symbol["selectionRange"].is_object(), "an outline entry needs a place to jump to");
    }

    let other = buffer("other");
    editor.open(&other, "package hearth\n");
    let matched = editor
        .ask("workspace/symbol", json!({"query": "hearth"}))
        .expect("workspace symbols answer");
    let found = matched.as_array().expect("workspace symbols are a list");
    assert_eq!(found.len(), 1, "the query must narrow to one package: {matched}");
    assert_eq!(found[0]["name"], "hearth");
    assert_eq!(
        found[0]["location"]["uri"], other,
        "a workspace symbol without a location cannot be opened"
    );
    assert!(found[0]["range"].is_null(), "a workspace symbol carries a location, not a range");
    editor.goodbye();
}

#[test]
fn folding_ranges_follow_the_brace_nesting() {
    let mut editor = authoring();
    let folds = editor.ask("textDocument/foldingRange", whole()).expect("folding ranges answer");
    let pairs: Vec<(u64, u64)> = folds
        .as_array()
        .expect("folds are a list")
        .iter()
        .filter_map(|fold| Some((fold["startLine"].as_u64()?, fold["endLine"].as_u64()?)))
        .collect();
    assert_eq!(pairs, vec![(0, 3), (5, 9)], "the package block and the stack are the two folds");
    editor.goodbye();
}

#[test]
fn formatting_trims_the_ends_of_lines_and_then_has_nothing_left_to_do() {
    let mut editor = Editor::attached();
    editor.hello();
    let uri = buffer("ragged");
    editor.open(
        &uri,
        "package ragged {   \n    root src\t\n}\n\nzones {\n    floor floor.zig  \n}\n",
    );

    let edits = editor
        .ask("textDocument/formatting", json!({"textDocument": {"uri": uri}, "options": {}}))
        .expect("formatting answers");
    let edits = edits.as_array().expect("edits are a list");
    assert_eq!(edits.len(), 1, "one whole-document rewrite, not a patch per line");
    let tidied = edits[0]["newText"].as_str().expect("an edit carries replacement text");
    assert_eq!(tidied, "package ragged {\n    root src\n}\n\nzones {\n    floor floor.zig\n}\n");
    assert_eq!(edits[0]["range"]["start"], json!({"line": 0, "character": 0}));

    let settled = editor.edit(&uri, tidied);
    assert!(settled.is_empty(), "the tidied text must still parse, and got {settled:?}");
    let again = editor
        .ask("textDocument/formatting", json!({"textDocument": {"uri": uri}, "options": {}}))
        .expect("formatting answers");
    assert_eq!(again, json!([]), "formatting an already-formatted buffer must be a no-op");
    editor.goodbye();
}

#[test]
fn rename_rewrites_a_zone_everywhere_and_refuses_a_keyword() {
    let mut editor = authoring();

    let prepared =
        editor.ask("textDocument/prepareRename", at(7, 5)).expect("prepareRename answers");
    assert_eq!(prepared["placeholder"], "engine");
    assert_eq!(prepared["range"]["start"], json!({"line": 7, "character": 4}));

    let mut renaming = at(7, 5);
    renaming["newName"] = json!("kernel");
    let workspace = editor.ask("textDocument/rename", renaming).expect("rename answers");
    let edits =
        workspace["changes"][buffer("layered")].as_array().expect("edits for the open document");
    assert_eq!(edits.len(), 2, "the declaration and the seal both name the zone");
    assert!(
        edits.iter().all(|edit| edit["newText"] == "kernel"),
        "every edit of one rename carries the same new name"
    );

    // `zones` is a keyword, not a zone. Renaming it would rewrite the language itself.
    assert_eq!(editor.ask("textDocument/prepareRename", at(5, 2)).expect("answers"), Value::Null);
    let mut forbidden = at(5, 2);
    forbidden["newName"] = json!("layers");
    assert_eq!(editor.ask("textDocument/rename", forbidden).expect("answers"), Value::Null);
    editor.goodbye();
}

#[test]
fn semantic_tokens_are_five_tuple_deltas_and_leave_comments_alone() {
    let mut editor = Editor::attached();
    editor.hello();
    let uri = buffer("tokens");
    editor.open(&uri, "package demo {\n// keep this out of it\n    root src\n}\n");

    let tokens = editor
        .ask("textDocument/semanticTokens/full", json!({"textDocument": {"uri": uri}}))
        .expect("semantic tokens answer");
    let data: Vec<u64> =
        tokens["data"].as_array().expect("token data").iter().filter_map(Value::as_u64).collect();
    assert_eq!(data.len() % 5, 0, "semantic tokens are five numbers each");
    assert_eq!(
        data,
        vec![0, 0, 7, 0, 0, 2, 4, 4, 0, 0],
        "`package` on line 0 and `root` on line 2 — the `keep` on line 1 is inside a comment, \
         and a keyword token there paints comment text in keyword colour"
    );
    editor.goodbye();
}

#[test]
fn semantic_tokens_paint_whole_words_and_read_a_law_as_a_type() {
    let mut editor = Editor::attached();
    let legend = editor.hello()["capabilities"]["semanticTokensProvider"]["legend"]["tokenTypes"]
        .as_array()
        .expect("the legend lists its token types")
        .iter()
        .filter_map(|kind| kind.as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    let uri = buffer("painted");
    editor.open(
        &uri,
        "package demo\n\nzones {\n    engine packages/**\n}\n\n\
         seal engine through engine.zig\nvariance seal a.zig -> b.zig because \"why\"\n",
    );

    let tokens = editor
        .ask("textDocument/semanticTokens/full", json!({"textDocument": {"uri": uri}}))
        .expect("semantic tokens answer");
    let data: Vec<u64> =
        tokens["data"].as_array().expect("token data").iter().filter_map(Value::as_u64).collect();
    let painted: Vec<(u64, u64, u64, u64)> = data
        .chunks_exact(5)
        .scan((0, 0), |(line, start), token| {
            *line += token[0];
            *start = if token[0] == 0 { *start + token[1] } else { token[1] };
            Some((*line, *start, token[2], token[3]))
        })
        .collect();

    let keyword = 0;
    let kind = |name: &str| {
        legend.iter().position(|listed| listed == name).expect("the legend lists {name}") as u64
    };
    // `through` and `because` are absent on purpose: the vocabulary here is the one the
    // completion menu offers, which is the statement openers plus their descriptions, not
    // every word the grammar accepts. The editors' own highlighters carry the wider list, and
    // `editors.rs` holds them to covering at least this one.
    assert_eq!(
        painted,
        vec![
            (0, 0, 7, keyword),      // package
            (2, 0, 5, keyword),      // zones — and NOT `package` inside `packages/**` below
            (6, 0, 4, keyword),      // seal, opening its own statement
            (7, 0, 8, keyword),      // variance
            (7, 9, 4, kind("type")), // seal again, here naming a law rather than a statement
        ],
        "`packages/**` on line 3 contains the word `package`, and a keyword painted by \
         substring lands across the first seven characters of a path"
    );

    // Every type the legend promises has to be something the server produces, or a client is
    // told to reserve a colour for a token it will never receive.
    let produced: Vec<u64> = painted.iter().map(|&(.., kind)| kind).collect();
    for (index, name) in legend.iter().enumerate() {
        assert!(
            produced.contains(&(index as u64)),
            "the legend promises `{name}` but nothing in this buffer painted it — either \
             paint it or stop advertising it"
        );
    }
    editor.goodbye();
}

#[test]
fn a_code_action_appears_only_when_there_is_something_to_fix() {
    let mut editor = Editor::attached();
    editor.hello();
    let uri = buffer("actions");
    let asking = json!({
        "textDocument": {"uri": uri},
        "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
        "context": {"diagnostics": []}
    });

    editor.open(&uri, "package tidy\n");
    assert_eq!(
        editor.ask("textDocument/codeAction", asking.clone()).expect("code actions answer"),
        json!([]),
        "a clean buffer must not offer a fix that changes nothing"
    );

    editor.edit(&uri, "package tidy   \n");
    let actions = editor.ask("textDocument/codeAction", asking).expect("code actions answer");
    let actions = actions.as_array().expect("actions are a list");
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0]["title"], "Trim trailing whitespace");
    assert_eq!(actions[0]["kind"], "source.fixAll.zoning");
    assert_eq!(actions[0]["edit"]["changes"][uri][0]["newText"], "package tidy\n");
    editor.goodbye();
}

#[test]
fn diagnostics_follow_a_document_through_change_save_and_close() {
    let mut editor = Editor::attached();
    editor.hello();
    let uri = buffer("lifecycle");

    let broken = editor.open(&uri, "module demo\nzones { 😀 }\n");
    let first = broken.first().expect("a malformed contract must be reported");
    assert_eq!(first["source"], "zoning");
    assert_eq!(first["severity"], 1);
    assert!(
        first["message"].as_str().is_some_and(|message| !message.is_empty()),
        "a diagnostic with no message is a squiggle that says nothing"
    );
    assert!(
        first["range"]["start"]["line"].as_u64().is_some_and(|line| line < 2),
        "the fault must point inside the two lines that exist: {first}"
    );

    let fixed = editor.edit(&uri, CONTRACT);
    assert!(fixed.is_empty(), "fixing the text must retract the diagnostic, and left {fixed:?}");

    editor.tell("textDocument/didSave", json!({"textDocument": {"uri": uri}}));
    assert!(editor.diagnostics(&uri).is_empty(), "saving must republish, not resurrect");

    editor.tell("workspace/didChangeWatchedFiles", json!({"changes": []}));
    assert!(editor.diagnostics(&uri).is_empty(), "a watched-file change republishes every buffer");

    editor.tell("textDocument/didClose", json!({"textDocument": {"uri": uri}}));
    assert!(
        editor.diagnostics(&uri).is_empty(),
        "closing must clear the buffer's diagnostics, or they outlive the window"
    );
    editor.goodbye();
}

#[test]
fn a_real_violation_reaches_the_source_file_that_caused_it() {
    let package = editor::fixture("fail", "trespass");
    let offender = package.join("src/other.zig");
    let uri = editor::uri_of(&offender);
    let text = std::fs::read_to_string(&offender).expect("the fixture must be readable");

    let mut editor = Editor::attached();
    editor.hello();
    let found = editor.open(&uri, &text);

    let violation = found
        .first()
        .unwrap_or_else(|| panic!("`{uri}` breaks the keep law and must say so in its own buffer"));
    assert_eq!(violation["code"], "keep", "a diagnostic must name the law that fired");
    assert_eq!(violation["source"], "zoning");
    assert_eq!(violation["range"]["start"]["line"], 0, "the import is on the first line");
    assert!(
        violation["message"].as_str().is_some_and(|message| message.contains("guest list")),
        "the buffer must carry the same remedy the command line prints: {violation}"
    );
    editor.goodbye();
}

#[test]
fn utf16_positions_survive_text_that_is_no_longer_ascii() {
    let mut editor = Editor::attached();
    editor.hello();
    let uri = buffer("wide");
    // `café` is five bytes and four UTF-16 units; the emoji is four bytes and two units.
    // Every column below is the column an editor sends, so a byte offset used as a column
    // lands in the wrong place and only ever in files like this one.
    editor.open(&uri, "package café\n\nzones {\n    café café/**\n}\n// 😀 done\n");

    let word = json!({"textDocument": {"uri": uri}, "position": {"line": 3, "character": 6}});
    let references =
        editor.ask("textDocument/references", word.clone()).expect("references answer");
    let ranges = references.as_array().expect("references are a list");
    assert_eq!(
        ranges
            .iter()
            .filter_map(|found| found["range"]["start"]["line"].as_u64())
            .collect::<Vec<_>>(),
        vec![0, 3],
        "the package name and the zone, and not `café/**`, which is a different word: {references}"
    );
    assert_eq!(ranges[1]["range"]["start"], json!({"line": 3, "character": 4}));
    assert_eq!(
        ranges[1]["range"]["end"],
        json!({"line": 3, "character": 8}),
        "`café` ends four UTF-16 units after it starts, not five bytes"
    );

    let hover = editor.ask("textDocument/hover", word).expect("hover answers");
    assert!(
        hover["contents"]["value"].as_str().is_some_and(|value| value.contains("café")),
        "a column past a multi-byte character must still resolve the word under it: {hover}"
    );

    let symbols = editor
        .ask("textDocument/documentSymbol", json!({"textDocument": {"uri": uri}}))
        .expect("symbols answer");
    assert_eq!(symbols[0]["name"], "café");
    assert_eq!(symbols[0]["range"]["end"], json!({"line": 0, "character": 12}));
    editor.goodbye();
}

#[test]
fn an_unknown_request_is_refused_and_an_unknown_notification_is_ignored() {
    let mut editor = authoring();

    let refused = editor
        .ask("textDocument/inlayHint", whole())
        .expect_err("a capability the server never advertised must not be answered");
    assert_eq!(refused.code, -32601, "the protocol's code for method-not-found");
    assert!(
        refused.message.contains("textDocument/inlayHint"),
        "the refusal must name what was refused: {}",
        refused.message
    );

    // A client is free to send notifications the server never asked for. Ignoring them is
    // the protocol; dying on them would take the whole session down with it.
    editor.tell("$/setTrace", json!({"value": "verbose"}));
    editor.tell("textDocument/willSave", json!({"textDocument": {"uri": buffer("layered")}}));
    let still_working = editor.ask("textDocument/hover", at(5, 2)).expect("the session survives");
    assert!(still_working["contents"]["value"].as_str().is_some_and(|v| v.contains("zones")));
    editor.goodbye();
}

#[test]
fn the_installed_executable_speaks_the_protocol_over_real_pipes() {
    // Everything above rides an in-memory channel, which skips the two things every editor
    // depends on and nothing else covers: Content-Length framing on a real pipe, and the
    // order `serve_stdio` drops the connection in — hold it across the join and a clean
    // `exit` deadlocks the process instead of ending it, which an editor sees as a server
    // that never restarts.
    let mut editor = Editor::spawned();
    let welcome = editor.hello();
    assert_eq!(welcome["serverInfo"]["name"], "zoning");
    assert_eq!(
        welcome["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION"),
        "the version an editor logs must be the version that shipped"
    );
    assert!(welcome["capabilities"]["hoverProvider"].as_bool().unwrap_or_default());

    let uri = buffer("piped");
    let faults = editor.open(&uri, "module demo\nzones { 😀 }\n");
    assert!(!faults.is_empty(), "diagnostics must cross the pipe");
    let hover = editor
        .ask(
            "textDocument/hover",
            json!({
                "textDocument": {"uri": uri}, "position": {"line": 0, "character": 1}
            }),
        )
        .expect("a request must cross the pipe and come back");
    assert!(hover["contents"]["value"].is_string());

    // `goodbye` requires exit status zero, which is the deadlock assertion.
    editor.goodbye();
}
