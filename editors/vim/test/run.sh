#!/usr/bin/env bash
# Run every Vim suite in every Vim on this machine.
#
# Vim and Neovim are two implementations of the same runtime files, and they disagree often
# enough to be worth running both: `l:` scope at script level, where `:echo` goes in silent
# mode, and whether an autoload name may be defined in the wrong file are all differences
# this suite has already tripped over. Neither is required to be installed - a machine with
# only one of them still gets a real run - but a machine with neither is a failure, because
# then nothing ran and the exit code would say everything is fine.
set -uo pipefail

here="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
suites=("$here"/*.vim)
status=0
ran=0

for editor in vim nvim; do
  command -v "$editor" >/dev/null 2>&1 || continue
  for suite in "${suites[@]}"; do
    name="$(basename "$suite" .vim)"
    [ "$name" = harness ] && continue
    ran=$((ran + 1))
    if [ "$editor" = nvim ]; then
      "$editor" --headless -u NONE -i NONE -S "$suite" || status=1
    else
      "$editor" -Nu NONE -n -es -S "$suite" || status=1
    fi
  done
done

if [ "$ran" -eq 0 ]; then
  echo "run.sh: neither vim nor nvim is installed, so nothing was checked" >&2
  exit 1
fi
exit "$status"
