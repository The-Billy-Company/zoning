"""A relative import within the same zone, and a standard-library import that
needs no grant at all — proof `ambient()` actually excuses it.
"""

import os
from . import util


def greet(name: str) -> str:
    return util.shout(f"hello, {name}, from pid {os.getpid()}")
