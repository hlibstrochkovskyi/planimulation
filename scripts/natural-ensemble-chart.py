#!/usr/bin/env python3
"""Generate a self-contained HTML+SVG overview from natural_ensemble JSON output."""

import argparse
import html
import json
import math
from pathlib import Path


def percentile(sorted_vals, p):
    if not sorted_vals:
        return 0.0
    idx = (len(sorted_vals) - 1) * p
    lo = math.floor(idx)
    hi = math.ceil(idx)
    if lo == hi:
        return sorted_vals[lo]
    return sorted_vals[lo] + (sorted_vals[hi] - sorted_vals[lo]) * (idx - lo)


def svg_forest_range(seeds):
    rows = sorted(seeds, key=lambda r: r["vegetation"]["forestShareFirstHalfMean"])
    w, pad_l, pad_r, pad_t, row_h = 860, 115, 36, 36, 22
    h = pad_t + len(rows) * row_h + 36
    max_x = max(
        0.55,
        max(r["vegetation"]["forestShareMax"] for r in rows) + 0.04,
    )
    plot_w = w - pad_l - pad_r
    sx = lambda v: pad_l + (v / max_x) * plot_w

    parts = [
        f'<svg viewBox="0 0 {w} {h}" width="100%" role="img" aria-label="Forest share ranges across seeds">'
    ]
    # Grid lines
    step = 0.1
    v = 0.0
    while v <= max_x + 1e-9:
        x = sx(v)
        parts.append(
            f'<line x1="{x:.1f}" y1="{pad_t - 6}" x2="{x:.1f}" y2="{h - 28}" stroke="#e2e8f0" stroke-width="1"/>'
        )
        parts.append(
            f'<text x="{x:.1f}" y="{h - 10}" text-anchor="middle" font-size="11" fill="#475569">{v*100:.0f}%</text>'
        )
        v += step

    for idx, r in enumerate(rows):
        y = pad_t + idx * row_h + row_h / 2
        veg = r["vegetation"]
        flagged = len(r["problematicReasons"]) > 0
        named = r["seed"] in ("first-light", "5c151f46", "4836187e")
        label_color = "#b91c1c" if flagged else ("#0f766e" if named else "#1e293b")
        font_weight = "700" if (flagged or named) else "400"
        bar_color = "#ef4444" if flagged else ("#0d9488" if named else "#3b82f6")

        parts.append(
            f'<text x="{pad_l - 8}" y="{y + 4:.1f}" text-anchor="end" font-size="11" font-weight="{font_weight}" fill="{label_color}">{html.escape(r["seed"])}</text>'
        )
        x_min = sx(veg["forestShareMin"])
        x_max = sx(veg["forestShareMax"])
        x0 = sx(veg["forestShareYear0"])
        x_end = sx(veg["forestShareEnd"])
        parts.append(
            f'<line x1="{x_min:.1f}" y1="{y:.1f}" x2="{x_max:.1f}" y2="{y:.1f}" stroke="{bar_color}" stroke-width="5" stroke-linecap="round" opacity="0.45"/>'
        )
        parts.append(
            f'<circle cx="{x0:.1f}" cy="{y:.1f}" r="3.5" fill="#0f172a"><title>Year 0: {veg["forestShareYear0"]*100:.1f}%</title></circle>'
        )
        parts.append(
            f'<circle cx="{x_end:.1f}" cy="{y:.1f}" r="3.5" fill="{bar_color}" stroke="#ffffff" stroke-width="1"><title>Year {r["years"]}: {veg["forestShareEnd"]*100:.1f}%</title></circle>'
        )

    # Legend
    parts.append(
        f'<circle cx="{pad_l}" cy="14" r="3.5" fill="#0f172a"/><text x="{pad_l + 8}" y="18" font-size="11" fill="#334155">Year 0</text>'
    )
    parts.append(
        f'<circle cx="{pad_l + 70}" cy="14" r="3.5" fill="#3b82f6"/><text x="{pad_l + 78}" y="18" font-size="11" fill="#334155">End year (bar: min–max)</text>'
    )
    parts.append(
        f'<circle cx="{pad_l + 240}" cy="14" r="3.5" fill="#0d9488"/><text x="{pad_l + 248}" y="18" font-size="11" fill="#0f766e">Named seed</text>'
    )
    parts.append(
        f'<circle cx="{pad_l + 345}" cy="14" r="3.5" fill="#ef4444"/><text x="{pad_l + 353}" y="18" font-size="11" fill="#b91c1c">Flagged</text>'
    )
    parts.append("</svg>")
    return "".join(parts)


