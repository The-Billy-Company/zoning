// Run the shipped TextMate grammar the way VS Code runs it.
//
// The grammar is a pile of Oniguruma patterns whose behavior is the *interaction* between
// them - which one wins at a position, and what a `begin`/`end` pair swallows on the way. A
// test that greps the JSON for a word proves the word is written down; this loads the same
// grammar into the same engine VS Code uses and asks what scope a character ends up in,
// which is the only question a user can see the answer to.

import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

// Both packages are CommonJS, so the whole surface arrives on the default export.
import oniguruma from "vscode-oniguruma";
import textmate from "vscode-textmate";

const root = resolve(import.meta.dirname, "..");
const path = resolve(root, "syntaxes/zoning.tmLanguage.json");

await oniguruma.loadWASM(
  await readFile(resolve(root, "node_modules/vscode-oniguruma/release/onig.wasm")),
);

const registry = new textmate.Registry({
  onigLib: Promise.resolve({
    createOnigScanner: (patterns) => new oniguruma.OnigScanner(patterns),
    createOnigString: (line) => new oniguruma.OnigString(line),
  }),
  loadGrammar: async (scope) =>
    scope === "source.zoning"
      ? textmate.parseRawGrammar(await readFile(path, "utf8"), path)
      : null,
});

const grammar = await registry.loadGrammar("source.zoning");
if (!grammar) {
  throw new Error("source.zoning did not load; the grammar's scopeName has moved");
}

/**
 * Tokenize a whole document, carrying the rule stack across lines the way an editor does -
 * without which a multi-line construct is graded as if every line were the first.
 *
 * @param {string} text
 * @returns {textmate.IToken[][]}
 */
export function tokenize(text) {
  let stack = textmate.INITIAL;
  return text.split("\n").map((line) => {
    const { tokens, ruleStack } = grammar.tokenizeLine(line, stack);
    stack = ruleStack;
    return tokens;
  });
}

/**
 * The most specific scope covering the first occurrence of `needle` on the given line.
 * "Most specific" is the last scope on the token, which is what a theme resolves against.
 *
 * @param {string} text the whole document, so multi-line state is honored
 * @param {number} line zero-based
 * @param {string} needle
 * @returns {string}
 */
export function scopeAt(text, line, needle) {
  const source = text.split("\n")[line];
  const column = source.indexOf(needle);
  if (column < 0) {
    throw new Error(`line ${line} does not contain ${JSON.stringify(needle)}: ${source}`);
  }
  const found = tokenize(text)[line].find(
    ({ startIndex, endIndex }) => startIndex <= column && column < endIndex,
  );
  if (!found) {
    throw new Error(`nothing tokenized at line ${line} column ${column}`);
  }
  return found.scopes.at(-1) ?? "";
}
