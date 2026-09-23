// The <Demo> specs the pages reference (`spec="spec/x.fxspec.json"`) have to be
// fetchable at runtime, so the snippets' spec files are copied into public/.
// Run before `dev` and `build`; public/spec is generated, not checked in.
import { cpSync, mkdirSync, rmSync } from "node:fs";

const from = new URL("../snippets/spec/", import.meta.url);
const to = new URL("../public/spec/", import.meta.url);
rmSync(to, { recursive: true, force: true });
mkdirSync(to, { recursive: true });
cpSync(from, to, { recursive: true });
