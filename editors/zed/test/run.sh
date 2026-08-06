#!/usr/bin/env bash
# Everything checkable about the Zed extension without running Zed.
#
# Three things can be wrong with a Zed language: the grammar can parse the wrong tree, a
# query can fail to compile, and a query can compile and quietly match nothing - which is
# what a grammar rename leaves behind, and what Zed itself gives no sign of, because an
# unmatched query is simply an unpainted buffer. The corpus and highlight tests cover the
# first, and the query pass below covers the other two by running each query against a
# contract that exercises every construct and requiring the captures Zed reads to appear.
set -uo pipefail

here="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
grammar="$here/../grammar"
queries="$here/../languages/zoning"
fixture="$here/fixture.zone"
cli="$grammar/node_modules/.bin/tree-sitter"

if [ ! -x "$cli" ]; then
  echo "run.sh: tree-sitter is not installed; run \`npm ci\` in editors/zed/grammar" >&2
  exit 1
fi

checks=0
status=0
check() { # check <name> <condition-exit-code>
  checks=$((checks + 1))
  if [ "$2" -eq 0 ]; then
    echo "ok $checks - $1"
  else
    echo "not ok $checks - $1"
    status=1
  fi
}

# The grammar, and the highlight annotations beside it. A fixture whose tree carries an ERROR
# node has been seen to hang the highlighter rather than fail it, so this is bounded: a
# malformed fixture must be a red job and not a job that never ends.
(cd "$grammar" && ./node_modules/.bin/tree-sitter generate --abi=14) >/dev/null 2>&1
check "the parser regenerates from grammar.js" $?
(cd "$grammar" && "$cli" test >"$here/.tree-sitter-test.log" 2>&1) &
runner=$!
waited=0
while kill -0 "$runner" 2>/dev/null && [ "$waited" -lt 120 ]; do
  sleep 1
  waited=$((waited + 1))
done
if kill -0 "$runner" 2>/dev/null; then
  kill -9 "$runner" 2>/dev/null
  check "corpus and highlight tests finish (they hung after ${waited}s)" 1
else
  wait "$runner"
  check "corpus and highlight tests pass" $?
fi
grep -E '^\s+(✓|✗)' "$here/.tree-sitter-test.log" 2>/dev/null | sed 's/^/#   /'
rm -f "$here/.tree-sitter-test.log"

# Each query compiles, and each one still finds what Zed asks it for.
captures_of() {
  (cd "$grammar" && "$cli" query "$queries/$1" "$fixture" -c 2>/dev/null) |
    sed -n 's/.*capture: [0-9]* - \([a-z._]*\),.*/\1/p;s/.*capture: \([a-z._]*\), start.*/\1/p' |
    sort -u
}

for query in highlights indents brackets outline; do
  (cd "$grammar" && "$cli" query "$queries/$query.scm" "$fixture" >/dev/null 2>&1)
  check "$query.scm compiles and runs" $?
done

for wanted in comment string string.escape number keyword type title operator \
  punctuation.bracket; do
  captures_of highlights.scm | grep -qx "$wanted"
  check "highlights.scm still paints $wanted" $?
done

for wanted in indent end; do
  captures_of indents.scm | grep -qx "$wanted"
  check "indents.scm still captures $wanted" $?
done

for wanted in open close; do
  captures_of brackets.scm | grep -qx "$wanted"
  check "brackets.scm still captures $wanted" $?
done

for wanted in item name; do
  captures_of outline.scm | grep -qx "$wanted"
  check "outline.scm still captures $wanted" $?
done

# Zed ignores a capture name it does not know, so a typo is invisible in the editor: the
# buffer is simply unpainted where the rule was meant to apply. The names Zed's themes key
# off are a closed set, so anything outside it is a mistake caught here instead of by eye.
known=$(
  cat <<'EOF'
comment
string
string.escape
string.special
number
keyword
type
title
variable
variable.special
constant
function
property
operator
punctuation
punctuation.bracket
punctuation.delimiter
punctuation.list_marker
label
attribute
embedded
tag
emphasis
link_uri
link_text
primary
predictive
hint
EOF
)
while read -r found; do
  [ -z "$found" ] && continue
  printf '%s\n' "$known" | grep -qxF "$found"
  check "highlights.scm capture @$found is one Zed knows" $?
done <<EOF
$(captures_of highlights.scm)
EOF

echo "1..$checks"
[ "$status" -eq 0 ] && echo "# zed: $checks passed" || echo "# zed: some of $checks failed"
exit "$status"
