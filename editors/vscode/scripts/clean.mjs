import { mkdir, rm } from "node:fs/promises";

await Promise.all(
  ["artifacts", "dist"].map((path) =>
    rm(path, { force: true, recursive: true }),
  ),
);
await mkdir("artifacts", { recursive: true });
