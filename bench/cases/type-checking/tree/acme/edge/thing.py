"""The upper layer. Nothing below it may reach in here — not even for a type."""

from __future__ import annotations


class Upper:
    """A name worth importing only to annotate with."""


def upper() -> str:
    return "edge"
