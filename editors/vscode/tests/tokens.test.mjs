import assert from "node:assert/strict";
import test from "node:test";

import { scopeAt } from "./tokenize.mjs";

const CONTRACT = [
  "package layered {",
  "    root src",
  "    language zig",
  '    exclude "vendor/**"',
  "}",
  "",
  "// a comment about the layers",
  "",
  "zones {",
  "    floor floor.zig",
  "    engine engine/**",
  "}",
  "",
  "seal engine through engine.zig open to face.zig",
  "keep floor to nobody",
  "use ledger hyper by engine",
  "limit reach to 2 hops",
  "forbid cycles across directories",
  'variance seal face.zig -> engine/parts.zig because "retire through the facade"',
  "variance cycle {",
  "    floor.zig",
  "    engine.zig",
  "}",
  "because",
  "    \\\\ the loader has to see the engine to load it",
].join("\n");

test("a declaration names itself and its subject", () => {
  assert.equal(scopeAt(CONTRACT, 0, "package"), "keyword.declaration.package.zoning");
  assert.equal(scopeAt(CONTRACT, 0, "layered"), "entity.name.namespace.zoning");
  assert.equal(scopeAt(CONTRACT, 8, "zones"), "keyword.control.zoning");
  assert.equal(scopeAt(CONTRACT, 13, "seal"), "keyword.declaration.zoning");
  assert.equal(scopeAt(CONTRACT, 13, "engine "), "entity.name.section.zoning");
  assert.equal(scopeAt(CONTRACT, 14, "keep"), "keyword.declaration.zoning");
  assert.equal(scopeAt(CONTRACT, 15, "use"), "keyword.declaration.zoning");
});

test("every keyword in the middle of a statement is a keyword", () => {
  for (const [line, word] of [
    [1, "root"],
    [2, "language"],
    [3, "exclude"],
    [13, "through"],
    [13, "open"],
    [14, "nobody"],
    [15, "by"],
    [16, "limit"],
    [16, "reach"],
    [16, "hops"],
    [17, "forbid"],
    [17, "cycles"],
    [17, "across"],
    [17, "directories"],
    [18, "because"],
  ]) {
    assert.equal(
      scopeAt(CONTRACT, line, word),
      "keyword.control.zoning",
      `\`${word}\` on line ${line}`,
    );
  }
});

test("a law is a type where a variance names one, and a keyword where it opens a statement", () => {
  // `seal`, `keep`, `use`, and `reach` each name both a statement and a law. The word after
  // `variance` is the law, and the same word at the head of its own line is the statement.
  assert.equal(scopeAt(CONTRACT, 18, "seal"), "support.type.law.zoning");
  assert.equal(scopeAt(CONTRACT, 19, "cycle"), "support.type.law.zoning");
  assert.equal(scopeAt(CONTRACT, 13, "seal"), "keyword.declaration.zoning");
  assert.equal(scopeAt(CONTRACT, 14, "keep"), "keyword.declaration.zoning");
  assert.equal(scopeAt(CONTRACT, 15, "use"), "keyword.declaration.zoning");
  assert.equal(scopeAt(CONTRACT, 16, "reach"), "keyword.control.zoning");
});

test("a zone is named by where it sits, so the block has to be entered to see it", () => {
  assert.equal(scopeAt(CONTRACT, 9, "floor "), "entity.name.section.zoning");
  assert.equal(scopeAt(CONTRACT, 10, "engine "), "entity.name.section.zoning");
  const nested = [
    "zones {",
    "    engine {",
    "        engine/**",
    "    }",
    "    face face.zig",
    "}",
    "",
    "keep face.zig to nobody",
  ].join("\n");
  assert.equal(scopeAt(nested, 1, "engine"), "entity.name.section.zoning");
  assert.equal(scopeAt(nested, 2, "engine/**"), "string.unquoted.path.zoning");
  // The nested block closes on its own brace, so the zone after it is still a zone …
  assert.equal(scopeAt(nested, 4, "face "), "entity.name.section.zoning");
  // … and the outer brace still closes the block, rather than the file running on inside it.
  assert.equal(scopeAt(nested, 7, "keep"), "keyword.declaration.zoning");
});

test("globs, strings, numbers, and operators each read as themselves", () => {
  assert.equal(scopeAt(CONTRACT, 10, "engine/**"), "string.unquoted.path.zoning");
  assert.equal(scopeAt(CONTRACT, 3, "vendor/**"), "string.quoted.double.zoning");
  assert.equal(scopeAt(CONTRACT, 16, "2"), "constant.numeric.integer.zoning");
  assert.equal(scopeAt(CONTRACT, 18, "->"), "keyword.operator.arrow.zoning");
  assert.equal(scopeAt(CONTRACT, 0, "{"), "punctuation.section.block.zoning");
});

test("a folded reason reads as prose, and its marker as a marker", () => {
  assert.equal(scopeAt(CONTRACT, 24, "\\\\"), "punctuation.definition.string.zoning");
  assert.equal(scopeAt(CONTRACT, 24, "loader"), "string.unquoted.folded-reason.zoning");
});

test("nothing inside a comment is code", () => {
  const prose = [
    "package demo",
    "",
    "// the package keyword and a core/** glob and 12 hops and a -> arrow",
  ].join("\n");
  for (const word of ["package", "core/**", "12", "hops", "->"]) {
    assert.equal(
      scopeAt(prose, 2, word),
      "comment.line.double-slash.zoning",
      `\`${word}\` inside a comment`,
    );
  }
});

test("a trailing comment does not swallow the statement in front of it", () => {
  const trailing = "zones { // core/** looks like a glob\n";
  assert.equal(scopeAt(trailing, 0, "zones"), "keyword.control.zoning");
  assert.equal(scopeAt(trailing, 0, "//"), "punctuation.definition.comment.zoning");
  assert.equal(scopeAt(trailing, 0, "core/**"), "comment.line.double-slash.zoning");
});

test("an escape inside a reason is an escape", () => {
  const escaped = 'variance seal a.zig -> b.zig because "retire the \\"facade\\""\n';
  assert.equal(scopeAt(escaped, 0, "\\\""), "constant.character.escape.zoning");
});
