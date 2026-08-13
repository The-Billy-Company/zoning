`bench/` races this gate against the tools people already use, and checks that it is
right. Three rungs: `correctness` asks every tool the same question on committed cases
whose expected verdict comes from the language reference rather than from our own
output; `speed` builds one clean layered package, writes the same rule as a `.zone`
contract, an `.importlinter` layers contract, and `tach.toml` modules, and times all
three; `compiler` asks whether `zig` itself says anything about a real cross-directory
cycle, which is the only incumbent Zig has.

Only a case where *we* miss something the language says we should catch can fail the
run. A rival disagreeing is reported and never fatal - a benchmark that breaks when
somebody else ships a release is a benchmark that gets deleted. The oracle rung runs in
CI with `--no-uv`, so it is hermetic and downloads nothing.

Two rules keep the numbers honest. A tool that failed to run is never scored as a tool
that found a violation, even though both exit non-zero: a moved flag or a config the
current release parses differently is reported as `error`, quoting what the tool said,
instead of becoming somebody's win. And where a rival cannot express a rule at all it
is reported `n/a` with the reason in its own terms - `import-linter` and `tach` judge
modules, so a file-granular guest list has no spelling in either config language, and
calling that a failure to answer would be a lie in our favour.

The corpus is deliberately clean, because a dirty one measures how fast a tool can give
up. A clean graph has no early exit in it: every tool has to read every file to say
yes, which is the honest worst case and also what CI pays on every green run.
