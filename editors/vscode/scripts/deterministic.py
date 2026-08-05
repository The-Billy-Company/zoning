"""Rewrite a VSIX with stable ordering, metadata, and timestamps."""

from __future__ import annotations

import sys
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

EPOCH = (1980, 1, 1, 0, 0, 0)


def repack(source: Path, destination: Path) -> None:
    with ZipFile(source) as archive:
        entries = [(name, archive.read(name)) for name in sorted(archive.namelist())]

    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_suffix(".tmp")
    with ZipFile(temporary, "w", compression=ZIP_DEFLATED, compresslevel=9) as archive:
        for name, contents in entries:
            info = ZipInfo(name, EPOCH)
            info.compress_type = ZIP_DEFLATED
            info.create_system = 3
            info.external_attr = (0o755 if name.endswith("/") else 0o644) << 16
            archive.writestr(info, contents)
    temporary.replace(destination)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit("usage: deterministic.py SOURCE.vsix DESTINATION.vsix")
    repack(*(Path(argument) for argument in sys.argv[1:]))
