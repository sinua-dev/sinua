// @sinua/voice -- Web VoiceSources for SinuaView (docs/audio-pipeline.md).
// Import a vendor from its own subpath so an app only pulls the SDK it uses:
//   @sinua/voice/livekit  /openai  /gemini  /elevenlabs  /mic  /tone
// This root holds the shared, SDK-free pieces.
export type { AgentState, TranscriptTiming, TranscriptUpdate, VoiceMetrics, VoiceSource } from "@sinua/core";
// The vendor-free transcript rules every source uses (./transcript.ts, design note 39):
// for your own VoiceSource, feed it your vendor's fragments and level ticks.
export {
  TranscriptAssembler,
  TRANSCRIPT_AUDIBLE_LEVEL,
  TRANSCRIPT_CUT_GRACE_MS,
  TRANSCRIPT_REVEAL_CHARS_PER_SECOND,
  TRANSCRIPT_SEGMENT_DELAY_MS,
  TRANSCRIPT_NEW_UTTERANCE_GAP_MS,
  TRANSCRIPT_USER_CLOSE_CHARS,
  TRANSCRIPT_USER_SILENCE_MS,
} from "./transcript.js";
export { AudioAnalysis } from "./analysis.js";
export { PcmAudioGraph } from "./PcmAudioGraph.js";
export type { PlaybackState, PcmAudioGraphStartOptions } from "./PcmAudioGraph.js";
// The credential contract every vendor source takes (./credential.ts).
export type { CredentialOptions, CredentialProvider, SinuaCredential } from "./credential.js";
