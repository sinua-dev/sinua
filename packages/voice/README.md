# @sinua/voice

Web voice sources for Sinua: LiveKit, OpenAI Realtime, Gemini Live, ElevenLabs, the
device microphone and a silent test tone — one entry point each, so an app only pulls the
SDK it uses.

```bash
npm i @sinua/voice           # + livekit-client only if you use LiveKit
```

```ts
import { mount } from "@sinua/web";
import { GeminiLiveVoiceSource } from "@sinua/voice/gemini";

const voice = new GeminiLiveVoiceSource({ credential: ephemeralTokenFromMyBackend });
const fx = mount(canvas, { pattern: "speaking", voice });
await voice.connect();          // from a user gesture
```

Credentials come from your backend and never leave the source. `@sinua/core` is a
types-only peer dependency, so no wasm is pulled in. Apache-2.0.
