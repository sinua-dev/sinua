// The typed component template shared by the two React renderers: the web
// (react.mjs -> packages/web/src/generated) and React Native
// (react-native.mjs -> packages/react-native/src/generated).
//
// Not in common.mjs on purpose. common.mjs is imported by every emitter,
// Swift and Kotlin included, and holds language-agnostic helpers; a JSX
// template shared by exactly two of them belongs in a module named for those
// two.
//
// Everything the two renderers may do differently is a field of `renderer`
// below, and test/react-renderers.test.mjs asserts that the emitted files
// differ in those fields and nothing else. Adding a seventh axis of
// divergence should mean adding a field AND changing that test -- never an
// `if (native)` in the template.
import { HEADER_LINES, flatProps } from "./common.mjs";

const header = () => HEADER_LINES.map((l) => `// ${l}`).join("\n");
const str = (s) => JSON.stringify(s);

/**
 * @typedef {object} Renderer
 * @property {string[]} preamble      Import lines before the view import (React, and RN's `View`).
 * @property {string}   viewModule    Where SinuaView/SinuaViewProps come from, relative to generated/.
 * @property {string}   moduleSuffix  Appended to relative specifiers: ".js" for the web's ESM
 *                                    resolution, "" for Metro's extensionless one.
 * @property {string}   effect        How the template calls useEffect.
 * @property {string}   blankView     What a mismatched spec renders instead: an empty host view
 *                                    carrying the caller's layout.
 * @property {(P: string) => string[]} indexTail  Extra lines at the end of index.ts.
 */

/** @param {Renderer} r */
export function emitComponent(m, naming, r) {
  const P = naming.typePrefix;
  const lower = P.charAt(0).toLowerCase() + P.slice(1);
  const fn = `${lower}${m.typeName.slice(P.length)}Overrides`;
  const x = r.moduleSuffix;
  const contents = `${header()}

${r.preamble.join("\n")}
import { SinuaView, type SinuaViewProps } from "${r.viewModule}";
import { ${fn}, ${lower}SpecError, type ${P}Size, type ${m.typeName}Params, type ${m.typeName}Pattern } from "./${P}Params${x}";

type Options = Omit<SinuaViewProps, "spec" | "pattern" | "state" | "specState" | "size" | "overrides" | "inputs" | "voiceLevelInput" | "crossFade">;

/** ${m.label}, typed: \`<${m.typeName} pattern=${str(m.patterns[0].id)} />\` plus any of its parameters. */
export type ${m.typeName}Props = Options &
  (
    | (${m.typeName}Params & {
        pattern: ${m.typeName}Pattern;
        size?: ${P}Size;
        /** The agent's lifecycle state: the built-in voice-state behaviour moves this pattern. */
        state?: string;
        inputs?: Record<string, number>;
        spec?: undefined;
      })
    | {
        /** An FX Spec (JSON text or object) describing a ${m.object}; anything else draws nothing and calls \`onError\`. */
        spec: string | object;
        /** The spec's lifecycle state. */
        state?: string;
        inputs?: Record<string, number>;
        voiceLevelInput?: string;
        crossFade?: number;
        pattern?: undefined;
      }
  ) & { onError?: (message: string) => void };

/** A thin wrapper over SinuaView (no painting of its own). */
export function ${m.typeName}(props: ${m.typeName}Props) {
  const specError = props.spec != null ? ${lower}SpecError(props.spec, ${str(m.object)}) : null;
  const { onError } = props;
  ${r.effect}(() => {
    if (specError) (onError ?? console.error)(\`${m.typeName}: \${specError}\`);
  }, [specError, onError]);
  if (props.spec != null) {
    const { spec, state, onError: _e, pattern: _p, ...rest } = props;
    if (specError) return ${r.blankView};
    return <SinuaView {...rest} spec={spec} state={state} />;
  }
  const { pattern, size, state, inputs, onError: _e, ...rest } = props;
  const overrides = ${fn}(pattern, rest as ${m.typeName}Params);
  const options: Record<string, unknown> = { ...rest };
  for (const k of PARAM_KEYS) delete options[k];
  return <SinuaView {...(options as Options)} pattern={pattern} size={size} state={state} inputs={inputs} overrides={overrides} />;
}

const PARAM_KEYS = ${JSON.stringify([...flatProps(m).map((p) => p.name), ...m.groups.map((g) => g.name)])};
`;
  return { path: `${m.typeName}.tsx`, contents };
}

/** @param {Renderer} r */
export function emitIndex(models, naming, r) {
  const P = naming.typePrefix;
  const x = r.moduleSuffix;
  const lines = [
    `export * from "./${P}Params${x}";`,
    ...models.map((m) => `export { ${m.typeName}, type ${m.typeName}Props } from "./${m.typeName}${x}";`),
    ...r.indexTail(P),
  ];
  return { path: "index.ts", contents: `${header()}\n\n${lines.join("\n")}\n` };
}
