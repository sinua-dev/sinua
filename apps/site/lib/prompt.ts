/**
 * The "Copy prompt" text: everything a coding agent (Claude Code, Cursor, Codex)
 * needs to put one design into a project -- the install line for each platform,
 * the FX Spec file itself, and the code that loads it (the Studio's own exporter,
 * @sinua/snippets, so it is the same code a person would copy). The file carries
 * the design; the prompt only says where it goes.
 */
import { buildSnippets } from "@sinua/snippets";
import type { OrbSize } from "@sinua/core";
import core from "../../../packages/core/package.json";
import { specFile, specText } from "./spec-file";

export const VERSION = core.version;

export function designPrompt(object: string, pattern: string, label: string, size: OrbSize, changed: Record<string, number>): string {
  const name = `${object}-${pattern}.fxspec.json`;
  const file = specText(specFile(object, pattern, size, changed)).trimEnd();
  const load = buildSnippets({ state: pattern, size, overrides: changed, specFile: name })
    .file.map((t) => `${t.label}:\n\`\`\`\n${t.code.trimEnd()}\n\`\`\``)
    .join("\n\n");
  return [
    `Add a Sinua visual to this project: the ${label} ${object} pattern, from the FX Spec file below.`,
    ``,
    `1. Install Sinua ${VERSION} (public beta) for this project's platform:`,
    `   - Web: npm i @sinua/web@beta @sinua/core@beta`,
    `   - iOS (SwiftPM): https://github.com/sinua-dev/sinua-swift, from "${VERSION}", product "Sinua"`,
    `   - Android (Gradle): implementation("dev.sinua:sinua-view:${VERSION}")`,
    `   - React Native: not on npm yet; build it from source (https://sinua.dev/docs/getting-started/installation/)`,
    ``,
    `2. Save this as ${name} (on iOS add it to the app target; on Android put it in app/src/main/assets/):`,
    `\`\`\`json`,
    file,
    `\`\`\``,
    ``,
    `3. Load it in a view, using the block for this project's platform:`,
    ``,
    load,
    ``,
    `4. For a voice agent, give the view its state (idle, listening, thinking, speaking) with \`state\`, or a voice source: https://sinua.dev/docs/connect/voice/`,
    ``,
    `Reference for agents: https://sinua.dev/llms.txt`,
  ].join("\n");
}
