"""Reaching down into `core`, which the stack allows — and which drags
`acme/core/__init__.py` in with it, from outside. That edge is real."""

from __future__ import annotations

from acme.core.leaf import lower


def call() -> str:
    return lower()
