//! One error type for the whole crate.
//!
//! A `.zone` file being malformed is a different kind of failure from a directory
//! that cannot be read or an editor CLI that will not cooperate — the first carries
//! a span and renders with a caret under the offending token, the rest are already
//! worded for a person and nothing downstream ever needs to match on why. [`Error`]
//! keeps that distinction instead of flattening everything to text on the way in,
//! so every fallible function in the crate returns the same [`Result`] and gets
//! there with `?` rather than a hand-rolled `.map_err(|e| e.to_string())`.

use std::fmt;
use std::io;

use crate::ordinance::Fault;

/// Every way running `zoning` can fail.
#[derive(Clone, Debug)]
pub enum Error {
    /// A `.zone` contract is malformed, or claims something the tree contradicts.
    /// See [`Fault`]'s own `Display` for the span-and-caret rendering.
    Fault(Fault),
    /// Everything else: a bad invocation, an unreadable path, a broken editor
    /// integration. Already worded for a person to read — `main` prints it and
    /// stops, and nothing else in the crate matches on it.
    Message(String),
}

/// Shorthand for a [`Result`](std::result::Result) whose error is [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fault(fault) => fmt::Display::fmt(fault, f),
            Self::Message(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Fault(fault) => Some(fault),
            Self::Message(_) => None,
        }
    }
}

impl From<Fault> for Error {
    fn from(fault: Fault) -> Self {
        Self::Fault(fault)
    }
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Self::Message(message.to_owned())
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Message(error.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Message(error.to_string())
    }
}