def svg_npp_vs_precip(seeds, laws_cfg):
    w, h = 420, 310
    pad_l, pad_r, pad_t, pad_b = 54, 20, 24, 44
    plot_w = w - pad_l - pad_r
    plot_h = h - pad_t - pad_b

    precs = [r["climate"]["landPrecipitationMeanMm"] for r in seeds]
    npps_lo = [r["vegetation"]["nppMinPg"] for r in seeds]
    npps_hi = [r["vegetation"]["nppMaxPg"] for r in seeds]
    min_p = max(0.0, math.floor((min(precs) - 40) / 50) * 50)
    max_p = math.ceil((max(precs) + 40) / 50) * 50
    min_y = min(laws_cfg["nppMinPg"] - 10, min(npps_lo) - 10)
    max_y = max(laws_cfg["nppMaxPg"] + 10, max(npps_hi) + 10)

    sx = lambda p: pad_l + (p - min_p) / max(1.0, max_p - min_p) * plot_w
    sy = lambda n: pad_t + plot_h - (n - min_y) / max(1.0, max_y - min_y) * plot_h

    parts = [f'<svg viewBox="0 0 {w} {h}" width="100%" role="img" aria-label="Land NPP vs land precipitation">']
    # Shaded plausible band
    by1 = sy(laws_cfg["nppMaxPg"])
    by2 = sy(laws_cfg["nppMinPg"])
    parts.append(
        f'<rect x="{pad_l}" y="{by1:.1f}" width="{plot_w}" height="{by2 - by1:.1f}" fill="#ecfdf5" opacity="0.85"/>'
    )
    parts.append(
        f'<line x1="{pad_l}" y1="{sy(120):.1f}" x2="{w - pad_r}" y2="{sy(120):.1f}" stroke="#059669" stroke-dasharray="4 3" stroke-width="1"/>'
    )
    parts.append(
        f'<text x="{w - pad_r - 4}" y="{sy(120) - 4:.1f}" text-anchor="end" font-size="10" fill="#047857">Earth ~120 Pg/yr</text>'
    )

    for v in range(int(math.ceil(min_y / 40) * 40), int(max_y) + 1, 40):
        y = sy(v)
        parts.append(f'<line x1="{pad_l}" y1="{y:.1f}" x2="{w - pad_r}" y2="{y:.1f}" stroke="#e2e8f0"/>')
        parts.append(f'<text x="{pad_l - 6}" y="{y + 3:.1f}" text-anchor="end" font-size="10" fill="#475569">{v}</text>')

    p_step = 100
    for p in range(int(math.ceil(min_p / p_step) * p_step), int(max_p) + 1, p_step):
        x = sx(p)
        parts.append(f'<line x1="{x:.1f}" y1="{pad_t}" x2="{x:.1f}" y2="{pad_t + plot_h}" stroke="#e2e8f0"/>')
        parts.append(f'<text x="{x:.1f}" y="{h - 24}" text-anchor="middle" font-size="10" fill="#475569">{p}</text>')

    for r in seeds:
        x = sx(r["climate"]["landPrecipitationMeanMm"])
        y = sy(r["vegetation"]["nppMeanPg"])
        y_lo = sy(r["vegetation"]["nppMinPg"])
        y_hi = sy(r["vegetation"]["nppMaxPg"])
        flagged = len(r["problematicReasons"]) > 0
        named = r["seed"] in ("first-light", "5c151f46", "4836187e")
        col = "#ef4444" if flagged else ("#0d9488" if named else "#2563eb")
        parts.append(f'<line x1="{x:.1f}" y1="{y_lo:.1f}" x2="{x:.1f}" y2="{y_hi:.1f}" stroke="{col}" opacity="0.45" stroke-width="1.5"/>')
        parts.append(
            f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{4.5 if (named or flagged) else 3.5}" fill="{col}"><title>{html.escape(r["seed"])}: {r["vegetation"]["nppMeanPg"]:.1f} Pg/yr ({r["climate"]["landPrecipitationMeanMm"]:.0f} mm)</title></circle>'
        )

    parts.append(f'<text x="{pad_l + plot_w / 2:.1f}" y="{h - 6}" text-anchor="middle" font-size="11" fill="#334155">Land mean precipitation (mm/yr)</text>')
    parts.append(f'<text x="14" y="{pad_t + plot_h / 2:.1f}" text-anchor="middle" font-size="11" fill="#334155" transform="rotate(-90 14 {pad_t + plot_h / 2:.1f})">Land NPP (Pg dry matter/yr)</text>')
    parts.append("</svg>")
    return "".join(parts)


