# Voice-reactive audio pipeline

## Why this exists

The product direction (`docs/gpt.md`, `docs/gemini.md`, and the session
that followed them) specifically targets apps built on realtime voice AI
models (OpenAI Realtime, Gemini Live, etc.) wanting a visual identity that
reacts to the actual conversation — not just a looping animation. This
doc covers what's built: real audio capture + analysis in the Web Studio,
the vendor adapters that feed it from a live voice model (OpenAI Realtime,
Gemini Live, ElevenLabs, LiveKit — see *Built vendor adapters*, below),
and the engine-side opts it all feeds. None of the vendor adapters has
been verified against a live session yet; the local mic and the
synthesized test tone prove the pipeline end-to-end.

## The core design decision: don't invent a schema, align with LiveKit's `AgentState`

Verified directly from `livekit/agents` source
(`livekit-agents/livekit/agents/voice/events.py`), not guessed:
```python
AgentState = Literal["initializing", "idle", "listening", "thinking", "speaking"]
UserState = Literal["speaking", "listening", "away"]
```
This is already the vocabulary three-plus production voice-agent stacks
(LiveKit, and anything built on it) share, and LiveKit's own
`BarVisualizer`/`useMultibandTrackVolume` (their actual shipped source was
read for this, not assumed) confirmed the exact pattern this engine now
uses: a lifecycle state driving a highlight/sequencer animation, plus a
real per-band volume array driving reactive height/intensity. The Web Studio's
`audio/types.ts` `AgentState` type uses the identical five values, and
`signal::modes::bar`'s `voiceStateCode` opt (`0..4`) encodes the same
five states as a number (a flat `HashMap<String, f64>` opts map can't carry
a string).

**Why not adopt a specific vendor's raw event schema instead?** OpenAI's
Realtime API events (`response.output_audio.delta`, `input_audio_buffer.
speech_started`, ...) are OpenAI-specific and carry OpenAI's own
idiosyncrasies; Gemini Live's `BidiGenerateContent*` messages are a
different shape entirely (see the transport table below). Converging on
LiveKit's small, already-multi-vendor-proven `AgentState` vocabulary
instead of either vendor's own schema is what keeps a future provider
adapter a translation into five known values, not a redesign.

## Transport diversity — why one adapter per provider isn't one shared function

Real, current (2026) transport shapes, researched specifically because the
user wanted this decision grounded in reality, not assumption:

| Provider | Transport | Audio format | Notes |
|---|---|---|---|
| OpenAI Realtime (GA since Aug 2025) | WebRTC (client) or WebSocket (server) | PCM16, 24kHz both directions | Ephemeral key → SDP POST to `/v1/realtime/calls`; events over an `"oai-events"` data channel separate from the media track |
| Gemini Live | WebSocket only, no WebRTC option | PCM16, **16kHz in / 24kHz out** (asymmetric) | Structured session setup (`BidiGenerateContentSetup` must be the first message). **Re-verified 2026-09-17:** a *connection* lives ~10 min (`goAway.timeLeft` precedes the cut) and is resumable via `sessionResumption` handles (valid 2 h); an audio-only *session* is capped at 15 min unless `contextWindowCompression` is set. Current stable model `gemini-3.8-live`. Ephemeral tokens (`POST /v1beta/auth_tokens`) go on a separate `BidiGenerateContentConstrained` endpoint as `access_token`. **No server-side user-VAD start/stop message exists** — only `interrupted` and input transcription; see *Built vendor adapters* |
| Amazon Nova Sonic | HTTP/2 bidirectional event stream (a third transport shape) | PCM16, 16kHz in | Different enough that a generic "socket" abstraction doesn't cover it either |
| ElevenLabs Conversational AI | WebSocket (`wss://api.elevenlabs.io/v1/convai/conversation?agent_id=…`, subprotocol `convai`) | Negotiated per agent in `conversation_initiation_metadata`: `pcm_8000…48000` or `ulaw_8000` out, PCM in (e.g. `pcm_44100` out / `pcm_16000` in) | **Re-verified 2026-09-18.** Exposes a **`vad_score`** event (probability the user is speaking, 0..1, on by default) — the textbook "prefer a provider's native VAD/state signal over an energy heuristic" case, now used by `ElevenLabsVoiceSource`. `ping`/`pong` keep-alive, `interruption` with an `event_id` watermark, a turn-end signal (`agent_response_complete`, and `audio_event.is_final` on the last chunk; re-checked 2026-09-20), no resumption. Public agents need no secret; private agents use a 15-min signed URL minted server-side. See *Built vendor adapters* |
| LiveKit (infrastructure, not a vendor) | An existing LiveKit Room via `livekit-client` (WebRTC underneath) | Whatever the agent publishes; a normal remote `MediaStreamTrack` | **Added 2026-09-18.** A LiveKit Agents voice agent already publishes its lifecycle as the `lk.agent.state` participant attribute (`initializing/idle/listening/thinking/speaking` — the vocabulary this project adopted from LiveKit) and its voice as a subscribed track; `LiveKitVoiceSource` forwards both and, on its primary path, handles **no credential at all** (the Room is the app's). See *Built vendor adapters* |

WebRTC hands you a live `MediaStreamTrack` (trivial: an `AnalyserNode`
attaches directly). WebSocket/HTTP2 paths hand you raw PCM chunks that must
be decoded and scheduled onto an `AudioContext` timeline manually
(gap-free playback needs a running `currentTime` cursor, and the analyser
must be gated on the *scheduled playback timeline*, not on chunk-receipt
time, or the visual would show "speaking" before the audio is actually
audible). These are genuinely different code paths, not one shared
function — hence a `VoiceSource` interface with one implementation per
transport shape, not per vendor.

```ts
// @sinua/core (re-exported by @sinua/voice)
interface VoiceSource {
  connect(): Promise<void>;
  disconnect(): void;
  onMetrics(cb: (m: { level: number; bands: number[] }) => void): void;
  onStateChange(cb: (s: AgentState) => void): void;
}
```

Built today: `LocalMicVoiceSource` (real `getUserMedia` mic — needs no
vendor API at all), `TestToneVoiceSource` (a synthesized speech-like
burst pattern routed through the identical analysis path, so it exercises
real Web Audio code without needing a mic or a live conversation — see
*Studio wiring*, below), and four vendor adapters: `OpenAIRealtimeVoiceSource`
(OpenAI Realtime), `GeminiLiveVoiceSource` (Gemini Live),
`ElevenLabsVoiceSource` and `LiveKitVoiceSource`. See *Built vendor
adapters*, below. Not built: Azure OpenAI and Amazon Nova Sonic (see
*Roadmap*). Each would be a new class implementing the same interface,
not a redesign.

## One pattern, four voice states

A view given `pattern` + `state` (or a bound source, whose `AgentState` supplies the
state) applies the engine's voice-state profile: per state a speed multiplier, engine
overrides (`ink`, a signed `audioStrength` for the listening inhale, particles, glow) and
the name of the input that drives `audioLevel`. The precedence table is in
[`fx-view.md`](fx-view.md), *Voice states without a spec*.

## React Native: the native sources from JS

`@sinua/react-native`'s `createVoiceSource({ vendor, … })` creates one of the
native sources above in a registry and returns a handle (`connect`, `disconnect`,
`release`, `onStateChange`, `onError`, `onInterrupt`). A view binds it with
`voice={handle}`, exactly like the native `VoiceSource` parameter, so the frame
loop, analysis and state mapping are the native ones. Credentials are passed once
into the native source; `getCredential` (OpenAI) round-trips per reconnect.
The vendor SDKs are opt-in: CocoaPods subspecs on iOS, the `sinua.voiceVendors`
Gradle property on Android. See [`fx-view.md`](fx-view.md), *Voice sources in React Native*.

## The Web package: `@sinua/voice`

Every Web source above ships as `@sinua/voice` (`packages/voice`,
Apache-2.0). There is one subpath per source, so an app only pulls what it
imports. The Studio consumes the same package.

| Import | Class | Needs |
|---|---|---|
| `@sinua/voice/openai` | `OpenAIRealtimeVoiceSource` | nothing (WebRTC + `fetch`) |
| `@sinua/voice/gemini` | `GeminiLiveVoiceSource` | nothing (WebSocket + an AudioWorklet) |
| `@sinua/voice/elevenlabs` | `ElevenLabsVoiceSource` | nothing (WebSocket + an AudioWorklet) |
| `@sinua/voice/livekit` | `LiveKitVoiceSource` | the optional peer `livekit-client` |
| `@sinua/voice/mic` | `LocalMicVoiceSource` | nothing |
| `@sinua/voice/tone` | `TestToneVoiceSource` | nothing (no device, no network) |
| `@sinua/voice` | shared: `AudioAnalysis`, `PcmAudioGraph`, the types | nothing |

```ts
import { mount } from "@sinua/web";
import { GeminiLiveVoiceSource } from "@sinua/voice/gemini";

const voice = new GeminiLiveVoiceSource({ credential: ephemeralToken }); // auth_tokens/… from your backend
const fx = mount(canvas, { spec, voice });  // the view binds the source; you connect it
await voice.connect();                       // from a user gesture (mic permission + autoplay)
```

- **Dependencies.** `@sinua/core` is a peer dependency, used for types
  only: no module of the package imports it at runtime, so no wasm is pulled
  in. `livekit-client` is an *optional* peer, installed only by apps that use
  `/livekit`. Only `/livekit` reaches it; a test walks each subpath's import
  graph to prove this.
- **Tests.** `packages/voice/test` runs every source on node fakes
  (`AudioContext`, `WebSocket`, `RTCPeerConnection`, `getUserMedia`,
  `fetch`), with no device, network or sound. It covers each vendor's
  handshake, the state mapping, barge-in and the setup error paths.
- **History.** The sources lived in the Web Studio until
  2026-09-19; `WebRTCVoiceSource` and `PCMStreamVoiceSource` took their
  native names on the move.

## Built vendor adapters

### `OpenAIRealtimeVoiceSource` — OpenAI Realtime over WebRTC

`packages/voice/src/OpenAIRealtimeVoiceSource.ts`. Before building it, the
OpenAI row of the transport table above was re-verified against the
*current* docs and OpenAI's own shipped code, not recalled — it held up
unchanged: developers.openai.com's `guides/voice-webrtc` and the
`realtime-sessions/create-realtime-client-secret` reference,
`openai/openai-realtime-console` (`App.jsx` for the browser handshake,
`server.js` for the mint), and `openai/openai-agents-js`'s
`openaiRealtimeWebRtc.ts` (the SDK's own transport, which additionally
refuses raw API keys in a browser unless explicitly opted in, and detects
ephemeral keys by their `ek_` prefix — both adopted here).

**Handshake** (all endpoints verbatim from those sources):
1. `POST https://api.openai.com/v1/realtime/client_secrets` with the real
   API key, body `{ session: { type: "realtime", model, audio: {...} },
   expires_after: { anchor: "created_at", seconds: 600 } }` → `{ value:
   "ek_…", expires_at }`.
2. `getUserMedia` → `RTCPeerConnection` → `addTrack(mic)` →
   `createDataChannel("oai-events")` → `createOffer` → `POST
   https://api.openai.com/v1/realtime/calls?model=…` with `Authorization:
   Bearer ek_…`, `Content-Type: application/sdp`, body `offer.sdp` → the
   answer SDP as text → `setRemoteDescription`.
3. The model's voice arrives as a normal remote `MediaStreamTrack`
   (`pc.ontrack`). It is attached to a hidden `<audio autoplay>` (so it's
   audible, and so Chrome actually pumps the track) **and** to the exact
   same `AnalyserNode` → `AudioAnalysis` graph `LocalMicVoiceSource` builds
   — `AudioAnalysis` itself is untouched. So `{level, bands}` here are the
   *model's* voice, which is what Orb's pulse and `signal`'s bars should
   follow. The zero-RMS watchdog is reused, but only counts while a
   response is active: a remote track legitimately reads exact zeros
   during silence, and the documented Chrome flat-zero bug is specifically
   about WebRTC tracks, so this is where the watchdog earns its keep.

**State mapping** — vendor events, not the energy heuristic, drive
`AgentState` (the research above said to prefer a vendor's own VAD/state
signal whenever one exists; OpenAI has them under `server_vad`). Same
mapping LiveKit's `realtime_model.py` and Pipecat's
`openai/realtime/llm.py` use for their user-/bot-speaking frames:

| Server event | `AgentState` |
|---|---|
| data channel open | `listening` |
| `input_audio_buffer.speech_started` | `listening` (user talking — the barge-in moment if we were `speaking`) |
| `input_audio_buffer.speech_stopped`, `response.created` | `thinking` |
| `output_audio_buffer.started` (WebRTC-only) | `speaking` |
| `output_audio_buffer.stopped` / `.cleared` (WebRTC-only) | `listening` |
| `response.done` while still `thinking` (no audio produced) | `listening` |
| data channel closed, peer connection `failed`/`closed` | `initializing` while a replacement session is set up (see *Reconnect*, below), then `listening`; `idle` if it gives up |
| `error` with a fatal code (`invalid_api_key`, `insufficient_quota`, …) | logged; the next drop is not retried |

The `output_audio_buffer.started/stopped` pair is the WebRTC transport's
own "audio is actually audible / fully drained" signal — exactly the "gate
on playback, not chunk receipt" rule this doc set for PCM adapters, given
for free here. It is documented inconsistently (OpenAI added it to the
reference in May 2025 per their community forum, Azure's reference lists
it, one current reference page only shows `output_audio_buffer.clear`/
`cleared`), so the adapter treats it as optional: until one has been seen
in a session, `speaking` is entered when the remote track's level rises
during an active response and left ~300 ms after it falls following
`response.done`. Once any `output_audio_buffer.*` event arrives, the
energy fallback stands down for the rest of the session.

**Credentials — the security note, taken seriously.** The adapter takes
one `credential` string:
- `ek_…` → used directly for the SDP POST. This is the production shape:
  *your backend* mints it via `client_secrets`, the browser never holds a
  long-lived key, and the `ek_` expires (600 s default) and is bound to
  one session. A shipped product uses only this path.
- anything else → treated as a raw API key, and the adapter mints the
  `ek_` **from the browser**. This is dev-only demo wiring so the local
  Studio can talk to a real model without a backend (this project has
  none). The key lives only in the
  adapter instance and the Studio's React state: never persisted, never in
  a URL, never logged. If the browser-side mint fails (CORS, 401), the
  error tells you to mint server-side and paste the `ek_` instead, with
  the curl one-liner. Nothing in this repo contains or stores a real key.

Session config (`server_vad` turn detection so the documented
`speech_started/stopped` events are guaranteed, the output voice, optional
instructions) is passed at mint time, not via a later `session.update` —
so a backend-minted `ek_`, whose config that backend owns, takes the exact
same connect path with nothing overridden.

#### Reconnect

What OpenAI Realtime allows (researched 2026-09-18). **There is no session
resumption.**
- The WebRTC guide documents no resume, rejoin or ICE-restart path.
- The conversations guide caps a session at **60 minutes**.
- OpenAI's community threads (`community.openai.com/t/reconnect-to-lost-session/1045641`,
  `openai/openai-realtime-api-beta#83`) land on the same answer: a dropped
  call can only be *replaced*. You open a new session and re-submit prior
  turns with `conversation.item.create`.

