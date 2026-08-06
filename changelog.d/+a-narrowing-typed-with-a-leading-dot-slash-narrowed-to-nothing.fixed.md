`--under` compared its argument to the sweep's rows as a string, and the sweep's
rows are repo-relative posix paths. So `--under libs/kernels` worked and `--under
./libs/kernels` matched nothing — as did any absolute path, which is what a
shell's tab-completion hands you and what a CI script is entitled to write. The
failure mode is the worst one available to a gate: a narrowing to nothing reads
exactly like a clean tree, so `zone verify --under ./libs/kernels` judged no
package at all and exited 0.

The argument is now resolved as a place rather than compared as a spelling —
absolute, `./`-prefixed, and `.` itself all name the subtree they obviously mean,
and a path outside the tree being swept still narrows to nothing, because that is
what it means.
