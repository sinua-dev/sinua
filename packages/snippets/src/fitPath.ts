/**
 * SVG paths into a character recipe's shape (design note 18): `fitPath` reads any SVG
 * path (relative commands, `H V S T`, arcs), turns arcs into cubics (the engine takes
 * `M L H V C S Q T Z`, no `A`), and scales it into a box of the 200-unit recipe space,
 * keeping its proportions. Text only -- no engine; the Studio's editor and the docs use it.
 */

/** A box in the recipe's 200-unit space: [x0, y0, x1, y1]. */
export type Box = [number, number, number, number];

export interface FitResult {
  /** The fitted path: absolute `M L C Q Z`, one decimal. */
  d: string;
  /** Subpaths after the first: the engine draws them as holes. */
  holes: number;
  /** What was dropped or may not draw as expected. */
  warnings: string[];
}

type Pt = [number, number];
/** One absolute segment: a move, a line, a quadratic or a cubic, or a close. */
type Seg =
  | { k: "M"; p: Pt }
  | { k: "L"; p: Pt }
  | { k: "Q"; c: Pt; p: Pt }
  | { k: "C"; c1: Pt; c2: Pt; p: Pt }
  | { k: "Z" };

/** The engine's limits (docs/fx-spec.md, *Custom shapes*). */
const MAX_COMMANDS = 512;
const MAX_BYTES = 16 * 1024;

const ARGS: Record<string, number> = { M: 2, L: 2, H: 1, V: 1, C: 6, S: 4, Q: 4, T: 2, A: 7, Z: 0 };

function tokens(d: string): Array<string | number> {
  const out: Array<string | number> = [];
  const re = /([MLHVCSQTAZmlhvcsqtaz])|([-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(d))) out.push(m[1] ?? Number(m[2]));
  return out;
}

/** An arc (SVG's endpoint form) as cubics of at most 90° each (SVG 1.1, F.6.5). */
function arc(p0: Pt, rx: number, ry: number, rot: number, large: boolean, sweep: boolean, p: Pt): Seg[] {
  if (rx === 0 || ry === 0) return [{ k: "L", p }];
  if (p0[0] === p[0] && p0[1] === p[1]) return [];
  rx = Math.abs(rx);
  ry = Math.abs(ry);
  const phi = (rot * Math.PI) / 180;
  const [cos, sin] = [Math.cos(phi), Math.sin(phi)];
  const dx = (p0[0] - p[0]) / 2;
  const dy = (p0[1] - p[1]) / 2;
  const x1 = cos * dx + sin * dy;
  const y1 = -sin * dx + cos * dy;
  const lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
  if (lambda > 1) {
    rx *= Math.sqrt(lambda);
    ry *= Math.sqrt(lambda);
  }
  const num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
  const den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
  const f = (large === sweep ? -1 : 1) * Math.sqrt(Math.max(0, num / den));
  const cx1 = (f * rx * y1) / ry;
  const cy1 = (-f * ry * x1) / rx;
  const cx = cos * cx1 - sin * cy1 + (p0[0] + p[0]) / 2;
  const cy = sin * cx1 + cos * cy1 + (p0[1] + p[1]) / 2;
  const angle = (ux: number, uy: number, vx: number, vy: number) => Math.atan2(ux * vy - uy * vx, ux * vx + uy * vy);
  const t1 = angle(1, 0, (x1 - cx1) / rx, (y1 - cy1) / ry);
  let dt = angle((x1 - cx1) / rx, (y1 - cy1) / ry, (-x1 - cx1) / rx, (-y1 - cy1) / ry);
  if (!sweep && dt > 0) dt -= 2 * Math.PI;
  if (sweep && dt < 0) dt += 2 * Math.PI;
  const n = Math.max(1, Math.ceil(Math.abs(dt) / (Math.PI / 2) - 1e-9));
  const step = dt / n;
  const k = (4 / 3) * Math.tan(step / 4);
  const at = (t: number): Pt => [cx + rx * Math.cos(t) * cos - ry * Math.sin(t) * sin, cy + rx * Math.cos(t) * sin + ry * Math.sin(t) * cos];
  const tangent = (t: number): Pt => [-rx * Math.sin(t) * cos - ry * Math.cos(t) * sin, -rx * Math.sin(t) * sin + ry * Math.cos(t) * cos];
  const out: Seg[] = [];
  for (let i = 0; i < n; i++) {
    const a = t1 + i * step;
    const b = a + step;
    const [pa, pb, ta, tb] = [at(a), at(b), tangent(a), tangent(b)];
    out.push({
      k: "C",
      c1: [pa[0] + k * ta[0], pa[1] + k * ta[1]],
      c2: [pb[0] - k * tb[0], pb[1] - k * tb[1]],
      p: i === n - 1 ? p : pb,
    });
  }
  return out;
}

