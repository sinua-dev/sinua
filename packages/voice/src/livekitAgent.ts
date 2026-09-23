import type { AgentState } from "@sinua/core";

/**
 * The DOM-free, SDK-free half of `LiveKitVoiceSource`: how a LiveKit agent
 * participant is recognised and how its published state maps onto this
 * project's `AgentState`. Kept apart from the adapter so it can be tested
 * under plain node without `livekit-client` in the graph.
 *
 * Sources (fetched):
 * - `livekit/protocol` `livekit_models.proto`: `ParticipantInfo.Kind
 *   { STANDARD=0, INGRESS=1, EGRESS=2, SIP=3, AGENT=4, … }` -- the numeric
 *   value `livekit-client`'s `ParticipantKind.AGENT` carries.
 * - `livekit/components-js` `packages/core/src/helper/participant-
 *   attributes.ts`: `ParticipantAgentAttributes.AgentState =
 *   'lk.agent.state'`, `PublishOnBehalf = 'lk.publish_on_behalf'`; and
 *   `useVoiceAssistant.ts` / `useAgent.ts`: the primary agent is the
 *   `AGENT`-kind participant *without* `lk.publish_on_behalf`; a worker
 *   participant whose `lk.publish_on_behalf` equals the agent's identity
 *   may be the one actually publishing the audio.
 * - `docs.livekit.io/agents/build/events/`: `lk.agent.state` values are
 *   `initializing | idle | listening | thinking | speaking` -- this
 *   project's `AgentState` vocabulary was adopted from LiveKit in the first
 *   place, so the mapping is identity.
 */
export const AGENT_STATE_ATTRIBUTE = "lk.agent.state";
export const PUBLISH_ON_BEHALF_ATTRIBUTE = "lk.publish_on_behalf";
/** `ParticipantInfo.Kind.AGENT` in livekit/protocol. */
export const PARTICIPANT_KIND_AGENT = 4;

const AGENT_STATES: ReadonlySet<string> = new Set<AgentState>([
  "initializing",
  "idle",
  "listening",
  "thinking",
  "speaking",
]);

export type Attributes = Readonly<Record<string, string>> | undefined;

/** The agent's published lifecycle state, or `null` if absent/unknown. */
export function agentStateFromAttributes(attrs: Attributes): AgentState | null {
  const raw = attrs?.[AGENT_STATE_ATTRIBUTE];
  return raw && AGENT_STATES.has(raw) ? (raw as AgentState) : null;
}

/** components-js's rule: an AGENT-kind participant that is not publishing on someone else's behalf. */
export function isPrimaryAgent(kind: number, attrs: Attributes): boolean {
  return kind === PARTICIPANT_KIND_AGENT && !(attrs && PUBLISH_ON_BEHALF_ATTRIBUTE in attrs);
}

/** A worker participant carrying media for `agentIdentity`. */
export function publishesForAgent(kind: number, attrs: Attributes, agentIdentity: string): boolean {
  return kind === PARTICIPANT_KIND_AGENT && attrs?.[PUBLISH_ON_BEHALF_ATTRIBUTE] === agentIdentity;
}

/**
 * LiveKit gives a frontend no interruption event (docs.livekit.io
 * frontends/build/agent-state: `lk.agent.state` only), so the barge-in
 * moment is inferred: the agent leaves `speaking` for `listening`/`thinking`
 * while the local user is an active speaker (`Participant.isSpeaking`,
 * server-side active-speaker detection). A normal end of turn has the user
 * silent, so it doesn't match. Known false positive: LiveKit's "false
 * interruption" recovery resumes speaking afterwards -- that still flashes.
 */
export function isInferredBargeIn(prev: AgentState, next: AgentState, userSpeaking: boolean): boolean {
  return prev === "speaking" && (next === "listening" || next === "thinking") && userSpeaking;
}

/**
 * The Studio's one credential field carries two values for LiveKit --
 * `"<wss://…> <token>"`, whitespace-separated (a two-field layout is the
 * UI/UX pass's call). Missing parts come back as empty strings; the adapter
 * reports the error, so the factory never throws.
 */
export function parseUrlAndToken(credential: string): { url: string; token: string } {
  const parts = credential.trim().split(/\s+/).filter(Boolean);
  const url = parts.find((p) => /^wss?:\/\//i.test(p)) ?? "";
  const token = parts.find((p) => p !== url) ?? "";
  return { url, token };
}
