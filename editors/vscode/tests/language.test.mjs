import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import test from "node:test";

// `language-configuration.json` is the half of the editing experience the grammar does not
// touch: what `//` comments with, what a brace auto-closes into, what counts as one word for
// a double-click, and when a newline indents. Every entry is a regex or a pair, so each one
// can be run here against the lines it is meant to fire on - which is how the indent rule
// turned out to catch three block forms out of six.

const configuration = JSON.parse(
  await readFile(
    resolve(import.meta.dirname, "../language-configuration.json"),
    "utf8",
  ),
);

const increase = new RegExp(configuration.indentationRules.increaseIndentPattern);
const decrease = new RegExp(configuration.indentationRules.decreaseIndentPattern);
const word = new RegExp(configuration.wordPattern, "g");

test("comments and pairs match the language", () => {
  assert.equal(configuration.comments.lineComment, "//");
  assert.deepEqual(configuration.brackets, [["{", "}"]]);
  assert.deepEqual(
    configuration.autoClosingPairs.map(({ open, close }) => `${open}${close}`),
    ["{}", '""'],
  );
  const quote = configuration.autoClosingPairs.find(({ open }) => open === '"');
  assert.deepEqual(
    quote.notIn,
    ["comment", "string"],
    "typing a quote inside a reason must not open a second one",
  );
});

test("a word is a whole path, so a double-click selects one", () => {
  for (const [line, expected] of [
    ["    engine engine/**", ["engine", "engine/**"]],
    ["    root src/kernel", ["root", "src/kernel"]],
    ["seal engine through engine.zig", ["seal", "engine", "through", "engine.zig"]],
    ["variance cycle a -> b because", ["variance", "cycle", "a", "->", "b", "because"]],
  ]) {
    assert.deepEqual([...line.matchAll(word)].map(([found]) => found), expected, line);
  }
  assert.deepEqual(
    [...'exclude "vendor/**"'.matchAll(word)].map(([found]) => found),
    ["exclude", "vendor/**"],
    "the quotes are not part of the path they wrap",
  );
});

test("every block form opens an indent", () => {
  for (const line of [
    "package layered {",
    "workspace {",
    "zones {",
    "    engine {",
    "keep face.zig to {",
    "use ledger by {",
    "variance cycle {",
    "zones { // with a trailing comment",
    "\tengine {",
  ]) {
    assert.match(line, increase, `\`${line}\` should open an indent`);
  }
});

test("a line that is not a block opener does not", () => {
  for (const line of [
    "package layered",
    "    root src",
    "}",
    "    }",
    "// zones { inside a comment",
    "    engine engine/**",
    "",
  ]) {
    assert.doesNotMatch(line, increase, `\`${line}\` should not open an indent`);
  }
});

test("a closing brace closes one, wherever it sits", () => {
  for (const line of ["}", "    }", "\t}"]) {
    assert.match(line, decrease, `\`${line}\` should close an indent`);
  }
  for (const line of ["    engine {", "package layered {", "// }"]) {
    assert.doesNotMatch(line, decrease, `\`${line}\` should not close an indent`);
  }
});

test("a reason indents the line that continues it", () => {
  const next = new RegExp(configuration.indentationRules.indentNextLinePattern);
  assert.match("variance cycle a -> b because", next);
  assert.doesNotMatch("    root src", next);
});
