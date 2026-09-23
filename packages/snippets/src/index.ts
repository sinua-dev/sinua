/**
 * The code the Studio and the docs site hand out for a design (docs/fx-view.md):
 * **Code** is the design as SinuaView props on each platform, **File** adds the
 * exported `.fxspec.json` and loads it. Text only -- no engine here, so the
 * caller passes an already-resolved design (the Studio resolves through
 * `resolveFxSpec`). The native Studios port this text exactly
 * (the native Studios' Snippets.swift and Snippets.kt);
 * test/snippets.test.mjs is the byte-for-byte lock they are diffed against.
 */

/** One platform's tab in the export panel. */
export interface SnippetTab {
  id: string;
  label: string;
  code: string;
}

/** Engine sizes a design can be exported at. */
export type OrbSize = 20 | 32 | 64;


export interface SnippetInput {
  /** The state (Signal: the style) -- what the spec-less variant renders. */
  state: string;
  size: OrbSize;
  /** Sparse engine-key overrides only -- the spec-less variant's `overrides`. */
  overrides: Record<string, number>;
  /** Preset speed multiplier (the spec carries its own `speed`). */
  speed?: number;
  /** The Spec menu's file name, e.g. `orb-working.fxspec.json`. */
  specFile: string;
}

/**
 * Snippet numbers: at most 4 decimals ("0.045", not "0.05" or
 * "0.30000000000000004"). The native ports use the same rule, and keys are
 * sorted, so all three Studios print the same text.
 */
export function snipNum(v: number): string {
  return Number(v.toFixed(4)).toString();
}

const sorted = (overrides: Record<string, number>) => Object.keys(overrides).sort().map((k) => [k, snipNum(overrides[k])] as const);

function jsObject(overrides: Record<string, number>): string {
  return `{ ${sorted(overrides)
    .map(([k, v]) => `${k}: ${v}`)
    .join(", ")} }`;
}

/** Kotlin's `Map<String, Double>` needs double literals (`2.0`, not `2`). */
const kotlinDouble = (s: string) => (s.includes(".") ? s : `${s}.0`);

export interface Snippets {
  /** Code mode: the design as props (spec-less SinuaView). Doesn't round-trip lifecycle states, bindings or performance. */
  code: SnippetTab[];
  /** File mode: add the exported .fxspec.json and hand it to SinuaView(spec:). The FX Spec tab follows in the panel. */
  file: SnippetTab[];
}

/**
 * The export panel's two modes (docs/fx-view.md): **Code** is the design as
 * SinuaView props on each platform; **File** adds the Spec menu's .fxspec.json
 * (the canonical format) and loads it. The native Studios port this text
 * exactly (the native Studios' Snippets.swift and Snippets.kt).
 */