That is exactly what LiveKit's shipped client does
(`livekit-agents/…/llm/_realtime/openai.py`, `_main_task`/`_reconnect`):
- `max_retry` 3, the first retry after 0.1 s;
- on success it re-sends the session config plus the chat context, with
  user turns as `input_text` and assistant turns as `output_text` items;
- a fatal-code list stops the loop. Its comment explains why: retrying
  those loops forever.

`OpenAIRealtimeVoiceSource` follows that design (`realtimeReconnect.ts` holds the
DOM-free rules):

- **Trigger:** peer connection `failed`/`closed` or the data channel
  closing, while the caller still wants to be connected. A transient
  `disconnected` doesn't count, and a user `disconnect()` never reconnects.
- **State, honestly:** `initializing` during the attempt, then `listening`
  on success. After the last failed attempt it goes `idle`, which is the
  cue for an app (and the Studio panel) to reset. One zeroed metrics
  reading goes out at the drop, so visuals settle instead of freezing on
  the last frame.
- **Policy:** `reconnect: { maxAttempts = 3, baseDelayMs = 1000, maxDelayMs
  = 8000 }`. The first try comes after 100 ms, then exponential backoff
  with ±20 % jitter. `reconnect: false` restores drop-to-`idle`.
- **Fatal, no retry:** HTTP 400/401/403 from `/calls` or `client_secrets`,
  or a drop that follows a fatal `error.code` (LiveKit's four:
  `insufficient_quota`, `invalid_api_key`, `account_deactivated`,
  `billing_hard_limit_reached`). These give up at once. 408/425/429/5xx
  and network errors are retried.
- **Credentials, never stale:** each attempt gets a fresh one.
  - `getCredential: () => Promise<string>` is called for every connect and
    reconnect. That's the production shape: your backend mints a new `ek_`.
  - Without it, a raw dev key re-mints each time.
  - A pasted `ek_` alone is single-session and expiring, so it is **not**
    reused; that case still ends in `idle`.
- **Context:** finalized turns are replayed into the new session as text
  items, newest first up to 8000 chars / 40 items. Assistant turns come
  from `response.output_audio_transcript.done`. User turns come from
  `conversation.item.input_audio_transcription.completed`, which only
  arrives if the session has input transcription on. Without it the model
  gets its own side of the conversation back, not the user's. Turn it on
  at mint time if context continuity matters.
- **Not copied from LiveKit:** its proactive recycle every 20 min. A
  planned mid-conversation cut is audible, and the 60-minute server cut
  is already handled as an ordinary drop.

Verified under node against fakes of `RTCPeerConnection`/`fetch`/
`getUserMedia` driving the real class:
- a drop reconnects with a fresh credential and replays the transcript in order;
- 5xx errors give up after the configured attempts; a 401 or a fatal code gives up at once;
- a pasted `ek_` and `reconnect: false` go `idle`;
- a `disconnect()` during backoff stops the loop.

Not yet exercised against a real dropped call.

### `GeminiLiveVoiceSource` — Gemini Live over WebSocket

`packages/voice/src/GeminiLiveVoiceSource.ts`, with the DOM-free PCM
helpers in `pcm.ts`. The second transport *shape*: no track is handed to
us, so the adapter captures the mic itself and ships PCM16 @ 16 kHz, and
decodes/schedules the model's PCM16 chunks itself. Built against the
current reference (`ai.google.dev/api/live`, camelCase JSON) and Google's
own browser sample (`google-gemini/live-api-web-console`: `audio-
recorder.ts` + its worklet for capture, `audio-streamer.ts` for gap-free
scheduling), with LiveKit's `realtime_api.py` and Pipecat's
`gemini_live/llm.py` as the adapter-shape references. No `@google/genai`
dependency — the wire schema is documented and small, and the Studio has
no runtime dependency beyond React.

**Wire messages** (verbatim shapes): `wss://generativelanguage.googleapis.
com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerate
Content?key=…` (or `…BidiGenerateContentConstrained?access_token=…` for a
token); first message `{"setup": {"model": "models/gemini-3.8-live",
"generationConfig": {"responseModalities": ["AUDIO"]},
"inputAudioTranscription": {}, "sessionResumption": {…},
"contextWindowCompression": {"slidingWindow": {}}}}` → `{"setupComplete":
{}}`; mic `{"realtimeInput": {"audio": {"data": <base64>, "mimeType":
"audio/pcm;rate=16000"}}}` in 1024-sample (64 ms) chunks from an
`AudioWorklet` on a 16 kHz `AudioContext` (the context resamples the mic
for us); model audio in `serverContent.modelTurn.parts[].inlineData`.
One discrepancy found and handled: the reference and the Live guide say
24 kHz output, but the official raw-WebSocket getting-started sample
labels the same stream `audio/pcm;rate=16000` — so the adapter always
parses `rate=` from the actual chunk's mimeType (`pcm.ts`'s
`parsePcmRate`) and only falls back to 24000.

