use std::collections::HashMap;

use lsp_server::{Request, Response};
use serde_json::{Value, json};

use crate::ordinance::Law;

const KEYWORDS: &[(&str, &str)] = &[
    ("package", "Name the governed package."),
    ("workspace", "Hold a set of packages together and share their settings."),
    ("member", "Claim the packages that hang off this workspace."),
    ("root", "Name the package's source root."),
    ("language", "Select the source dialect."),
    ("facade", "Name files allowed to reach every zone."),
    ("exclude", "Hold generated or vendored source out of judgment."),
    ("zones", "Declare ordered architectural layers."),
    ("seal", "Expose a deep module only through its entry file."),
    ("keep", "Restrict a region to an explicit guest list."),
    ("use", "Grant an external module to selected zones."),
    ("forbid cycles", "Forbid cycles that cross directory boundaries."),
    ("limit reach", "Limit how far an import may climb."),
    ("variance", "Ratify one precise exception with a reason."),
];

pub(super) fn capabilities() -> Value {
    json!({
        "textDocumentSync": {"openClose": true, "change": 1, "save": {"includeText": true}},
        "completionProvider": {"triggerCharacters": [" ", "."]},
        "hoverProvider": true,
        "definitionProvider": true,
        "referencesProvider": true,
        "documentSymbolProvider": true,
        "workspaceSymbolProvider": true,
        "foldingRangeProvider": true,
        "documentFormattingProvider": true,
        "renameProvider": {"prepareProvider": true},
        "codeActionProvider": true,
        // Only what the server actually paints. Comments and strings are the grammar's, and a
        // legend entry nothing ever emits is a promise a client has no way to test.
        "semanticTokensProvider": {
            "legend": {"tokenTypes": ["keyword", "type"], "tokenModifiers": []},
            "full": true
        }
    })
}

pub(super) fn respond(documents: &HashMap<String, String>, request: Request) -> Response {
    let result = match request.method.as_str() {
        "textDocument/completion" => completion(),
        "textDocument/hover" => hover(documents, &request.params),
        "textDocument/definition" => locations(documents, &request.params, true),
        "textDocument/references" => locations(documents, &request.params, false),
        "textDocument/documentSymbol" => document_symbols(documents, &request.params),
        "workspace/symbol" => workspace_symbols(documents, &request.params),
        "textDocument/foldingRange" => folding_ranges(documents, &request.params),
        "textDocument/formatting" => formatting(documents, &request.params),
        "textDocument/prepareRename" => prepare_rename(documents, &request.params),
        "textDocument/rename" => rename(documents, &request.params),
        "textDocument/codeAction" => code_actions(documents, &request.params),
        "textDocument/semanticTokens/full" => {
            json!({"data": semantic_tokens(documents, &request.params)})
        }
        _ => {
            return Response::new_err(
                request.id,
                -32601,
                format!("unsupported method {}", request.method),
            );
        }
    };
    Response::new_ok(request.id, result)
}

fn completion() -> Value {
    let mut items: Vec<Value> = KEYWORDS
        .iter()
        .map(|(label, detail)| json!({"label": label, "kind": 14, "detail": detail}))
        .collect();
    items.extend(
        Law::NAMES
            .iter()
            .map(|name| json!({"label": name, "kind": 13, "detail": "architectural law"})),
    );
    json!({"isIncomplete": false, "items": items})
}

fn hover(documents: &HashMap<String, String>, params: &Value) -> Value {
    let Some((_, _, word, _)) = document_word(documents, params) else {
        return Value::Null;
    };
    let description = KEYWORDS
        .iter()
        .find(|(keyword, _)| keyword.split_whitespace().any(|part| part == word))
        .map_or("A name in this zoning contract.", |(_, description)| *description);
    json!({"contents": {"kind": "markdown", "value": format!("**`{word}`**\n\n{description}")}})
}

fn locations(documents: &HashMap<String, String>, params: &Value, first_only: bool) -> Value {
    let Some((uri, text, word, _)) = document_word(documents, params) else {
        return Value::Null;
    };
    let mut found = word_ranges(text, word)
        .into_iter()
        .map(|range| json!({"uri": uri, "range": range}))
        .collect::<Vec<_>>();
    if first_only {
        found.truncate(1);
        found.into_iter().next().unwrap_or(Value::Null)
    } else {
        Value::Array(found)
    }
}

fn document_symbols(documents: &HashMap<String, String>, params: &Value) -> Value {
    document_text(documents, params).map_or_else(|| json!([]), |text| Value::Array(symbols(text)))
}

