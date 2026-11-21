#!/usr/bin/env python3
"""Render the natural ensemble JSON as standalone HTML and SVG."""
import argparse
import html
import json
import statistics
from pathlib import Path

INK = "#172933"
MUTED = "#526471"
GRID = "#d9e3e7"
TEAL = "#137a73"
AMBER = "#b66318"
RED = "#b63b38"


def escape(value):
    return html.escape(str(value), quote=True)


def text(x, y, label, *, size=12, color=INK, anchor="start", bold=False):
    weight = 650 if bold else 400
    return (f'<text x="{x:.1f}" y="{y:.1f}" fill="{color}" font-size="{size}" '
            f'font-weight="{weight}" text-anchor="{anchor}">{escape(label)}</text>')


def line(x1, y1, x2, y2, color=GRID, width=1):
    return (f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" '
            f'stroke="{color}" stroke-width="{width}" stroke-linecap="round"/>')


def dot(x, y, color, hover, size=4):
    return (f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{size}" fill="{color}" '
            f'stroke="#fff"><title>{escape(hover)}</title></circle>')


def panel(width, height, title, elements):
    return (width, height, title, "".join(elements))


def svg(chart, *, x=0, y=0):
    width, height, title, elements = chart
    return (f'<svg xmlns="http://www.w3.org/2000/svg" x="{x}" y="{y}" '
            f'width="{width}" height="{height}" viewBox="0 0 {width} {height}" '
            f'role="img" font-family="system-ui, sans-serif"><title>{escape(title)}</title>'
            f'{elements}</svg>')


def status_color(run):
    if any(not law["pass"] for law in run["laws"]):
        return RED
    return AMBER if run["problematicReasons"] else TEAL


def scale(domain, endpoints):
    low, high = domain
    left, right = endpoints
    return lambda value: left + (right - left) * (value - low) / (high - low)


def range_panel(rows, title, unit, values, digits):
    width, left, right = 540, 156, 518
    top, spacing = 83, 19
    height = top + spacing * len(rows) + 20
    rows = sorted(rows, key=lambda run: values(run)[1])
    measures = [values(run) for run in rows]
    high = max(v[2] for v in measures)
    x = scale((0., high * 1.05), (left, right))
    elements = [
        text(12, 22, title, size=17, bold=True),
        text(12, 45, f"{unit} · bars show annual ranges", color=MUTED),
    ]
    for j in range(6):
        value = high * 1.05 * j / 5
        px = x(value)
        elements += [
            text(px, 65, f"{value:.{digits}f}", size=11, color=MUTED, anchor="middle"),
            line(px, 72, px, height - 19),
        ]
    for j, (run, (low, center, high)) in enumerate(zip(rows, measures)):
        row = top + spacing * j
        color = status_color(run)
        elements += [
            text(left - 8, row + 4, run["seed"], anchor="end"),
            line(x(low), row, x(high), row, color, 3),
            dot(x(center), row, color, f"{run['seed']}: {low:.2f} / {center:.2f} / {high:.2f}"),
        ]
    return panel(width, height, title, elements)


def tick(value, maximum):
    if maximum >= 20:
        return f"{value:,.0f}"
    if maximum >= 2:
        return f"{value:.1f}"
    return f"{value:.2f}"


def scatter(rows, title, subtitle, x_axis, y_axis, points):
    width, height = 540, 304
    left, right, top, bottom = 63, 514, 79, 252
    x_max = max(p[0] for p in points) * 1.10
    y_max = max(p[1] for p in points) * 1.10 or 1
    x = scale((0, x_max), (left, right))
    y = scale((0, y_max), (bottom, top))
    elements = [text(12, 22, title, size=17, bold=True), text(12, 45, subtitle, color=MUTED)]
    for step in range(6):
        xv, yv = x_max * step / 5, y_max * step / 5
        px, py = x(xv), y(yv)
        elements += [
            line(px, top, px, bottom), line(left, py, right, py),
            text(px, bottom + 17, tick(xv, x_max), size=11, color=MUTED, anchor="middle"),
            text(left - 8, py + 4, tick(yv, y_max), size=11, color=MUTED, anchor="end"),
        ]
    elements.append(text((left + right) / 2, 292, x_axis, color=MUTED, anchor="middle"))
    elements.append(text(left, 67, y_axis, color=MUTED))
    for run, (x_value, y_value, censored) in zip(rows, points):
        px, py = x(x_value), y(y_value)
        hover = f"{run['seed']}: {x_value:.3g}, {y_value:.3g}"
        if censored:
            elements.append(
                f'<rect x="{px - 4:.1f}" y="{py - 4:.1f}" width="8" height="8" '
                f'fill="white" stroke="{AMBER}" stroke-width="2">'
                f'<title>{escape(hover)}, unrecovered at last observation</title></rect>'
            )
        else:
            elements.append(dot(px, py, status_color(run), hover))
    return panel(width, height, title, elements)