**The gate — `speaking` follows the playback timeline, not chunk receipt.**
Chunks arrive faster than real time. Each decoded chunk becomes an
`AudioBufferSourceNode` started at a running cursor on a separate 24 kHz
playback context (`cursor = max(cursor, now + 0.1 s)` at the start of a
burst, `cursor += duration` per chunk — Google's `audio-streamer.ts`
scheduling). The `AnalyserNode` hangs off the gain node those sources play
through, so `AudioAnalysis` can only read what is *audible right now*, and
`tick()` derives `speaking` from `now < cursor`. A chunk received but not
yet audible, or a buffer starved mid-generation, reads as `thinking`;
drained after `generationComplete`/`turnComplete`/`waitingForInput` reads
as `listening`. `serverContent.interrupted` (the user barged in) stops
every scheduled source after a 30 ms gain ramp (a hard cut clicks) and
resets the cursor, exactly what the Live guide instructs.

**State mapping.** Gemini exposes no server-side user-VAD start/stop
message (`speechState` is deprecated in the reference with an undefined
replacement; LiveKit treats `interrupted` as input-speech-started, Pipecat
states outright "exposes an `interrupted` event but no turn-start/-end"),
so "the user stopped talking" is never signalled and `thinking` is short
by construction. What is used: `setupComplete` → `listening`;
`interrupted` → `listening`; input transcription arriving (Gemini's own
ASR — a vendor signal, not an energy heuristic) → `listening`; first chunk
of a turn → `thinking`; audible → `speaking`; drained + done →
`listening`; `goAway`/unexpected close → `initializing` while reconnecting
with the stored `sessionResumptionUpdate.newHandle` (3 attempts, mic and
playback graphs untouched), then `listening` or, if that fails, `idle`.

**Credentials — same rule as `OpenAIRealtimeVoiceSource`.** `auth_tokens/…` →
ephemeral token minted by your backend (`POST /v1beta/auth_tokens`, can
lock model/config server-side), sent as `access_token` on the Constrained
endpoint: the production shape. Anything else → raw API key on `?key=`,
**dev-only demo wiring**: memory only, never persisted or logged; it sits
in the socket URL only because a browser `WebSocket` cannot set an auth
header and that is the API's documented key path — one more reason a
product must use tokens.

### `ElevenLabsVoiceSource` — ElevenLabs Conversational AI over WebSocket

`packages/voice/src/ElevenLabsVoiceSource.ts`. Same transport shape as
Gemini, different protocol — so the *audio graph* the two share was
extracted into `packages/voice/src/PcmAudioGraph.ts` (worklet mic
capture at any input rate, cursor-scheduled gap-free playback at any
output rate, the analyser on the playback gain node, the barge-in fade)
and `GeminiLiveVoiceSource` was refactored onto it with no behaviour
change; the two adapters keep their own transport/state layers, which
genuinely differ. Built against the current docs and ElevenLabs' own
shipped SDK (`elevenlabs/packages`, `packages/client/src/` —
`WebSocketConnection.ts`, `BaseConversation.ts`, `VoiceConversation.ts`,
`platform/web/{input,output}.ts`), citations in the agents log.

**Wire messages**: socket opened with the `"convai"` subprotocol exactly
as the SDK does; first client message
`{"type":"conversation_initiation_client_data"}` (optional
`conversation_config_override`); first server message
`conversation_initiation_metadata` with `agent_output_audio_format` /
`user_input_audio_format` — the graph is built only after it, at exactly
those rates (`pcm_*` decoded by `pcm16ToFloat32`, `ulaw_8000` by a G.711
μ-law decoder added to `pcm.ts` from the `rochars/alawmulaw` reference
implementation; PCM input only, a μ-law-input agent is rejected with a
clear error). Mic: `{"user_audio_chunk": <base64 PCM16>}`. Server:
`audio` (`audio_event.audio_base_64`, `event_id`), `interruption`
(`interruption_event.event_id`), `ping` (`ping_event.event_id`) answered
at once with `{"type":"pong","event_id"}`, `vad_score`
(`vad_score_event.vad_score`), `user_transcript`, `agent_response`.
Audio events that arrive between the metadata and the graph coming up
(agents often greet first) are queued and flushed, not dropped.

**State mapping — the vendor VAD does the work here:**

