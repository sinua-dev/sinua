// Catalog (spec/parameters.json, families' contract Rev 2) -> ComponentModel[].
// Pure: no fs, no Date/random, so every emitter's output is deterministic.
// Throws on anything it can't type exactly (e.g. number[] without bounds).

export const pascal = (s) => s.replace(/(^|[-_.\s])(\w)/g, (_, __, c) => c.toUpperCase());
export const camel = (s) => {
  const p = pascal(s);
  return p.charAt(0).toLowerCase() + p.slice(1);
};
const kebab = (path) =>
  path
    .replace(/\./g, "-")
    .replace(/([a-z0-9])([A-Z])/g, "$1-$2")
    .toLowerCase();

export const EVENTS = [
  { id: "frame", react: "onFrame", wc: "fxframe", payload: "FxFrameStats" },
  { id: "error", react: "onError", wc: "fxerror", payload: "FxDiagnostic[] | Error" },
];

/** "progress0..progress3" -> ["progress0", ..., "progress3"] */
function expandEngineKeys(spec) {
  const m = /^([A-Za-z]+)(\d+)\.\.\1(\d+)$/.exec(spec ?? "");
  if (!m) throw new Error(`codegen: can't expand engineKeys "${spec}"`);
  const out = [];
  for (let i = Number(m[2]); i <= Number(m[3]); i++) out.push(`${m[1]}${i}`);
  return out;
}

function mergeType(types, path, obj) {
  const set = [...new Set(types)].sort();
  const key = set.join("|");
  if (set.length === 1) return set[0];
  if (key === "integer|number") return "number";
  if (key === "number|number[]") return "number|number[]";
  throw new Error(`codegen: ${obj}.${path} has incompatible types across patterns: ${key}`);
}

/** Build one merged prop from the definitions a path resolves to in this object. */
function buildProp(path, uses, catalog, sizesByPattern) {
  const defs = uses.map((u) => catalog.definitions[u.ref]);
  const first = defs[0];
  const types = defs.map((d) => d.type);
  const type = mergeType(types, path, uses[0].object);
  const scalars = defs.filter((d) => d.type !== "number[]");
  const arrays = defs.filter((d) => d.type === "number[]");
  const choiceSets = [...new Set(defs.filter((d) => d.choices).map((d) => JSON.stringify(d.choices)))];
  if (choiceSets.length > 1) throw new Error(`codegen: ${uses[0].object}.${path} has different choices per pattern`);
  for (const a of arrays) {
    if (a.minItems == null || a.maxItems == null) throw new Error(`codegen: ${path} (number[]) has no minItems/maxItems`);
  }
  const segments = path.split(".");
  const defaultByPattern = {};
  const ranges = {};
  for (const u of uses) {
    const d = catalog.definitions[u.ref];
    ranges[u.pattern] = { ref: u.ref, type: d.type, min: d.min, max: d.max, minItems: d.minItems ?? null, maxItems: d.maxItems ?? null };
    if (u.default !== undefined) defaultByPattern[u.pattern] = u.default;
    else defaultByPattern[u.pattern] = Object.fromEntries(sizesByPattern[u.pattern].map((s) => [String(s), d.fallback]));
  }
  const num = (xs) => xs.filter((x) => typeof x === "number");
  const mins = num(defs.map((d) => d.min));
  const maxs = num(defs.map((d) => d.max));
  return {
    kind: "value",
    path,
    segments,
    root: segments.length > 1 ? segments[0] : null,
    name: camel(segments[segments.length - 1]),
    key: scalars[0]?.key ?? first.key,
    engineKeys: arrays.length ? expandEngineKeys(arrays[0].items?.engineKeys) : null,
    group: first.group,
    type,
    min: mins.length ? Math.min(...mins) : null,
    max: maxs.length ? Math.max(...maxs) : null,
    minItems: arrays[0]?.minItems ?? null,
    maxItems: arrays[0]?.maxItems ?? null,
    lengthFrom: arrays[0]?.lengthFrom ?? null,
    choices: first.choices
      ? first.choices.map((c) => {
          if (!Number.isInteger(c.value)) throw new Error(`codegen: ${path} choice ${c.spec} has a non-integer value`);
          return { value: c.value, label: c.label, spec: c.spec, caseName: camel(c.spec) };
        })
      : null,
    label: first.label,
    description: first.description,
    unit: first.unit ?? null,
    step: first.step ?? null,
    category: first.category,
    tier: first.tier,
    deprecated: null,
    aliases: [...new Set(defs.flatMap((d) => d.aliases ?? []))].sort(),
    attr: kebab(path),
    attribute: !type.includes("[]"),
    arrayAttrFormat: type.includes("[]") ? "csv" : null,
    patterns: uses.map((u) => u.pattern),
    // For "number|number[]": the patterns that take the list (ring `tracking`); the others take `key`.
    listPatterns: type === "number|number[]" ? uses.filter((u) => catalog.definitions[u.ref].type === "number[]").map((u) => u.pattern) : null,
    ranges,
    default: first.fallback ?? null,
    defaultByPattern,
  };
}

