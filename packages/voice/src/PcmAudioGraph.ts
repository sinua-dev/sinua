import { AudioAnalysis } from "./analysis.js";
import type { VoiceMetrics } from "@sinua/core";

/**
 * The Web Audio half every PCM-over-socket adapter shares (`GeminiLiveVoiceSource`
 * for Gemini Live, `ElevenLabsVoiceSource`): mic capture at a vendor's input
 * rate, gap-free scheduled playback of decoded chunks at the vendor's output
 * rate, and the analyser that feeds `AudioAnalysis` -- hung off the *playback*
 * gain node, so `{level, bands}` can only ever describe what is audible right
 * now, never what merely arrived. Extracted from the Gemini adapter once a
 * second vendor needed the identical graph; the transport/state layers stay
 * per-adapter because those genuinely differ.
 *
 * Capture: `AudioContext({sampleRate: inputRate})` resamples the mic for us;
 * an `AudioWorklet` loaded from a Blob URL (Google's `audio-recorder.ts`
 * trick -- no separate asset to serve) posts fixed-size Float32 chunks; the
 * Int16/base64 encoding stays in `pcm.ts` where it is tested.
 *
 * Playback: each decoded chunk becomes an `AudioBufferSourceNode` started at a
 * running cursor (`max(cursor, now + 0.1 s)` at the start of a burst, then
 * `cursor += duration` -- Google's `audio-streamer.ts` scheduling). A burst is
 * "audible" once the playback clock passes `burstStart`, "queued" before that,
 * "drained" once it passes the cursor -- the gate docs/audio-pipeline.md
 * requires: `speaking` follows the playback timeline, not chunk receipt.
 */
export type PlaybackState = "queued" | "audible" | "drained";

export interface PcmAudioGraphStartOptions {
  /** An already-acquired mic stream (request it first so the permission prompt precedes any network work). */
  mic: MediaStream;
  inputRate: number;
  outputRate: number;
  /** Samples per posted mic chunk. Default 1024 (~64 ms at 16 kHz -- between Google's 128 ms and LiveKit's 50 ms). */
  chunkSamples?: number;
  onChunk: (samples: Float32Array) => void;
}

const DEFAULT_CHUNK_SAMPLES = 1024;
const INITIAL_BUFFER_S = 0.1; // lead before a burst starts playing (Google's `initialBufferTime`)
const INTERRUPT_FADE_S = 0.03; // short gain ramp before cutting sources, so a barge-in doesn't click
const WORKLET_NAME = "sinua-pcm-capture";

function workletSource(chunkSamples: number): string {
  // Runs on the audio thread: accumulate mono Float32 samples, post a chunk.
  return `
class PcmCapture extends AudioWorkletProcessor {
  constructor() {
    super();
    this.buf = new Float32Array(${chunkSamples});
    this.n = 0;
  }
  process(inputs) {
    const ch = inputs[0] && inputs[0][0];
    if (!ch) return true;
    for (let i = 0; i < ch.length; i++) {
      this.buf[this.n++] = ch[i];
      if (this.n === this.buf.length) {
        const chunk = this.buf.slice(0);
        this.port.postMessage(chunk, [chunk.buffer]);
        this.n = 0;
      }
    }
    return true;
  }
}
registerProcessor(${JSON.stringify(WORKLET_NAME)}, PcmCapture);
`;
}

export class PcmAudioGraph {
  private mic: MediaStream | null = null;
  private captureCtx: AudioContext | null = null;
  private captureSource: MediaStreamAudioSourceNode | null = null;
  private captureNode: AudioWorkletNode | null = null;
  private workletUrl: string | null = null;

  private playbackCtx: AudioContext | null = null;
  private gain: GainNode | null = null;
  private analysis: AudioAnalysis | null = null;
  private scheduled = new Set<AudioBufferSourceNode>();
  private cursor = 0;
  private burstStart = 0;

  /** `getUserMedia({audio: true})`, separated so adapters can prompt before opening a socket. */
  static requestMic(): Promise<MediaStream> {
    return navigator.mediaDevices.getUserMedia({ audio: true });
  }

