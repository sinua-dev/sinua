// @sinua/react-native/catalog: the copy of Sinua's catalog pack is spec/catalog/catalog-1.json.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { SINUA_CATALOG } from "../src/catalog.ts";

test("the RN pack is spec/catalog/catalog-1.json", () => {
  const file = JSON.parse(readFileSync(new URL("../../../spec/catalog/catalog-1.json", import.meta.url), "utf8"));
  assert.deepEqual(SINUA_CATALOG, file);
});
