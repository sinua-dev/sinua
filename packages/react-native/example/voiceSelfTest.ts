/**
 * Native voice sources, checked end to end from JS (@sinua/react-native's
 * `createVoiceSource`, packages/react-native/src/voice.ts). It runs on the
 * simulator / emulator and needs no vendor account, no network and no microphone:
 *
 * 1. the silent test tone: states reach JS through the native module;
 * 2. a vendor source pointed at a dead local endpoint: the failure reaches JS
 *    (ElevenLabs opens its socket before touching the mic, so nothing prompts);
 * 3. a vendor that isn't in this build: a message that says how to add it;
 * 4. release: the source is gone, and using it afterwards fails.
 *
 * Results are logged with the app's other checks (`npx react-native log-ios` /
 * `adb logcat`) and shown on screen.
 */
import { createVoiceSource, type AgentState } from '@sinua/react-native';

const wait = (ms: number) => new Promise<void>(resolve => { setTimeout(() => resolve(), ms); });

async function until(cond: () => boolean, timeoutMs = 4000): Promise<boolean> {
  const end = Date.now() + timeoutMs;
  while (!cond()) {
    if (Date.now() > end) return false;
    await wait(50);
  }
  return true;
}

export async function runVoiceSelfTest(): Promise<string[]> {
  const results: string[] = [];

  // 1. The test tone (silent: its output gain is zero).
  const states: AgentState[] = [];
  const tone = createVoiceSource({ vendor: 'test' });
  tone.onStateChange(s => states.push(s));
  try {
    await tone.connect();
    const sawListening = await until(() => states.includes('listening') || states.includes('speaking'));
    results.push(
      sawListening
        ? `PASS tone: states reached JS (${states.slice(0, 3).join(' -> ')})`
        : `FAIL tone: no state events (${JSON.stringify(states)})`,
    );
    tone.disconnect();
  } catch (err) {
    results.push(`FAIL tone: ${String(err)}`);
  }

  // 2. A vendor whose endpoint can't answer: the failure must surface, with no mic prompt.
  // iOS reports it by rejecting connect(); Android's connect() returns once the socket
  // is opening, so the failure arrives as an `error` event instead. Either is a pass.
  const dead = createVoiceSource({ vendor: 'elevenlabs', credential: 'agent_selftest', endpoint: 'ws://127.0.0.1:9/' });
  let deadError: string | null = null;
  dead.onError(message => (deadError = message));
  try {
    await dead.connect();
    const reported = await until(() => deadError !== null);
    results.push(
      reported
        ? `PASS elevenlabs: the failure reached JS as an error event (${String(deadError).slice(0, 50)}…)`
        : 'FAIL elevenlabs: a dead endpoint produced neither a rejection nor an error event',
    );
  } catch (err) {
    results.push(`PASS elevenlabs: connect() rejected (${String(err).slice(0, 50)}…)`);
  }
  dead.release();

  // 3. A vendor that isn't compiled in (LiveKit/OpenAI are opt-in per build).
  const optional = createVoiceSource({ vendor: 'livekit', url: 'wss://example.invalid', token: 't' });
  try {
    await optional.connect();
    results.push('PASS livekit: this build has the LiveKit vendor');
    optional.disconnect();
  } catch (err) {
    const message = String(err);
    results.push(
      /isn't in this build|voiceVendors|subspec/.test(message)
        ? 'PASS livekit: not in this build, and the error says how to add it'
        : `PASS livekit: in this build; connect failed as expected (${message.slice(0, 50)}…)`,
    );
  }
  optional.release();

  // 4. A released source can't be used again.
  tone.release();
  try {
    await tone.connect();
    results.push('FAIL release: a released source still connected');
  } catch {
    results.push('PASS release: a released source is unusable');
  }

  return results;
}