export function buildSnippets({ state, size, overrides, speed = 1, specFile }: SnippetInput): Snippets {
  const base = specFile.replace(/\.fxspec\.json$/, "");
  const has = Object.keys(overrides).length > 0;
  const file = `// ${specFile}: the file from the Studio's Spec menu.`;
  const orFile = `// Or use the file: Spec menu → ${specFile} (File tab).`;

  // Props per syntax (size/speed only when not the default).
  const jsProps = [`pattern="${state}"`, size !== 64 ? `size={${size}}` : "", has ? `overrides={${jsObject(overrides)}}` : "", speed !== 1 ? `speed={${snipNum(speed)}}` : ""]
    .filter(Boolean)
    .join(" ");
  const jsOpts = [`pattern: "${state}"`, size !== 64 ? `size: ${size}` : "", has ? `overrides: ${jsObject(overrides)}` : "", speed !== 1 ? `speed: ${snipNum(speed)}` : ""]
    .filter(Boolean)
    .join(", ");
  const swiftArgs = [
    `pattern: "${state}"`,
    size !== 64 ? `size: ${size}` : "",
    has ? `overrides: [${sorted(overrides).map(([k, v]) => `"${k}": ${v}`).join(", ")}]` : "",
    speed !== 1 ? `speed: ${snipNum(speed)}` : "",
  ]
    .filter(Boolean)
    .join(", ");
  const kotlinArgs = [
    `pattern = "${state}"`,
    size !== 64 ? `size = ${size}u` : "",
    has ? `overrides = mapOf(${sorted(overrides).map(([k, v]) => `"${k}" to ${kotlinDouble(v)}`).join(", ")})` : "",
    speed !== 1 ? `speed = ${kotlinDouble(snipNum(speed))}` : "",
  ]
    .filter(Boolean)
    .join(", ");
  const composeImports =
    `import androidx.compose.foundation.layout.size\n` +
    `import androidx.compose.runtime.Composable\n` +
    `import androidx.compose.ui.Modifier\n` +
    `import androidx.compose.ui.unit.dp\n` +
    `import dev.sinua.view.SinuaView\n`;

  const code: SnippetTab[] = [
    {
      id: "react",
      label: "React",
      code:
        `import { SinuaView } from "@sinua/web/react";\n\n` +
        `export function Visual() {\n` +
        `  return <SinuaView ${jsProps} style={{ width: 160, height: 160 }} />;\n` +
        `}\n\n${orFile}`,
    },
    {
      id: "web",
      label: "Web",
      code:
        `import { mount } from "@sinua/web";\n\n` +
        `const fx = mount(document.querySelector("canvas")!, { ${jsOpts} });\n` +
        `// fx.update({ ... }) changes it later; fx.destroy() when done.\n\n${orFile}`,
    },
    {
      id: "swiftui",
      label: "SwiftUI",
      code:
        `import SwiftUI\n` +
        `import Sinua\n\n` +
        `struct Visual: View {\n` +
        `    var body: some View {\n` +
        `        SinuaView(${swiftArgs}).frame(width: 160, height: 160)\n` +
        `    }\n` +
        `}\n\n${orFile}`,
    },
    {
      id: "compose",
      label: "Compose",
      code:
        composeImports +
        `\n@Composable\n` +
        `fun Visual() {\n` +
        `    SinuaView(${kotlinArgs}, modifier = Modifier.size(160.dp))\n` +
        `}\n\n${orFile}`,
    },
    {
      id: "rn",
      label: "React Native",
      code:
        `import { SinuaView } from "@sinua/react-native";\n\n` +
        `export function Visual() {\n` +
        `  return <SinuaView ${jsProps} style={{ width: 160, height: 160 }} />;\n` +
        `}\n\n${orFile}`,
    },
  ];

  const fileTabs: SnippetTab[] = [
    {
      id: "react",
      label: "React",
      code:
        `import { SinuaView } from "@sinua/web/react";\n` +
        `${file}\n` +
        `import spec from "./${specFile}";\n\n` +
        `export function Visual() {\n` +
        `  return <SinuaView spec={spec} style={{ width: 160, height: 160 }} />;\n` +
        `}`,
    },
    {
      id: "web",
      label: "Web",
      code:
        `import { mount } from "@sinua/web";\n` +
        `${file}\n` +
        `import spec from "./${specFile}";\n\n` +
        `const fx = mount(document.querySelector("canvas")!, { spec });\n` +
        `// fx.update({ ... }) changes it later; fx.destroy() when done.`,
    },
    {
      id: "swiftui",
      label: "SwiftUI",
      code:
        `import SwiftUI\n` +
        `import Sinua\n\n` +
        `${file}\n` +
        `// Add it to your app target (Copy Bundle Resources).\n` +
        `let spec = try! String(contentsOf: Bundle.main.url(forResource: "${base}", withExtension: "fxspec.json")!, encoding: .utf8)\n\n` +
        `struct Visual: View {\n` +
        `    var body: some View {\n` +
        `        SinuaView(spec: spec).frame(width: 160, height: 160)\n` +
        `    }\n` +
        `}`,
    },
    {
      id: "compose",
      label: "Compose",
      code:
        composeImports.replace("import androidx.compose.runtime.Composable\n", "import androidx.compose.runtime.Composable\nimport androidx.compose.runtime.remember\n").replace(
          "import androidx.compose.ui.Modifier\n",
          "import androidx.compose.ui.Modifier\nimport androidx.compose.ui.platform.LocalContext\n"
        ) +
        `\n${file}\n` +
        `// Put it in app/src/main/assets/.\n` +
        `@Composable\n` +
        `fun Visual() {\n` +
        `    val context = LocalContext.current\n` +
        `    val spec = remember { context.assets.open("${specFile}").bufferedReader().use { it.readText() } }\n` +
        `    SinuaView(spec = spec, modifier = Modifier.size(160.dp))\n` +
        `}`,
    },
    {
      id: "rn",
      label: "React Native",
      code:
        `import { SinuaView } from "@sinua/react-native";\n` +
        `${file}\n` +
        `import spec from "./${specFile}";\n\n` +
        `export function Visual() {\n` +
        `  return <SinuaView spec={spec} style={{ width: 160, height: 160 }} />;\n` +
        `}`,
    },
  ];

  return { code, file: fileTabs };
}

/** File mode opens by default when the design carries what props code can't: lifecycle states, bindings, a performance block. */
export function hasLifecycle(doc: { states?: unknown; bindings?: unknown; performance?: unknown }): boolean {
  const nonEmpty = (v: unknown) => v != null && (typeof v !== "object" || Object.keys(v).length > 0);
  return nonEmpty(doc.states) || nonEmpty(doc.bindings) || nonEmpty(doc.performance);
}

export const LIFECYCLE_NOTE = "This design has lifecycle states, bindings or a performance block. The code below drops them: use the File tab.";

// ---- Typed components --------------------------------------------------
// The generated per-object components (SinuaOrb, SinuaRing, …) take named
// props instead of engine keys. Turning a design into those props needs the
// parameter catalog, which lives in the engine package -- so the caller passes
// it in and this file stays dependency-free.

/** The parts of `parameterCatalog()` a typed snippet needs. */
export interface CatalogLike {
  objects: { id: string; component: string; patterns: { id: string; params?: { ref: string }[] }[] }[];
  definitions: Record<string, { key: string; path: string; scope: string; type?: string; choices?: { spec: string; value: number }[] | null }>;
}

