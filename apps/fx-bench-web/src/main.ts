// sinua bench, Web (docs/bench.md). Every case in spec/bench/cases.json
// runs through @sinua/web's mount() twice -- lowPower off, then on --
// for warmup + measure seconds; the per-frame onFrame stats become one
// result per spec/bench/result.schema.json. `?seconds=3&cases=a,b&auto=1`.
import { estimateCost, type OrbState } from "@sinua/core";
import { DEFAULT_LOW_POWER, mount } from "@sinua/web";
import benchJson from "../../../spec/bench/cases.json";
import { summarize } from "./metrics";

type Power = "normal" | "low";
interface BenchCase { id: string; label: string; state: string; overrides: Record<string, number> }
const bench = benchJson as unknown as { version: number; size: number; warmupSeconds: number; measureSeconds: number; cases: BenchCase[] };
const q = new URLSearchParams(location.search);
const measureS = Number(q.get("seconds") ?? bench.measureSeconds);
const warmupS = bench.warmupSeconds;
const only = q.get("cases")?.split(",");
const cases = bench.cases.filter((c) => !only || only.includes(c.id));

const $ = (id: string) => document.getElementById(id)!;
const canvas = $("stage") as HTMLCanvasElement;
const status = (s: string) => ($("status").textContent = s);
const nextFrame = () => new Promise<number>((res) => requestAnimationFrame(res));

/** The display's refresh rate: the median rAF interval over half a second. */
async function refreshHz(): Promise<number> {
  const ts: number[] = [];
  const t0 = await nextFrame();
  let t = t0;
  while (t - t0 < 500) ts.push((t = await nextFrame()));
  const d = ts.map((x, i) => x - (i ? ts[i - 1] : t0)).sort((a, b) => a - b);
  return Math.round(1000 / d[d.length >> 1]);
}

const loafSupported = typeof PerformanceObserver !== "undefined" && PerformanceObserver.supportedEntryTypes?.includes("long-animation-frame");

async function runCase(c: BenchCase, power: Power, hz: number) {
  const dts: number[] = [];
  const computes: number[] = [];
  const paints: number[] = [];
  let measuring = false;
  let loafs = 0;
  const obs = loafSupported
    ? new PerformanceObserver((l) => { if (measuring) loafs += l.getEntries().length; })
    : null;
  obs?.observe({ type: "long-animation-frame" });
  const handle = mount(canvas, {
    state: c.state,
    overrides: c.overrides,
    lowPower: power === "low",
    reducedMotion: "never",
    theme: "auto",
    onFrame: (s) => {
      if (!measuring) return;
      dts.push(s.dtMs);
      computes.push(s.computeMs);
      paints.push(s.paintMs);
    },
  });
  await new Promise((res) => setTimeout(res, warmupS * 1000));
  measuring = true;
  const t0 = performance.now();
  await new Promise((res) => setTimeout(res, measureS * 1000));
  measuring = false;
  const durationS = (performance.now() - t0) / 1000;
  handle.destroy();
  obs?.disconnect();
  const effective = power === "low" ? { ...c.overrides, ...DEFAULT_LOW_POWER.overrides } : c.overrides;
  const cost = estimateCost(c.state as OrbState, bench.size as 64, effective);
  return {
    id: c.id,
    power,
    ...summarize(dts, computes, paints, durationS, hz, power === "low" ? DEFAULT_LOW_POWER.maxFps : null),
    cpuPct: null,
    platformJankFrames: null,
    longAnimationFrames: loafSupported ? loafs : null,
    cost: cost && { class: cost.class, elements: cost.elements, coverage: round(cost.coverage), blurLoad: round(cost.blurLoad) },
  };
}

const round = (x: number) => Math.round(x * 1000) / 1000;

async function run() {
  ($("run") as HTMLButtonElement).disabled = true;
  status("measuring refresh rate…");
  const hz = await refreshHz();
  const result = {
    schemaVersion: 1,
    platform: "web",
    device: {
      model: navigator.platform || "browser",
      os: navigator.userAgent,
      isSimulator: !/Android|iPhone|iPad/.test(navigator.userAgent),
      refreshHz: hz,
      userAgent: navigator.userAgent,
    },
    startedAt: new Date().toISOString(),
    measureSeconds: measureS,
    casesVersion: bench.version,
    cases: [] as Awaited<ReturnType<typeof runCase>>[],
  };
  let k = 0;
  const total = cases.length * 2;
  for (const c of cases) {
    for (const power of ["normal", "low"] as Power[]) {
      status(`${++k}/${total} · ${c.label} · ${power}`);
      result.cases.push(await runCase(c, power, hz));
    }
  }
  (window as unknown as { __benchResult: unknown }).__benchResult = result;
  status(`done · ${hz} Hz · ${result.device.isSimulator ? "desktop browser: not device numbers" : "device"}`);
  renderTable(result.cases);
  const dl = $("download") as HTMLButtonElement;
  dl.disabled = false;
  dl.onclick = () => {
    const a = document.createElement("a");
    a.href = URL.createObjectURL(new Blob([JSON.stringify(result, null, 2)], { type: "application/json" }));
    a.download = `bench-web-${result.startedAt.replace(/[:.]/g, "-")}.json`;
    a.click();
  };
  ($("run") as HTMLButtonElement).disabled = false;
}

function renderTable(rows: Awaited<ReturnType<typeof runCase>>[]) {
  const head = "<tr><th>case</th><th>power</th><th>fps</th><th>frame p95</th><th>compute p95</th><th>paint p95</th><th>dropped</th><th>hitch ms/s</th><th>cost</th></tr>";
  const body = rows
    .map((r) => `<tr><td>${r.id}</td><td>${r.power}</td><td>${r.fps.toFixed(1)}</td><td>${r.frameMs.p95.toFixed(1)}</td><td>${r.computeMs.p95.toFixed(2)}</td><td>${r.paintMs.p95.toFixed(2)}</td><td>${r.droppedFrames}</td><td>${r.hitchRatioMsPerS.toFixed(1)}</td><td>${r.cost?.class ?? "-"}</td></tr>`)
    .join("");
  $("table").innerHTML = `<table>${head}${body}</table>`;
}

$("run").addEventListener("click", run);
if (q.get("auto") === "1") run();
