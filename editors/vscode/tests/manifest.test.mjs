import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import test from "node:test";

const root = resolve(import.meta.dirname, "..");
const manifest = JSON.parse(
  await readFile(resolve(root, "package.json"), "utf8"),
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

test("everything the manifest points at is on disk and wired to the language", async () => {
  const [language] = manifest.contributes.languages;
  const [grammar] = manifest.contributes.grammars;
  assert.equal(language.id, "zoning");
  assert.equal(grammar.language, language.id, "a grammar for another language paints nothing");
  for (const path of [
    language.configuration,
    language.icon.light,
    language.icon.dark,
    grammar.path,
    manifest.main,
  ]) {
    await readFile(resolve(root, path), "utf8");
  }
  const declared = JSON.parse(await readFile(resolve(root, grammar.path), "utf8"));
  assert.equal(
    declared.scopeName,
    grammar.scopeName,
    "the manifest and the grammar must agree on the scope name, or nothing loads",
  );
});

test("the extension activates on the language it contributes", () => {
  const [language] = manifest.contributes.languages;
  assert.ok(
    manifest.activationEvents.includes(`onLanguage:${language.id}`),
    "opening a contract has to be enough to start the server",
  );
  for (const { command } of manifest.contributes.commands) {
    assert.ok(
      manifest.activationEvents.includes(`onCommand:${command}`),
      `${command} is in the palette, so it has to be able to wake the extension`,
    );
  }
});

test("the executable is a setting, so an unpublished build can be pointed at", () => {
  const setting = manifest.contributes.configuration.properties["zoning.executablePath"];
  assert.equal(setting.default, "zoning", "the default is the name the crate installs");
  assert.equal(
    setting.scope,
    "machine-overridable",
    "a path is a property of the machine, not of the repository",
  );
});

test("bundle delegates to an external zoning executable", async () => {
  const bundle = await readFile(resolve(root, "dist/extension.js"), "utf8");
  assert.match(bundle, /lsp/);
  assert.match(bundle, /--stdio/);
  assert.doesNotMatch(bundle, /base64.*zoning/);
});
