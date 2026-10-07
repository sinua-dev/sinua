// Native voice sources for React Native (docs/fx-view.md, *React Native*):
// the same sources iOS and Android ship (LiveKit, OpenAI Realtime, Gemini Live,
// ElevenLabs, the device mic, a test tone), created natively and bound to a view.
//
// The app owns the source, as on Web and native: it connects it (from a user
// action -- the mic prompt and audio need one), reads its state, may share it
// between views, and releases it. A view only binds what it is given.
//
// Credentials: the one contract every platform shares (docs/audio-pipeline.md,
// *Credentials for a real integration*). Pass `credentialUrl` (your endpoint,
// answering `{ credential, expiresAt?, url? }`) or a `credential` provider: both are
// resolved here in JS and handed to the native source on every connect and
// reconnect, so an expired or single-use credential is never reused. A plain
// string is one fixed value. Credentials are never stored, never logged, and never
// part of a view's props. A raw API key is refused by the native source.
import { NativeEventEmitter, NativeModules } from "react-native";

/** The agent's lifecycle, as every sinua voice source reports it (docs/audio-pipeline.md). */
export type AgentState = "initializing" | "idle" | "listening" | "thinking" | "speaking";

/**
 * One transcript update (docs/audio-pipeline.md, *Transcripts*): the turn's text so far, the
 * same shape as `@sinua/core`'s. Display only -- Sinua keeps nothing beyond the current turn
 * and sends it nowhere.
 */
export interface TranscriptUpdate {
  role: "user" | "assistant";
  /** Everything visible so far in this turn (cumulative, never a diff). */
  text: string;
  /** True exactly once per turn, on its last update. */
  final: boolean;
  /** Stable for the whole turn: role + a counter that never resets for the source ("u3", "a4"). */
  turnId: string;
  /** The assistant turn was cut by a barge-in; `text` is the full spoken part. */
  truncated?: boolean;
  /** The vendor's own timing, when it has one. */
  startMs?: number;
  endMs?: number;
}

/** How a source times its transcript: per character, per fragment, already in step, or revealed with the speech. */
export type TranscriptTiming = "chars" | "segments" | "synced" | "none";

/** The vendors that send transcripts here, and how they time them (the others: `supportsTranscript` false). */
const TRANSCRIPT_TIMING: Partial<Record<string, TranscriptTiming>> = {
  simulated: "synced",
  openai: "none",
  gemini: "none",
  elevenlabs: "chars",
  livekit: "synced",
};

/** What your credential endpoint answers, the same JSON on every platform and vendor. */
export interface SinuaCredential {
  /** `ek_…` (OpenAI), `auth_tokens/…` (Gemini), a signed `wss://` URL (ElevenLabs) or a room JWT (LiveKit). */
  credential: string;
  /** When it stops working, in Unix seconds, if the vendor says. */
  expiresAt?: number;
  /** LiveKit only: the server URL the token is for. */
  url?: string;
}

/** Returns a fresh credential; called on every connect and reconnect. A string counts as `{ credential }`. */
export type CredentialProvider = () => Promise<SinuaCredential | string>;

/** How a vendor source gets its credential. */
export interface CredentialOptions {
  /** A provider (the production shape), or one fixed value (a pasted ephemeral, or ElevenLabs' public agent id). */
  credential?: string | CredentialProvider;
  /** Your endpoint: POSTed (no body, `Cache-Control: no-store`) on every connect and reconnect; answers `SinuaCredential`. */
  credentialUrl?: string;
}

