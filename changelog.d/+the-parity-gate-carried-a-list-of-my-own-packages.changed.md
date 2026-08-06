`tools/differential.py` held a hardcoded table of seven contracts by name and
repo-relative path — the packages that happened to sit beside this one on the
machine it was written on. That is two problems in one line each. A public
repository shipped a list of somebody's private tree, and a contributor cloning
it got seven `skip` lines and a gate that proved nothing, with no hint that the
list was the thing to edit.

It sweeps the surrounding workspace instead, taking any `<pkg>/<pkg>.zone` or
`<pkg>/contract/<pkg>.zone` it finds and skipping this repository, whose own
fixtures are contracts written to fail rather than packages to check. Point it
somewhere specific with `--contract PATH`, repeatable. The list nobody could
keep in step is gone, and the documentation examples that named real packages
now name `acme`, like every other example here.