/** Any SVG path as absolute segments (arcs as cubics). Throws on a malformed path. */
function parse(d: string): Seg[] {
  const tk = tokens(d);
  const segs: Seg[] = [];
  let cur: Pt = [0, 0];
  let start: Pt = [0, 0];
  let lastC: Pt | null = null; // the last cubic's second control point (for S)
  let lastQ: Pt | null = null; // the last quadratic's control point (for T)
  let cmd = "";
  let i = 0;
  const num = (): number => {
    const v = tk[i++];
    if (typeof v !== "number") throw new Error(`a number was expected at token ${i}`);
    return v;
  };
  while (i < tk.length) {
    if (typeof tk[i] === "string") cmd = tk[i++] as string;
    else if (!cmd) throw new Error("the path must start with a command (M)");
    const up = cmd.toUpperCase();
    const rel = cmd !== up;
    if (!(up in ARGS)) throw new Error(`unknown command ${cmd}`);
    const pt = (x: number, y: number): Pt => (rel ? [cur[0] + x, cur[1] + y] : [x, y]);
    let c: Pt | null = null;
    let q: Pt | null = null;
    switch (up) {
      case "M": {
        cur = start = pt(num(), num());
        segs.push({ k: "M", p: cur });
        cmd = rel ? "l" : "L"; // further pairs are lines
        break;
      }
      case "L":
        cur = pt(num(), num());
        segs.push({ k: "L", p: cur });
        break;
      case "H": {
        const x = num();
        cur = [rel ? cur[0] + x : x, cur[1]];
        segs.push({ k: "L", p: cur });
        break;
      }
      case "V": {
        const y = num();
        cur = [cur[0], rel ? cur[1] + y : y];
        segs.push({ k: "L", p: cur });
        break;
      }
      case "C": {
        const c1 = pt(num(), num());
        c = pt(num(), num());
        cur = pt(num(), num());
        segs.push({ k: "C", c1, c2: c, p: cur });
        break;
      }
      case "S": {
        const c1: Pt = lastC ? [2 * cur[0] - lastC[0], 2 * cur[1] - lastC[1]] : cur;
        c = pt(num(), num());
        cur = pt(num(), num());
        segs.push({ k: "C", c1, c2: c, p: cur });
        break;
      }
      case "Q": {
        q = pt(num(), num());
        cur = pt(num(), num());
        segs.push({ k: "Q", c: q, p: cur });
        break;
      }
      case "T": {
        q = lastQ ? [2 * cur[0] - lastQ[0], 2 * cur[1] - lastQ[1]] : cur;
        cur = pt(num(), num());
        segs.push({ k: "Q", c: q, p: cur });
        break;
      }
      case "A": {
        const [rx, ry, rot, large, sweep] = [num(), num(), num(), num(), num()];
        const p = pt(num(), num());
        segs.push(...arc(cur, rx, ry, rot, large !== 0, sweep !== 0, p));
        cur = p;
        break;
      }
      case "Z":
        segs.push({ k: "Z" });
        cur = start;
        break;
    }
    lastC = c;
    lastQ = q;
  }
  return segs;
}

/** The path's bounding box (curves sampled), or null for an empty path. */
export function pathBox(d: string): Box | null {
  return boxOf(parse(d));
}

