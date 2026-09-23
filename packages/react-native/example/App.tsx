/**
 * sinua React Native smoke test.
 *
 * Calls the classic native module bridge (@sinua/react-native ->
 * ios/SinuaCore.swift or android/.../SinuaCoreModule.kt -> the same
 * core_engine binary packages/ios and packages/android already validate)
 * and checks the result against a golden vector from
 * spec/orbs-golden.json (working-64, t=0.6), then exercises
 * frameWithOverrides (the Studio's live-tweaking entry point, now reachable
 * from RN too), then a handful of spec/sinua-golden.json checkpoints
 * (sinuaCheckpoints.json: a polyline-heavy, a dot-heavy, a coloured
 * additive-orb and an input-driven case) through the same bridge -- the
 * part of RN the iOS/Android package tests don't cover is exactly this
 * JS <-> native-module serialization, polylines and colour included --
 * and finally an FX Spec round trip (runFxSpecCheck: resolve, render,
 * a diagnosed typo, a color conversion; docs/fx-spec.md).
 * Logs to the console (visible via `npx react-native log-ios` /
 * `adb logcat`) and renders on screen.
 *
 * @format
 */

import { useEffect, useState } from 'react';
import { SafeAreaView, StyleSheet, Text, View } from 'react-native';
import {
  SinuaView,
  frame,
  frameFromFxSpec,
  frameWithOverrides,
  fxColorToHsl,
  resolveFxSpec,
  type OrbFrame,
  type OrbState,
} from '@sinua/react-native';
import sinuaCheckpoints from './sinuaCheckpoints.json';
import { runSinuaCheckpoints, runFxSpecCheck } from './sinuaCheck';
import { runVoiceSelfTest } from './voiceSelfTest';

const EXPECTED_DOT_COUNT = 516;
const EXPECTED_FIRST_X = 32.34438;
const EXPECTED_FIRST_Y = 30.683937;

// Swift's `[String: Any]` (ios/SinuaCore.swift's dictionary(from:)) does
// not preserve key insertion order the way a JS object literal or Rust's
// serde_json (field-declaration order) does -- two structurally identical
// OrbFrames can bridge to JS objects with their keys enumerated in a
// different order, so a plain `JSON.stringify(a) === JSON.stringify(b)`
// is not a valid equality check on this platform. Compare by value instead.
function framesEqual(a: OrbFrame | null, b: OrbFrame | null): boolean {
  if (!a || !b) return a === b;
  if (a.dots.length !== b.dots.length || a.lines.length !== b.lines.length) return false;
  return (
    a.dots.every((d, i) => {
      const o = b.dots[i];
      return d.x === o.x && d.y === o.y && d.z === o.z && d.r === o.r && d.white === o.white && d.a === o.a;
    }) &&
    a.lines.every((l, i) => {
      const o = b.lines[i];
      return (
        l.x1 === o.x1 &&
        l.y1 === o.y1 &&
        l.x2 === o.x2 &&
        l.y2 === o.y2 &&
        l.white === o.white &&
        l.a === o.a &&
        l.w === o.w
      );
    })
  );
}

async function runSmokeTest(): Promise<string> {
  const stock = await frame('working', 64, 0.6);
  if (!stock) return 'FAIL: frame() resolved to null';
  const dx = Math.abs(stock.dots[0].x - EXPECTED_FIRST_X);
  const dy = Math.abs(stock.dots[0].y - EXPECTED_FIRST_Y);
  if (stock.dots.length !== EXPECTED_DOT_COUNT || dx >= 1e-4 || dy >= 1e-4) {
    return (
      `FAIL: dots=${stock.dots.length} (expected ${EXPECTED_DOT_COUNT}), ` +
      `first=(${stock.dots[0]?.x}, ${stock.dots[0]?.y}) (expected (${EXPECTED_FIRST_X}, ${EXPECTED_FIRST_Y}))`
    );
  }

  const empty = await frameWithOverrides('searching', 64, 0.6, {});
  const overridden = await frameWithOverrides('searching', 64, 0.6, { scanMul: 8.0 });
  const stockSearching = await frame('searching', 64, 0.6);
  if (!empty || !overridden || !stockSearching) return 'FAIL: frameWithOverrides resolved to null';
  if (!framesEqual(empty, stockSearching)) {
    return 'FAIL: empty overrides did not reproduce the stock frame';
  }
  if (framesEqual(overridden, stockSearching)) {
    return 'FAIL: scanMul override did not change the rendered frame';
  }

  const sinuaErr = await runSinuaCheckpoints(sinuaCheckpoints, (state, size, t, overrides) =>
    frameWithOverrides(state as OrbState, size as 20 | 32 | 64, t, overrides),
  );
  if (sinuaErr) return `FAIL (sinua golden): ${sinuaErr}`;

  const fxErr = await runFxSpecCheck({
    resolveFxSpec,
    frameFromFxSpec,
    fxColorToHsl,
    frameWithOverrides: (state: string, size: number, t: number, overrides: Record<string, number>) =>
      frameWithOverrides(state as OrbState, size as 20 | 32 | 64, t, overrides),
  });
  if (fxErr) return `FAIL (FX Spec): ${fxErr}`;

  return 'PASS: frame() matches golden vectors, frameWithOverrides sanity holds, sinua checkpoints match, FX Spec resolves';
}

