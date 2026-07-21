#!/usr/bin/env python3
"""Genera un HTML interactivo de una traza PBS JSONL (tiles, reservas, tren).

Uso:
  python3 scripts/view_pbs_trace.py \\
    crates/openttdrs-core/tests/fixtures/parity/train_pbs_15_3_openttd.jsonl \\
    /tmp/pbs_trace.html

Opciones:
  --signal X,Y[,label]   Anota una señal (repetible). Por defecto, en el
                         fixture train_pbs_15_3, (46,37)=path.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def fail(msg: str) -> None:
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(1)


def load_trace(path: Path) -> tuple[dict, list[dict]]:
    rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
    if not rows:
        fail("traza vacía")
    meta = rows[0] if rows[0].get("kind") == "metadata" else {}
    body = [r for r in rows if r.get("kind") in ("initial", "tick")]
    if not body:
        fail("sin filas initial/tick")
    return meta, body


def default_signals_for(source: str) -> list[dict]:
    if "train_pbs_15_3" in source:
        return [{"x": 46, "y": 37, "label": "path"}]
    return []


def build_payload(meta: dict, body: list[dict], signals: list[dict]) -> dict:
    series = []
    visited: list[dict] = []
    seen: set[tuple[int, int]] = set()
    rail_set: set[tuple[int, int]] = set()

    for row in body:
        trains = row.get("trains") or []
        reservations = row.get("rail_reservations") or []
        for rr in reservations:
            rail_set.add((int(rr["x"]), int(rr["y"])))
        primary = trains[0] if trains else None
        if primary is not None:
            key = (int(primary["x"]), int(primary["y"]))
            rail_set.add(key)
            if key not in seen:
                seen.add(key)
                visited.append({"x": key[0], "y": key[1], "first_tick": int(row["tick"])})
        series.append(
            {
                "kind": row["kind"],
                "tick": int(row["tick"]),
                "trains": [
                    {
                        "vehicle": t.get("vehicle"),
                        "x": int(t["x"]),
                        "y": int(t["y"]),
                        "progress": int(t.get("progress", 0)),
                        "speed": int(t.get("speed", 0)),
                        "direction": int(t.get("direction", 0)),
                    }
                    for t in trains
                ],
                "reservations": [
                    {"x": int(r["x"]), "y": int(r["y"]), "track_bits": int(r["track_bits"])}
                    for r in reservations
                ],
            }
        )

    return {
        "meta": {
            "producer": meta.get("producer"),
            "source_path": meta.get("source_path"),
            "openttd_commit": meta.get("openttd_commit"),
            "max_ticks": meta.get("max_ticks"),
            "schema_version": meta.get("schema_version"),
        },
        "series": series,
        "visited": visited,
        "rails": [{"x": x, "y": y} for x, y in sorted(rail_set)],
        "signals": signals,
    }


HTML_TEMPLATE = r"""<!DOCTYPE html>
<html lang="es">
<head>
<meta charset="utf-8"/>
<title>PBS trace viewer</title>
<style>
  :root { color-scheme: light dark; }
  body { font-family: ui-sans-serif, system-ui, sans-serif; margin: 1.25rem; line-height: 1.4; }
  h1 { font-size: 1.25rem; margin: 0 0 0.25rem; }
  .sub { opacity: 0.75; font-size: 0.9rem; margin-bottom: 1rem; }
  .row { display: flex; flex-wrap: wrap; gap: 1rem; align-items: flex-start; }
  .panel { border: 1px solid color-mix(in srgb, CanvasText 25%, transparent); padding: 0.75rem; border-radius: 6px; }
  #map { background: color-mix(in srgb, Canvas 92%, CanvasText); }
  .controls { min-width: 280px; flex: 1; }
  label { display: block; margin: 0.5rem 0 0.25rem; font-size: 0.85rem; opacity: 0.8; }
  input[type=range] { width: 100%; }
  table { border-collapse: collapse; width: 100%; font-size: 0.9rem; }
  th, td { text-align: left; padding: 0.25rem 0.4rem; border-bottom: 1px solid color-mix(in srgb, CanvasText 15%, transparent); }
  .legend span { display: inline-block; width: 12px; height: 12px; margin-right: 0.35rem; vertical-align: middle; border: 1px solid color-mix(in srgb, CanvasText 40%, transparent); }
  .c-rail { background: #6b7280; }
  .c-res { background: #2563eb; }
  .c-vis { background: #d97706; }
  .c-sig { background: #dc2626; }
  .c-train { background: #059669; }
  code { font-size: 0.85rem; }
</style>
</head>
<body>
  <h1>Viewer traza PBS</h1>
  <div class="sub" id="subtitle"></div>
  <div class="row">
    <div class="panel">
      <svg id="map" width="640" height="220"></svg>
      <div class="legend" style="margin-top:0.5rem;font-size:0.85rem">
        <div><span class="c-rail"></span>vía / corredor</div>
        <div><span class="c-res"></span>reserva PBS</div>
        <div><span class="c-vis"></span>tile visitado</div>
        <div><span class="c-sig"></span>señal</div>
        <div><span class="c-train"></span>tren (muestra actual)</div>
      </div>
    </div>
    <div class="panel controls">
      <label for="tick">Muestra (initial + ticks)</label>
      <input id="tick" type="range" min="0" max="0" value="0"/>
      <div id="tickLabel" style="margin:0.35rem 0 0.75rem;font-weight:600"></div>
      <table>
        <tbody id="stats"></tbody>
      </table>
      <h3 style="margin:1rem 0 0.35rem;font-size:1rem">Tiles visitados (orden)</h3>
      <ol id="visited" style="margin:0;padding-left:1.2rem;font-size:0.9rem"></ol>
    </div>
  </div>
<script>
const DATA = __DATA__;
const CELL = 36;
const PAD = 24;

function bbox() {
  const xs = [], ys = [];
  for (const r of DATA.rails) { xs.push(r.x); ys.push(r.y); }
  for (const s of DATA.signals) { xs.push(s.x); ys.push(s.y); }
  for (const row of DATA.series) for (const t of row.trains) { xs.push(t.x); ys.push(t.y); }
  const minX = Math.min(...xs) - 1, maxX = Math.max(...xs) + 1;
  const minY = Math.min(...ys) - 1, maxY = Math.max(...ys) + 1;
  return { minX, maxX, minY, maxY };
}

function project(x, y, box) {
  return [PAD + (x - box.minX) * CELL, PAD + (y - box.minY) * CELL];
}

function render(idx) {
  const row = DATA.series[idx];
  const box = bbox();
  const svg = document.getElementById('map');
  const w = PAD * 2 + (box.maxX - box.minX + 1) * CELL;
  const h = PAD * 2 + (box.maxY - box.minY + 1) * CELL;
  svg.setAttribute('width', w);
  svg.setAttribute('height', h);
  const res = new Set(row.reservations.map(r => r.x + ',' + r.y));
  const vis = new Set(DATA.visited.map(v => v.x + ',' + v.y));
  let parts = [];
  for (const r of DATA.rails) {
    const [px, py] = project(r.x, r.y, box);
    const key = r.x + ',' + r.y;
    let fill = '#6b7280';
    if (vis.has(key)) fill = '#d97706';
    if (res.has(key)) fill = '#2563eb';
    parts.push(`<rect x="${px}" y="${py}" width="${CELL-3}" height="${CELL-3}" fill="${fill}" rx="3"/>`);
    parts.push(`<text x="${px+4}" y="${py+14}" font-size="10" fill="#fff">${r.x},${r.y}</text>`);
  }
  for (const s of DATA.signals) {
    const [px, py] = project(s.x, s.y, box);
    parts.push(`<circle cx="${px+CELL/2-1}" cy="${py+CELL/2-1}" r="7" fill="#dc2626"/>`);
    parts.push(`<text x="${px+2}" y="${py+CELL-4}" font-size="9" fill="#fff">${s.label||'sig'}</text>`);
  }
  for (const t of row.trains) {
    const [px, py] = project(t.x, t.y, box);
    parts.push(`<rect x="${px+6}" y="${py+6}" width="${CELL-15}" height="${CELL-15}" fill="#059669" rx="4"/>`);
  }
  svg.innerHTML = parts.join('');
  document.getElementById('tickLabel').textContent =
    `${row.kind} · tick ${row.tick} · muestra ${idx}/${DATA.series.length-1}`;
  const t0 = row.trains[0];
  const stats = [
    ['Tren tile', t0 ? `${t0.x},${t0.y}` : '—'],
    ['progress', t0 ? t0.progress : '—'],
    ['speed', t0 ? t0.speed : '—'],
    ['direction', t0 ? t0.direction : '—'],
    ['Reservas PBS', row.reservations.map(r => `${r.x},${r.y}`).join(' → ') || '—'],
  ];
  document.getElementById('stats').innerHTML = stats.map(([k,v]) =>
    `<tr><th>${k}</th><td><code>${v}</code></td></tr>`).join('');
}

function main() {
  const m = DATA.meta || {};
  document.getElementById('subtitle').textContent =
    `${m.source_path || ''} · producer=${m.producer || '?'} · commit=${(m.openttd_commit||'').slice(0,12)}`;
  document.getElementById('visited').innerHTML = DATA.visited.map(v =>
    `<li><code>${v.x},${v.y}</code> desde tick ${v.first_tick}</li>`).join('');
  const slider = document.getElementById('tick');
  slider.max = String(DATA.series.length - 1);
  slider.value = '0';
  slider.addEventListener('input', () => render(Number(slider.value)));
  render(0);
}
main();
</script>
</body>
</html>
"""


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("trace", type=Path, help="Traza JSONL PBS")
    ap.add_argument("out", type=Path, nargs="?", default=Path("/tmp/pbs_trace.html"))
    ap.add_argument(
        "--signal",
        action="append",
        default=[],
        help="Señal X,Y o X,Y,label (repetible)",
    )
    args = ap.parse_args()
    if not args.trace.is_file():
        fail(f"no existe {args.trace}")

    meta, body = load_trace(args.trace)
    signals: list[dict] = []
    for raw in args.signal:
        parts = raw.split(",")
        if len(parts) < 2:
            fail(f"--signal inválido: {raw}")
        label = parts[2] if len(parts) > 2 else "signal"
        signals.append({"x": int(parts[0]), "y": int(parts[1]), "label": label})
    if not signals:
        signals = default_signals_for(str(meta.get("source_path") or args.trace))

    payload = build_payload(meta, body, signals)
    html = HTML_TEMPLATE.replace("__DATA__", json.dumps(payload, separators=(",", ":")))
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(html, encoding="utf-8")
    print(f"OK: {args.out} ({len(payload['series'])} muestras, {len(payload['visited'])} tiles visitados)")


if __name__ == "__main__":
    main()
