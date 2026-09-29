# Voice credential endpoints

Your app's voice source asks **your** server for a short-lived credential; your
server mints it with the vendor key it keeps secret. These are minimal endpoints
for that, one per platform, all answering the same JSON:

```json
{ "credential": "ek_…", "expiresAt": 1790000000 }
{ "credential": "<room jwt>", "url": "wss://…", "expiresAt": 1790000000 }
```

| Platform | File | Route |
|---|---|---|
| Next.js (App Router) | `nextjs/app/api/voice/[vendor]/route.ts` | `POST /api/voice/<vendor>` |
| Express | `express/server.ts` | `POST /api/voice/<vendor>` |
| Cloudflare Worker | `cloudflare-worker/src/worker.ts` | `POST /api/voice/<vendor>` |
| Supabase Edge Function | `supabase/functions/voice/index.ts` | `POST /functions/v1/voice/<vendor>` |

Each one uses `shared/credentials.ts` (copy it too). The model, voice and
instructions are set there, on the server. `<vendor>` is `openai`, `gemini`,
`elevenlabs` or `livekit`. Environment: `OPENAI_API_KEY`, `GEMINI_API_KEY`,
`ELEVENLABS_API_KEY` + `ELEVENLABS_AGENT_ID`, `LIVEKIT_API_KEY` +
`LIVEKIT_API_SECRET` + `LIVEKIT_URL`, and optionally `VOICE_INSTRUCTIONS`.

**Wire in your auth first.** Every template refuses all requests until
`currentUserId` returns a signed-in user; otherwise anyone could spend your
quota. For a quick local try, `VOICE_ALLOW_ANONYMOUS=1` opens it. Never set that
in production. Or skip the server entirely while you experiment:
`npx @sinua/voice dev-proxy`.

Client side, the same on every platform:

```ts
new OpenAIRealtimeVoiceSource({ credentialUrl: "/api/voice/openai" });
```

`npm run check` type-checks every template and runs `test/` (vendor HTTP mocked).
