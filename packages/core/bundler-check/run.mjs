// The packed packages, bundled like an app would (production mode, tree-shaking on)
// by esbuild, Vite (Rollup) and webpack; each bundle is then run. Two things here are
// only ever caught by running a *production* bundle:
//   engine    @sinua/core's wasm is inlined and initialised by a top-level initSync().
//             If a bundler decides that module is side-effect-free (wasm-pack's
//             pkg/package.json once said so), the init is dropped and the first engine
//             call throws.
//   element   `import "@sinua/web/element"` exists only to call customElements.define().
//             A package-wide `sideEffects: false` (2026-09-20) let bundlers drop that
//             import whole, and the tag was then never registered -- silently, with no
//             error anywhere. "/elements" registers the typed tags the same way, through
//             its own generated define entry, and is listed in `sideEffects` for the same
//             reason; the pure `SinuaElements.js` it wraps stays tree-shakeable, and
//             calling defineSinuaElements() by hand keeps working (both are checked).
// See docs/platforms/web.md. The bundles target the browser; they run here in node,
// which has the globals the engine init uses (atob, WebAssembly); the element scenarios
// get a minimal customElements/HTMLElement stub. `npm test` from
// packages/core/bundler-check (after `npm run build` in packages/core and packages/web).
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, readdirSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import * as esbuild from "esbuild";
import { build as viteBuild } from "vite";
import webpack from "webpack";

const CORE = join(dirname(fileURLToPath(import.meta.url)), "..");
const WEB = join(CORE, "../web");

// Each scenario is installed twice: from the packed tarball (what npm users get) and as
// a symlink to the working tree (what file: consumers in this repo get -- the Studio,
// the bench; pkg/ holds extra wasm-pack files there, e.g. the old pkg/package.json).
const scenarios = {
  engine: {
    packages: { core: CORE },
    entry: `import { frame } from "@sinua/core";\nexport const dots = frame("glowing", 64, 1).dots.length;\n`,
    async check(mod) {
      if (mod.dots !== 260) throw new Error(`expected 260 dots, got ${mod.dots}`);
    },
    what: "the bundle initialises the engine",
  },
  element: {
    packages: { core: CORE, web: WEB },
    entry: `import "@sinua/web/element";\n`,
    tags: ["sinua-view"],
    what: "the bundle registers <sinua-view>",
  },
  elements: {
    packages: { core: CORE, web: WEB },
    entry: `import "@sinua/web/elements";\n`,
    tags: ["sinua-orb", "sinua-signal", "sinua-ring", "sinua-core", "sinua-beacon"],
    what: "the bundle registers the typed tags",
  },
  "elements-by-call": {
    packages: { core: CORE, web: WEB },
    entry: `import { defineSinuaElements } from "@sinua/web/elements";\ndefineSinuaElements();\n`,
    tags: ["sinua-orb", "sinua-beacon"],
    what: "the explicit defineSinuaElements() call still registers them",
  },
};

function consumer(scenario, kind) {
  const work = mkdtempSync(join(tmpdir(), `sinua-bundlers-${kind}-`));
  for (const [name, src] of Object.entries(scenario.packages)) {
    const pkgDir = join(work, `node_modules/@sinua/${name}`);
    mkdirSync(dirname(pkgDir), { recursive: true });
    if (kind === "tarball") {
      const tgz = execFileSync("npm", ["pack", "--ignore-scripts", "--silent", "--pack-destination", work], { cwd: src, encoding: "utf8" }).trim().split("\n").pop();
      mkdirSync(pkgDir);
      execFileSync("tar", ["xzf", join(work, tgz), "-C", pkgDir, "--strip-components=1"]);
    } else {
      symlinkSync(src, pkgDir, "dir");
    }
  }
  writeFileSync(join(work, "entry.js"), scenario.entry);
  return work;
}

const bundlers = {
  async esbuild(work, out) {
    await esbuild.build({ entryPoints: [join(work, "entry.js")], bundle: true, minify: true, format: "esm", platform: "browser", preserveSymlinks: false, outfile: join(out, "esbuild.mjs"), logLevel: "error" });
    return join(out, "esbuild.mjs");
  },
  async vite(work, out) {
    const dir = join(out, "vite");
    await viteBuild({ root: work, logLevel: "error", configFile: false, build: { outDir: dir, lib: { entry: join(work, "entry.js"), formats: ["es"], fileName: "vite" }, minify: true } });
    return join(dir, readdirSync(dir).find((f) => /\.m?js$/.test(f)));
  },
  async webpack(work, out) {
    const dir = join(out, "webpack");
    const stats = await new Promise((resolve, reject) =>
      webpack(
        {
          mode: "production",
          context: work,
          entry: join(work, "entry.js"),
          target: "web",
          experiments: { outputModule: true },
          output: { path: dir, filename: "webpack.mjs", module: true, library: { type: "module" } },
        },
        (err, s) => (err ? reject(err) : resolve(s))
      )
    );
    if (stats.hasErrors()) throw new Error(stats.toString("errors-only"));
    return join(dir, "webpack.mjs");
  },
};

// Enough of a custom-elements environment for a registration to run in node.
function stubDom() {
  const defined = [];
  globalThis.HTMLElement = class {};
  globalThis.customElements = {
    define: (tag) => defined.push(tag),
    get: (tag) => (defined.includes(tag) ? globalThis.HTMLElement : undefined),
  };
  return defined;
}

let failed = 0;
for (const [scenarioName, scenario] of Object.entries(scenarios)) {
  for (const kind of ["tarball", "working-tree"]) {
    const work = consumer(scenario, kind);
    for (const [name, bundle] of Object.entries(bundlers)) {
      try {
        const file = await bundle(work, join(work, "dist"));
        const defined = scenario.tags ? stubDom() : null;
        const mod = await import(pathToFileURL(file).href);
        if (scenario.check) await scenario.check(mod);
        for (const tag of scenario.tags ?? []) {
          if (!defined.includes(tag)) throw new Error(`<${tag}> was never defined (tree-shaken away?); defined: ${defined.join(", ") || "nothing"}`);
        }
        console.log(`ok   ${scenarioName} / ${kind} / ${name}: ${scenario.what}`);
      } catch (err) {
        failed++;
        console.error(`FAIL ${scenarioName} / ${kind} / ${name}: ${err.message.split("\n")[0]}`);
      }
    }
  }
}
process.exit(failed ? 1 : 0);
