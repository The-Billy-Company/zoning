"""The import under test: real in the source, absent at runtime."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from acme.edge.thing import Upper


def lower(seen: Upper | None = None) -> str:
    return "core" if seen is None else "edge"