  async start(opts: PcmAudioGraphStartOptions): Promise<void> {
    this.mic = opts.mic;

    const captureCtx = new AudioContext({ sampleRate: opts.inputRate });
    this.captureCtx = captureCtx;
    await captureCtx.resume().catch(() => undefined);
    this.workletUrl = URL.createObjectURL(
      new Blob([workletSource(opts.chunkSamples ?? DEFAULT_CHUNK_SAMPLES)], { type: "application/javascript" })
    );
    await captureCtx.audioWorklet.addModule(this.workletUrl);
    const captureNode = new AudioWorkletNode(captureCtx, WORKLET_NAME, {
      numberOfInputs: 1,
      numberOfOutputs: 1,
      channelCount: 1,
    });
    captureNode.port.onmessage = (e: MessageEvent<Float32Array>) => opts.onChunk(e.data);
    const captureSource = captureCtx.createMediaStreamSource(opts.mic);
    captureSource.connect(captureNode);
    // Zero-gain path to the destination, same reason as TestToneVoiceSource:
    // some browsers only keep a graph processing when it reaches the sink.
    const silent = captureCtx.createGain();
    silent.gain.value = 0;
    captureNode.connect(silent);
    silent.connect(captureCtx.destination);
    this.captureSource = captureSource;
    this.captureNode = captureNode;

    const playbackCtx = new AudioContext({ sampleRate: opts.outputRate });
    this.playbackCtx = playbackCtx;
    await playbackCtx.resume().catch(() => undefined);
    const gain = playbackCtx.createGain();
    const analyser = playbackCtx.createAnalyser();
    analyser.fftSize = 512; // see LocalMicVoiceSource for why not 256
    analyser.smoothingTimeConstant = 0; // AudioAnalysis does its own attack/release
    gain.connect(playbackCtx.destination);
    gain.connect(analyser);
    this.gain = gain;
    this.analysis = new AudioAnalysis(analyser);
  }

  /** Schedule decoded samples (any rate -- the buffer carries it, Web Audio resamples). */
  enqueue(samples: Float32Array, rate: number): void {
    const ctx = this.playbackCtx;
    if (!ctx || !this.gain || samples.length === 0) return;
    const buffer = ctx.createBuffer(1, samples.length, rate);
    // `.set` rather than `copyToChannel`: the latter's TS 5.7 signature
    // wants `Float32Array<ArrayBuffer>` specifically, which a DataView-
    // backed decode can't promise; `.set` takes any ArrayLike<number>.
    buffer.getChannelData(0).set(samples);
    const src = ctx.createBufferSource();
    src.buffer = buffer;
    src.connect(this.gain);
    const now = ctx.currentTime;
    if (this.cursor < now) {
      // New burst: a small lead so the following chunks are already queued
      // by the time this one ends, instead of a gap after every first chunk.
      this.cursor = now + INITIAL_BUFFER_S;
      this.burstStart = this.cursor;
    }
    src.start(this.cursor);
    this.cursor += buffer.duration;
    this.scheduled.add(src);
    src.onended = () => {
      this.scheduled.delete(src);
      src.disconnect();
    };
  }

  playbackState(): PlaybackState {
    const ctx = this.playbackCtx;
    if (!ctx || this.cursor === 0) return "drained";
    const now = ctx.currentTime;
    if (now >= this.cursor) return "drained";
    return now >= this.burstStart ? "audible" : "queued";
  }

  /** Stop everything scheduled and reset the cursor; `fade` ramps gain down first (a hard cut clicks). */
  clearPlayback(fade: boolean): void {
    const ctx = this.playbackCtx;
    if (ctx && this.gain && fade && this.scheduled.size > 0) {
      // Google's `audio-streamer.ts` ramps gain down before dropping the
      // queue for the same reason.
      const now = ctx.currentTime;
      const g = this.gain.gain;
      g.cancelScheduledValues(now);
      g.setValueAtTime(g.value, now);
      g.linearRampToValueAtTime(0, now + INTERRUPT_FADE_S);
      g.setValueAtTime(1, now + INTERRUPT_FADE_S + 0.005);
      for (const s of this.scheduled) {
        try {
          s.stop(now + INTERRUPT_FADE_S);
        } catch {
          /* not started or already stopped */
        }
      }
    } else {
      for (const s of this.scheduled) {
        try {
          s.stop();
        } catch {
          /* not started or already stopped */
        }
        s.disconnect();
      }
    }
    this.scheduled.clear();
    this.cursor = 0;
    this.burstStart = 0;
  }

  /** Current `{level, bands}` of what is playing, or `null` before `start()`. */
  read(): VoiceMetrics | null {
    return this.analysis ? this.analysis.read() : null;
  }

  stop(): void {
    if (this.captureNode) {
      this.captureNode.port.onmessage = null;
      this.captureNode.disconnect();
    }
    this.captureNode = null;
    this.captureSource?.disconnect();
    this.captureSource = null;
    this.mic?.getTracks().forEach((t) => t.stop());
    this.mic = null;
    void this.captureCtx?.close().catch(() => undefined);
    this.captureCtx = null;
    if (this.workletUrl) URL.revokeObjectURL(this.workletUrl);
    this.workletUrl = null;
    this.clearPlayback(false);
    this.gain?.disconnect();
    this.gain = null;
    this.analysis = null;
    void this.playbackCtx?.close().catch(() => undefined);
    this.playbackCtx = null;
  }
}