def svg_flicker_vs_drift(seeds, laws_cfg):
    w, h = 420, 310
    pad_l, pad_r, pad_t, pad_b = 54, 20, 24, 44
    plot_w = w - pad_l - pad_r
    plot_h = h - pad_t - pad_b

    flicks = [r["vegetation"]["classChangesPerLandRegionPerYear"] * 100.0 for r in seeds]
    drifts = [r["vegetation"]["forestShareDriftPp"] for r in seeds]
    max_f = max(laws_cfg["maxFlickerRate"] * 100.0 * 1.15, max(flicks) * 1.1)
    max_d = max(laws_cfg["maxDriftPp"] * 1.15, max(drifts) * 1.15)

    sx = lambda f: pad_l + (f / max_f) * plot_w
    sy = lambda d: pad_t + plot_h - (d / max_d) * plot_h

    parts = [f'<svg viewBox="0 0 {w} {h}" width="100%" role="img" aria-label="Class flicker vs 500-year forest drift">']
    # Safe pass region
    tx = sx(laws_cfg["maxFlickerRate"] * 100.0)
    ty = sy(laws_cfg["maxDriftPp"])
    parts.append(
        f'<rect x="{pad_l}" y="{ty:.1f}" width="{tx - pad_l:.1f}" height="{pad_t + plot_h - ty:.1f}" fill="#ecfdf5" opacity="0.85"/>'
    )
    parts.append(
        f'<line x1="{tx:.1f}" y1="{pad_t}" x2="{tx:.1f}" y2="{pad_t + plot_h}" stroke="#dc2626" stroke-dasharray="4 3" stroke-width="1.2"/>'
    )
    parts.append(
        f'<line x1="{pad_l}" y1="{ty:.1f}" x2="{w - pad_r}" y2="{ty:.1f}" stroke="#dc2626" stroke-dasharray="4 3" stroke-width="1.2"/>'
    )

    for i in range(5):
        fv = max_f * i / 4.0
        x = sx(fv)
        parts.append(f'<line x1="{x:.1f}" y1="{pad_t}" x2="{x:.1f}" y2="{pad_t + plot_h}" stroke="#e2e8f0"/>')
        parts.append(f'<text x="{x:.1f}" y="{h - 24}" text-anchor="middle" font-size="10" fill="#475569">{fv:.2f}%</text>')
    for i in range(5):
        dv = max_d * i / 4.0
        y = sy(dv)
        parts.append(f'<line x1="{pad_l}" y1="{y:.1f}" x2="{w - pad_r}" y2="{y:.1f}" stroke="#e2e8f0"/>')
        parts.append(f'<text x="{pad_l - 6}" y="{y + 3:.1f}" text-anchor="end" font-size="10" fill="#475569">{dv:.2f}</text>')

    for r in seeds:
        fv = r["vegetation"]["classChangesPerLandRegionPerYear"] * 100.0
        dv = r["vegetation"]["forestShareDriftPp"]
        x, y = sx(fv), sy(dv)
        flagged = len(r["problematicReasons"]) > 0
        named = r["seed"] in ("first-light", "5c151f46", "4836187e")
        col = "#ef4444" if flagged else ("#0d9488" if named else "#2563eb")
        parts.append(
            f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{4.5 if (named or flagged) else 3.5}" fill="{col}"><title>{html.escape(r["seed"])}: flicker {fv:.2f}%/yr, drift {dv:.2f} pp</title></circle>'
        )

    parts.append(f'<text x="{pad_l + plot_w / 2:.1f}" y="{h - 6}" text-anchor="middle" font-size="11" fill="#334155">Class changes per land region per year (%)</text>')
    parts.append(f'<text x="14" y="{pad_t + plot_h / 2:.1f}" text-anchor="middle" font-size="11" fill="#334155" transform="rotate(-90 14 {pad_t + plot_h / 2:.1f})">|Half 2 − Half 1| forest drift (pp)</text>')
    parts.append("</svg>")
    return "".join(parts)


