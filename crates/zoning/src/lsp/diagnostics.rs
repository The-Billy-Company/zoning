use std::fs;
use std::path::{Path, PathBuf};

use lsp_server::{Connection, Notification};
use serde_json::{Value, json};

use super::language::{range, utf16_column};
use crate::judge;
use crate::ordinance::{Fault, Ordinance};
use crate::survey::{self, Ask, Survey};

pub(super) fn publish(connection: &Connection, uri: &str, text: &str) -> Result<(), String> {
    let path = uri_path(uri);
    if path.extension().is_none_or(|extension| extension != "zone") {
        return send(connection, uri, &architecture(&path, text));
    }
    let dialect = survey::dialect("zig").ok_or("the zig dialect is missing")?;
    let diagnostics: Vec<Value> =
        Ordinance::analyze(&path, text, dialect).faults.iter().map(diagnostic).collect();
    send(connection, uri, &diagnostics)
}

pub(super) fn clear(connection: &Connection, uri: &str) -> Result<(), String> {
    send(connection, uri, &[])
}

fn architecture(path: &Path, text: &str) -> Vec<Value> {
    let Some((contract, survey)) = owning_contract(path) else {
        return Vec::new();
    };
    let absolute = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    judge::judge(&survey, &contract)
        .findings
        .into_iter()
        .filter(|finding| {
            survey.repo_root.join(&finding.path).canonicalize().unwrap_or_default() == absolute
        })
        .map(|finding| {
            let line = finding.line.saturating_sub(1);
            let source = text.lines().nth(line).unwrap_or_default();
            let start = utf16_column(source, finding.col.saturating_sub(1));
            let end = utf16_column(source, finding.col.saturating_sub(1) + finding.width.max(1));
            json!({
                "range": range(line, start, line, end.max(start + 1)),
                "severity": 1,
                "source": "zoning",
                "code": finding.law.as_str(),
                "message": finding.message
            })
        })
        .collect()
}

fn owning_contract(path: &Path) -> Option<(Ordinance, Survey)> {
    let dialect = survey::dialect("zig")?;
    for ancestor in path.ancestors().skip(1) {
        let directory = ancestor.join("contract");
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let contract_path = entry.path();
            if contract_path.extension().is_none_or(|extension| extension != "zone") {
                continue;
            }
            let Ok(contract) = Ordinance::read(&contract_path, dialect) else {
                continue;
            };
            if !path.starts_with(&contract.module_root) {
                continue;
            }
            let repo_root = crate::repo_root(ancestor);
            let survey = Survey::of(&Ask {
                repo_root: &repo_root,
                module_root: &contract.module_root,
                exclude: &contract.exclude,
                dialect: contract.dialect,
                tracked: None,
            });
            return Some((contract, survey));
        }
    }
    None
}

fn send(connection: &Connection, uri: &str, diagnostics: &[Value]) -> Result<(), String> {
    let notification = Notification::new(
        "textDocument/publishDiagnostics".to_owned(),
        json!({"uri": uri, "diagnostics": diagnostics}),
    );
    connection.sender.send(notification.into()).map_err(|error| error.to_string())
}

fn diagnostic(fault: &Fault) -> Value {
    let line = fault.span.line.saturating_sub(1);
    let start = fault.span.col.saturating_sub(1);
    let text = fault.source.lines().nth(line).unwrap_or_default();
    let start = utf16_column(text, start);
    let end = utf16_column(text, fault.span.col.saturating_sub(1) + fault.span.width.max(1));
    json!({
        "range": range(line, start, line, end.max(start + 1)),
        "severity": 1,
        "source": "zoning",
        "message": fault.message
    })
}

fn uri_path(uri: &str) -> PathBuf {
    Path::new(uri.strip_prefix("file://").unwrap_or(uri)).to_path_buf()
}