fn workspace_symbols(documents: &HashMap<String, String>, params: &Value) -> Value {
    let query = params.get("query").and_then(Value::as_str).unwrap_or_default();
    Value::Array(
        documents
            .iter()
            .flat_map(|(uri, text)| {
                symbols(text).into_iter().filter_map(move |mut symbol| {
                    let name = symbol.get("name")?.as_str()?;
                    if !name.contains(query) {
                        return None;
                    }
                    symbol["location"] = json!({"uri": uri, "range": symbol["range"].clone()});
                    symbol.as_object_mut()?.remove("range");
                    symbol.as_object_mut()?.remove("selectionRange");
                    Some(symbol)
                })
            })
            .collect(),
    )
}

fn symbols(text: &str) -> Vec<Value> {
    text.lines()
        .enumerate()
        .filter_map(|(line, source)| {
            let trimmed = source.trim_start();
            let kind = ["package ", "member ", "seal ", "keep ", "use ", "variance "]
                .iter()
                .find(|prefix| trimmed.starts_with(**prefix))?;
            let name = trimmed
                .strip_prefix(kind)?
                .split(|character: char| character.is_whitespace() || character == '{')
                .next()
                .unwrap_or_default();
            let start = source.find(name).unwrap_or_default();
            let range = range(
                line,
                utf16_column(source, start),
                line,
                utf16_column(source, start + name.len()),
            );
            Some(json!({"name": name, "kind": 5, "range": range, "selectionRange": range}))
        })
        .collect()
}

fn folding_ranges(documents: &HashMap<String, String>, params: &Value) -> Value {
    let Some(text) = document_text(documents, params) else {
        return json!([]);
    };
    let mut stack = Vec::new();
    let mut folds = Vec::new();
    for (line, source) in text.lines().enumerate() {
        for character in source.chars() {
            match character {
                '{' => stack.push(line),
                '}' => {
                    if let Some(start) = stack.pop().filter(|start| *start < line) {
                        folds.push(json!({"startLine": start, "endLine": line}));
                    }
                }
                _ => {}
            }
        }
    }
    Value::Array(folds)
}

fn formatting(documents: &HashMap<String, String>, params: &Value) -> Value {
    let Some(text) = document_text(documents, params) else {
        return json!([]);
    };
    let formatted = formatted(text);
    if formatted == text {
        return json!([]);
    }
    json!([{"range": range(0, 0, text.lines().count(), 0), "newText": formatted}])
}

fn prepare_rename(documents: &HashMap<String, String>, params: &Value) -> Value {
    document_word(documents, params).map_or(Value::Null, |(_, text, word, range)| {
        if zone_names(text).contains(&word) {
            json!({"range": range, "placeholder": word})
        } else {
            Value::Null
        }
    })
}

fn rename(documents: &HashMap<String, String>, params: &Value) -> Value {
    let Some((uri, text, word, _)) = document_word(documents, params) else {
        return Value::Null;
    };
    if !zone_names(text).contains(&word) {
        return Value::Null;
    }
    let new_name = params.get("newName").and_then(Value::as_str).unwrap_or(word);
    let edits: Vec<Value> = word_ranges(text, word)
        .into_iter()
        .map(|range| json!({"range": range, "newText": new_name}))
        .collect();
    json!({"changes": {uri: edits}})
}

fn code_actions(documents: &HashMap<String, String>, params: &Value) -> Value {
    let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) else {
        return json!([]);
    };
    let Some(text) = documents.get(uri) else {
        return json!([]);
    };
    let formatted = formatted(text);
    if formatted == *text {
        return json!([]);
    }
    json!([{
        "title": "Trim trailing whitespace",
        "kind": "source.fixAll.zoning",
        "isPreferred": true,
        "edit": {"changes": {uri: [{
            "range": range(0, 0, text.lines().count(), 0),
            "newText": formatted
        }]}}
    }])
}

fn formatted(text: &str) -> String {
    format!("{}\n", text.lines().map(str::trim_end).collect::<Vec<_>>().join("\n"))
}

fn zone_names(text: &str) -> Vec<&str> {
    let mut inside = false;
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.starts_with("zones") && line.contains('{') {
                inside = true;
                return None;
            }
            if inside && line.starts_with('}') {
                inside = false;
                return None;
            }
            if inside && !line.starts_with("//") { line.split_whitespace().next() } else { None }
        })
        .collect()
}

/// The token types this server paints, in the order the legend declares them.
const KEYWORD: u32 = 0;
const TYPE: u32 = 1;

