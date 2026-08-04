//! One record per finding, for whatever reads it.
//!
//! Hand-written rather than borrowed: the shape is six string fields and an integer,
//! and a serialization framework in a tool with no other dependencies would be the
//! largest thing in the binary. Emit only, never parse — nothing here ever reads
//! JSON back, which is what makes a hundred lines the right size.

use std::fmt::Write as _;

use crate::judge::Verdict;

/// Every finding and every stale declaration across the judged packages, as a JSON
/// array. Stale declarations carry the law `"stale"`, which is not one of the six —
/// they are the contract failing, not the code.
#[must_use]
pub fn records(verdicts: &[Verdict]) -> String {
    let mut rows: Vec<String> = Vec::new();
    for verdict in verdicts {
        for finding in &verdict.findings {
            rows.push(object(&[
                ("package", Value::Text(&verdict.package)),
                ("law", Value::Text(finding.law.as_str())),
                ("file", Value::Text(&finding.path)),
                ("line", Value::Number(finding.line)),
                ("subject", Value::Text(&finding.subject)),
                ("message", Value::Text(&finding.message)),
            ]));
        }
        for stale in &verdict.stale {
            rows.push(object(&[
                ("package", Value::Text(&verdict.package)),
                ("law", Value::Text("stale")),
                ("subject", Value::Text(stale)),
                ("message", Value::Text("declaration matches nothing")),
            ]));
        }
    }
    if rows.is_empty() {
        return "[]\n".to_owned();
    }
    format!("[\n{}\n]\n", rows.join(",\n"))
}

enum Value<'a> {
    Text(&'a str),
    Number(usize),
}

fn object(fields: &[(&str, Value<'_>)]) -> String {
    let body: Vec<String> = fields
        .iter()
        .map(|(key, value)| match value {
            Value::Text(text) => format!("    \"{key}\": \"{}\"", escape(text)),
            Value::Number(n) => format!("    \"{key}\": {n}"),
        })
        .collect();
    format!("  {{\n{}\n  }}", body.join(",\n"))
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_characters_and_quotes_survive_a_round_trip_by_eye() {
        assert_eq!(escape("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(escape("line\nbreak"), "line\\nbreak");
        assert_eq!(escape("bell\u{7}"), "bell\\u0007");
        assert_eq!(escape("plain — dash"), "plain — dash", "utf-8 passes through");
    }

    #[test]
    fn no_findings_is_an_empty_array_not_an_empty_string() {
        assert_eq!(records(&[]), "[]\n");
    }
}