def svg_drought_response(seeds):
    w, h = 420, 290
    pad_l, pad_r, pad_t, pad_b = 54, 20, 24, 44
    plot_w = w - pad_l - pad_r
    plot_h = h - pad_t - pad_b

    drops = [r["vegetation"]["worstDrought"]["threeYearWoodyLoss"] for r in seeds]
    recs = [r["vegetation"]["worstDrought"]["recoveryYears"] for r in seeds]
    finite_recs = [v for v in recs if v is not None]
    max_rec = max(30, (max(finite_recs) if finite_recs else 30) + 10)
    min_drop = max(0.0, min(drops) - 0.05)
    max_drop = min(1.0, max(drops) + 0.05)

    sx = lambda d: pad_l + (d - min_drop) / max(0.05, max_drop - min_drop) * plot_w
    sy = lambda t: pad_t + plot_h - (min(t, max_rec) / max_rec) * plot_h

    parts = [f'<svg viewBox="0 0 {w} {h}" width="100%" role="img" aria-label="Worst 3-year regional drought loss and recovery time">']
    for i in range(5):
        dv = min_drop + (max_drop - min_drop) * i / 4.0
        x = sx(dv)
        parts.append(f'<line x1="{x:.1f}" y1="{pad_t}" x2="{x:.1f}" y2="{pad_t + plot_h}" stroke="#e2e8f0"/>')
        parts.append(f'<text x="{x:.1f}" y="{h - 24}" text-anchor="middle" font-size="10" fill="#475569">{dv:.2f}</text>')
    for i in range(5):
        tv = max_rec * i / 4.0
        y = sy(tv)
        parts.append(f'<line x1="{pad_l}" y1="{y:.1f}" x2="{w - pad_r}" y2="{y:.1f}" stroke="#e2e8f0"/>')
        parts.append(f'<text x="{pad_l - 6}" y="{y + 3:.1f}" text-anchor="end" font-size="10" fill="#475569">{tv:.0f}</text>')

    for r in seeds:
        d = r["vegetation"]["worstDrought"]
        x = sx(d["threeYearWoodyLoss"])
        rec = d["recoveryYears"]
        if rec is None:
            y = sy(max_rec)
            parts.append(
                f'<rect x="{x - 4:.1f}" y="{y - 4:.1f}" width="8" height="8" fill="#f59e0b" stroke="#92400e"><title>{html.escape(r["seed"])}: loss {d["threeYearWoodyLoss"]:.2f}, did not recover before yr {r["years"]}</title></rect>'
            )
        else:
            y = sy(rec)
            col = "#0d9488" if r["seed"] in ("first-light", "5c151f46", "4836187e") else "#2563eb"
            parts.append(
                f'<circle cx="{x:.1f}" cy="{y:.1f}" r="4" fill="{col}"><title>{html.escape(r["seed"])}: loss {d["threeYearWoodyLoss"]:.2f}, recovered in {rec} yr</title></circle>'
            )

    parts.append(f'<text x="{pad_l + plot_w / 2:.1f}" y="{h - 6}" text-anchor="middle" font-size="11" fill="#334155">Largest 3-year woody cover loss</text>')
    parts.append(f'<text x="14" y="{pad_t + plot_h / 2:.1f}" text-anchor="middle" font-size="11" fill="#334155" transform="rotate(-90 14 {pad_t + plot_h / 2:.1f})">Recovery years (within 0.05)</text>')
    parts.append("</svg>")
    return "".join(parts)


