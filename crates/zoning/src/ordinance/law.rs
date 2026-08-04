//! The six laws, named once.
//!
//! Each exists because a compiler structurally cannot enforce it. They are a
//! closed set on purpose: a boundary language whose vocabulary grows per project
//! stops being a language and becomes a config file.

use std::fmt;

/// One of the seven things a contract can be violated on.
///
/// Five are about the inside of a package — where a file sits and who may reach it.
/// Two are about the outside: [`Law::Use`] when an import names a module the zone was
/// never granted, [`Law::Escape`] when a path climbs out of the module root. A
/// dependency leaves a package by exactly those two routes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Law {
    /// An import pointed up the stack.
    Zone,
    /// An import reached past a sealed directory's entry file.
    Seal,
    /// An importer not on the guest list reached into a kept region.
    Keep,
    /// An import cycle crossing a directory boundary.
    Cycle,
    /// An import climbed more directories than the ceiling allows.
    Reach,
    /// An import named an outside module this zone was not granted.
    Use,
    /// An import climbed out of the module root entirely.
    Escape,
}

impl Law {
    /// Every law, in reporting order.
    pub const ALL: [Self; 7] =
        [Self::Zone, Self::Seal, Self::Keep, Self::Cycle, Self::Reach, Self::Use, Self::Escape];

    /// The spellings a `variance` may name, in the same order as [`Law::ALL`].
    pub const NAMES: [&'static str; 7] =
        ["zone", "seal", "keep", "cycle", "reach", "use", "escape"];

    /// How this law is spelled in a contract.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    /// Parse a law name, or `None` if it is not one of the seven.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::NAMES.iter().position(|n| *n == name).map(|i| Self::ALL[i])
    }

    /// What to do about a violation — the one line a reader needs before deciding
    /// between fixing the code and ratifying the exception.
    #[must_use]
    pub fn remedy(self) -> &'static str {
        match self {
            Self::Zone => {
                "Move the dependency down the stack, or ratify the edge with \
                 `variance zone … because \"…\"`."
            }
            Self::Seal => {
                "Re-export what the caller needs from the seal's entry file, or widen \
                 that seal's `open to` list."
            }
            Self::Keep => {
                "Move the shared declaration down to a module both sides already stand \
                 on — widening the guest list is how a peer boundary dissolves."
            }
            Self::Cycle => {
                "Break the cycle by moving the shared declaration into a lower zone \
                 both sides may depend on."
            }
            Self::Reach => {
                "Move the file nearer what it depends on, or lower `limit reach` \
                 deliberately (it is a ceiling you lower, never raise to go green)."
            }
            Self::Use => {
                "Grant it with `use <module> by <zone>`, or take the dependency in a \
                 zone that already carries it — the point of the grant is that a new \
                 outside dependency is a decision somebody makes, not one an import \
                 makes quietly."
            }
            Self::Escape => "Declare the cross-module dependency as a named module in the build.",
        }
    }
}

impl fmt::Display for Law {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