type PropValue = number | string | boolean | (number | string | boolean)[];
export interface TypedDesign {
  /** e.g. `SinuaOrb`. */
  component: string;
  pattern: string;
  /** Nested props: `{ ringCount: 3, glow: { strength: 0.6 } }`. */
  props: Record<string, PropValue | Record<string, PropValue>>;
  /** Engine keys with no catalog path: they keep the view form. */
  leftover: Record<string, number>;
}

/**
 * Engine overrides -> the typed component's props, using the catalog's
 * `key` -> `path` mapping (`glowStrength` -> `glow.strength`). Indexed keys
 * (`progress0`, `segment3`) collapse into one list prop. Anything the catalog
 * doesn't describe is reported in `leftover`, so a caller can fall back.
 */
export function toTypedProps(object: string, pattern: string, overrides: Record<string, number>, catalog: CatalogLike): TypedDesign {
  const model = catalog.objects.find((o) => o.id === object);
  const patternRefs = new Set((model?.patterns.find((p) => p.id === pattern)?.params ?? []).map((p) => p.ref));
  const byKey = new Map<string, { path: string; def: CatalogLike["definitions"][string] }>();
  for (const [ref, def] of Object.entries(catalog.definitions)) {
    if (def.scope !== "shared" && !patternRefs.has(ref)) continue;
    if (!byKey.has(def.key)) byKey.set(def.key, { path: def.path, def });
  }
  const props: TypedDesign["props"] = {};
  const lists: Record<string, (number | string | boolean)[]> = {};
  const leftover: Record<string, number> = {};

  const set = (path: string, value: PropValue) => {
    const [head, tail] = path.split(".");
    if (tail === undefined) props[head] = value;
    else {
      const group = (props[head] as Record<string, PropValue>) ?? {};
      group[tail] = value;
      props[head] = group;
    }
  };
  const asValue = (def: CatalogLike["definitions"][string], n: number): PropValue => {
    const choice = def.choices?.find((c) => c.value === n);
    if (choice) return choice.spec;
    return def.type === "boolean" ? n >= 0.5 : n;
  };

  for (const key of Object.keys(overrides).sort()) {
    const n = overrides[key];
    const direct = byKey.get(key);
    if (direct) {
      set(direct.path, asValue(direct.def, n));
      continue;
    }
    // `progress0` / `segment12`: one list prop on the base key.
    const m = /^([a-zA-Z]+?)(\d+)$/.exec(key);
    const base = m ? byKey.get(m[1]) : undefined;
    if (m && base) {
      const item = asValue(base.def, n);
      (lists[base.path] ??= [])[Number(m[2])] = Array.isArray(item) ? item[0] : item;
      continue;
    }
    leftover[key] = n;
  }
  for (const [path, values] of Object.entries(lists)) set(path, [...values].map((v) => v ?? 0));
  return { component: model?.component ?? object, pattern, props, leftover };
}

const jsValue = (v: unknown): string => (typeof v === "string" ? JSON.stringify(v) : Array.isArray(v) ? `[${v.map(jsValue).join(", ")}]` : typeof v === "number" ? snipNum(v) : String(v));

/** `pattern="working" ringCount={3} glow={{ strength: 0.6 }}` for JSX. */
function jsxProps(design: TypedDesign, size: OrbSize, speed: number): string {
  const parts = [`pattern=${JSON.stringify(design.pattern)}`];
  for (const [name, value] of Object.entries(design.props)) {
    parts.push(
      value !== null && typeof value === "object" && !Array.isArray(value)
        ? `${name}={{ ${Object.entries(value).map(([k, v]) => `${k}: ${jsValue(v)}`).join(", ")} }}`
        : `${name}={${jsValue(value)}}`
    );
  }
  if (size !== 64) parts.push(`size={${size}}`);
  if (speed !== 1) parts.push(`speed={${snipNum(speed)}}`);
  return parts.join(" ");
}

/**
 * The same design as the generated typed component, for the docs and the
 * gallery: React and React Native. `leftover` keys (engine keys the catalog
 * has no prop for) mean the caller should keep the view form instead.
 */
export function buildTypedSnippets({ design, size, speed = 1 }: { design: TypedDesign; size: OrbSize; speed?: number }): SnippetTab[] {
  const props = jsxProps(design, size, speed);
  return [
    {
      id: "react",
      label: "React",
      code:
        `import { ${design.component} } from "@sinua/web/components";\n\n` +
        `export function Visual() {\n` +
        `  return <${design.component} ${props} style={{ width: 160, height: 160 }} />;\n` +
        `}`,
    },
    {
      id: "rn",
      label: "React Native",
      code:
        `import { ${design.component} } from "@sinua/react-native";\n\n` +
        `export function Visual() {\n` +
        `  return <${design.component} ${props} style={{ width: 160, height: 160 }} />;\n` +
        `}`,
    },
  ];
}
