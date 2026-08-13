"""The import under test: absolute, and pointing at its own package."""

from __future__ import annotations

from acme.core.helper import aid


def lower() -> str:
    return aid()
