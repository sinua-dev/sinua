// Files the site serves as they are, copied in before `dev` and `build`; none is checked in:
// - public/spec: the snippets' spec files, which <Demo spec="spec/x.fxspec.json"> fetches;
// - public/schema: the JSON Schemas at the URLs their `$id`s and every example's `$schema`
//   name (https://sinua.dev/schema/<name>.json), so editors can complete and check a file;
// - public/examples: example files a page offers for download or draws with <Demo>;
// - lib/generated/parameters.json: the engine's parameter catalog (spec/parameters.json, the
//   same data as `parameterCatalog()` from @sinua/core/dev, plus descriptions), so the landing
//   and the gallery read it as data instead of loading the dev entry's wasm.
import { cpSync, mkdirSync, rmSync } from "node:fs";

const site = (p) => new URL(`../${p}`, import.meta.url);
const repo = (p) => new URL(`../../../${p}`, import.meta.url);

function fresh(dir) {
  rmSync(site(dir), { recursive: true, force: true });
  mkdirSync(site(dir), { recursive: true });
}

fresh("public/spec/");
cpSync(site("snippets/spec/"), site("public/spec/"), { recursive: true });

fresh("public/schema/");
cpSync(repo("spec/fx-spec-1.schema.json"), site("public/schema/fx-spec-1.json"));
cpSync(repo("spec/character-recipe-1.schema.json"), site("public/schema/character-recipe-1.json"));

fresh("public/examples/");
cpSync(repo("spec/examples/remix-latte.fxspec.json"), site("public/examples/remix-latte.fxspec.json"));

fresh("lib/generated/");
cpSync(repo("spec/parameters.json"), site("lib/generated/parameters.json"));