def charts(rows):
    def forest(run):
        v = run["vegetation"]
        return (
            100 * v["forestShareMin"],
            50 * (v["forestShareFirstHalfMean"] + v["forestShareSecondHalfMean"]),
            100 * v["forestShareMax"],
        )

    def foragers(run):
        density = run["wildFood"]["capacityPeoplePerIceFreeKm2"]
        return tuple(density[key] * 100 for key in ("min", "mean", "max"))

    droughts = [(r, r["vegetation"]["worstDrought"]) for r in rows
                if r["vegetation"]["worstDrought"]["region"] is not None]
    report = [
        range_panel(rows, "Forest cover by seed", "% of land · dot is the 1,000-year mean", forest, 0),
        range_panel(rows, "Forager capacity by seed", "People / 100 km² ice-free land · dot is the mean", foragers, 1),
        scatter(rows, "Rainfall and productivity", "Each point is one standard seed.",
                "Land precipitation (mm/year)", "Mean NPP (Pg dry matter/year)",
                [(r["climate"]["landPrecipitationMeanMm"], r["vegetation"]["nppMeanPg"], False) for r in rows]),
    ]
    if droughts:
        report.append(scatter(
            [r for r, _ in droughts], "Largest three-year woody loss",
            "Open square shows follow-up for a region that had yet to recover.",
            "Woody cover lost (fraction of region)", "Years after loss",
            [(d["threeYearWoodyLoss"], d["recoveryYears"] if d["recoveryYears"] is not None else d["followUpYears"],
              d["recoveryYears"] is None) for _, d in droughts],
        ))
    return report


def svg_overview(data, panels):
    width = 1104
    height = 77 + sum(max(c[1] for c in panels[i:i + 2]) for i in range(0, len(panels), 2))
    elements = [
        f'<rect width="{width}" height="{height}" fill="#f7faf9"/>',
        text(24, 30, "Planimulation · Natural-world ensemble", size=22, bold=True),
        text(24, 55, f"{len(data['standard'])} standard seeds · {data['years']} years · red: failed check · amber: prolonged recovery", color=MUTED),
    ]
    y = 71
    for i in range(0, len(panels), 2):
        for col, chart in enumerate(panels[i:i + 2]):
            elements.append(svg(chart, x=12 + 540 * col, y=y))
        y += max(c[1] for c in panels[i:i + 2])
    return svg(panel(width, height, "Natural-world ensemble", elements))


