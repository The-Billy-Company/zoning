"""The import under test: a real dependency that never appears as an import."""

from __future__ import annotations

import importlib


def lower() -> str:
    reached = importlib.import_module("acme.edge.thing")
    return str(reached.upper())