fn semantic_tokens(documents: &HashMap<String, String>, params: &Value) -> Vec<u32> {
    let Some(text) = document_text(documents, params) else {
        return Vec::new();
    };
    let mut previous_line = 0;
    let mut previous_start = 0;
    let mut data = Vec::new();
    for (line, source) in text.lines().enumerate() {
        // Only the code half of the line. An editor layers semantic tokens over the
        // grammar's own scopes, so a keyword token inside a comment does not merely add
        // nothing — it repaints comment text in keyword colour.
        let code = source.split_once("//").map_or(source, |(before, _)| before);
        for (start, word, kind) in painted(code) {
            let line_delta = line as u32 - previous_line;
            let start = utf16_column(source, start) as u32;
            let start_delta = if line_delta == 0 { start - previous_start } else { start };
            data.extend([line_delta, start_delta, word.encode_utf16().count() as u32, kind, 0]);
            previous_line = line as u32;
            previous_start = start;
        }
    }
    data
}

/// What is worth painting on one line of code, left to right.
///
/// Whole words only. A keyword found by substring lands inside the paths that contain it —
/// `packages/**` would come back carrying a `package` keyword across its first seven
/// characters — and a keyword list alone cannot tell `seal engine through …`, where the word
/// opens a statement, from `variance seal …`, where the same word names a law. Only the word
/// after `variance` can be a law, so that is the one place it is looked for.
fn painted(code: &str) -> impl Iterator<Item = (usize, &str, u32)> {
    let mut after_variance = false;
    words(code).filter_map(move |(start, word)| {
        let law = after_variance && Law::parse(word).is_some();
        after_variance = word == "variance";
        if law {
            Some((start, word, TYPE))
        } else if KEYWORDS
            .iter()
            .flat_map(|(keyword, _)| keyword.split_whitespace())
            .any(|keyword| keyword == word)
        {
            Some((start, word, KEYWORD))
        } else {
            None
        }
    })
}

/// The whitespace-separated words of `code`, each with the byte it starts at.
fn words(code: &str) -> impl Iterator<Item = (usize, &str)> {
    code.char_indices()
        .filter(|&(at, c)| {
            !c.is_whitespace() && code[..at].chars().next_back().is_none_or(char::is_whitespace)
        })
        .map(|(at, _)| {
            let end = code[at..].find(char::is_whitespace).map_or(code.len(), |gap| at + gap);
            (at, &code[at..end])
        })
}

fn document_text<'a>(documents: &'a HashMap<String, String>, params: &Value) -> Option<&'a str> {
    let uri = params.pointer("/textDocument/uri")?.as_str()?;
    documents.get(uri).map(String::as_str)
}

fn document_word<'a>(
    documents: &'a HashMap<String, String>,
    params: &Value,
) -> Option<(String, &'a str, &'a str, Value)> {
    let uri = params.pointer("/textDocument/uri")?.as_str()?;
    let text = documents.get(uri)?;
    let line = params.pointer("/position/line")?.as_u64()? as usize;
    let character = params.pointer("/position/character")?.as_u64()? as usize;
    let source = text.lines().nth(line)?;
    let byte = byte_at_utf16(source, character);
    let start = source[..byte].rfind(|c: char| !is_word(c)).map_or(0, |index| index + 1);
    let end = source[byte..].find(|c: char| !is_word(c)).map_or(source.len(), |index| byte + index);
    let word = &source[start..end];
    let range = range(line, utf16_column(source, start), line, utf16_column(source, end));
    Some((uri.to_owned(), text, word, range))
}

fn word_ranges(text: &str, word: &str) -> Vec<Value> {
    text.lines()
        .enumerate()
        .flat_map(|(line, source)| {
            source.match_indices(word).filter_map(move |(start, _)| {
                let before = source[..start].chars().next_back();
                let after = source[start + word.len()..].chars().next();
                (!before.is_some_and(is_word) && !after.is_some_and(is_word)).then(|| {
                    range(
                        line,
                        utf16_column(source, start),
                        line,
                        utf16_column(source, start + word.len()),
                    )
                })
            })
        })
        .collect()
}

pub(super) fn range(start_line: usize, start: usize, end_line: usize, end: usize) -> Value {
    json!({
        "start": {"line": start_line, "character": start},
        "end": {"line": end_line, "character": end}
    })
}

pub(super) fn utf16_column(line: &str, byte: usize) -> usize {
    line.get(..byte.min(line.len())).unwrap_or(line).encode_utf16().count()
}

fn byte_at_utf16(line: &str, column: usize) -> usize {
    let mut units = 0;
    for (byte, character) in line.char_indices() {
        if units >= column {
            return byte;
        }
        units += character.len_utf16();
    }
    line.len()
}

fn is_word(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '-' | '/' | '.')
}
