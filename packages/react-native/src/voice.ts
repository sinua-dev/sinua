// Native voice sources for React Native (docs/fx-view.md, *React Native*):
// the same sources iOS and Android ship (LiveKit, OpenAI Realtime, Gemini Live,
// ElevenLabs, the device mic, a test tone), created natively and bound to a view.
//
// The app owns the source, as on Web and native: it connects it (from a user
// action -- the mic prompt and audio need one), reads its state, may share it
// between views, and releases it. A view only binds what it is given.
//
// Credentials live in the native source's memory only: they are passed once to
// `create`, never stored, never logged, and never part of a view's props. For
// OpenAI, pass `getCredential` instead -- it's called again for every reconnect,
// which an `ek_` (single-use) needs.
import { NativeEventEmitter, NativeModules } from "react-native";

/** The agent's lifecycle, as every sinua voice source reports it (docs/audio-pipeline.md). */
export type AgentState = "initializing" | "idle" | "listening" | "thinking" | "speaking";

/** Vendor SDKs are opt-in per platform; an unavailable vendor rejects `connect()` with how to add it. */
export type VoiceSourceConfig =
  | { vendor: "test" }
  | { vendor: "mic" }
  /** A LiveKit room: the access token comes from your backend. */
  | { vendor: "livekit"; url: string; token: string; publishMicrophone?: boolean }
  /** OpenAI Realtime: an `ek_…` from your backend. `getCredential` is asked again on every reconnect. */
  | {
      vendor: "openai";
      credential?: string;
      getCredential?: () => Promise<string>;
      model?: string;
      voice?: string;
      instructions?: string;
      /**
       * Opt in to a raw, long-lived API key. Without it `connect()` refuses one
       * before the mic or the call. Local demos only; ship an `ek_…` minted by
       * your backend instead.
       */
      allowInsecureApiKey?: boolean;
    }
  /** Gemini Live: an ephemeral `auth_tokens/…` from your backend. */
  | {
      vendor: "gemini";
      credential: string;
      model?: string;
      instructions?: string;
      endpoint?: string;
      /** See the OpenAI entry: opt in to a raw, long-lived API key. Local demos only. */
      allowInsecureApiKey?: boolean;
    }
  /** ElevenLabs: a public agent's id, or a signed `wss://` URL from your backend. */
  | { vendor: "elevenlabs"; credential: string; endpoint?: string };

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
}

interface VoiceNativeModule {
  create(config: Record<string, unknown>): Promise<string>;
  connect(id: string): Promise<void>;
  disconnect(id: string): void;
  release(id: string): void;
  provideCredential(requestId: string, credential: string | null, error: string | null): void;
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
  event: "state" | "error" | "interrupt" | "credentialRequest";
  state?: AgentState;
  message?: string;
  requestId?: string;
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
      for (const l of listeners) if (l.id === e.id && l.event === e.event) (l.cb as (p: unknown) => void)(e.state ?? e.message);
    });
  }
  const listener: Listener = { id, event, cb };
  listeners.push(listener);
  return () => {
    const i = listeners.indexOf(listener);
    if (i >= 0) listeners.splice(i, 1);
  };
}

/** `getCredential` per source, kept in JS: the native side asks, and the answer goes straight to it. */
const providers = new Map<string, () => Promise<string>>();

async function credentialRequest(id: string, requestId: string): Promise<void> {
  const provider = providers.get(id);
  if (!provider) return nativeOrThrow().provideCredential(requestId, null, "no getCredential for this source");
  try {
    nativeOrThrow().provideCredential(requestId, await provider(), null);
  } catch (err) {
    nativeOrThrow().provideCredential(requestId, null, err instanceof Error ? err.message : String(err));
  }
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
  const { getCredential, ...rest } = config as VoiceSourceConfig & { getCredential?: () => Promise<string> };
  const id = `${config.vendor}-${(idCounter += 1)}-${Date.now().toString(36)}`;
  if (getCredential) providers.set(id, getCredential);
  const created = nativeOrThrow()
    .create({ ...rest, id, hasCredentialProvider: getCredential != null })
    .then(() => undefined);
  created.catch(() => undefined); // surfaced by connect(); an unhandled rejection here would be noise
  let released = false;
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