def build_html(data):
    seeds = data["standard"]
    extremes = data["extremes"]
    laws_cfg = data["lawThresholds"]

    forest_means = sorted(
        (r["vegetation"]["forestShareFirstHalfMean"] + r["vegetation"]["forestShareSecondHalfMean"]) * 50.0
        for r in seeds
    )
    npps = sorted(r["vegetation"]["nppMeanPg"] for r in seeds)
    flicks = sorted(r["vegetation"]["classChangesPerLandRegionPerYear"] * 100.0 for r in seeds)
    drifts = sorted(r["vegetation"]["forestShareDriftPp"] for r in seeds)
    passed = sum(1 for r in seeds if len(r["problematicReasons"]) == 0)

    ext_rows = []
    for e in extremes:
        ok = len(e["problematicReasons"]) == 0
        badge = '<span class="ok">PASS</span>' if ok else f'<span class="fail">FAIL</span> ({html.escape("; ".join(e["problematicReasons"]))})'
        v = e["vegetation"]
        ext_rows.append(
            f"<tr><td><code>{html.escape(e['case'])}</code></td>"
            f"<td><code>{html.escape(e['seed'])}</code></td>"
            f"<td>{v['forestShareYear0']*100:.1f}% → {v['forestShareEnd']*100:.1f}%</td>"
            f"<td>{v['forestShareDriftPp']:.2f} pp</td>"
            f"<td>{v['classChangesPerLandRegionPerYear']*100:.2f}%</td>"
            f"<td>{v['nppMeanPg']:.1f}</td>"
            f"<td>{v['globalLandFactorSd']:.3f}</td>"
            f"<td>{badge}</td></tr>"
        )

    flagged_html = ""
    flagged = [r for r in seeds if r["problematicReasons"]]
    if flagged:
        items = "".join(
            f"<li><code>{html.escape(r['seed'])}</code>: {html.escape('; '.join(r['problematicReasons']))}</li>"
            for r in flagged
        )
        flagged_html = f'<div class="card warn"><h2>Flagged standard seeds ({len(flagged)})</h2><ul>{items}</ul></div>'
    else:
        flagged_html = '<div class="card good"><h2>Standard ensemble laws</h2><p>All standard seeds passed every checked law.</p></div>'

    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width,initial-scale=1"/>