function boxOf(segs: Seg[]): Box | null {
  let b: Box | null = null;
  const add = ([x, y]: Pt) => {
    b = b ? [Math.min(b[0], x), Math.min(b[1], y), Math.max(b[2], x), Math.max(b[3], y)] : [x, y, x, y];
  };
  let cur: Pt = [0, 0];
  let start: Pt = [0, 0];
  for (const s of segs) {
    if (s.k === "Z") {
      cur = start;
      continue;
    }
    if (s.k === "M") start = s.p;
    if (s.k === "C" || s.k === "Q") {
      for (let i = 1; i < 16; i++) {
        const t = i / 16;
        const u = 1 - t;
        if (s.k === "C") {
          add([
            u * u * u * cur[0] + 3 * u * u * t * s.c1[0] + 3 * u * t * t * s.c2[0] + t * t * t * s.p[0],
            u * u * u * cur[1] + 3 * u * u * t * s.c1[1] + 3 * u * t * t * s.c2[1] + t * t * t * s.p[1],
          ]);
        } else {
          add([u * u * cur[0] + 2 * u * t * s.c[0] + t * t * s.p[0], u * u * cur[1] + 2 * u * t * s.c[1] + t * t * s.p[1]]);
        }
      }
    }
    add(s.p);
    cur = s.p;
  }
  return b;
}

/** One decimal, shortest form ("12", "12.5"; never "-0"). */
const r1 = (v: number) => String(Math.round(v * 10) / 10 + 0);

/**
 * Fit an SVG path into `box` (default: the whole 200-unit space, 10 units in), centred,
 * keeping its proportions. Arcs become cubics; relative and shorthand commands become
 * absolute `M L C Q Z`. Throws when the path can't be read; a path over the engine's
 * limits comes back with a warning (the engine would refuse it).
 */
export function fitPath(d: string, box: Box = [10, 10, 190, 190]): FitResult {
  const segs = parse(d);
  const warnings: string[] = [];
  const b = boxOf(segs);
  if (!b) throw new Error("the path is empty");
  const [w, h] = [b[2] - b[0], b[3] - b[1]];
  const s = Math.min((box[2] - box[0]) / (w || 1), (box[3] - box[1]) / (h || 1));
  const ox = (box[0] + box[2]) / 2 - ((b[0] + b[2]) / 2) * s;
  const oy = (box[1] + box[3]) / 2 - ((b[1] + b[3]) / 2) * s;
  const f = ([x, y]: Pt) => `${r1(x * s + ox)} ${r1(y * s + oy)}`;
  const parts: string[] = [];
  let moves = 0;
  for (const g of segs) {
    if (g.k === "M") moves++;
    parts.push(
      g.k === "Z" ? "Z" : g.k === "C" ? `C${f(g.c1)} ${f(g.c2)} ${f(g.p)}` : g.k === "Q" ? `Q${f(g.c)} ${f(g.p)}` : `${g.k}${f(g.p)}`
    );
  }
  const out = parts.join(" ");
  if (parts.length > MAX_COMMANDS) warnings.push(`${parts.length} commands: the engine takes at most ${MAX_COMMANDS}`);
  if (out.length > MAX_BYTES) warnings.push(`${out.length} bytes: the engine takes at most ${MAX_BYTES}`);
  return { d: out, holes: Math.max(0, moves - 1), warnings };
}

/**
 * The `d` of every `<path>` in an SVG file, joined into one path (each its own subpaths),
 * and what was ignored: `transform`s and shapes that aren't paths.
 */
export function svgPaths(svg: string): { d: string; warnings: string[] } {
  const warnings: string[] = [];
  const ds: string[] = [];
  for (const m of svg.matchAll(/<path\b[^>]*>/g)) {
    const tag = m[0];
    const d = /\sd\s*=\s*("([^"]*)"|'([^']*)')/.exec(tag);
    if (d) ds.push(d[2] ?? d[3]);
    if (/\stransform\s*=/.test(tag)) warnings.push("a path's transform is ignored");
  }
  if (/<(rect|circle|ellipse|polygon|polyline|line)\b/.test(svg)) warnings.push("only <path> elements are read (convert shapes to paths)");
  if (/<g\b[^>]*\stransform\s*=/.test(svg)) warnings.push("a group's transform is ignored");
  if (ds.length === 0) throw new Error("no <path> in the file");
  return { d: ds.join(" "), warnings: [...new Set(warnings)] };
}
