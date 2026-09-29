// Many small views at once (roadmap 10, "compact mode"): `?grid=50&px=32&seconds=5`.
// Web only and informational -- not a spec/bench case, since the three bench apps
// share one case list and result schema. It answers one question, before and after
// a view change: what do N small views cost the page per second?
//
//   draws/s    how many view frames were painted, all views together
//   workMs/s   their compute + paint time per second (onFrame), all views together
//   frame p95  the page's own rAF interval, from a separate observer loop
//   dropped    display intervals the observer missed (metrics.ts's rule)
//   loaf       long animation frames, where the browser reports them
import { mount } from "@sinua/web";
import { pct, summarize } from "./metrics";

export interface GridResult {
  views: number;
  px: number;
  state: string;
  seconds: number;
  refreshHz: number;
  drawsPerS: number;
  workMsPerS: number;
  frameMs: ReturnType<typeof pct>;
  droppedFrames: number;
  longAnimationFrames: number | null;
}

const nextFrame = () => new Promise<number>((res) => requestAnimationFrame(res));

export async function runGrid(views: number, px: number, seconds: number, state: string, hz: number, host: HTMLElement): Promise<GridResult> {
  const wrap = document.createElement("div");
  wrap.style.cssText = `display:grid;grid-template-columns:repeat(auto-fill,${px}px);gap:4px;max-width:640px`;
  host.append(wrap);
  let measuring = false;
  let draws = 0;
  let work = 0;
  const handles = Array.from({ length: views }, () => {
    const c = document.createElement("canvas");
    c.style.cssText = `width:${px}px;height:${px}px;margin:0`;
    wrap.append(c);
    return mount(c, {
      state,
      reducedMotion: "never",
      onFrame: (s) => {
        if (!measuring) return;
        draws++;
        work += s.computeMs + s.paintMs;
      },
    });
  });
  const loafOk =
    typeof PerformanceObserver !== "undefined" && PerformanceObserver.supportedEntryTypes?.includes("long-animation-frame");
  let loafs = 0;
  const obs = loafOk ? new PerformanceObserver((l) => { if (measuring) loafs += l.getEntries().length; }) : null;
  obs?.observe({ type: "long-animation-frame" });

  await new Promise((res) => setTimeout(res, 1000)); // warmup
  const dts: number[] = [];
  measuring = true;
  const t0 = performance.now();
  let last = await nextFrame();
  while (last - t0 < seconds * 1000) {
    const now = await nextFrame();
    dts.push(now - last);
    last = now;
  }
  measuring = false;
  const durationS = (performance.now() - t0) / 1000;
  obs?.disconnect();
  handles.forEach((h) => h.destroy());
  wrap.remove();
  const s = summarize(dts, [], [], durationS, hz, null);
  return {
    views,
    px,
    state,
    seconds: Math.round(durationS * 1000) / 1000,
    refreshHz: hz,
    drawsPerS: Math.round(draws / durationS),
    workMsPerS: Math.round((work / durationS) * 100) / 100,
    frameMs: s.frameMs,
    droppedFrames: s.droppedFrames,
    longAnimationFrames: loafOk ? loafs : null,
  };
}