def html_overview(data, panels):
    standard = data["standard"]
    passed = sum(all(law["pass"] for law in r["laws"]) for r in standard)
    forest = statistics.median(50 * (r["vegetation"]["forestShareFirstHalfMean"] +
                                    r["vegetation"]["forestShareSecondHalfMean"]) for r in standard)
    capacity = statistics.median(r["wildFood"]["capacityPeople"]["mean"] for r in standard) / 1e6
    figures = "".join(f"<figure>{svg(chart)}</figure>" for chart in panels)
    flagged = "".join(
        f"<li><strong>{escape(r['seed'])}</strong> ({escape(r['case'])}). "
        f"{escape('. '.join(r['problematicReasons']))}.</li>"
        for r in standard + data["extremes"] if r["problematicReasons"]
    )
    extremes = []
    for run in data["extremes"]:
        v, f = run["vegetation"], run["wildFood"]
        failures = ", ".join(law["name"] for law in run["laws"] if not law["pass"])
        cells = [
            run["case"], run["seed"], f"{100 * v['forestShareYear0']:.1f}% → {100 * v['forestShareEnd']:.1f}%",
            f"{v['forestShareDriftPp']:.2f}", f"{100 * v['classChangesPerLandRegionPerYear']:.2f}%",
            f"{v['nppMeanPg']:.1f}", f"{f['capacityPeople']['mean'] / 1e6:.2f}", failures or "All passed",
        ]
        extremes.append(f'<tr style="--status:{status_color(run)}">' + "".join(f"<td>{escape(c)}</td>" for c in cells) + "</tr>")
    models = " · ".join(escape(model) for model in data["models"].values())
    return f"""<!DOCTYPE html>
<html lang="en">
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Planimulation · Natural-world ensemble</title>
<style>
body {{ margin: 0 auto; padding: 2.2rem 1.1rem; max-width: 1120px; background: #f7faf9; color: {INK}; font: 15px/1.5 system-ui, sans-serif; }}
h1 {{ font-size: clamp(1.8rem, 5vw, 3rem); line-height: 1.1; margin: 0 0 .4rem; }}
h2 {{ margin: 2rem 0 .6rem; font-size: 1.25rem; }}
p {{ color: {MUTED}; }}
dl {{ display: flex; flex-wrap: wrap; gap: 1rem 3rem; border-block: 1px solid {GRID}; padding: .8rem 0; }}
dt {{ color: {MUTED}; }} dd {{ margin: 0; font-size: 1.8rem; font-weight: 650; }}
.grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(min(420px, 100%), 1fr)); gap: 1rem; }}
figure {{ margin: 0; background: #fff; border: 1px solid {GRID}; border-radius: .6rem; }}
figure svg {{ display: block; width: 100%; height: auto; }}
section {{ overflow-x: auto; }}
table {{ width: 100%; border-collapse: collapse; font-variant-numeric: tabular-nums; font-size: .87rem; }}
th, td {{ text-align: left; padding: .5rem .6rem; border-bottom: 1px solid {GRID}; }}
th {{ background: #eaf1ee; }} tr td:last-child {{ color: var(--status); }}
a {{ color: {TEAL}; }}
</style>
<h1>Natural-world ensemble</h1>
<p>Planimulation · multiscale-geography-4 · subdivision 6 · {models} · {data['years']} years</p>
<dl>
  <div><dt>Standard seeds passing every check</dt><dd>{passed} / {len(standard)}</dd></div>
  <div><dt>Median forest share</dt><dd>{forest:.1f}%</dd></div>
  <div><dt>Median forager capacity</dt><dd>{capacity:.1f} million</dd></div>
</dl>
<p>Red marks a failed check. Amber marks prolonged woody recovery.</p>
<div class="grid">{figures}</div>
<section>
<h2>Extreme settings</h2>
<table>
<thead><tr><th>Case</th><th>Seed</th><th>Forest 0 → end</th><th>Drift pp</th><th>Flicker</th><th>NPP Pg/year</th><th>Foragers M</th><th>Checks</th></tr></thead>
<tbody>{''.join(extremes)}</tbody>
</table>
</section>
<h2>Flagged runs</h2>
{f'<ul>{flagged}</ul>' if flagged else '<p>None.</p>'}
<p><a href="ensemble.json">Measurements and checks</a> · <a href="ensemble.md">Seed tables</a> · <a href="overview.svg">SVG overview</a></p>
"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input_json", type=Path)
    parser.add_argument("output_html", type=Path)
    args = parser.parse_args()
    data = json.loads(args.input_json.read_text(encoding="utf-8"))
    panels = charts(data["standard"])
    args.output_html.parent.mkdir(parents=True, exist_ok=True)
    args.output_html.write_text(html_overview(data, panels), encoding="utf-8")
    args.output_html.with_suffix(".svg").write_text(svg_overview(data, panels), encoding="utf-8")


if __name__ == "__main__":
    main()
