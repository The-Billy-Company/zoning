import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import test from "node:test";

const root = resolve(import.meta.dirname, "..");
const manifest = JSON.parse(
  await readFile(resolve(root, "package.json"), "utf8"),
);
const grammar = JSON.parse(
  await readFile(
    resolve(root, "syntaxes/zoning.tmLanguage.json"),
    "utf8",
  ),
);

test("manifest registers language, commands, and bundled client", () => {
  assert.deepEqual(manifest.contributes.languages[0].extensions, [".zone"]);
  assert.deepEqual(
    manifest.contributes.commands.map(({ command }) => command),
    ["zoning.setup", "zoning.status"],
  );
  assert.equal(manifest.dependencies["vscode-languageclient"], "^9.0.1");
  assert.equal(manifest.main, "./dist/extension.js");
});

test("grammar covers every declaration and comment spelling", () => {
  const encoded = JSON.stringify(grammar);
  for (const word of [
    "package",
    "workspace",
    "member",
    "zones",
    "seal",
    "keep",
    "use",
    "limit",
    "forbid",
    "variance",
    "because",
  ]) {
    assert.match(encoded, new RegExp(`\\\\b.*${word}|${word}.*\\\\b`));
  }
  assert.match(encoded, /double-slash/);
  assert.match(encoded, /folded-reason/);
});

test("bundle delegates to an external zoning executable", async () => {
  const bundle = await readFile(resolve(root, "dist/extension.js"), "utf8");
  assert.match(bundle, /lsp/);
  assert.match(bundle, /--stdio/);
  assert.doesNotMatch(bundle, /base64.*zoning/);
});
