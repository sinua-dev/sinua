// The TypeScript params file both TS emitters write (React Native and React):
// the typed parameter interfaces, the pure props -> engine-overrides table and
// `toOverrides`, plus the spec-object guard. Dependency-free on purpose, so
// `node --test` can load it directly and neither package needs the other.
import { HEADER_LINES, sharedGroups, flatProps, docLine } from "./common.mjs";

const header = () => HEADER_LINES.map((l) => `// ${l}`).join("\n");
const str = (s) => JSON.stringify(s);
const jsdoc = (text, indent) => `${indent}/** ${text.replace(/\*\//g, "*&#47;")} */`;

function tsType(prop) {
  if (prop.choices) return prop.choices.map((c) => str(c.spec)).join(" | ");
  switch (prop.type) {
    case "number": case "integer": return "number";
    case "boolean": return "boolean";
    case "number[]": return "number[]";
    case "number|number[]": return "number | number[]";
  }
  throw new Error(`rn: unhandled type ${prop.type}`);
}

/** Mapping table entry for one prop (read by `toOverrides`). */
function entry(prop) {
  const e = { key: prop.key };
  if (prop.choices) e.choices = Object.fromEntries(prop.choices.map((c) => [c.spec, c.value]));
  if (prop.engineKeys) e.keys = prop.engineKeys;
  if (prop.listPatterns) e.listPatterns = prop.listPatterns;
  return e;
}

export function emitParams(models, naming) {
  const P = naming.typePrefix;
  const lower = P.charAt(0).toLowerCase() + P.slice(1);
  const groups = sharedGroups(models);
  const groupTypes = groups.map((g) => {
    const fields = g.props.map((p) => `${jsdoc(docLine(p, models[0]), "  ")}\n  ${p.name}?: ${tsType(p)};`).join("\n");
    return `${jsdoc(`${g.label}${g.description ? `: ${g.description}` : ""} (\`${g.path}.*\`; unset fields keep the pattern's value).`, "")}\nexport interface ${g.typeName} {\n${fields}\n}`;
  });
  const groupTable = Object.fromEntries(groups.map((g) => [g.name, Object.fromEntries(g.props.map((p) => [p.name, entry(p)]))]));
  const perObject = models.map((m) => {
    const flat = flatProps(m);
    const fields = [
      ...flat.map((p) => `${jsdoc(docLine(p, m), "  ")}\n  ${p.name}?: ${tsType(p)};`),
      ...m.groups.map((g) => `  ${g.name}?: ${g.typeName};`),
    ].join("\n");
    const table = Object.fromEntries(flat.map((p) => [p.name, entry(p)]));
    return `export type ${m.typeName}Pattern =\n  | ${m.patterns.map((p) => str(p.id)).join("\n  | ")};

/** ${m.label} parameters; unset ones keep the pattern's values. */
export interface ${m.typeName}Params {
${fields}
}

const ${m.object.toUpperCase()}_TABLE: Table = ${JSON.stringify(table)};

/** The engine overrides a ${m.typeName} hands to SinuaView. */
export function ${lower}${m.typeName.slice(P.length)}Overrides(pattern: ${m.typeName}Pattern, params: ${m.typeName}Params): Record<string, number> {
  return toOverrides(${m.object.toUpperCase()}_TABLE, pattern, params as Record<string, unknown>);
}`;
  });
  const contents = `${header()}
// No imports: node --test loads this file directly (type stripping).

/** The sizes the engine resolves. */
export type ${P}Size = 20 | 32 | 64;

${groupTypes.join("\n\n")}

type Entry = { key: string; choices?: Record<string, number>; keys?: string[]; listPatterns?: string[] };
type Table = Record<string, Entry>;
const GROUPS: Record<string, Table> = ${JSON.stringify(groupTable)};

function put(o: Record<string, number>, e: Entry, v: unknown, pattern: string): void {
  if (v === undefined || v === null) return;
  if (typeof v === "boolean") o[e.key] = v ? 1 : 0;
  else if (typeof v === "string") {
    const n = e.choices?.[v];
    if (n === undefined) throw new Error(\`unknown value "\${v}" for \${e.key}\`);
    o[e.key] = n;
  } else if (Array.isArray(v) || (e.listPatterns && e.listPatterns.includes(pattern))) {
    const xs = Array.isArray(v) ? (v as number[]) : [v as number];
    if (e.keys && (!e.listPatterns || e.listPatterns.includes(pattern))) e.keys.forEach((k, i) => { if (i < xs.length) o[k] = xs[i]; });
    else if (xs.length) o[e.key] = xs[0];
  } else o[e.key] = v as number;
}

function toOverrides(table: Table, pattern: string, params: Record<string, unknown>): Record<string, number> {
  const o: Record<string, number> = {};
  for (const [name, e] of Object.entries(table)) put(o, e, params[name], pattern);
  for (const [name, group] of Object.entries(GROUPS)) {
    const g = params[name] as Record<string, unknown> | undefined;
    if (g) for (const [field, e] of Object.entries(group)) put(o, e, g[field], pattern);
  }
  return o;
}

/** Why a typed component drew nothing: the spec's \`object\` is another one. */
export function ${lower}SpecError(spec: string | object, expected: string): string | null {
  let doc: unknown = spec;
  if (typeof spec === "string") {
    try { doc = JSON.parse(spec); } catch { return null; } // SinuaView reports unreadable specs
  }
  const found = (doc as { object?: unknown } | null)?.object;
  return typeof found === "string" && found !== expected
    ? \`this FX Spec describes a "\${found}" (its \\\`object\\\`), not a "\${expected}": use the matching component, or <SinuaView spec>\`
    : null;
}

${perObject.join("\n\n")}
`;
  return { path: `${P}Params.ts`, contents };
}
