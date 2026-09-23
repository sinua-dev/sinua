// Browser audio / network fakes for node: no device, no socket, no sound.
// install() replaces the globals the sources touch; restore() puts them back.

const param = (value = 0) => ({ value, setValueAtTime() {}, linearRampToValueAtTime() {}, cancelScheduledValues() {} });
const node = (extra = {}) => ({ connect() {}, disconnect() {}, ...extra });

export const audio = { byte: 0, time: 0, contexts: [] };

export class FakeAudioContext {
  constructor(opts = {}) {
    this.sampleRate = opts.sampleRate ?? 48000;
    this.state = "running";
    this.destination = node();
    this.audioWorklet = { addModule: async () => {} };
    audio.contexts.push(this);
  }
  get currentTime() {
    return audio.time;
  }
  async resume() {}
  async close() {
    this.state = "closed";
  }
  createGain() {
    return node({ gain: param(1) });
  }
  createOscillator() {
    return node({ type: "sine", frequency: param(440), start() {}, stop() {} });
  }
  createAnalyser() {
    return node({
      fftSize: 2048,
      smoothingTimeConstant: 0.8,
      get frequencyBinCount() {
        return this.fftSize / 2;
      },
      getByteFrequencyData(arr) {
        arr.fill(audio.byte);
      },
      getFloatTimeDomainData(arr) {
        arr.fill(audio.byte / 255);
      },
    });
  }
  createMediaStreamSource() {
    return node();
  }
  createBuffer(_channels, length, rate) {
    const data = new Float32Array(length);
    return { duration: length / rate, getChannelData: () => data };
  }
  createBufferSource() {
    return node({ buffer: null, start() {}, stop() {}, onended: null });
  }
}

export class FakeAudioWorkletNode {
  static last = null;
  constructor() {
    this.port = { onmessage: null };
    FakeAudioWorkletNode.last = this;
  }
  connect() {}
  disconnect() {}
}

export class FakeWebSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;
  static instances = [];
  constructor(url, protocols) {
    this.url = url;
    this.protocols = protocols;
    this.readyState = 0;
    this.sent = [];
    FakeWebSocket.instances.push(this);
  }
  open() {
    this.readyState = 1;
    this.onopen?.({});
  }
  receive(obj) {
    this.onmessage?.({ data: JSON.stringify(obj) });
  }
  send(text) {
    this.sent.push(JSON.parse(text));
  }
  close(code = 1000, reason = "") {
    if (this.readyState === 3) return;
    this.readyState = 3;
    this.onclose?.({ code, reason });
  }
}

export class FakeRTCPeerConnection {
  static last = null;
  constructor() {
    this.connectionState = "new";
    this.tracks = [];
    FakeRTCPeerConnection.last = this;
  }
  addTrack(t) {
    this.tracks.push(t);
  }
  createDataChannel(label) {
    this.dc = { label, readyState: "connecting", sent: [], send(t) { this.sent.push(JSON.parse(t)); }, close() {} };
    return this.dc;
  }
  async createOffer() {
    return { type: "offer", sdp: "v=0 fake-offer" };
  }
  async setLocalDescription(d) {
    this.local = d;
  }
  async setRemoteDescription(d) {
    this.remote = d;
    queueMicrotask(() => {
      this.dc.readyState = "open";
      this.dc.onopen?.();
    });
  }
  getReceivers() {
    return [];
  }
  close() {
    this.connectionState = "closed";
  }
}

export const mic = { requests: 0, stopped: 0 };
const track = () => ({ kind: "audio", enabled: true, readyState: "live", stop: () => mic.stopped++ });
export const stream = () => {
  const t = track();
  return { getAudioTracks: () => [t], getTracks: () => [t] };
};

const defaultRespond = async () => ({ ok: true, status: 200, text: async () => "v=0 fake-answer", json: async () => ({}) });
export const net = { requests: [], respond: defaultRespond };

/**
 * Just enough `document` for the one DOM call the adapters make: a remote
 * WebRTC track needs an `<audio>` sink or Chrome won't pump it through Web
 * Audio at all, so `attachRemote` creates one. Without this, firing `ontrack`
 * on the fake peer connection throws and that whole path stays untestable.
 */
export const media = { elements: [] };
const audioElement = () => {
  const el = { tagName: "AUDIO", autoplay: false, srcObject: null, plays: 0, paused: false };
  el.play = async () => {
    el.plays++;
    el.paused = false;
  };
  el.pause = () => {
    el.paused = true;
  };
  media.elements.push(el);
  return el;
};

const saved = {};
const KEYS = ["AudioContext", "AudioWorkletNode", "WebSocket", "RTCPeerConnection", "MediaStream", "fetch", "navigator", "document"];

export function install() {
  for (const k of KEYS) saved[k] = Object.getOwnPropertyDescriptor(globalThis, k);
  const set = (k, value) => Object.defineProperty(globalThis, k, { value, configurable: true, writable: true });
  set("AudioContext", FakeAudioContext);
  set("AudioWorkletNode", FakeAudioWorkletNode);
  set("WebSocket", FakeWebSocket);
  set("RTCPeerConnection", FakeRTCPeerConnection);
  set("MediaStream", class { getAudioTracks() { return []; } getTracks() { return []; } });
  set("fetch", async (url, init) => {
    net.requests.push({ url: String(url), init });
    return net.respond(url, init);
  });
  set("document", {
    createElement: (tag) => {
      if (String(tag).toLowerCase() !== "audio") throw new Error(`fake document: no <${tag}>`);
      return audioElement();
    },
  });
  set("navigator", {
    mediaDevices: {
      getUserMedia: async () => {
        mic.requests++;
        return stream();
      },
    },
  });
  audio.byte = 0;
  audio.time = 0;
  audio.contexts = [];
  FakeWebSocket.instances = [];
  // Reset the request/mic counters and the responder too, so a test can assert
  // an absolute count ("no fetch happened") without depending on what ran
  // before it in the file.
  net.requests = [];
  net.respond = defaultRespond;
  mic.requests = 0;
  mic.stopped = 0;
  media.elements = [];
}

export function restore() {
  for (const k of KEYS) {
    if (saved[k]) Object.defineProperty(globalThis, k, saved[k]);
    else delete globalThis[k];
  }
}

export const tick = (ms) => new Promise((r) => setTimeout(r, ms));
/** Waits until `fn()` is truthy (polling microtasks + short timers). */
export async function until(fn, ms = 1000) {
  const end = Date.now() + ms;
  while (!fn()) {
    if (Date.now() > end) throw new Error("until: timed out");
    await tick(2);
  }
}
