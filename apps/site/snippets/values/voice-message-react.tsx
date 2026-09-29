import { useState } from "react";
import { SinuaVoiceMessage } from "@sinua/web/react";

// A voice message in a chat bubble: the waveform, the played part in full ink, and a
// drag or tap anywhere on it to seek. Your app plays the audio; Sinua draws it.
export function VoiceMessage({ peaks, audio }: { peaks: number[]; audio: HTMLAudioElement }) {
  const [progress, setProgress] = useState(0);
  audio.ontimeupdate = () => setProgress(audio.currentTime / audio.duration);
  return (
    <SinuaVoiceMessage
      envelope={peaks} // up to 64 loudness values, 0..1
      progress={progress}
      onSeek={(p) => (audio.currentTime = p * audio.duration)}
      style={{ width: 220, height: 40 }}
    />
  );
}

// Sinua decodes no audio. One way to get the peaks in a browser:
export async function peaksOf(url: string, count = 48): Promise<number[]> {
  const data = await (await fetch(url)).arrayBuffer();
  const samples = (await new AudioContext().decodeAudioData(data)).getChannelData(0);
  const step = Math.floor(samples.length / count);
  const peaks = Array.from({ length: count }, (_, i) => {
    let max = 0;
    for (let j = i * step; j < (i + 1) * step; j++) max = Math.max(max, Math.abs(samples[j]));
    return max;
  });
  const top = Math.max(...peaks) || 1;
  return peaks.map((p) => p / top);
}
