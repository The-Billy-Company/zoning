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
instead of becoming somebody's win. Reading the output for that is the last net rather
than the first, because the likeliest way to race a tool that is not installed is a
launcher standing where it should be, whose complaint looks nothing like a crash - so
every tool says its own version first, in the tree it is about to judge, since a shim
resolves per directory. A version has a digit in it and a complaint does not, which is
the whole test; no launcher is named anywhere, because a list of launchers is wrong the
day somebody uses one nobody here has heard of. And where a rival cannot express a rule at all it
is reported `n/a` with the reason in its own terms - `import-linter` and `tach` judge
modules, so a file-granular guest list has no spelling in either config language, and
calling that a failure to answer would be a lie in our favour.

The corpus is deliberately clean, because a dirty one measures how fast a tool can give
up. A clean graph has no early exit in it: every tool has to read every file to say
yes, which is the honest worst case and also what CI pays on every green run.
