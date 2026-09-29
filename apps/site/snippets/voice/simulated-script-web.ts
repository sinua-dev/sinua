import { SimulatedVoiceSource, type ConversationScript } from "@sinua/core";

// Your own conversation: the same JSON shape on every platform.
const script: ConversationScript = {
  name: "Booking",
  loop: true,
  turns: [
    { state: "idle", seconds: 1.5 },
    { state: "listening", seconds: 2.5, voice: "user", line: "A table for two at eight?" },
    { state: "thinking", seconds: 1.2 },
    { state: "speaking", seconds: 3, voice: "agent", line: "Booked for eight, by the window." },
    // The user talks over the agent: views show the barge-in cue.
    { state: "listening", seconds: 1.5, voice: "user", bargeIn: true, line: "Make it 8:30." },
  ],
};

export const voice = new SimulatedVoiceSource(script);

// A timeline of your own: play(), pause(), seek(t), loop, time, duration, turns.
export function scrubTo(seconds: number) {
  voice.seek(seconds); // seeking into a barge-in turn never fires the cue
}
