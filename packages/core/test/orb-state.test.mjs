// The `OrbState` union (src/index.ts) names every pattern the engine has: a pattern
// added to the engine but not to the union type-checks as an error for every TS
// caller (it happened for talking/playing/framing, caught by the Studio's tsc).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { parameterCatalog } from "../dist/index.js";

test("OrbState lists exactly the catalog's patterns", () => {
  const src = readFileSync(new URL("../src/index.ts", import.meta.url), "utf8");
  // The union runs to its first `| "name";` line (comments inside it may hold semicolons).
  const start = src.indexOf("export type OrbState =");
  const body = src.slice(start, src.indexOf('";\n', start) + 2);
  const inType = [...body.matchAll(/^\s*\|\s*"([a-z]+)"/gm)].map((m) => m[1]).sort();
  const inCatalog = parameterCatalog().objects.flatMap((o) => o.patterns.map((p) => p.id)).sort();
  assert.deepEqual(inType, inCatalog);
});
