// Module resolution for the node tests. Two jobs:
//
//  1. "react-native" -> the stub next door, so src/voice.ts can be imported
//     in node at all.
//  2. Extensionless relative imports -> the .ts/.tsx file they mean. The
//     generated sources (src/generated/*) and src/index.ts import each other
//     the way TypeScript and Metro resolve -- `./SinuaParams`, `../SinuaView`
//     -- which node's ESM resolver does not follow. Without this, nothing
//     under src/generated/ can be imported from a test at all.
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, join } from "node:path";
import { existsSync } from "node:fs";

const STUB = pathToFileURL(join(dirname(fileURLToPath(import.meta.url)), "react-native-stub.mjs")).href;

/** The extensions TypeScript would try, in its order. */
const EXTENSIONS = [".ts", ".tsx", ".mjs", ".js"];

export function resolve(specifier, context, next) {
  if (specifier === "react-native") return { url: STUB, shortCircuit: true };
  if (specifier.startsWith(".") && context.parentURL) {
    const base = new URL(specifier, context.parentURL);
    if (!existsSync(fileURLToPath(base))) {
      for (const ext of EXTENSIONS) {
        const candidate = new URL(specifier + ext, context.parentURL);
        if (existsSync(fileURLToPath(candidate))) return { url: candidate.href, shortCircuit: true };
      }
    }
  }
  return next(specifier, context);
}
