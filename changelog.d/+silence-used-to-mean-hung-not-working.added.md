A run that has to shell out to an editor's own CLI — `cursor --install-extension`,
`--list-extensions` — genuinely costs seconds, and the first-use setup did that with
nothing on screen, so a terminal that had gone quiet for a few seconds looked identical
to one that had hung. The main verbs had the same gap on a slow disk or a large `--under`
sweep: silence while `verify`/`status`/`show`/`map` read every contract, then the whole
report at once.

Both now start a small braille spinner on standard error the moment there is real work
to do, and stop it — clearing the line — the instant an answer is ready to print. It
costs nothing on the fast path: nothing renders until the call has run for 150ms, so a
`map` over a handful of small packages never draws a frame it would have to immediately
erase. Off a terminal, under `CI`, or with `ZONING_NO_SETUP` set, it never spawns a
thread at all — the report and its exit code are unchanged either way, and stdout never
carries a spinner byte.