<title>Planimulation Natural-World Ensemble Overview</title>
<style>
  body {{ font-family: system-ui, -apple-system, Segoe UI, Roboto, sans-serif; margin: 24px auto; max-width: 960px; color: #0f172a; background: #f8fafc; line-height: 1.45; padding: 0 16px; }}
  h1 {{ font-size: 1.5rem; margin-bottom: 4px; }}
  h2 {{ font-size: 1.1rem; margin: 0 0 10px 0; }}
  .sub {{ color: #475569; margin-bottom: 18px; font-size: 0.92rem; }}
  .kpis {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(175px, 1fr)); gap: 12px; margin-bottom: 18px; }}
  .kpi {{ background: #fff; border: 1px solid #e2e8f0; border-radius: 8px; padding: 12px 14px; }}
  .kpi .label {{ font-size: 0.78rem; color: #64748b; text-transform: uppercase; letter-spacing: 0.03em; }}
  .kpi .val {{ font-size: 1.35rem; font-weight: 700; margin-top: 2px; }}
  .kpi .note {{ font-size: 0.8rem; color: #475569; }}
  .card {{ background: #fff; border: 1px solid #e2e8f0; border-radius: 8px; padding: 16px; margin-bottom: 16px; }}
  .card.warn {{ border-color: #fca5a5; background: #fef2f2; }}
  .card.good {{ border-color: #86efac; background: #f0fdf4; }}
  .grid2 {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(380px, 1fr)); gap: 16px; margin-bottom: 16px; }}
  table {{ width: 100%; border-collapse: collapse; font-size: 0.85rem; }}
  th, td {{ text-align: left; padding: 6px 8px; border-bottom: 1px solid #e2e8f0; }}
  th {{ background: #f1f5f9; font-weight: 600; }}
  .ok {{ color: #15803d; font-weight: 700; }}
  .fail {{ color: #b91c1c; font-weight: 700; }}
  code {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 0.85em; }}
</style>
</head>
<body>
  <h1>Natural-World Ensemble Overview</h1>
  <div class="sub"><code>multiscale-geography-4</code> (subdivision 6, 40,962 regions) · <code>climate-normals-1</code> · <code>vegetation-2</code> ({data["years"]} years, {len(seeds)} seeds)</div>
  <div class="kpis">
    <div class="kpi"><div class="label">Laws passed</div><div class="val">{passed} / {len(seeds)}</div><div class="note">Standard ensemble seeds</div></div>
    <div class="kpi"><div class="label">Forest share</div><div class="val">{percentile(forest_means, 0.5):.1f}%</div><div class="note">IQR {percentile(forest_means, 0.25):.1f}–{percentile(forest_means, 0.75):.1f}% (min {forest_means[0]:.1f}, max {forest_means[-1]:.1f})</div></div>
    <div class="kpi"><div class="label">Land NPP</div><div class="val">{percentile(npps, 0.5):.1f} Pg/yr</div><div class="note">IQR {percentile(npps, 0.25):.1f}–{percentile(npps, 0.75):.1f} (range {npps[0]:.1f}–{npps[-1]:.1f})</div></div>
    <div class="kpi"><div class="label">Class flicker</div><div class="val">{percentile(flicks, 0.5):.2f}%/yr</div><div class="note">Max {flicks[-1]:.2f}%/yr (limit {laws_cfg["maxFlickerRate"]*100:.1f}%)</div></div>
    <div class="kpi"><div class="label">500-yr drift</div><div class="val">{percentile(drifts, 0.5):.2f} pp</div><div class="note">Max {drifts[-1]:.2f} pp (limit {laws_cfg["maxDriftPp"]:.1f} pp)</div></div>
  </div>
  {flagged_html}
  <div class="card">
    <h2>Forest share range across {len(seeds)} seeds (Year 0, min–max, end year)</h2>
    {svg_forest_range(seeds)}
  </div>
  <div class="grid2">
    <div class="card">
      <h2>Land NPP vs land mean precipitation</h2>
      {svg_npp_vs_precip(seeds, laws_cfg)}
    </div>
    <div class="card">
      <h2>Class flicker vs 500-year forest drift</h2>
      {svg_flicker_vs_drift(seeds, laws_cfg)}
    </div>
  </div>
  <div class="card">
    <h2>Worst 3-year regional drought response (amber square = unrecovered by year {data["years"]})</h2>
    {svg_drought_response(seeds)}
  </div>
  <div class="card">
    <h2>Extreme supported settings on 3 named seeds</h2>
    <table>
      <thead><tr><th>Case</th><th>Seed</th><th>Forest 0 → end</th><th>Drift</th><th>Flicker</th><th>NPP (Pg/yr)</th><th>Factor SD</th><th>Laws</th></tr></thead>
      <tbody>{"".join(ext_rows)}</tbody>
    </table>
  </div>
</body>
</html>
"""


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("input_json", type=Path)
    p.add_argument("output_html", type=Path)
    args = p.parse_args()
    data = json.loads(args.input_json.read_text(encoding="utf-8"))
    args.output_html.parent.mkdir(parents=True, exist_ok=True)
    args.output_html.write_text(build_html(data), encoding="utf-8")
    print(f"Wrote {args.output_html}")


if __name__ == "__main__":
    main()
