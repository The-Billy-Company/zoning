The `discipline` job pins ruff and then runs it with no config, so the repo's Python was
being judged by ruff's defaults - and it had never actually passed under them. Twelve
findings, none of them in the Rust the tool is made of, so scheduled CI on `main` was red
for a reason nobody was going to go looking for.

Two things were wrong. Seven `# noqa` directives named rules the default set doesn't
enable, so they suppressed nothing and ruff flagged each one as dead; the `E402` four were
dead twice over, since ruff already exempts imports that follow a `sys.path` preamble. And
all five `bench/*.py` carried a shebang while none of them was executable. Only `bench.py`
has an entry block, so it is executable now and the other four - which are imported, never
run - lost a shebang that was decorative.

`ruff format` had never run at all: the check step aborts on the line above it. Two files
are reformatted to the 88 columns the rest of the tree already used.
