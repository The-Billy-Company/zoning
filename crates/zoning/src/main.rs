//! The `zoning` command.

mod command;

use std::process::ExitCode;

fn main() -> ExitCode {
    match command::run() {
        Ok(code) => code,
        Err(problem) => {
            eprintln!("zone: {problem}");
            ExitCode::from(2)
        }
    }
}
