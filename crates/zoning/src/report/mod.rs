//! Turning a verdict into bytes somebody reads.
//!
//! Three audiences, three renderings, one model behind them. The human report leads
//! with the verdict and closes with the remedy for each law that failed, because a
//! gate whose output does not say what to do next gets silenced. The JSON is one
//! record per finding, for whatever reads it. The map is the contract as a picture:
//! the stack, top to bottom, with the weight of each zone beside it.

mod human;
mod json;
mod map;

pub use human::{show, suggest, verdict};
pub use json::records;
pub use map::map;

/// Terminal colour, or nothing at all when the destination is not a terminal.
///
/// Structurally invisible to a machine: a pipe, a redirect, and `--json` get plain
/// bytes, so nothing a script captures moves because a human happened to run it too.
#[derive(Clone, Copy)]
pub struct Ink {
    /// A violation.
    pub red: &'static str,
    /// A remedy.
    pub yellow: &'static str,
    /// A pass.
    pub green: &'static str,
    /// Secondary detail.
    pub dim: &'static str,
    /// Back to normal.
    pub reset: &'static str,
}

impl Ink {
    /// Colour, for a terminal.
    pub const LIT: Self = Self {
        red: "\u{1b}[0;31m",
        yellow: "\u{1b}[0;33m",
        green: "\u{1b}[0;32m",
        dim: "\u{1b}[2m",
        reset: "\u{1b}[0m",
    };

    /// No colour, for everything else.
    pub const PLAIN: Self = Self { red: "", yellow: "", green: "", dim: "", reset: "" };
}
