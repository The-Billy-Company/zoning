"""A layer that re-exports its own helper — the ordinary Python package shape."""

from __future__ import annotations

from acme.core.helper import aid

__all__ = ["aid"]