/** Vendor SDKs are opt-in per platform; an unavailable vendor rejects `connect()` with how to add it. */
export type VoiceSourceConfig =
  | { vendor: "test" }
  | { vendor: "mic" }
  /**
   * A simulated conversation (no audio, no mic, no network): a built-in sample
   * (`calendar`, `quick-answer`, `long-answer`, `barge-in`) or your own script
   * (docs/audio-pipeline.md, *Simulated conversations*). Plays and loops on `connect()`.
   */
  | { vendor: "simulated"; sample?: string; script?: string | Record<string, unknown>; loop?: boolean }
  /** A LiveKit room: the access token (and `url`) come from your backend. */
  | { vendor: "livekit"; url: string; token: string; publishMicrophone?: boolean }
  | ({ vendor: "livekit"; publishMicrophone?: boolean } & CredentialOptions)
  /** OpenAI Realtime: an `ek_…` from your backend; the model, voice and instructions are set where it's minted. */
  | ({
      vendor: "openai";
      /** @deprecated Pass the same function as `credential`. */
      getCredential?: () => Promise<string>;
    } & CredentialOptions)
  /** Gemini Live: an ephemeral `auth_tokens/…` from your backend, which locks the model, voice and instructions. */
  | ({
      vendor: "gemini";
      model?: string;
      /** @deprecated Set instructions where your backend mints the token (it locks them). */
      instructions?: string;
      endpoint?: string;
    } & CredentialOptions)
  /** ElevenLabs: a public agent's id, or a signed `wss://` URL from your backend. */
  | ({ vendor: "elevenlabs"; endpoint?: string } & CredentialOptions);

export interface VoiceSourceHandle {
  /** Opaque native id; `<SinuaView voice={handle}>` passes it across. */
  readonly id: string;
  readonly vendor: VoiceSourceConfig["vendor"];
  /** Call from a user action. Rejects if the vendor isn't installed, the credential is missing, or setup fails. */
  connect(): Promise<void>;
  disconnect(): void;
  /** Disconnects and drops the native source. The handle is unusable afterwards. */
  release(): void;
  /** The agent's lifecycle state, as the views use it. */
  onStateChange(cb: (state: AgentState) => void): () => void;
  /**
   * Failures after the connection was started. On iOS these also reject `connect()`;
   * on Android `connect()` returns as soon as the socket is opening, so the failure
   * arrives here. Handle both.
   */
  onError(cb: (message: string) => void): () => void;
  /** The user talked over the agent (where the vendor signals it). */
  onInterrupt(cb: () => void): () => void;
  /**
   * Mutes or unmutes the microphone: silence goes out and the session stays up. Views
   * bound to this source show the muted cue (docs/audio-pipeline.md, *Mute*).
   */
  setMuted(muted: boolean): void;
  /** The last `setMuted` value the native side confirmed. */
  readonly muted: boolean;
  /** `true` once the session is up, `false` when it ends (your disconnect, a hang-up, a drop). */
  onConnectionChange(cb: (connected: boolean) => void): () => void;
  onMuteChange(cb: (muted: boolean) => void): () => void;
  /**
   * Live transcript updates for both speakers (docs/audio-pipeline.md, *Transcripts*). Works
   * before `connect()`, so the first turn isn't missed. Only fires where `supportsTranscript`.
   */
  onTranscript(cb: (u: TranscriptUpdate) => void): () => void;
  /** Whether this vendor sends transcripts on React Native. */
  readonly supportsTranscript: boolean;
  /** How this vendor times its transcript (`none` when it sends none). */
  readonly transcriptTiming: TranscriptTiming;
}

interface VoiceNativeModule {
  create(config: Record<string, unknown>): Promise<string>;
  connect(id: string): Promise<void>;
  disconnect(id: string): void;
  setMuted(id: string, muted: boolean): void;
  release(id: string): void;
  provideCredential(requestId: string, credential: string | null, url: string | null, error: string | null, fatal: boolean): void;
}

const LINKING_ERROR =
  "@sinua/react-native: the native module SinuaVoice isn't linked. Rebuild the app (pod install / gradle sync) after adding the package.";

// Looked up per call, not at import: the module registers during startup.
const nativeOrThrow = (): VoiceNativeModule => {
  const m = NativeModules.SinuaVoice as VoiceNativeModule | undefined;
  if (!m) throw new Error(LINKING_ERROR);
  return m;
};

/** One `sinua-voice` event from the native module. */
interface VoiceEvent {
  id: string;
  event: "state" | "error" | "interrupt" | "connection" | "mute" | "transcript" | "credentialRequest";
  state?: AgentState;
  message?: string;
  connected?: boolean;
  muted?: boolean;
  requestId?: string;
  // "transcript": the update's fields, flat.
  role?: "user" | "assistant";
  text?: string;
  final?: boolean;
  turnId?: string;
  truncated?: boolean;
  startMs?: number;
  endMs?: number;
}

