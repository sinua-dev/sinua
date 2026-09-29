#!/usr/bin/env node
// @sinua/voice's command line. One command for now:
//   npx @sinua/voice dev-proxy [--port 8787] [--allow-origin https://studio.example]
import { configuredVendors, DEFAULT_PORT, loadEnv, startDevProxy, VENDORS } from "./dev-proxy.mjs";

const USAGE = `usage: sinua-voice dev-proxy [--port <n>] [--allow-origin <origin>]...

Serves short-lived voice credentials on http://127.0.0.1:<port>/<vendor>
from the keys in ./.env and ./.env.local, for local development:
  POST /openai      OPENAI_API_KEY
  POST /gemini      GEMINI_API_KEY
  POST /elevenlabs  ELEVENLABS_API_KEY, ELEVENLABS_AGENT_ID
  POST /livekit     LIVEKIT_API_KEY, LIVEKIT_API_SECRET, LIVEKIT_URL (LIVEKIT_ROOM)
Optional: SINUA_MODEL_OPENAI, SINUA_VOICE_OPENAI, SINUA_MODEL_GEMINI,
SINUA_VOICE_GEMINI, SINUA_INSTRUCTIONS.
Then: new OpenAIRealtimeVoiceSource({ credentialUrl: "http://127.0.0.1:<port>/openai" })`;

const [command, ...args] = process.argv.slice(2);
if (command !== "dev-proxy") {
  console.log(USAGE);
  process.exit(command === undefined || command === "--help" || command === "-h" ? 0 : 2);
}

let port = DEFAULT_PORT;
const allowOrigins = [];
for (let i = 0; i < args.length; i++) {
  const a = args[i];
  if (a === "--port") port = Number(args[++i]);
  else if (a === "--allow-origin") allowOrigins.push(args[++i]);
  else if (a === "--help" || a === "-h") {
    console.log(USAGE);
    process.exit(0);
  } else {
    console.error(`unknown option ${a}\n\n${USAGE}`);
    process.exit(2);
  }
}
if (!Number.isInteger(port) || port < 0 || port > 65535) {
  console.error(`bad --port ${port}`);
  process.exit(2);
}

const env = loadEnv(process.cwd());
const ready = configuredVendors(env);
const server = await startDevProxy({ env, port, allowOrigins, log: (line) => console.log(`  ${line}`) });
const { port: bound } = server.address();
console.log(`sinua voice dev-proxy on http://127.0.0.1:${bound} (this machine only)`);
for (const v of Object.keys(VENDORS)) {
  console.log(`  ${ready.includes(v) ? "ready  " : "not set"} POST /${v}${ready.includes(v) ? "" : `  (needs ${VENDORS[v].join(", ")})`}`);
}
if (allowOrigins.length) console.log(`  also allowing origins: ${allowOrigins.join(", ")}`);
if (!ready.length) console.log("  No vendor keys found in .env / .env.local.");
