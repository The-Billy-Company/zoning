import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const json = async (path) =>
  JSON.parse(await readFile(resolve(root, path), "utf8"));

const manifest = await json("package.json");
const cargo = await readFile(resolve(root, "../../Cargo.toml"), "utf8");
await Promise.all([
  json("language-configuration.json"),
  json("syntaxes/zoning.tmLanguage.json"),
]);

if (manifest.main !== "./dist/extension.js") {
  throw new Error("package main must name the deterministic bundle");
}
if (manifest.contributes.languages[0].extensions.join() !== ".zone") {
  throw new Error("the extension must register exactly the .zone suffix");
}
if (manifest.dependencies?.zoning || manifest.files?.includes("zoning")) {
  throw new Error("the VSIX must not bundle the zoning executable");
}
const cargoVersion = cargo.match(/^\s*version\s*=\s*"([^"]+)"/m)?.[1];
if (manifest.version !== cargoVersion) {
  throw new Error(
    `extension ${manifest.version} and zoning ${cargoVersion ?? "unknown"} versions differ`,
  );
}

for (const [copy, source] of [
  ["icons/zoning.svg", "../../assets/zoning.svg"],
  ["icons/zoning-light.svg", "../../assets/zoning-light.svg"],
  ["icons/zoning-dark.svg", "../../assets/zoning-dark.svg"],
]) {
  const [actual, expected] = await Promise.all([
    readFile(resolve(root, copy), "utf8"),
    readFile(resolve(root, source), "utf8"),
  ]);
  if (actual !== expected) {
    throw new Error(`${copy} is not an exact copy of ${source}`);
  }
}

console.log("validated VS Code manifests and exact SVG copies");