| Signal | `AgentState` |
|---|---|
| metadata received, graph up | `listening` |
| `vad_score` rises above 0.5 (user talking) | `listening` |
| `vad_score` falls below 0.5 with nothing queued, or `user_transcript` (finalized utterance) | `thinking` |
| `audio` received, not yet audible | `thinking` |
| playback audible (`PcmAudioGraph` gate) | `speaking` |
| `agent_response_complete`, or an `audio` chunk with `is_final` | the reply is over. Chunks that trail it don't reopen the reply; the user's next turn (speech, transcript, barge-in) does. If nothing is left to play (a text-only or empty reply), `listening` at once |
| playback drained **after** the reply is over | `listening`: the end of the agent's turn |
| playback drained **before** the reply is over | `thinking`: a network stall mid-reply, not a turn end. More audio → `speaking` again |
| a reply still open with no audio for 10 s | `listening` (a guard against a lost `complete`) |
| `interruption` | 30 ms fade, queue cleared, `event_id` watermark set (later chunks below it are dropped — the SDK's own rule), `listening` |
| `thinking` with no reply open and no audio for 4 s | `listening` (a guard against VAD flicker on noise, not vendor semantics) |
| close code 1000 (agent ended the call) / any other close | `idle`; no auto-reconnect — a conversation is not resumable |

The 0.5 threshold is this adapter's choice on the documented probability,
not a vendor constant. Unlike the SDK, which flips its own "speaking" mode
on chunk *receipt*, `speaking` here follows the playback timeline, the
same gate as the Gemini adapter.

**Credentials.** An `agent_id` connects to a **public** agent with no
secret anywhere — the one vendor path in this project that needs no
dev-only caveat at all. A `wss://` credential is a **signed URL** for a
private agent, minted by the integrator's backend (`GET
/v1/convai/conversation/get-signed-url?agent_id=…` with the `xi-api-key`
header → `{ signed_url }`, valid 15 minutes, an already-open socket
outlives it): the production shape. The API key itself is never accepted
by the adapter.

### `LiveKitVoiceSource` — an existing LiveKit Room's agent

`packages/voice/src/LiveKitVoiceSource.ts` (+ the DOM-free rules in
`livekitAgent.ts`), on `livekit-client` — the one runtime dependency any
adapter needed, because a LiveKit Room *is* the transport. Built on the
client SDK directly, not `@livekit/components-react`'s hooks, so it stays
framework-agnostic like every other adapter. The SDK surface was
re-verified from `livekit/client-sdk-js`, `livekit/protocol` and
`livekit/components-js` source (the React hooks `useVoiceAssistant` /
`useAgent` are the reference for how LiveKit itself finds the agent), not
taken from this project's earlier competitive research.

**Why it's different from the three vendor adapters:** nothing is
decoded, scheduled or negotiated. A LiveKit Agents voice agent already
publishes its lifecycle as the participant attribute `lk.agent.state`
with values `initializing | idle | listening | thinking | speaking` —
literally this project's `AgentState`, adopted from LiveKit in the first
place, so the mapping is identity — and its voice arrives as a subscribed
remote track (`RoomEvent.TrackSubscribed`, `track.mediaStreamTrack`). The
adapter points the unchanged `AudioAnalysis` graph at that track (the
same `MediaStream([mediaStreamTrack]) → createMediaStreamSource →
AnalyserNode` chain `livekit-client`'s own `createAudioAnalyser` builds)
and forwards the attribute on `RoomEvent.ParticipantAttributesChanged`.

**Agent discovery** (components-js's rule): the primary agent is the
`ParticipantKind.AGENT` participant *without* `lk.publish_on_behalf`; a
worker participant whose `lk.publish_on_behalf` equals the agent's
identity may be the one carrying the audio, so both are watched. Found on
join (already-subscribed publications are scanned, since they never fire
`TrackSubscribed` for a late listener) or on `ParticipantConnected`. If
the agent never publishes a state (older agents), an energy fallback on
its track gives `speaking`/`listening`; the WebRTC zero-RMS watchdog is
reused, counted only while the agent says it is speaking.

**Two entry points:**
- `{ room }` — **attach to the app's Room.** The primary path and the
  point of the adapter: the app already has a connected `Room`, so this
  library never sees a credential, never connects, publishes or
  disconnects that Room, and does not `attach()` the agent's track to an
  element (the app already renders it; a second attach would double the
  audio). `disconnect()` only removes the adapter's listeners and analyser.
- `{ url, token }` — **own Room**, the Studio demo: connects, publishes
  the mic so the agent hears the user, `startAudio()` (a user gesture is
  present), attaches the track so the agent is audible, errors if no agent
  joins within 20 s (components-js's default), disconnects on teardown.
  The token is a short-lived, room-scoped JWT the developer mints
  server-side (`lk token create`, or their token server) — the production
  shape already; LiveKit's sandbox token server is deprecated and its
  successor's HTTP shape isn't published, so the Studio takes `url` +
  `token` directly (one field, whitespace-separated, for now).

### Credentials for a real integration (not the Studio)

The adapter subsections above describe what the *Studio* does. This one is
for a developer shipping Orb/Signal in an actual product (a fitness app, or
anything else), because the Studio's key field is the one thing in this
pipeline that must **not** be copied into an app. The rules, in the order
they apply:

1. **The real API key never touches this library, and never touches the
   browser.** Neither adapter has any code path that needs a long-lived
   secret in production. The only place a raw key is accepted is the
   Studio's `VoiceVendorControls` field, and both adapters treat it as
   what it is — local demo wiring (memory only, never persisted, never
   logged), clearly not for shipping. An app must not offer that field.
2. **The integrator's own backend holds the secret and mints a short-lived
   credential, server-side, per session.** This is the vendors' own
   documented pattern, not ours:
   - OpenAI: `POST https://api.openai.com/v1/realtime/client_secrets`
     with `Authorization: Bearer <secret key>` and the session config
     (`type: "realtime"`, `model`, `audio.input.turn_detection`,
     `audio.output.voice`, optional `instructions`) → `{ value: "ek_…",
     expires_at }`. The `ek_` expires (600 s by default, 10 s–2 h
     selectable) and is bound to one Realtime session; the session config
     is fixed at mint time, so the client can't change the model or
     instructions.
   - Gemini: `POST https://generativelanguage.googleapis.com/v1beta/
     auth_tokens` with the secret key → a token whose `name` is
     `auth_tokens/…`. `uses` (default 1), `expireTime` (default 30 min)
     and `newSessionExpireTime` (default 1 min — the window in which the
     token can *start* a session) bound it, and `liveConnectConstraints`
     can lock the model and setup config server-side so the client can't
     alter them. Within a token's lifetime the ~10-minute connection
     limit is still crossed with `sessionResumption`, which
     `GeminiLiveVoiceSource` already does.
   - ElevenLabs: for a **private** agent, `GET https://api.elevenlabs.io/
     v1/convai/conversation/get-signed-url?agent_id=…` with the
     `xi-api-key` header → `{ "signed_url": "wss://…&conversation_
     signature=…" }`, valid 15 minutes — that URL is the credential. A
     **public** agent needs no minting at all: the frontend passes the
     `agent_id`, no secret exists on either side.
   - LiveKit: nothing to mint for this library. Pass the app's connected
     `Room` (`new LiveKitVoiceSource({ room })`) and no credential flows
     through Orb/Signal at all; the Room's own token is the short-lived
     JWT the app's backend already mints with the project's API key/secret.
3. **The frontend asks its own backend for that credential, then hands it
   straight to the adapter.** That string is the only thing this library
   ever sees:

   ```ts
   // Your app, not this repo -- sketch of the whole client side.
   const mintOpenAI = async () =>
     (await fetch("/api/voice-session", { method: "POST" }).then((r) => r.json())).credential;
   const { credential } = await fetch("/api/voice-session", { method: "POST" }).then((r) => r.json());
   // credential is "ek_…" (OpenAI), "auth_tokens/…" (Gemini) or a signed
   // "wss://…" URL / public agent_id (ElevenLabs); the adapters pick the
   // production path from that shape and never mint anything.
   const source =
     vendor === "openai" ? new OpenAIRealtimeVoiceSource({ getCredential: mintOpenAI }) // fresh ek_ per (re)connect
     : vendor === "gemini" ? new GeminiLiveVoiceSource({ credential })
     : new ElevenLabsVoiceSource({ credential });
   source.onMetrics((m) => { /* -> audioLevel / audioBandN overrides */ });
   source.onStateChange((s) => { /* -> voiceStateCode */ });
   await source.connect();
   ```

   The `/api/voice-session` handler is the integrator's: authenticate the
   user, decide which vendor/model/voice/instructions this session gets,
   call the vendor's mint endpoint with the secret key from the server's
   own secret store, return only the short-lived credential.
4. **This project has no backend and does not ship one** (it is why the
   Studio has a dev-only key field at all). Minting the ephemeral
   credential server-side is the integrator's responsibility; this library
   deliberately contains no minting code beyond the Studio's demo path, no
   secret storage, and no opinion on the integrator's auth — it takes a
   credential string and a `VoiceSource` interface, nothing more.

### A raw API key is refused unless you opt in

Rule 1 above used to be advice. It is now enforced, identically on Web, iOS
and Android: a credential that is not the vendor's short-lived shape is
**refused in `connect()`**, before the microphone, the audio graph or the
socket, so a refused credential never opens a device or a connection.

```ts
new GeminiLiveVoiceSource({ credential: rawKey });
// Error: GeminiLiveVoiceSource: refusing a raw, long-lived API key. Pass a
// short-lived credential (auth_tokens/…) minted by your own backend
// (POST …/v1beta/auth_tokens). For a local demo only, set
// `allowInsecureApiKey: true`.
```

`allowInsecureApiKey` (Swift/Kotlin: `allowInsecureApiKey: Bool = false`;
React Native: the same field on `createVoiceSource`'s config) connects anyway
and logs one warning per connect. It exists for exactly one situation — a
developer pasting their own key into a local demo, which is what the Studio's
credential field is — and the warning is there so that situation can't quietly
become a deployment. It applies to whatever `getCredential` /
`credentialProvider` returns too, so a backend that starts handing out raw keys
is caught on the next reconnect rather than silently accepted.

The rule lives in one file per platform so the three can't drift:
`packages/voice/src/insecureCredential.ts`,
`packages/ios/Sources/SinuaVoice/InsecureCredential.swift`,
`packages/android/src/main/kotlin/dev/sinua/voice/InsecureCredential.kt` — same
message text, asserted by a test in each. Shape for the shape's own sake:
`openai-agents-js` refuses a raw key in a browser unless `useInsecureApiKey` is
set, and that is the same source this project's OpenAI adapter was built from.

Only OpenAI and Gemini can be handed a raw key at all. ElevenLabs takes a public
`agent_id` (not a secret) or a signed `wss://` URL, and LiveKit takes a Room or a
room token, so neither has anything to gate.

### What actually travels, per vendor and per platform

The credential *classification* — `ek_…`, `auth_tokens/…`, `wss://…`, else raw —
is the same code path on all three platforms. The transport is too, with one
exception:

| Vendor | Web | iOS / Android |
|---|---|---|
| OpenAI Realtime | `Authorization: Bearer` on the HTTPS call | same |
| ElevenLabs | `?agent_id=` or the signed URL as-is, `convai` subprotocol | same |
| LiveKit | token handed to the SDK; nothing of ours touches it | same |
| **Gemini Live** | **`?access_token=` / `?key=` in the socket URL** | `Authorization: Token …` / `x-goog-api-key` **headers** |

**The Gemini row is a real weakness on the Web, and it is not fixable here.** A
browser `WebSocket` cannot set request headers, and the Live API accepts exactly
two methods — [ai.google.dev/gemini-api/docs/ephemeral-tokens](https://ai.google.dev/gemini-api/docs/ephemeral-tokens):
*"ephemeral tokens must either be passed in an `access_token` query parameter, or
in an HTTP `Authorization` prefixed by the auth-scheme `Token`."* Google's own
JS SDK appends the key to the URL for this API path, so the Web adapter is doing
the vendor's documented browser thing. A credential in a URL can reach proxy
logs, server logs and history in a way a header does not — which is the argument
for the ephemeral token, not against the adapter: an `auth_tokens/…` is
single-use by default and expires in ~30 minutes, so a leaked URL is worth much
less than a leaked key. Native has no such constraint and puts it in a header.

No adapter on any platform logs, persists or otherwise stores a credential.

## Wiring a `VoiceSource` into Orb/Signal — the `VoiceOverrides` helper

`packages/core/src/voice.ts`, exported from `@sinua/core` next to
`frameWithOverrides`. Before it, an integrator with a connected
`VoiceSource` still had to hand-build the opts map, and had to already
know three things the Studio learned the hard way: the exact key
encodings, that a ~30 fps metrics feed has to be eased at render rate or
the visual visibly steps (the stutter above), and the `muted` cue. The
Studio's two panels each carried their own copy of that logic; the helper
packages it once, with the Studio's tuned numbers, DOM-free and clock-free
(dt-driven) so it runs wherever the engine does and can be tested under
plain node. The `VoiceSource` / `AgentState` / `VoiceMetrics` types now
live there too (the Studio's `audio/types.ts` re-exports them), so an app
imports one package for the whole voice contract.

The five-minute path:

```ts
import { frameWithOverrides, VoiceOverrides } from "@sinua/core";

const voice = VoiceOverrides.bind(source, { history: { count: 40 } }); // history only for "scrolling"
await source.connect();
// per rendered frame, dt = seconds since the previous frame:
const frame = frameWithOverrides("signaling", 64, t, { ...myKnobs, ...voice.overrides(dt) });
// voice.state / voice.metrics are there for a lifecycle pill and a level meter.
```

What `overrides(dt)` emits, and what reads it:

| Key(s) | Engine reader | Notes |
|---|---|---|
| `audioLevel`, `audioStrength` | `primitives::apply_audio_reactive` (every `orbs` state) | `audioLevel` eased at `levelEaseRate` 7/s (~150 ms, the Orb Studio's value — it smooths a whole-orb breathing motion); `audioStrength` default 0.18, `0` disables; a no-op for `signal` frames (no dots) |
| `audioBandCount`, `audioBand0..15` | `spectrum.rs`, `bar.rs`, `waveform.rs` via `primitives::audio_band` | eased at `bandEaseRate` 24/s (~40 ms, the Signal Studio's value — just enough to bridge two analysis updates); capped at 16, the engine's key count; `audioBandCount: 0` means "no real audio, use the synthetic pattern" |
| `voiceStateCode` | every `signal` style (`bar.rs`'s `VoiceState`) | `idle 0 / initializing 1 / listening 2 / thinking 3 / speaking 4`; ignored by `orbs`, which never auto-switch on voice state |
| `historyCount`, `history0..N-1`, `historyPhase` | `scroll.rs` | only with `history: { count, hz }`; the caller-owned ring buffer `signal.md` describes, pre-filled with silence, pushed at `hz` (default 12) with the *peak* level since the previous push and a remainder-carrying cadence, `historyPhase` = fraction of the interval elapsed |
| `muted`, `mutedTint`, `mutedHue` | `primitives::apply_muted` (every family) | from the tracker's `muted` flag / options; the animation slowdown that goes with it is the caller's clock (see `signal.md`), not an opt |
| `interruptAge` (+ `interruptDuration`/`Strength`/`Tint`/`Hue`) | `primitives::apply_interrupt` (every family) | only for `window` seconds (default 1) after `interrupt()` or a bound source's `onInterrupt`; the age grows with the *unclamped* `dt` (it's an age on the caller's clock, per the engine contract — the 0.1 s clamp is for easing only); companions only when set in `interrupt: {…}`; `interrupt: false` never emits it. See *Barge-in* below |

`voiceOverrides(metrics, state, opts)` is the pure, stateless form of the
same mapping (no easing, no history) for callers that already smooth their
own readings. `bind` takes a `VoiceSource`'s single metrics and state
callbacks — read `metrics`/`state` off the tracker instead of subscribing
twice, or skip `bind` and feed `push`/`setState` from your own callbacks
(a native app with its own capture, a LiveKit `useMultibandTrackVolume`
hook). Both Studio panels now use it this way (see *Studio wiring*).
`levelEaseRate`/`bandEaseRate: Infinity` means "no easing, pass the raw
reading through". The Orb Studio uses it for bands, because `spectrum`'s
bars were tuned on raw bands.

### Barge-in: `onInterrupt` → `interruptAge`

The engine's one-shot barge-in flash (`primitives::apply_interrupt`, see
`docs/engine.md`) needs the *moment* the user cut the agent off, as an age.
`VoiceSource` has an optional `onInterrupt(cb)` for it. It is optional so
that a plain mic, a test tone, or a native source that doesn't have the
signal is still a valid `VoiceSource`. `VoiceOverrides.bind` subscribes to
it when it exists. `voice.interrupt()` is public for a manual trigger or a
native callback.

What counts: the user barged in **while the agent was outputting audio**.
Speech over `thinking` or `listening` doesn't count, because there's no
output to cut and the flash would be a false alarm. It fires once, before
the matching state change. Per adapter (sources fetched 2026-09-18):

| Adapter | Trigger | Source |
|---|---|---|
| `OpenAIRealtimeVoiceSource` | `input_audio_buffer.speech_started` while `speaking` | OpenAI *Realtime conversations* guide: that event is the interruption; the server cancels the response and WebRTC auto-truncates |
| `GeminiLiveVoiceSource` | `serverContent.interrupted` while `speaking` or with playback still scheduled (`PcmAudioGraph.playbackState() !== "drained"`) | Gemini Live reference: "a client message has interrupted current model generation … stop and empty the current playback queue". Audio can be queued before it's audible, and cutting it is still a real interruption |
| `ElevenLabsVoiceSource` | `interruption` event, same guard, plus audio still waiting for the graph | ElevenLabs websocket reference + SDK (`VoiceConversation.ts`) |
| `LiveKitVoiceSource` | **inferred:** `lk.agent.state` leaves `speaking` for `listening`/`thinking` while `room.localParticipant.isSpeaking` (`isInferredBargeIn` in `livekitAgent.ts`) | LiveKit's frontend agent-state docs expose `lk.agent.state` only, with no client-side interruption event. `Participant.isSpeaking` is the server's active-speaker flag (livekit-client 2.22.3). A normal end of turn has the user silent, so it doesn't match. Known false positive: LiveKit's "false interruption" recovery resumes speaking afterwards, and the flash has already played |
| `LocalMicVoiceSource`, `TestToneVoiceSource` | none (no agent to interrupt) | — |

## `AudioAnalysis` — the actual extraction math

`packages/voice/src/analysis.ts`, shared by both sources above so they
exercise identical code. Every number here came from reading real, shipped
implementations (LiveKit's `useMultibandTrackVolume`, ElevenLabs'
`LiveWaveform`), not invented:

- `AnalyserNode`, not `AudioWorklet` — audio-thread-native, effectively
  free for this purpose; `AudioWorklet` is for actual DSP/re-encoding, not
  visualization-grade reads. `fftSize` is 512 (both `LocalMicVoiceSource`
  and `TestToneVoiceSource`) — see *the band-uniformity bug*, below, for
  why this isn't the more common 256.
- Restricts to the voice-relevant bin range (`loFrac`/`hiFrac`, default
  0.05–0.4 of the FFT buffer) — discards DC/sub-bass and the mostly-noise
  high end.
- **Log-spaced (geometric) band boundaries, not equal-width chunks.**
  `bandCount` defaults to 16 (was 5 in an earlier version — see below).
  Band edges are computed as `exp(logLo + (logHi - logLo) * (b /
  bandCount))` for `b in 0..=bandCount`, not `lo + (hi - lo) * (b /
  bandCount)` — each band's width scales with frequency, the same
  principle Butterchurn and most spectrum visualizers use. This spreads out
  the low end (where a voice's fundamental/formants live) and widens the
  high end (sibilants/consonant noise), so adjacent bands actually diverge
  instead of tracking the same broadband envelope.
- Per-band value: average the raw byte magnitudes across each band's bin
  range, normalize to `0..1`, then `sqrt()` — the square root is the
  load-bearing perceptual-compression step, confirmed independently in
  both LiveKit's and ElevenLabs' own code: it keeps quiet/moderate speech
  visibly above zero instead of reading as near-silent.
- **Asymmetric attack/release smoothing** (`attack: 0.7`, `release: 0.12`
  defaults), not a flat symmetric time constant — fast rise, slow decay,
  grounded in real VU/PPM meter ballistics (IEC 60268-17), reads as
  "alive" rather than twitchy or sluggish.
- `level` is the max across bands, itself attack/release smoothed the same
  way.

### A real bug: every bar moved identically in mic mode

User-reported while testing `signal`'s `bar` style with a live microphone:
every bar appeared to run "the same animation," instead of each band
visibly differing the way a real EQ does. Two compounding root causes,
both in the *previous* version of this file:

1. **Band/bar-count mismatch.** `AudioAnalysis` was constructed with only
   `bandCount = 5`, while `bar.rs` renders more bars than that by default —
   its `band_idx` mapping formula ends up assigning the *same* band index
   to multiple adjacent bars, so those bars were, correctly, reading
   identical values; there weren't enough distinct bands to go around.
2. **Equal-width linear band chunking over a coarse 256-bin FFT.** Even
   after raising `bandCount`, a single human voice's energy is fairly
   correlated across a narrow, *linearly*-chunked slice of the spectrum —
   adjacent equal-width bands end up tracking the same broadband envelope
   rather than diverging, because linear chunking doesn't separate the
   fundamental from its nearby harmonics the way a log-spaced split does.

Fixed by raising `bandCount` to 16, switching to log-spaced band edges (see
above), raising `fftSize` to 512 (more raw bins in the voice-relevant range
gives 16 log-spaced bands enough resolution to actually differ instead of
each averaging over a near-identical handful of bins), and — since
`TestToneVoiceSource`'s original single pure-tone oscillator concentrates
all its energy in one FFT bin regardless of how many bands it's split into,
which would have made this fix look like it did nothing in Test Tone mode
specifically — replacing it with three oscillators at a speech-like
fundamental/formant-ish spread (160Hz / 520Hz / 1400Hz, gains 1.0/0.5/0.28)
so the test signal has actual spectral structure to differentiate. Verified
via `bar.rs`'s existing per-band tests plus manual mic/Test-Tone checks in
the Studio after the fix.

### A second, slower smoothing pass exists purely for visual fluidity

`AudioAnalysis` updates at ~30fps (`LocalMicVoiceSource`/
`TestToneVoiceSource`'s `UPDATE_MS`), matching both LiveKit's
(`updateInterval: 32`) and ElevenLabs' (`updateRate: 30`) own defaults —
deliberately decoupled from the ~60fps render loop, for the same reason
they do it: reading FFT data doesn't need to happen at full display
refresh rate, and it saves CPU/battery on mobile.

**This produced a real, user-reported stutter**: reading the 30fps value
directly in a 60fps render loop means the value holds for two frames then
jumps, which reads as choppy motion rather than a step in an otherwise
continuous animation. Fixed with a *second*, independent easing pass
(`smoothedAudioLevelRef` in `OrbStudio.tsx`/`SignalStudio.tsx`) that eases
toward the raw 30fps value every render tick (~150ms time constant) — so
the visual reads as continuous regardless of the underlying sample rate.
Lesson for any future per-tick data source sampled slower than the render
loop: sampling less often to save CPU is fine, but the *consumer* needs its
own interpolation layer, or the mismatch in update rates is directly
visible as stutter.

### A known real gotcha, guarded against

An `AnalyserNode` attached to a live `MediaStreamTrack` can silently start
reading flat/zero data mid-session with no error (a documented, currently
open Chrome/Android issue against WebRTC tracks specifically). Both
`LocalMicVoiceSource` and the future WebRTC adapter share this risk, so a
watchdog is already in place: if raw RMS reads exactly zero for ~1s
(`WATCHDOG_ZERO_FRAMES`) while the track's `readyState` is still `"live"`,
the `MediaStreamAudioSourceNode`/`AnalyserNode` pair is torn down and
recreated.

## Studio wiring

`OrbStudio.tsx` and `SignalStudio.tsx` both expose 🎙️ Live Mic / ▶️ Test
Tone buttons wired to the same `VoiceSource` pair, plus a
`VoiceVendorControls` row (`VoiceVendorControls.tsx`:
one credential field — password-type, React state only, never stored —
and one connect toggle per built vendor adapter, constructed through that
file's `createVendorVoiceSource` so the panels never import an adapter
directly). It is deliberately minimal wiring in the existing button
style; the Studio UI/UX pass owns how it should look. Both panels build
their voice overrides with one `VoiceOverrides` each (the helper above).
It's fed via `push`/`setState`/`onInterrupt` from the panel's own source
callbacks, stepped once per RAF tick with the real unclamped `dt`, and the
same instance ages the toolbar's Interrupt button (so the flash works with
no source connected). Orb uses `bandEaseRate: Infinity`; Signal uses
`audioStrength: 0` and `history: { count, hz: 12 }` with
`setHistoryCount` on the knob. The swap was checked against the pre-swap
code with a side-by-side harness on the same scripted feed: Orb and the
`signaling`/`waveform` maps are identical frame for frame. `scrolling`
differs only (a) after a frame stall >0.1 s, where the history cadence
now runs on the clamped `dt` (a one-time phase shift, same 12 Hz rate),
and (b) for ≤83 ms after a `historyCount` knob change (the buffer now
resizes immediately instead of at the next push). What each panel does
with the resulting `{level, bands}` differs by design, on purpose:

- **`OrbStudio`**: feeds `audioLevel`/`audioStrength` into the generic,
  mode-agnostic `apply_audio_reactive` post-process (see
  [`engine.md`'s *Audio-reactive pulse*](engine.md#audio-reactive-pulse))
  — a subtle breathing pulse on whichever orb state is currently selected.
  **Does NOT** let a connected `VoiceSource`'s lifecycle state
  auto-switch which orb *state* is displayed — this was tried and
  explicitly rejected: each orb state is a standalone product a developer
  picks for their app, so a "Speaking" voice event silently swapping
  `aurora` for `spectrum` underneath them would be wrong. Auto-state-
  switching code was written, then reverted for exactly this reason.
- **`SignalStudio`**: the opposite call, deliberately — a connected
  `VoiceSource`'s real lifecycle state **does** drive the current
  `signal` style's `voiceStateCode` directly (both `bar` and `waveform`
  read the same opt, see [`signal.md`](signal.md)), overriding the manual
  "Preview state" picker. For `signal`, driving its own built-in state
  machine from real voice state is the entire point of the family, not a
  state swap between different products.

Both keep audio-derived overrides (`audioLevel`, `audioBandN`,
`voiceStateCode`) **out of** the per-state slider overrides object that
also feeds the Studio's code-export snippet — "wherever the mic level
happened to be" has no business in exported code; it's merged in only at
the point of computing the live render.

## Native (iOS/Android): mic + test tone built, spec-exact parity with Web

Native apps get the same pipeline as the Web Studio as **library** code,
so a host app imports it directly:
- iOS: SwiftPM product `SinuaVoice` in `packages/ios`, next to
  `CoreEngine` but separate, so an app that never uses the mic doesn't link it.
- Android: package `dev.sinua.voice` in `packages/android`, with no new
  runtime dependency.

The platforms can't share code the way the Rust engine is shared, but they
share the *contract*:
- **Types:** the same `AgentState` vocabulary, `VoiceMetrics {level, bands}`
  and a `VoiceSource` interface (connect, disconnect, onMetrics,
  onStateChange, optional onInterrupt).
- **Output:** the same opts keys. `VoiceOverrides.overrides(dt)` returns the
  `[String: Double]` / `Map<String, Double>` that UniFFI's
  `frameWithOverrides` takes, with the same easing, interrupt age and
  scrolling history as Web's `voice.ts`.

| Piece | iOS (`SinuaVoice`) | Android (`dev.sinua.voice`) |
|---|---|---|
| Byte spectrum | `SpectrumAnalyser`: Blackman window + Accelerate `vDSP_fft_zrip` (its packed output is 2× the DFT, compensated) | `SpectrumAnalyser` + a 40-line radix-2 `Fft`; Android has no first-party FFT for mic audio, since `Visualizer` only taps an *output* session |
| Band math | `AudioAnalysis`, line-for-line port of `analysis.ts` | same |
| Mic | `LocalMicVoiceSource`: `AVAudioEngine` input tap → ring → 30 Hz main-queue tick. `configureSession: false` leaves `AVAudioSession` to an app that manages it. Asks permission in `connect()` | `LocalMicVoiceSource`: `AudioRecord` mono float at 48 kHz with a 44.1 kHz fallback (the only rate Android guarantees) on a reader thread → ring → 30 Hz main-looper tick |
| Test tone | `TestToneVoiceSource` over `TestToneGenerator`: Web's partials and burst envelope, synthesized sample-accurately, nothing played, no permission | same |
| DX helper | `VoiceOverrides` (+ pure `voiceOverrides`) | same |

**Why "the same bytes", and how it's proven.** Web's `AudioAnalysis`
consumes `AnalyserNode.getByteFrequencyData`, so native parity starts one
step earlier. The native `SpectrumAnalyser`s implement the Web Audio spec's
own definition (`WebAudio/web-audio-api` `index.bs`, *FFT Windowing and
Smoothing over Time*):
- a Blackman window, α 0.16;
- `X[k] = (1/N) Σ x̂[n] e^{−2πikn/N}`;
- smoothing τ = 0, the Studio's setting;
- `20·log10`, then `floor(255/(max−min)·(Y−min))` clipped to 0…255, with
  min/max −100/−30 dB and N = 512.

`spec/voice-golden.json` pins that down. `packages/core/scripts/gen-voice-golden.mjs`
builds it in four steps:
1. It synthesizes a fixed 2 s / 48 kHz signal: the test-tone partials under
   bursts, a noise stretch, silence.
2. Real Chrome renders the signal through an `AnalyserNode` in an
   `OfflineAudioContext` and reads the bytes every 1536 frames.
3. A spec-written float64 reference is checked against those bytes. It
   lands within one byte everywhere; 1 of 15 872 bins sits one byte off,
   at a `floor()` boundary between Chrome's float32 and float64.
4. The bytes go through the Studio's real `analysis.ts` for the expected
   `{level, bands}`, and a scripted feed goes through the real
   `VoiceOverrides` under four configs (default, Orb, Signal with
   history/interrupt/mute options, interrupt off) for the expected maps.

Results:

| Test | Byte spectrum vs Chrome | `{level, bands}` vs Web | `VoiceOverrides` vs Web |
|---|---|---|---|
| iOS `SinuaVoiceTests` (Simulator) | 1/15 872 bins off by one (vDSP float32), none more | max error 7e-18; speaking/listening agree on every tick | keys identical, values within 1e-9 (1.5e-15 measured), 150 steps × 4 configs |
| Android JVM tests (`VoiceGoldenTest`) | 1/15 872 off by one | max error 0 | same |
| `packages/core` `voice-golden.test.mjs` | the spec reference still reproduces Chrome (±1) | — | today's `voice.ts` still produces the recorded maps, so a Web-side change breaks this test instead of silently drifting from native |

**Sample rate.** Band edges are FFT-bin *fractions*, as on Web, so bands
match exactly at equal sample rates:
- Web runs at the `AudioContext` rate, usually 48 kHz.
- iOS uses the hardware input rate (48 kHz on current devices).
- Android tries 48 kHz first.

At 44.1 kHz each band sits about 8 % lower in frequency, which is the same
on every platform at that rate.

**Permissions.** The library never declares a permission for an app:
- iOS: the app's Info.plist needs `NSMicrophoneUsageDescription`, and
  `connect()` requests access (`AVAudioApplication` on 17+, else
  `AVAudioSession`). A refusal throws `VoiceSourceError.permissionDenied`.
- Android: the app declares `RECORD_AUDIO` and requests it at runtime.
  Without it, `connect()` throws `SecurityException`, and
  `isMicPermissionGranted(context)` checks. The library manifest leaves
  the permission out on purpose, so it never gets merged into an app that
  doesn't use the mic.

The Studio apps declare the permission and wire the peek-row buttons (see
`studio-ui-ux`'s log).

**Verified by simulator/JVM only.** The Simulator/emulator can't exercise
a real microphone meaningfully. Real-mic capture on a device is part of the
pending live checks.

### Native LiveKit: `SinuaLiveKit` (iOS) / `:sinua-livekit` (Android)

Built 2026-09-19. It mirrors the Web `LiveKitVoiceSource` (*Built vendor
adapters*): the same agent discovery, the same `lk.agent.state` identity
mapping, the same inferred barge-in, and the same energy fallback (0.05)
for agents that never publish a state. No vendor protocol and, on the
primary path, no credential.

**SDKs** (read from source at the latest tags, 2026-09-19):
- **iOS:** `livekit/client-sdk-swift` **2.17.0** (SPM, `from: "2.17.0"`,
  pinned in `packages/ios-livekit/Package.resolved`). Audio comes through
  `RemoteAudioTrack.add(audioRenderer:)`. The SDK documents
  `AudioRenderer.render(pcmBuffer:)` as "observe audio buffers before
  playback, e.g. for visualization"; it's called on the WebRTC audio
  thread. Room events arrive through `RoomDelegate` on an SDK-private
  queue and are hopped to main.
- **Android:** `io.livekit:livekit-android:2.28.2`. Audio comes through
  `RemoteAudioTrack.addSink(AudioTrackSink)`: interleaved int16 PCM on
  the WebRTC playout thread. Events come from `room.events`.
  - Needs JitPack for `audioswitch` (LiveKit's README). It's added in
    `packages/android/settings.gradle.kts`, filtered to the
    `com.github.davidliu` group.
- Both taps sit on the playout path, so the "analyse at playback time"
  rule (*PcmAudioGraph*) holds without a player of our own.

**Split: logic in the voice packages, glue in its own module.**
- `LiveKitAgentTracker` (`SinuaVoice/LiveKitAgent.swift`,
  `dev.sinua.voice/LiveKitAgent.kt`) is SDK-free. It holds:
  - the ported `livekitAgent.ts` rules;
  - the Web adapter's state machine, driven by plain events
    (`participantSeen` / `attributesChanged` / `participantLeft` /
    `audioAttached`, plus `tick()` at 30 Hz);
  - a `LiveKitPcmSink` for the audio thread. It keeps channel 0; int16 is
    scaled `/32768`, as WebRTC's own float conversion does. The samples go
    into the same `SampleRing` → `SpectrumAnalyser` → `AudioAnalysis`
    path as the mic.
- `LiveKitVoiceSource` in `packages/ios-livekit` (its own SwiftPM package)
  and `packages/android/livekit` (Gradle module `:sinua-livekit`) only
  translates Room events into those calls.
- **Why separate:** SwiftPM resolves every declared dependency of a
  package, so putting LiveKit in `packages/ios` would make every FxView
  consumer fetch the SDK and its WebRTC binary, and it would lift the
  core's swift-tools floor. Gradle has the same concern with native libs.
- **Cost:** one more dependency line for LiveKit users. The iOS package
  depends on `../ios` by path, which has to change when packages get
  published (SwiftPM wants a root `Package.swift`; that's an MVP item).
- iOS gotcha: LiveKit exports its own `AgentState`. Inside the glue, write
  `SinuaVoice.AgentState`.

**Two ways in**, the same as Web:
- `LiveKitVoiceSource(room:)` / `LiveKitVoiceSource(room)` attaches to the
  app's Room. It never connects, publishes, plays or disconnects it. It
  picks the agent up whenever it joins.
- `LiveKitVoiceSource(url:token:)` / `LiveKitVoiceSource.owned(context,
  url, token)` owns a Room: it connects, enables the mic, waits up to 20 s
  for an agent (components-js's default), and disconnects on teardown.
  - The token is a room JWT the app's backend mints.
  - Android's `VoiceSource.connect()` is synchronous, so the owned connect
    runs on the source's scope and reports failures to `onError`.
- No Web-style zero-RMS watchdog: that guards a Chrome `AnalyserNode`
  quirk, and the native path has no `AnalyserNode`.

**Verified:**
- Tracker unit tests with fake participants and PCM:
  - discovery (agent vs `publish_on_behalf` worker vs standard, first
    agent wins, late attributes);
  - the state mapping, with unknown values ignored;
  - the barge-in truth table plus the scripted sequence;
  - the energy fallback until a state is published;
  - agent leave → `initializing`, stop → `idle`;
  - sink → metrics identical to feeding `SpectrumAnalyser` directly;
  - int16 interleaved keeps channel 0 (and doesn't consume the caller's
    buffer);
  - non-int16 is ignored.
  - Results: iOS `SinuaVoiceTests` 18/18 (13 new); Android JVM 19/19
    (14 new).
- Both glue modules compile against the real SDKs:
  - `xcodebuild build -scheme SinuaLiveKit` (iOS Simulator);
  - `:sinua-livekit:assembleDebug`.
- **Not verified live.** That needs a real `wss://…` URL, a token and a
  dispatched agent from the user, on a device or simulator.

### Native Gemini Live: `SinuaGeminiLive` (iOS) / `:sinua-gemini` (Android)

Built 2026-09-19. It mirrors the Web `GeminiLiveVoiceSource`: the same
`setup` (model, `AUDIO`, input transcription, session resumption,
sliding-window compression), 16 kHz PCM16 up and 24 kHz down, the same state
mapping, the same "barge-in only if there was output to cut" rule, and
resume + reconnect on `goAway` / unexpected close (3 × 500 ms). The default
model is `gemini-3.8-live`.

**Research** (2026-09-19):
- The wire format is unchanged against `ai.google.dev/api/live` (updated
  2026-09-04). `speechState` is deprecated for `interactionStatus`, but its
  schema is undocumented. There is still no server→client user-VAD event,
  so the Web state mapping stands.
- **Auth goes in headers, not the URL.** The reference allows
  `Authorization: Token <t>` for ephemeral tokens, and Google's own
  `python-genai` (`live.py`, `_api_client.py`) connects with
  `Authorization: Token auth_tokens/…` or `x-goog-api-key`. Native sockets
  can set headers (a browser's can't), so the key/token never appears in
  the URL. That's an improvement over the Web path.
- **Firebase AI Logic was considered and not used:**
  - its Live API is Preview;
  - it needs a Firebase project, with App Check enforced from 2026-11-02;
  - it lists no 3.8 model;
  - Kotlin's `startAudioConversation()` plays audio internally, which
    loses the playback-time tap.

**Split.** Everything that can be tested without hardware lives in the
voice packages:
- `Pcm` (the `pcm.ts` port). Rounding is JS `Math.round`, i.e.
  `floor(x + 0.5)`. Android has its own base64, because `java.util.Base64`
  is API 26 and this library's minSdk is 24.
- `GeminiLiveSession`: protocol + state machine; it returns actions.
- `PlaybackTimeline` + `PcmAudioGraph`: the `PcmAudioGraph.ts` port.
  - Frames instead of `AudioContext.currentTime`: a 0.1 s lead, then
    gap-free.
  - The analyser is fed only the samples that have **played** by the
    player's clock (the analyser-on-gain-node rule), at 24 kHz like Web's
    playback context.
  - The `PcmAudioDevice` protocol abstracts the platform audio. ElevenLabs
    reuses all of this next.

The transport glue lives in its own product/module:
- **iOS `SinuaGeminiLive`** (a product of `packages/ios`, Foundation
  only): `URLSessionWebSocketTask`.
- **Android `:sinua-gemini`**: OkHttp **4.12.0**, not the latest 5.5.0:
  5.x needs kotlin-stdlib 2.2 and this build is on Kotlin 2.0.21. 4.12.0 is
  also what livekit-android uses. The module declares `INTERNET`;
  `RECORD_AUDIO` stays with the app.
- `LiveSocket` / `MainDispatcher` seams let the sources run against fakes.

**Audio devices:**
- **`AVPcmAudioDevice`:** one `AVAudioEngine`.
  - Mic tap with **voice processing** (Apple's echo canceller), converted
    to 16 kHz mono and chunked at 1024.
  - `AVAudioPlayerNode` at 24 kHz, with buffers scheduled at the player's
    own sample time.
  - Session `.playAndRecord` / `.voiceChat`, with a
    `configureSession: false` opt-out.
  - **It restarts the engine on `AVAudioEngineConfigurationChange`** and
    signals `onClockReset`, so the graph drops its timeline. On the
    simulator, enabling voice processing can stop the first engine right
    after start; on a phone, route changes do.
- **`AndroidPcmAudioDevice`:**
  - `AudioRecord` `VOICE_COMMUNICATION` (platform AEC, plus
    `AcousticEchoCanceler` if offered), mono float.
  - A streaming float `AudioTrack` with one writer thread that pads
    silence up to each scheduled frame, so `playbackHeadPosition` *is* the
    timeline clock.
- Interrupt `fade` isn't ported on either platform (hard cut). Whether that
  clicks audibly is a device check.
- Echo cancellation matters: without it, on speaker the model hears itself
  and barges in on its own voice. Web gets it from `getUserMedia` by
  default.

**Verified:**
- Core tests:
  - PCM (incl. the negative-half rounding and base64 against the JDK codec);
  - the timeline;
  - the graph over a fake device (incl. clock reset);
  - the session: setup/mic shape, turn → thinking → speaking → thinking
    on starve → listening, interrupt rules, transcription,
    resumption/goAway, credential routing.
  - Results: iOS `SinuaVoiceTests` 31 + 2 skipped; Android JVM 13 new.
- Glue end-to-end against an **in-process fake Gemini server** on
  localhost (iOS `Network.framework`, Android `mockwebserver` 4.12.0),
  with the real sockets and a fake audio clock. The test checks the auth
  header, setup, mic frames, a PCM turn driving the states and metrics,
  and `goAway` → a new socket carrying the resumption handle, then
  disconnect. Also: a rejected setup reports and returns to idle, and the
  credential/permission guards.
  - Results: iOS `SinuaGeminiLiveTests` 3/3; Android
    `:sinua-gemini` 3/3 (3 forced reruns stable).
- `AVPcmAudioDevice` on the simulator (engine start, clock, scheduling,
  reset) has an **opt-in** smoke test
  (`TEST_RUNNER_SINUA_AUDIO_SMOKE=1`). A simulator uses the Mac's real
  mic and speakers, so it's skipped by default and schedules only silence.
  It passed 3/3 when run.
- `AndroidPcmAudioDevice` has **not run on any device or emulator**.
- **Not verified live:** the real API, and real echo cancellation / audio
  quality on a phone.

### Native ElevenLabs: `SinuaElevenLabs` (iOS) / `:sinua-elevenlabs` (Android)

Built 2026-09-19. It mirrors the Web `ElevenLabsVoiceSource` over the raw
Agents WebSocket:
- the `convai` subprotocol;
- `conversation_initiation_client_data` (+ `conversation_config_override`);
- the graph starts only after `conversation_initiation_metadata`, at the
  **negotiated** rates (PCM, or μ-law output; the input must be PCM);
- `user_audio_chunk` up; `ping` → `pong` with the same `event_id`;
- `vad_score` > 0.5 → listening, falling → thinking; `user_transcript` →
  thinking;
- the playback gate for thinking / speaking / listening, and a 4 s
  thinking guard;
- `interruption` clears playback and drops late chunks with a lower
  `event_id`; a barge-in only if there was output to cut;
- no reconnect: close 1000 means the agent ended the conversation.

**Research** (2026-09-19):
- ElevenLabs' AsyncAPI WebSocket reference still documents audio (formats
  `pcm_8000…48000`, `ulaw_8000`), with no deprecation.
- `agent_response_complete` was new since the Web adapter. Since 2026-09-20 all three
  platforms use it (with `audio_event.is_final`) as the turn-end signal; see the state
  table in *Built vendor adapters*.
- **The official native SDKs now carry voice over LiveKit:**
  - `elevenlabs-swift-sdk` v3.3.1 depends on `client-sdk-swift`
    (`wss://livekit.rtc.elevenlabs.io`; its WebSocket is "text-only");
  - `elevenlabs-android` v0.12.2 depends on livekit-android + OkHttp.
- They weren't wrapped:
  - that would make every ElevenLabs user carry WebRTC;
  - the Android SDK keeps its `Room` private (callbacks only), so there's
    no agent audio to analyse;
  - behaviour would diverge from Web.
- A thin bridge to an app's own official-SDK `Conversation` is possible on
  iOS only (it exposes `agentAudioTrack` + `agentState`). It isn't built.

**Shared socket layer** (a refactor of 2a):
- `LiveSocket` now lives in `SinuaVoice` (iOS, Foundation only), and
  Android's OkHttp implementation is the new **`:sinua-websocket`**
  module (OkHttp 4.12.0; `api` from both vendor modules, since its
  `ClosedException` reaches apps through `onError`). So an app with Gemini
  + ElevenLabs gets one OkHttp.
- Both implementations take **subprotocols**.

**Order (all native vendor sources, after studio-ui-ux's report):** socket
→ setup / metadata → mic permission → audio. A bad or expired credential
fails without a permission prompt and without opening the mic (Gemini was
changed to this; LiveKit already worked this way). Gemini holds model audio
that arrives between `setupComplete` and the audio start.

**Verified** (fakes only; no mic, no sound):
- **Core:** iOS `ElevenLabsCoreTests` 11; Android `ElevenLabsCoreTest` 10:
  - formats and μ-law against the G.711 table;
  - messages and endpoint;
  - ping/pong;
  - pending audio flushed on graph start;
  - VAD / transcript / playback states and the 4 s guard;
  - interruption with the `event_id` drop rule;
  - a μ-law turn;
  - malformed frames.
- **Glue against an in-process fake server:** iOS `SinuaElevenLabsTests`
  5 (Network.framework, negotiating `convai`); Android
  `:sinua-elevenlabs` 3 (mockwebserver). They check:
  - the `convai` offer, agent id and overrides;
  - the graph at the negotiated rates;
  - mic frames and pong;
  - a PCM turn driving the states and metrics;
  - interruption + a dropped late chunk;
  - close 1000 → idle with no reconnect;
  - a non-PCM input and a bad agent both fail with no prompt and no audio
    started;
  - permission denied after the metadata never starts audio.
- **Gemini's order is covered too:** iOS 4/4, Android 4/4.
- **Not verified live:** that needs a public agent id or a signed URL.

### Native OpenAI Realtime: `SinuaOpenAI` (iOS) / `:sinua-openai` (Android)

Built 2026-09-19. It mirrors the Web `OpenAIRealtimeVoiceSource`:
- an ephemeral `ek_` (from your backend; a raw key mints one on the device,
  **dev only**, the same rule and wording as Web);
- SDP offer → `POST /v1/realtime/calls` (`Authorization: Bearer ek_…`,
  `Content-Type: application/sdp`), then the answer;
- events on the `oai-events` data channel; the model's audio as a remote
  track.

The state rules and the energy fallback are ported from Web (see the next
section), as is reconnect: a new session with a fresh credential from
`credentialProvider`, 3 attempts with LiveKit's backoff, the finalized
transcript replayed as `conversation.item.create`, and fatal codes/statuses
giving up at once.

**Transport: WebRTC.** `developers.openai.com/api/docs/guides/realtime`
recommends WebRTC for client devices and WebSocket "when connecting from a
trusted server"; ephemeral client secrets are "for browser or mobile
clients". A WebSocket from the device would have reused the PCM stack
(2a/2b), but it's off OpenAI's recommended path, so it wasn't done.

**WebRTC dependency decision: LiveKit's prefixed builds.** iOS uses
`livekit/webrtc-xcframework` (resolved 150.7871.2, the version
`client-sdk-swift` 2.17.0 pins exactly); Android uses
`io.github.webrtc-sdk:android-prefixed` 144.7559.14 (livekit-android
2.28.2's).
- Why:
  - they're maintained on current Chromium milestones (Google publishes no
    official mobile binaries);
  - the symbols are prefixed (`LKRTC…` / `livekit.org.webrtc`), so they
    can't clash with another WebRTC copy;
  - an app that also uses LiveKit, or ElevenLabs' official SDKs that ride
    LiveKit, resolves to the **same** binary instead of two.
- Cost: a large binary for OpenAI users, isolated in its own package/module
  (`packages/ios-openai`, like `ios-livekit`; `:sinua-openai`).
- Coupling:
  - iOS says `from: "150.7871.02"` (up to the next major). A future LiveKit
    major that moves its exact pin needs a bump here.
  - Gradle picks the highest version, so Android is fine.
- Rejected: community Google builds (unprefixed, clash-prone).

**How it's built:**
- WebRTC's own audio device does the capture (with echo cancellation) and
  the playback.
- The remote track is tapped (`LKRTCAudioTrack.add(renderer)` /
  `AudioTrack.addSink`) into `PcmTap`, the mic's spectrum path.
- Order: credential first, then mic permission (iOS prompt / Android
  RECORD_AUDIO check), then the call. A bad credential never prompts.
- Logic lives in the voice packages and is unit-tested:
  - `OpenAIRealtimeSession`: events → states, energy fallback, transcript,
    fatal codes;
  - `RealtimeReconnect` + `TranscriptLog`;
  - `OpenAIRealtimeSignaling`: request shape, status → fatal / retryable.

**Verified:**
- Core: iOS `OpenAIRealtimeCoreTests` 10 (incl. the HTTP path through a
  `URLProtocol` stub); Android `OpenAIRealtimeCoreTest` 8.
- Android `OpenAIHttpTest` 3: real OkHttp against `mockwebserver`, checking
  the exact `application/sdp` content type, the body and the answer, 401
  fatal / 503 retryable, and the dev `client_secrets` POST.
- The glue compiles against the real WebRTC binaries: `xcodebuild build
  -scheme SinuaOpenAI` (iOS Simulator) and `:sinua-openai:assembleDebug`.
- **The peer connection itself was never run.** A WebRTC call opens the real
  microphone and speakers (on the simulator/emulator, the Mac's), and the iOS
  build has no silent audio device. So SDP negotiation, the data channel, the
  renderer tap, reconnect and the echo canceller are compile-verified only.
  **Not verified live**: that needs an `ek_` (or a dev key) and ideally a
  device.

**Failure order (all native vendor sources: LiveKit, Gemini, ElevenLabs, OpenAI):** on every failure path the source tears down first, so `onStateChange` reports `idle`, and only *then* throws from `connect()` (iOS) or calls `onError` (Android). A consumer that stops listening on `idle` loses the error; keep the error handler attached until `connect()` has returned or `onError` has fired. (Raised by studio-ui-ux when wiring the native Studios.)

### Native vendor transports: the original plan (all four now built)

The same rules as Web apply: one adapter per transport shape, vendor state
signals preferred over energy, the playback-timeline gate, and credentials
minted by the integrator's backend (*Credentials for a real integration*).
LiveKit, Gemini Live, ElevenLabs and OpenAI Realtime are all built (above); this list is kept for the reasoning.

1. *(built: ElevenLabs, see above; kept for the reasoning)* **PCM over WebSocket**:
   `URLSessionWebSocketTask` / OkHttp, plus a playback player
   (`AVAudioEngine` + `AVAudioPlayerNode` / `AudioTrack`).
   - The analyser has to be fed the samples at **playback** time, i.e. the
     player's render tap, not socket receipt. That's the rule
     `PcmAudioGraph` follows on Web.
   - Protocol and state logic port from `GeminiLiveVoiceSource` /
     `ElevenLabsVoiceSource`.
2. **OpenAI Realtime** (WebRTC): needs a native WebRTC stack
   (`WebRTC.xcframework` / `org.webrtc`, heavy). Most native integrators
   are better served by OpenAI's own SDKs or a LiveKit agent in front of
   it, so it's lowest priority. The alternative is the Realtime WebSocket
   transport with the PCM approach above.

## Roadmap

- Four adapters exist: `OpenAIRealtimeVoiceSource` (OpenAI Realtime),
  `GeminiLiveVoiceSource` (Gemini Live) and `ElevenLabsVoiceSource`
  (ElevenLabs Conversational AI) — the latter two on the shared
  `PcmAudioGraph` — plus `LiveKitVoiceSource` for an existing LiveKit
  Room's agent (no credential handled by this library) — see *Built
  vendor adapters*. Still open: Azure OpenAI (WebRTC shape, a thin
  subclass) and Amazon Nova Sonic (PCM shape over an HTTP/2 event stream
  rather than a WebSocket — `PcmAudioGraph` already covers the audio
  side; the socket layer would be new). None has been verified against a
  live session yet.
- `OpenAIRealtimeVoiceSource` reconnects a dropped call. It opens a new session
  with a fresh credential and replays the transcript (see *Reconnect*).
  That's tested against fakes only; a real drop is part of the pending
  live tests.
- iOS/Android: native mic + test tone + `VoiceOverrides` are built (`SinuaVoice`, `dev.sinua.voice`) with byte-exact parity against Chrome/Web (see *Native*), and so are the native LiveKit source (`SinuaLiveKit`, `:sinua-livekit`, see *Native LiveKit*) native Gemini Live (`SinuaGeminiLive`, `:sinua-gemini`) and native ElevenLabs (`SinuaElevenLabs`, `:sinua-elevenlabs`), see their sections; and native OpenAI Realtime (`SinuaOpenAI`, `:sinua-openai`, WebRTC). None is verified live. Still open: the real-device mic check and live checks for all four (OpenAI's peer connection has never run at all; see its section).
- Barge-in (`onInterrupt` → `interruptAge`) is wired on all four vendor
  adapters (see *Barge-in* above). LiveKit's version is inferred and
  unverified live, like the rest.
- (Superseded: the `LiveKitVoiceSource` this bullet used to propose is
  built, against `livekit-client` directly rather than the React hooks.
  See *Built vendor adapters*.)
