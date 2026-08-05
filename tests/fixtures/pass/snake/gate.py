"""The facade: crosses into the core zone by an absolute import, and carries
the one external dependency this package's contract grants it by name.
"""

import vendorlib

from core import core


def run(name: str) -> str:
    vendorlib.touch()
    return core.greet(name)
