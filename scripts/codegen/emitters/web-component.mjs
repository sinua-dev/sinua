// Custom elements for the web (packages/web/src/generated): one tag per engine
// object (`<prefix-orb>`…, the prefix comes from config), registered through the authored base in
// `src/element-typed.ts`. The base wraps the view element, so a typed tag
// inherits its loop, pausing and events; this file only carries the catalog's
// names and the generated mapping.
import { HEADER_LINES, flatProps } from "./common.mjs";

const header = () => HEADER_LINES.map((l) => `// ${l}`).join("\n");
const str = (s) => JSON.stringify(s);
const kebab = (name) => name.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase();

export function emit(models, naming) {
  const P = naming.typePrefix;
  const lower = P.charAt(0).toLowerCase() + P.slice(1);
  const defs = models.map((m) => {
    const params = flatProps(m).map((p) => p.name);
    const groups = m.groups.map((g) => g.name);
    return `  ${str(m.tagName)}: {
    object: ${str(m.object)},
    params: ${JSON.stringify(params)},
    groups: ${JSON.stringify(groups)},
    toOverrides: ${lower}${m.typeName.slice(P.length)}Overrides as (pattern: string, params: Record<string, unknown>) => Record<string, number>,
    specError: ${lower}SpecError,
  }`;
  });
  const tagDocs = models
    .map((m) => ` * - \`<${m.tagName} pattern="${m.patterns[0].id}">\` -- ${m.label}; parameters as attributes (${flatProps(m)
      .slice(0, 3)
      .map((p) => kebab(p.name))
      .join(", ")}…), materials as properties (\`el.glow = { strength: 0.6 }\`).`)
    .join("\n");
  const contents = `${header()}

import { defineTypedElement, type TypedElementDefinition } from "../element-typed.js";
import { ${models.map((m) => `${lower}${m.typeName.slice(P.length)}Overrides`).join(", ")}, ${lower}SpecError } from "./${P}Params.js";

/**
 * One custom element per engine object:
${tagDocs}
 *
 * Scalar parameters and the view's own options are attributes (kebab-cased);
 * objects (\`spec\`, \`inputs\`, \`voice\`, the material groups) are properties.
 */
export const ${P}_ELEMENT_TAGS: Record<string, TypedElementDefinition> = {
${defs.join(",\n")},
};

/** Registers every tag. Idempotent, and a no-op where there is no \`customElements\` (a server). */
export function define${P}Elements(): void {
  for (const [tag, def] of Object.entries(${P}_ELEMENT_TAGS)) defineTypedElement(tag, def);
}

declare global {
  interface HTMLElementTagNameMap {
${models.map((m) => `    ${str(m.tagName)}: HTMLElement;`).join("\n")}
  }
}
`;
  // A side-effect entry that mirrors the authored `element-define.ts`, so
  // `import ".../elements"` registers the tags like `import ".../element"`
  // does. The module above stays pure, so the components entry can re-export
  // from it without dragging a registration into a consumer's bundle.
  const define = `${header()}

import { define${P}Elements } from "./${P}Elements.js";

define${P}Elements();

export * from "./${P}Elements.js";
`;
  return [
    { path: `${P}Elements.ts`, contents },
    { path: "elements-define.ts", contents: define },
  ];
}