export function buildModels(catalog, naming) {
  if (catalog.catalogVersion !== 1) throw new Error(`codegen: catalogVersion ${catalog.catalogVersion} (expected 1)`);
  const materialRefs = catalog.materials.flatMap((m) => m.params);
  return catalog.objects.map((obj) => {
    const typeName = `${naming.typePrefix}${pascal(obj.id)}`;
    const patterns = obj.patterns.map((p) => ({ id: p.id, label: p.label, caseName: camel(p.id), mode: p.mode, sizes: p.sizes }));
    const sizesByPattern = Object.fromEntries(obj.patterns.map((p) => [p.id, p.sizes]));
    // path -> uses, in catalog order (patterns first, then materials).
    const byPath = new Map();
    const use = (path, u) => {
      if (!byPath.has(path)) byPath.set(path, []);
      byPath.get(path).push(u);
    };
    for (const p of obj.patterns) {
      for (const e of p.params) {
        const d = catalog.definitions[e.ref];
        if (!d) throw new Error(`codegen: unknown ref ${e.ref}`);
        if (d.deprecated) continue; // renamed keys resolve through aliases; no prop
        use(d.path, { ref: e.ref, pattern: p.id, default: e.default, object: obj.id });
      }
    }
    for (const ref of materialRefs) {
      const d = catalog.definitions[ref];
      if (!d || d.deprecated) continue;
      for (const p of obj.patterns) {
        if ((byPath.get(d.path) ?? []).some((u) => u.pattern === p.id)) continue;
        const md = Object.values(p.materialDefaults ?? {}).find((m) => m && d.key in m);
        const def = md ? Object.fromEntries(p.sizes.map((s) => [String(s), md[d.key]])) : undefined;
        use(d.path, { ref, pattern: p.id, default: def, object: obj.id });
      }
    }
    const props = [...byPath.entries()].map(([path, uses]) => buildProp(path, uses, catalog, sizesByPattern));
    // Nested roots (glow, noise, ...): property-only group entries.
    const groups = [];
    for (const pr of props) {
      if (!pr.root) continue;
      let g = groups.find((x) => x.path === pr.root);
      if (!g) {
        const mat = catalog.materials.find((m) => m.id === pr.root);
        g = { kind: "group", path: pr.root, name: camel(pr.root), typeName: `${naming.typePrefix}${pascal(pr.root)}`, label: mat?.label ?? pascal(pr.root), description: mat?.description ?? null, attr: kebab(pr.root), attribute: false, children: [] };
        groups.push(g);
      }
      g.children.push(pr.path);
    }
    // families' catalog test pins that no flat path equals a material group root; throw if one reappears.
    const flatNames = props.filter((p) => !p.root).map((p) => p.name);
    for (const g of groups) if (flatNames.includes(g.name)) throw new Error(`codegen: ${obj.id}: "${g.name}" is both a prop and a group`);
    if (new Set(flatNames).size !== flatNames.length) throw new Error(`codegen: ${obj.id}: duplicate prop names`);
    return {
      object: obj.id,
      typeName,
      tagName: `${naming.tagPrefix}${obj.id}`,
      label: obj.label,
      catalogVersion: catalog.catalogVersion,
      fxSpec: catalog.fxSpec,
      patterns,
      props,
      groups,
      events: EVENTS,
    };
  });
}