// <SinuaView> (the Fabric component over the native SinuaViews). Views 1-3 use
// reducedMotion="always" -- SinuaView's static t = 0.6 frame -- so screenshots can
// be compared against the same frame painted by @sinua/web's drawFrame;
// view 4 animates and reports onFrame, and is paused from JS after 4 s (a prop
// update through Fabric's updateProps, without a remount).
export const FXVIEW_SPEC = {
  fxSpec: '1.8',
  object: 'orb',
  pattern: 'working',
  size: 64,
  color: { value: '#6E56CF', mix: 0.7 },
  materials: { glow: { strength: 0.25, radius: 3 } },
};

function SinuaViewSection() {
  const [frames, setFrames] = useState(0);
  const [paused, setPaused] = useState(false);
  useEffect(() => {
    const id = setTimeout(() => setPaused(true), 4000);
    return () => clearTimeout(id);
  }, []);
  return (
    <View style={styles.grid}>
      <SinuaView testID="fx-spec-light" style={styles.fx} spec={FXVIEW_SPEC} theme="light" reducedMotion="always" />
      <SinuaView testID="fx-state-overrides" style={styles.fx} pattern="tracking" overrides={{ progress0: 0.7, progress1: 0.4 }} theme="light" reducedMotion="always" />
      <SinuaView testID="fx-spec-dark" style={[styles.fx, styles.dark]} spec={FXVIEW_SPEC} theme="dark" reducedMotion="always" />
      <View style={styles.fx}>
        <SinuaView testID="fx-live" style={StyleSheet.absoluteFill} pattern="speaking" theme="light" paused={paused} onFrame={() => setFrames((n) => n + 1)} />
        <Text style={styles.counter}>{`onFrame ${frames}${paused ? ' · paused' : ''}`}</Text>
      </View>
    </View>
  );
}

function App() {
  const [status, setStatus] = useState('running…');
  const [voiceStatus, setVoiceStatus] = useState<string[]>(['voice: running…']);
  const [result, setResult] = useState<OrbFrame | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    frame('working', 64, 0.6)
      .then(setResult)
      .catch(() => {});
    runSmokeTest()
      .then((msg) => {
        setStatus(msg);
        console.log('[sinua smoke test]', msg);
      })
      .catch((e) => {
        setError(String(e));
        setStatus('ERROR');
        console.error('[sinua smoke test] error', e);
      });
    // Native voice sources (silent test tone + error paths; no mic, no network).
    runVoiceSelfTest()
      .then((lines) => {
        setVoiceStatus(lines);
        for (const line of lines) console.log('[sinua voice]', line);
      })
      .catch((e) => {
        setVoiceStatus([`ERROR ${String(e)}`]);
        console.error('[sinua voice] error', e);
      });
  }, []);

  return (
    <SafeAreaView style={styles.container}>
      <View style={styles.card}>
        <Text style={styles.title}>sinua / @sinua/react-native</Text>
        <Text style={styles.status}>{status}</Text>
        {result && (
          <Text style={styles.detail}>
            dots={result.dots.length} lines={result.lines.length}
          </Text>
        )}
        {error && <Text style={styles.error}>{error}</Text>}
        {voiceStatus.map((line) => (
          <Text key={line} style={styles.status}>{line}</Text>
        ))}
      </View>
      <SinuaViewSection />
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, alignItems: 'center', justifyContent: 'center', backgroundColor: '#f4f4f4' },
  card: { padding: 24, borderRadius: 12, backgroundColor: '#fff', alignItems: 'center', gap: 8 },
  title: { fontWeight: '600', fontSize: 15 },
  status: { fontSize: 13, textAlign: 'center' },
  detail: { fontSize: 12, color: '#888' },
  error: { fontSize: 12, color: '#c0392b' },
  grid: { flexDirection: 'row', flexWrap: 'wrap', width: 336, gap: 16, marginTop: 16 },
  fx: { width: 160, height: 160, backgroundColor: '#ffffff' },
  dark: { backgroundColor: '#16171a' },
  counter: { position: 'absolute', bottom: 2, left: 4, fontSize: 10, color: '#888' },
});

export default App;