/** A "transcript" event as the update apps get (optional fields only when set). */
function transcriptOf(e: VoiceEvent): TranscriptUpdate {
  const u: TranscriptUpdate = { role: e.role ?? "assistant", text: e.text ?? "", final: e.final === true, turnId: e.turnId ?? "" };
  if (e.truncated) u.truncated = true;
  if (typeof e.startMs === "number") u.startMs = e.startMs;
  if (typeof e.endMs === "number") u.endMs = e.endMs;
  return u;
}

type Listener = { id: string; event: string; cb: (payload: never) => void };
const listeners: Listener[] = [];
let emitter: NativeEventEmitter | null = null;

function subscribe(id: string, event: string, cb: (payload: never) => void): () => void {
  if (!emitter) {
    emitter = new NativeEventEmitter(NativeModules.SinuaVoice);
    emitter.addListener("sinua-voice", (raw) => {
      const e = raw as VoiceEvent;
      if (e.event === "credentialRequest") return void credentialRequest(e.id, e.requestId ?? "");
      const payload = e.event === "transcript" ? transcriptOf(e) : (e.state ?? e.message ?? e.connected ?? e.muted);
      for (const l of [...listeners]) if (l.id === e.id && l.event === e.event) (l.cb as (p: unknown) => void)(payload);
    });
  }
  const listener: Listener = { id, event, cb };
  listeners.push(listener);
  return () => {
    const i = listeners.indexOf(listener);
    if (i >= 0) listeners.splice(i, 1);
  };
}

/** A `credential` provider or `credentialUrl` per source, kept in JS: the native side asks, the answer goes straight to it. */
const providers = new Map<string, () => Promise<SinuaCredential>>();

/** Thrown for anything a retry can't fix: a 4xx, or an answer that isn't `SinuaCredential`. */
class FatalCredentialError extends Error {}

async function credentialRequest(id: string, requestId: string): Promise<void> {
  const provider = providers.get(id);
  if (!provider) return nativeOrThrow().provideCredential(requestId, null, null, "no credential provider for this source", true);
  try {
    const c = await provider();
    nativeOrThrow().provideCredential(requestId, c.credential, c.url ?? null, null, false);
  } catch (err) {
    nativeOrThrow().provideCredential(requestId, null, null, err instanceof Error ? err.message : String(err), err instanceof FatalCredentialError);
  }
}

/** Validates a provider's or an endpoint's answer (the same rules as `@sinua/voice`). Never echoes a value. */
export function parseCredential(raw: unknown): SinuaCredential {
  if (typeof raw === "string") {
    if (!raw.trim()) throw new FatalCredentialError("a credential is required");
    return { credential: raw.trim() };
  }
  const o = (raw ?? {}) as Record<string, unknown>;
  if (typeof raw !== "object" || typeof o.credential !== "string" || !o.credential.trim()) {
    throw new FatalCredentialError(`expected { credential: string, expiresAt?, url? }, got keys [${Object.keys(o).join(", ")}]`);
  }
  if (o.expiresAt !== undefined && typeof o.expiresAt !== "number") throw new FatalCredentialError("`expiresAt` must be Unix seconds (a number)");
  if (o.url !== undefined && typeof o.url !== "string") throw new FatalCredentialError("`url` must be a string");
  return {
    credential: o.credential.trim(),
    ...(o.expiresAt !== undefined ? { expiresAt: o.expiresAt as number } : {}),
    ...(o.url !== undefined ? { url: o.url as string } : {}),
  };
}

function urlProvider(url: string): () => Promise<SinuaCredential> {
  return async () => {
    // React Native's fetch has no `cache` option; the header asks every cache on the way to skip it.
    const res = await fetch(url, { method: "POST", headers: { Accept: "application/json", "Cache-Control": "no-store" } });
    if (!res.ok) {
      const retry = res.status === 408 || res.status === 425 || res.status === 429 || res.status >= 500;
      const message = `${url} returned ${res.status}`;
      throw retry ? new Error(message) : new FatalCredentialError(message);
    }
    let body: unknown;
    try {
      body = await res.json();
    } catch {
      throw new FatalCredentialError(`${url} did not return JSON`);
    }
    return parseCredential(body);
  };
}

