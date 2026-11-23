# Copyright (c) 2026 The Billy Company
"""One import, naming only the leaf — and initializing the package above it."""

from __future__ import annotations

from acme.deep.leaf import thing


def call() -> str:
    return thing()
