// The WebRTC native library (sinua-openai, sinua-livekit) reaches its Java side by name through
// JNI: JNI_OnLoad finds livekit.org.jni_zero.JniInit, native code calls back into livekit.org.webrtc.
// If an app's R8 strips or renames one of those classes, the first PeerConnectionFactory call
// aborts natively (no Java exception). 0.1.0-beta.9 shipped sinua-openai without the consumer
// rules that prevent it; this check keeps them honest without a device.
//
// It reads a minified release APK built with no app rules (apps/android-minify-smoke by default),
// takes every JNI-style class name (a/b/C) out of the WebRTC .so, keeps the ones that are the app's
// own classes (in R8's mapping.txt or usage.txt), and fails unless each one survived under its own
// name. The list follows a WebRTC upgrade by itself.
//   node scripts/android-r8-jni-check.mjs [app.apk mapping.txt usage.txt]
//   (after: cd packages/android && ./gradlew :android-minify-smoke:assembleRelease)
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const out = join(dirname(fileURLToPath(import.meta.url)), "../apps/android-minify-smoke/build/outputs");
const [apk, mappingPath, usagePath] = process.argv.length > 4 ? process.argv.slice(2, 5) : [
  join(out, "apk/release/android-minify-smoke-release.apk"),
  join(out, "mapping/release/mapping.txt"),
  join(out, "mapping/release/usage.txt"),
];

const entries = execFileSync("unzip", ["-Z1", apk], { encoding: "utf8" }).split("\n");
const so = entries.find((e) => /^lib\/(arm64-v8a|x86_64)\/lib(lk)?jingle_peerconnection_so\.so$/.test(e));
if (!so) throw new Error(`android-r8-jni-check: no WebRTC .so in ${apk} (does the app depend on sinua-openai?)`);
const bin = execFileSync("unzip", ["-p", apk, so], { maxBuffer: 1 << 30 }).toString("latin1");
// Every JNI-style class name in the binary; only the app's own classes are judged below.
const fromSo = new Set([...bin.matchAll(/[a-z][a-z0-9_]*(?:\/[A-Za-z0-9_$]+){2,}/g)].map((m) => m[0].replace(/\//g, ".")));

const kept = new Map(); // original -> name after R8, for the classes R8 kept
for (const m of readFileSync(mappingPath, "utf8").matchAll(/^(\S+) -> (\S+):$/gm)) kept.set(m[1], m[2]);
const removed = new Set([...readFileSync(usagePath, "utf8").matchAll(/^(\S+)$/gm)].map((m) => m[1]));

const ok = [], renamed = [], gone = [];
for (const c of [...fromSo].sort()) {
  if (kept.has(c)) (kept.get(c) === c ? ok : renamed).push(kept.get(c) === c ? c : `${c} -> ${kept.get(c)}`);
  else if (removed.has(c)) gone.push(c);
}
const total = ok.length + renamed.length + gone.length;
if (total === 0) throw new Error(`android-r8-jni-check: no app class named in ${so}; the scan is broken`);
console.log(`android-r8-jni-check: ${total} classes the WebRTC .so reaches by name; ${ok.length} kept, ${renamed.length} renamed, ${gone.length} removed`);
if (renamed.length + gone.length) {
  for (const [k, list] of [["renamed", renamed], ["removed", gone]]) if (list.length) console.error(`  ${k}: ${list.join(", ")}`);
  console.error("android-r8-jni-check: FAILED; the voice modules' consumer rules no longer cover WebRTC's JNI classes (packages/android/openai/consumer-rules.pro)");
  process.exit(1);
}
