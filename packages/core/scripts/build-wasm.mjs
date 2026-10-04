#!/usr/bin/env node
// Builds the engine's two wasm modules (design note 33) and inlines each:
//   pkg/      the default entry (@sinua/core): runtime exports only;
//   pkg-dev/  @sinua/core/dev: the same plus the Studio / dev tool exports
//             (cargo feature `dev`).
// Both keep the workspace's O3: `opt-level = "s"` was 55 KB smaller but drew frames
// 3-24 % slower, and the user chose speed (design note 33).
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const crate = join(here, "../../../crates/core_engine");
for (const [dir, cargo] of [["pkg", []], ["pkg-dev", ["--features", "dev"]]]) {
  const out = join(here, "..", dir);
  execFileSync(
    "wasm-pack",
    ["build", "--target", "web", "--out-dir", out, "--out-name", "sinua_core", "--", ...cargo],
    { cwd: crate, stdio: "inherit" }
  );
  execFileSync(process.execPath, [join(here, "inline-wasm.mjs"), out], { stdio: "inherit" });
}