/**
 * Creates a native voice source. Nothing connects, no permission is asked and no
 * network happens until `connect()`.
 *
 * ```ts
 * const voice = createVoiceSource({ vendor: "gemini", credential: tokenFromMyBackend });
 * voice.onStateChange(setAgentState);
 * <SinuaOrb pattern="speaking" voice={voice} />
 * await voice.connect();   // from a button press
 * ```
 */
let idCounter = 0;

export function createVoiceSource(config: VoiceSourceConfig): VoiceSourceHandle {
  const { getCredential, credentialUrl, credential, ...rest } = config as VoiceSourceConfig & {
    getCredential?: () => Promise<string>;
  } & CredentialOptions;
  const id = `${config.vendor}-${(idCounter += 1)}-${Date.now().toString(36)}`;
  // Functions never cross the bridge: providers stay here and answer each request.
  const fn = typeof credential === "function" ? credential : getCredential;
  const provider = credentialUrl ? urlProvider(credentialUrl) : fn ? async () => parseCredential(await fn()) : undefined;
  if (provider) providers.set(id, provider);
  if (config.vendor === "simulated" && typeof (rest as { script?: unknown }).script === "object") {
    // The native side takes the script as JSON text.
    (rest as { script?: unknown }).script = JSON.stringify((rest as { script?: unknown }).script);
  }
  const created = nativeOrThrow()
    .create({
      ...rest,
      ...(typeof credential === "string" && !provider ? { credential } : {}),
      id,
      hasCredentialProvider: provider != null,
    })
    .then(() => undefined);
  created.catch(() => undefined); // surfaced by connect(); an unhandled rejection here would be noise
  let released = false;
  let muted = false;
  subscribe(id, "mute", ((m: boolean) => (muted = m)) as (p: never) => void);
  const handle: VoiceSourceHandle = {
    id,
    vendor: config.vendor,
    async connect() {
      if (released) throw new Error("this voice source was released");
      await created;
      await nativeOrThrow().connect(id);
    },
    disconnect() {
      if (!released) nativeOrThrow().disconnect(id);
    },
    release() {
      if (released) return;
      released = true;
      providers.delete(id);
      for (let i = listeners.length - 1; i >= 0; i--) if (listeners[i].id === id) listeners.splice(i, 1);
      nativeOrThrow().release(id);
    },
    onStateChange: (cb) => subscribe(id, "state", cb),
    onError: (cb) => subscribe(id, "error", cb),
    onInterrupt: (cb) => subscribe(id, "interrupt", () => cb()),
    setMuted(m: boolean) {
      if (!released) nativeOrThrow().setMuted(id, m);
    },
    get muted() {
      return muted;
    },
    onConnectionChange: (cb) => subscribe(id, "connection", cb as (p: never) => void),
    onMuteChange: (cb) => subscribe(id, "mute", cb as (p: never) => void),
    onTranscript: (cb) => subscribe(id, "transcript", cb as (p: never) => void),
    supportsTranscript: TRANSCRIPT_TIMING[config.vendor] !== undefined,
    transcriptTiming: TRANSCRIPT_TIMING[config.vendor] ?? "none",
  };
  return handle;
}

/**
 * A view's `voice` prop -> the native component's props: a handle binds by id
 * through the registry, the shorthands stay strings.
 */
export function voiceProps(voice: "none" | "test" | "mic" | VoiceSourceHandle | undefined): {
  voice: "none" | "test" | "mic" | undefined;
  voiceSourceId: string | undefined;
} {
  if (isVoiceSourceHandle(voice)) return { voice: "none", voiceSourceId: voice.id };
  return { voice, voiceSourceId: undefined };
}

/** True for a handle from `createVoiceSource` (vs the `"test"` / `"mic"` shorthands). */
export function isVoiceSourceHandle(v: unknown): v is VoiceSourceHandle {
  return typeof v === "object" && v !== null && typeof (v as VoiceSourceHandle).id === "string" && typeof (v as VoiceSourceHandle).connect === "function";
}

/** The accessibility props -> the native component's (Codegen has no maps and no optional booleans). */
export function a11yNativeProps(labels: Record<string, string> | undefined, announce: boolean | undefined): {
  labelsJson: string | undefined;
  announce: "auto" | "on" | "off";
} {
  return { labelsJson: labels ? JSON.stringify(labels) : undefined, announce: announce == null ? "auto" : announce ? "on" : "off" };
}
