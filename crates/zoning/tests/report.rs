//! What the two audiences actually receive.
//!
//! A report is an interface: a person reads the human form at a terminal width they did
//! not choose, and a script parses the machine form without ever seeing it. Both fail
//! quietly — a list that runs off the right edge still looks like an answer, and a verb
//! that ignores `--json` still prints something — so both are asserted here, over a
//! package built wide enough that a list has somewhere to overflow to.

#![allow(clippy::expect_used, reason = "a test that cannot build its fixture has failed")]

mod dice;

use std::fmt::Write as _;
use std::path::Path;

use zoning::ordinance::Ordinance;
use zoning::report::{self, Ink};
use zoning::survey::{Ask, Survey};

/// How many callers the hub gets. Enough that their names cannot share one line, since
/// the defect under test only appears once a field's value outgrows the terminal.
const CALLERS: usize = 40;

/// A package with one file everybody imports, and long names throughout.
fn wide() -> (Ordinance, Survey) {
    let root = dice::scratch("report");
    let module = root.join("src");
    write(
        &module.join("core/the_shared_hub_everything_reaches.zig"),
        "pub const value: usize = 1;\n",
    );
    for i in 0..CALLERS {
        let mut text = String::new();
        let _ = writeln!(
            text,
            "const hub = @import(\"../core/the_shared_hub_everything_reaches.zig\");"
        );
        let _ = writeln!(text, "const std = @import(\"std\");");
        text.push_str("pub const value: usize = hub.value;\n");
        write(&module.join(format!("edge/a_caller_with_a_realistic_name_{i:02}.zig")), &text);
    }
    let contract = dice::file(
        &root,
        "report",
        "package report {\n    root     src\n    language zig\n}\n\n\
         zones {\n    core  core/**\n    edge  edge/**\n}\n",
    );
    let dialect = zoning::survey::dialect("zig").expect("zig ships in-tree");
    let ordinance = Ordinance::read(&contract, dialect).expect("the contract parses");
    let survey = Survey::of(&Ask {
        repo_root: &root,
        module_root: &ordinance.module_root,
        exclude: &ordinance.exclude,
        dialect: ordinance.dialect,
        package: &ordinance.package,
        tracked: None,
    });
    assert_eq!(survey.files.len(), CALLERS + 1, "the tree did not survey whole");
    (ordinance, survey)
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("mkdir");
    std::fs::write(path, text).expect("write a generated file");
}

const HUB: &str = "core/the_shared_hub_everything_reaches.zig";

/// Braces and brackets balance and no field dangles — enough structure to catch a
/// hand-written emitter dropping a comma, without taking on a JSON parser to check it.
fn well_formed(json: &str) {
    let (mut depth, mut in_text, mut escaped) = (0i32, false, false);
    for byte in json.bytes() {
        match byte {
            b'\\' if in_text => escaped = !escaped,
            b'"' if !escaped => in_text = !in_text,
            b'{' | b'[' if !in_text => depth += 1,
            b'}' | b']' if !in_text => depth -= 1,
            _ => escaped = false,
        }
        assert!(depth >= 0, "closed more than was opened:\n{json}");
    }
    assert!(!in_text, "a string was never closed:\n{json}");
    assert_eq!(depth, 0, "left {depth} container(s) open:\n{json}");
    assert!(!json.contains(",,"), "an empty slot between two commas:\n{json}");
}

#[test]
fn the_graph_answers_in_json_with_every_end_already_sited() {
    let (ordinance, survey) = wide();
    let json = report::graph(&ordinance, &survey);
    well_formed(&json);
    for key in ["\"package\"", "\"zones\"", "\"edges\"", "\"outside\"", "\"seals\"", "\"keeps\""] {
        assert!(json.contains(key), "{key} missing from the graph:\n{json}");
    }
    // Every edge carries the zone each end sits in. Recovering that from the zone list
    // means re-running the glob matching this pass already did, which is the work the
    // field exists to spare a consumer.
    assert!(json.contains(r#""from": "edge", "to": "core""#), "edges lost their zones:\n{json}");
    // A departure says why it was allowed to leave: the language put the module there,
    // or a grant did. `std` is both here.
    assert!(json.contains(r#""module": "std""#), "the departure went missing:\n{json}");
}

#[test]
fn both_questions_explain_answers_have_a_machine_form() {
    let (ordinance, survey) = wide();
    let caller = "edge/a_caller_with_a_realistic_name_00.zig";

    let node = report::machine::node(HUB, &ordinance, &survey);
    well_formed(&node.text);
    assert!(node.text.contains(r#""zone": "core""#), "{}", node.text);
    assert!(node.text.contains(r#""clean": true"#), "{}", node.text);
    // All forty callers, not the three the human form used to sample.
    assert_eq!(node.text.matches(r#""src":"#).count(), CALLERS, "{}", node.text);

    let up = report::machine::edge(HUB, caller, &ordinance, &survey);
    well_formed(&up.text);
    assert!(up.text.contains(r#""allowed": false"#), "core may not import edge:\n{}", up.text);
    let down = report::machine::edge(caller, HUB, &ordinance, &survey);
    well_formed(&down.text);
    assert!(down.text.contains(r#""allowed": true"#), "edge may import core:\n{}", down.text);
}

#[test]
fn a_long_list_wraps_under_its_field_instead_of_running_off_the_edge() {
    // The report used to print three names and `+6`. Printing all of them is only an
    // improvement if the extra names arrive on lines somebody can read: one 1600-column
    // row is the same lost information wearing a different shape.
    let (ordinance, survey) = wide();
    let answer = report::file(HUB, &ordinance, &survey, &Ink::PLAIN);
    let rows: Vec<&str> = answer.text.lines().collect();
    for line in &rows {
        assert!(
            line.chars().count() <= 90,
            "a {}-column line nobody can read the end of:\n{line}",
            line.chars().count()
        );
    }
    // Wrapped or not, every caller is named.
    for i in 0..CALLERS {
        let name = format!("a_caller_with_a_realistic_name_{i:02}.zig");
        assert!(answer.text.contains(&name), "{name} was dropped from the list");
    }
    // Continuations line up under the field they continue, so the block reads as one
    // value rather than as a new field whose name went missing.
    let wrapped = rows.iter().filter(|l| l.starts_with("             ")).count();
    assert!(wrapped > 1, "forty names fit on one line?\n{}", answer.text);
}

#[test]
fn the_clause_that_claimed_a_file_is_named_rather_than_the_whole_zone() {
    // A drafted contract's tangle zone holds hundreds of globs. Printing them all buries
    // the one fact asked for — which line of the contract reached *this* file.
    let (ordinance, survey) = wide();
    let answer = report::file(HUB, &ordinance, &survey, &Ink::PLAIN);
    let claimed = answer
        .text
        .lines()
        .find(|l| l.starts_with("  claimed"))
        .expect("a zoned file says what claimed it");
    assert!(claimed.contains("core/**"), "the claim does not quote the contract: {claimed}");
    assert!(claimed.contains("only path"), "one glob should say so: {claimed}");
}
