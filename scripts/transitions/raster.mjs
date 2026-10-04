// The transition suite's rasterizer (design note 31, TS4/TS5): a frame's ink coverage on an
// N×N grid (engine units 0..size). Dots are discs, lines and polylines stamped strokes,
// fills scanline polygons (even-odd). Colour is ignored: the suite measures motion.
export function raster(fr, size, N = 128, out = new Float32Array(N * N), k = 1) {
  if (!fr) return out;
  const s = N / size;
  const add = (x, y, v) => {
    const i = x | 0, j = y | 0;
    if (i < 0 || j < 0 || i >= N || j >= N) return;
    out[j * N + i] += v * k;
  };
  for (const d of fr.dots) {
    const cx = d.x * s, cy = d.y * s, r = Math.max(0.5, d.r * s), a = d.a;
    const r2 = r * r;
    for (let y = Math.floor(cy - r); y <= cy + r; y++)
      for (let x = Math.floor(cx - r); x <= cx + r; x++) {
        const dx = x + 0.5 - cx, dy = y + 0.5 - cy;
        if (dx * dx + dy * dy <= r2 + 0.5) add(x, y, a);
      }
  }
  const seg = (x1, y1, x2, y2, w, a) => {
    x1 *= s; y1 *= s; x2 *= s; y2 *= s;
    const L = Math.hypot(x2 - x1, y2 - y1), n = Math.max(1, Math.ceil(L / 0.5));
    const hw = Math.max(0.5, (w * s) / 2);
    for (let i = 0; i <= n; i++) {
      const x = x1 + ((x2 - x1) * i) / n, y = y1 + ((y2 - y1) * i) / n;
      if (hw <= 0.75) add(x, y, a * 0.5);
      else for (let yy = Math.floor(y - hw); yy <= y + hw; yy++) for (let xx = Math.floor(x - hw); xx <= x + hw; xx++) add(xx, yy, (a * 0.5) / hw);
    }
  };
  for (const l of fr.lines) seg(l.x1, l.y1, l.x2, l.y2, l.w, l.a);
  for (const p of fr.polylines) for (let i = 1; i < p.points.length; i++) seg(p.points[i - 1].x, p.points[i - 1].y, p.points[i].x, p.points[i].y, p.w, p.a);
  for (const f of fr.fills ?? []) {
    const rings = [f.points, ...(f.holes ?? [])];
    for (let j = 0; j < N; j++) {
      const y = (j + 0.5) / s, xs = [];
      for (const ring of rings)
        for (let i = 0, m = ring.length; i < m; i++) {
          const p = ring[i], q = ring[(i + 1) % m];
          if ((p.y > y) !== (q.y > y)) xs.push(p.x + ((y - p.y) * (q.x - p.x)) / (q.y - p.y));
        }
      xs.sort((a, b) => a - b);
      for (let i = 0; i + 1 < xs.length; i += 2)
        for (let x = Math.ceil(xs[i] * s - 0.5); x < xs[i + 1] * s - 0.5; x++) add(x, j, f.a ?? 1);
    }
  }
  return out;
}
export function diff(a, b) {
  let t = 0;
  for (let i = 0; i < a.length; i++) t += Math.abs(Math.min(1, a[i]) - Math.min(1, b[i]));
  return t / a.length;
}
