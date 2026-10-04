/**
 * A design as an FX Spec 1.8 file: the pattern and size, plus only the values
 * someone changed, each placed where the catalog says it lives in the file
 * (its `specPath`: `/params/x`, `/materials/glow/strength`, `/color/hue`, ...).
 * The engine owns that mapping, so this can't drift from what the resolver
 * accepts.
 *
 * A voice state is never baked in: in 1.8 the state profile comes from the
 * engine at run time, so the file describes the design and the view is handed
 * the state (`state: "listening"`), live.
 */
import type { OrbSize } from "@sinua/core";
import { parameterCatalog } from "./catalog";

type Json = Record<string, unknown>;

export function specFile(object: string, pattern: string, size: OrbSize, changed: Record<string, number>): Json {
  const catalog = parameterCatalog();
  const refs = catalog.objects.find((o) => o.id === object)?.patterns.find((p) => p.id === pattern)?.params.map((p) => p.ref) ?? [];
  const defs = catalog.definitions as unknown as Record<string, { key: string; specPath?: string | null }>;
  const file: Json = { fxSpec: "1.8", object, pattern, size };
  for (const [key, value] of Object.entries(changed)) {
    const ref = refs.find((r) => defs[r]?.key === key);
    const path = ref ? defs[ref]?.specPath : null;
    if (!path) continue; // not a design value of this pattern (a live input, say)
    const parts = path.split("/").filter(Boolean);
    let node = file;
    for (const part of parts.slice(0, -1)) node = (node[part] ??= {}) as Json;
    node[parts[parts.length - 1]] = value;
  }
  return file;
}

/** The file's text, as a download or the clipboard gets it. */
export const specText = (file: Json) => JSON.stringify(file, null, 2) + "\n";
