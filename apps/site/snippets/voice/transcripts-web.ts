import { SharedVoiceSource } from "@sinua/core";
import { OpenAILiveVoiceSource } from "@sinua/voice/openai";

// Captions under the visual: the text of both speakers, live. Sinua draws nothing here,
// keeps nothing beyond the current turn and sends the text nowhere.
const bubbles = new Map<string, string>();

export const voice = SharedVoiceSource.of(new OpenAILiveVoiceSource({ sessionUrl: "/api/voice/openai-live" }));
export const stop = voice.onTranscript((u) => {
  bubbles.set(u.turnId, u.text); // `text` is the whole turn so far: replace, don't append
  if (u.final) console.log(`${u.role}: ${u.text}${u.truncated ? " (cut off)" : ""}`); // once per turn
});
// Subscribe before connect() and the first turn isn't missed.
void voice.connect();
