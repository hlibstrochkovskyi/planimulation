# Foot traversal cost and travel time: `traversal-1`

November 21, 2026. First movement increment, preparing the natural world for
population groups ("Movement takes time and food", [DESIGN.md](../DESIGN.md)).
`traversal-1` is a read-only analytical model computed on demand in the native
core. At 40,962 regions, building both seasonal graphs took 15–24 ms and a
Dijkstra query took 1–2 ms in the three-seed release benchmark. World recipes and
fingerprints exclude the analysis. Terrain, water, climate and vegetation are
read-only inputs.

## Purpose

Before population groups can migrate, forage across regions or establish trade
routes, the environment needs a reproducible, explainable movement cost over
the region graph. `traversal-1` assigns directed edge travel times in days for
local summer and local winter, and solves deterministic least-time travel fields
and routes from one or more origin regions.

## Model

Protocol 9 sends the resolved `traversal::Settings` with the cost frame.
The TypeScript client validates them against `src/core/traversal.ts`. The
remaining coefficients are fixed by `traversal-1` and recorded below. The
analysis has no random stream.

**Edge speed from slope.** Each directed neighbor edge $u \to v$ of spherical
center distance $d_{uv}$ (meters) uses [Tobler's hiking function](https://escholarship.org/content/qt05r820mz/qt05r820mz.pdf)
on surface height $h$ (`water.level_meters` on frozen lakes, `terrain.elevation`
on dry land):

$$v_{\text{tobler}}(s_{uv}) = \max\!\left(0.05,\; 6 \exp\!\bigl(-3.5 \, |s_{uv} + 0.05|\bigr)\right) \text{ km/h}, \qquad s_{uv} = \frac{h(v) - h(u)}{d_{uv}}$$

Flat ground ($s = 0$) yields $5.04\text{ km/h}$; the peak ($6.0\text{ km/h}$)
occurs at a gentle $5\%$ downhill grade ($s = -0.05$). For equal-magnitude
grades, uphill travel is slower whenever the speed floor does not bind both.

**Neighborhood ruggedness proxy.** `World` stores one scalar bed elevation per
region and no sub-grid relief or roughness statistic. At subdivision 6
(~111 km spacing), direct center-to-center slopes rarely exceed $0.04$, under
which Tobler's function alone slows mountain belts by less than 15%. To reflect
unresolved valleys, passes and switchbacks, each dry region's mean absolute
surface-height difference to its neighbors, $\overline{\Delta h}_i$ (with wet
neighbors clamped to `water.level_meters` so ocean trenches do not create false
coastal cliffs), gives a coarse-grid ruggedness multiplier:

$$m_{\text{rugged}}(i) = \frac{1}{1 + \overline{\Delta h}_i / R_0}, \qquad R_0 = 600\text{ m}$$

Frozen lakes have $\overline{\Delta h}_i = 0$ ($m_{\text{rugged}} = 1$).
The 600 m scale leaves a 100 m neighborhood at 86% of its slope-adjusted speed
and halves it at 600 m. The term is uncalibrated across resolutions. It can also
penalize a broad, smooth escarpment while missing deep valleys inside a cell.

**Land cover multipliers.** The values are model assumptions informed by
cross-country measurements of movement and energy cost under different surfaces.
Each dry region's cover factor separates climate stress
(ice, tundra bog, desert heat/sand and tropical warmth) from woody obstruction:

$$m_{\text{cover}}(i) = m_{\text{climate}}(i) \cdot \bigl(1 - k_w(i)\, w_i\bigr)$$

When a `vegetation-2` state exists in the session, $w_i \in [0, 1]$ is the
region's current `woody` cover fraction (so multi-year drought dieback speeds up
passage and forest regrowth slows it). Otherwise $w_i$ falls back to
`climate-normals-1` biomes ($0.95$ for forest biomes, $0.15$ for savanna, $0$
for grassland, desert, tundra and ice).

| Terrain / cover class | $m_{\text{climate}}$ | Woody drag $k_w$ | Flat summer speed (km/h) | Flat 8 h/day (km/day) | Basis |
| --- | :---: | :---: | :---: | :---: | --- |
| Temperate grassland ($w = 0$) | 1.00 | 0.35 | 5.04 | 40.3 | Baseline open ground (Tobler flat; Soule & Goldman $\eta \approx 1.1$) |
| Savanna ($w = 0.15$) | 0.92 | 0.35 | 4.39 | 35.1 | Open grass with scattered trees and tropical warmth |
| Cold desert ($w = 0$) | 0.78 | 0.35 | 3.93 | 31.4 | Loose gravel/scrub and sparse water |
| Temperate forest ($w = 0.95$) | 1.00 | 0.35 | 3.36 | 26.9 | Off-path woodland / brush ($\eta \approx 1.5$) |
| Tropical seasonal forest ($w = 0.95$) | 0.92 | 0.35 | 3.09 | 24.7 | Seasonal undergrowth and warm humidity |
| Boreal forest ($w = 0.95$) | 1.00 | 0.42 | 3.03 | 24.2 | Conifer deadfall and mossy/peaty ground ($\eta \approx 1.6\text{--}1.8$) |
| Hot desert ($w = 0$) | 0.58 | 0.35 | 2.92 | 23.4 | Loose sand ($\eta \approx 1.8\text{--}2.1$), heat-limited hours and water load |
| Tundra / summer bog ($w = 0$) | 0.55 | 0.00 | 2.77 | 22.2 | Thawed active layer, tussock and muskeg ($\eta \approx 1.8\text{--}2.0$) |
| Tropical rainforest ($w = 0.95$) | 0.92 | 0.51 | 2.39 | 19.1 | Dense jungle understory, wet soils, heat ($\eta \approx 2.0\text{--}2.4$) |
| Ice sheet / glacier ($w = 0$) | 0.30 | 0.00 | 1.51 | 12.1 | Heuristic for difficult passage on firn, crevassed terrain and in extreme cold |
| Deep-winter frozen inland lake | — | — | 3.78* | 30.2* | Lake ice ($m_{\text{cover}} = 0.75$) before winter snow factor |

\*Before multiplying by $m_{\text{snow}}$ (typically $0.45\text{--}0.65$, yielding
13–20 km/day on frozen lakes). See [walking terrain experiments](https://doi.org/10.1371/journal.pone.0228682)
and [velocity-based terrain coefficients](https://intarch.ac.uk/journal/issue45/4/1.html).

**Seasons and snow.** `Summer` represents each region's local warm season (no
seasonal snowpack); `Winter` represents each region's local cold season. From
`climate-normals-1` monthly temperature and precipitation normals, let $n_{<0}$
be the count of months below $0\,^\circ\text{C}$ and $P_{<0}$ (mm) the
precipitation falling during those months. The winter snow severity index is

$$S_i = \min\!\left(1,\; \frac{n_{<0}}{6}\right)\left(0.35 + 0.65 \min\!\left(1,\; \frac{P_{<0}}{300\text{ mm}}\right)\right), \qquad m_{\text{snow}}(i, \text{Winter}) = \frac{1}{1 + 1.5\, S_i}$$

Regions with $n_{<0} = 0$ have $m_{\text{snow}} = 1$; deep snowy winters
($S_i = 1$) slow travel to $0.40\times$ summer speed. The 0.35 baseline gives
cold months a penalty even in very dry climates. Snow walking experiments
motivate the order of magnitude, while the index and conversion to speed are
model assumptions. [Pandolf et al.](https://pubmed.ncbi.nlm.nih.gov/1009916/)

**Water, frozen lakes and river crossings.**

- **Main ocean** (`body_id == main_ocean_id`): always impassable on foot in both
  seasons (sea ice is not modeled).
- **Inland lakes** (`body_id != 0 && body_id != main_ocean_id`): impassable in
  summer; passable in deep winter when `coldest_month <= -10 °C` and
  `freezing_months >= 3` (flat ice with $m_{\text{cover}} = 0.75 \times m_{\text{snow}}$).
- **River crossings (proxy):** `rivers-1` discharge is developed separately;
  `traversal-1` does not depend on it. Instead, `river_crossing_delay_from_area`
  proxies unfrozen crossing delay from static C1 drainage contributing dry-land
  area $A_i$:
  $$D_{\text{river}}(i) = 8\text{ h} \cdot \max\!\left(0,\; \log_{10}\!\left(\frac{A_i}{A_{\text{thresh}}(i)}\right)\right), \qquad A_{\text{thresh}}(i) = \max\!\left(10^{11}\text{ m}^2,\; 8\,\text{area}_i\right)$$
  At subdivision 6 (~12,450 km² per region), $A_{\text{thresh}} = 100{,}000\text{ km}^2$:
  a $10^6\text{ km}^2$ basin adds 8 hours (1 day) for fording/rafting. Crossing
  an edge $u \to v$ pays $\frac{1}{2}(D_u + D_v)$, scaled by $0.25$ when
  walking along the drainage link (`receivers[u] == v || receivers[v] == u`).
  A cell satisfying the deep-winter temperature rule contributes zero delay.
  The function is
  isolated so a hydraulic discharge field ($\text{m}^3/\text{s}$) can replace it
  without changing the edge integrator or Dijkstra solver.

**Edge time and walking day.** Walking half of $d_{uv}$ through region $u$ and
half through region $v$ takes

$$t_{\text{hours}}(u \to v) = \frac{d_{uv}/2000}{v_{\text{tobler}}(s_{uv})\, m_u} + \frac{d_{uv}/2000}{v_{\text{tobler}}(s_{uv})\, m_v} + t_{\text{river}}(u \to v), \qquad t_{\text{days}}(u \to v) = \frac{t_{\text{hours}}(u \to v)}{8\text{ h/day}}$$

where $m_i = m_{\text{rugged}}(i)\, m_{\text{cover}}(i)\, m_{\text{snow}}(i, \text{season})$.
The per-region summary field (`hours_per_100km`) averages round-trip hours per
100 km across passable incident edges (or uses flat local pace for isolated
single-cell islands).

**Least-time travel field.** `Model::travel_time_multi` runs deterministic
binary-heap Dijkstra over the precomputed CSR edge-day arrays from one or more
origins (up to 64). Heap pops break ties by smaller region ID, and edge
relaxation breaks exact time ties by shorter accumulated distance then smaller
predecessor ID, producing unique, reproducible travel times, route lengths and
predecessor trees.

## Desktop

Movement costs load with climate normals and refresh after vegetation advances
or restarts. The two Movement layers use the initial water geography, with
winter passability indicated on eligible lake cells.

- **Travel cost** shows a fixed logarithmic scale of 20–160 h/100 km over
  shaded relief.
- **Travel time** shows continuous log-time color and labeled contour levels
  from 7 to 1,460 days. Gray land is **Unreachable on foot (water)**. The
  displayed contours interpolate between region values and the inspector shows
  the underlying region value.
- **Summer / Winter** recomputes from the current origin. If thaw closes an origin
  on a frozen lake, the selection of origin is cleared.
- Choose **Travel time from here** on a passable region, then select a
  destination. The map and globe share an origin ring and a light route line
  with a dark outline. The flat line splits at the antimeridian. On the globe,
  **Frame route on globe** centers the camera on the route or the origin.
  The Overview and **Why this travel cost?** inspector show time, route length,
  walking pace and the slope, cover, snow and river factors.

`node --import tsx scripts/traversal-review.ts [OUTPUT_DIR] [seed]` captures cost
and travel time in both seasons and projections, plus the Factors inspector.
The review selects four separated origin types on the largest landmass.

## Measurements

Default new-world recipe (`multiscale-geography-4`, 40,962 regions, year-0
`vegetation-2` cover), `cargo run --release --locked --manifest-path native/Cargo.toml --example traversal_report`:

### Widest landmass end-to-end crossings

| Seed | Largest landmass | Great-circle span | Summer time / route / pace | Winter time / route / pace | Frozen lake cells / river cells | Setup / Dijkstra |
| --- | --- | ---: | --- | --- | :---: | --- |
| `first-light` | 3,881 regions (48.74 M km²) | 10,160 km | 422.6 d · 11,061 km (**26.2 km/d**) | 715.6 d · 11,074 km (**15.5 km/d**) | 5 / 597 | 23.9 ms / 1.7–1.7 ms |
| `5c151f46` | 8,198 regions (101.58 M km²) | 18,111 km | 923.2 d · 22,873 km (**24.8 km/d**) | 1,491.4 d · 22,816 km (**15.3 km/d**) | 44 / 644 | 19.6 ms / 1.8–1.8 ms |
| `4836187e` | 7,291 regions (90.63 M km²) | 19,623 km | 973.5 d · 26,443 km (**27.2 km/d**) | 1,445.7 d · 27,302 km (**18.9 km/d**) | 25 / 99 | 15.3 ms / 1.0–1.1 ms |

The modeled summer routes average **24.8–27.2 km/day**, within the
**20–30 km/day** historical comparison band. For example, the ORBIS project
uses 20 km/day for porters and 30 km/day for people traveling on foot.
Winter routes average **15.3–18.9 km/day** in the model. The days denote
walking days under perpetual local-season conditions. [ORBIS methodology](https://orbis.stanford.edu/orbis2012/ORBIS_v1paper_20120501.pdf)

### Median travel cost and daily pace by terrain class (summer / winter)

| Terrain class | `first-light` | `5c151f46` | `4836187e` |
| --- | --- | --- | --- |
| Temperate grassland | 23.4 / 24.3 h (34.3 / 32.9 km/d) | 23.1 / 25.8 h (34.7 / 31.1 km/d) | 23.3 / 23.5 h (34.3 / 34.1 km/d) |
| Savanna | 24.8 / 24.8 h (32.3 km/d) | 24.6 / 24.6 h (32.5 km/d) | 24.6 / 24.6 h (32.5 km/d) |
| Cold desert | 27.7 / 28.5 h (28.8 / 28.0 km/d) | 27.3 / 27.7 h (29.3 / 28.9 km/d) | 27.6 / 27.6 h (29.0 km/d) |
| Temperate forest | 34.8 / 42.4 h (23.0 / 18.9 km/d) | 34.3 / 40.0 h (23.3 / 20.0 km/d) | 32.9 / 37.5 h (24.3 / 21.3 km/d) |
| Tropical seasonal forest | 32.4 / 32.4 h (24.7 km/d) | 33.2 / 33.2 h (24.1 km/d) | 32.8 / 32.8 h (24.4 km/d) |
| Hot desert | 35.8 / 35.8 h (22.3 km/d) | 34.6 / 34.6 h (23.1 km/d) | 35.8 / 35.8 h (22.4 km/d) |
| Boreal forest | 37.2 / 79.6 h (21.5 / 10.1 km/d) | 37.9 / 81.8 h (21.1 / 9.8 km/d) | 38.1 / 84.4 h (21.0 / 9.5 km/d) |
| Tundra | 40.4 / 94.7 h (19.8 / 8.5 km/d) | 40.0 / 90.0 h (20.0 / 8.9 km/d) | 39.8 / 95.6 h (20.1 / 8.4 km/d) |
| Tropical rainforest | 46.8 / 46.8 h (17.1 km/d) | 45.5 / 45.5 h (17.6 km/d) | 45.4 / 45.4 h (17.6 km/d) |
| Ice sheet | 73.0 / 156.6 h (11.0 / 5.1 km/d) | 73.2 / 147.2 h (10.9 / 5.4 km/d) | 70.8 / 167.7 h (11.3 / 4.8 km/d) |
| Relief: flat (<100 m) vs mountains (>600 m), summer | 29.6 vs 62.2 h (27.1 vs 12.9 km/d) | 35.7 vs 56.0 h (22.4 vs 14.3 km/d) | 26.8 vs 51.1 h (29.8 vs 15.6 km/d) |

Units in the table are median hours per 100 km (`h`) and implied km/day at
8 h/day (`km/d`).

## Acceptance checks

Native unit tests in `native/src/traversal.rs`:

- Tobler flat speed (~5.04 km/h), downhill peak at $-5\%$ (6.0 km/h), and
  uphill/downhill asymmetry across grades $0.02\text{--}0.35$;
- deterministic Dijkstra matches exact brute-force all-paths enumeration on a
  small asymmetric directed graph with an unreachable node;
- directed triangle inequality $T(a, c) \le T(a, b) + T(b, c)$ holds across
  sampled land regions in both summer and winter;
- impassable water separates disconnected landmasses in summer and ocean blocks
  travel in both seasons, while a deep-winter lake cell becomes reachable from
  an adjacent dry cell;
- strict monotonic slowdown with woody cover (pure factor and full-world
  sparse vs dense comparison) and winter snowpack;
- deep-winter freeze thresholds, river-area proxy thresholds and multi-source
  Dijkstra equaling the elementwise minimum of single-source fields;
- bit-identical determinism and route-sum agreement with Dijkstra distances.

`tests/traversal.test.ts` checks protocol 9 framing, biome fallback before
vegetation initialization, automatic refresh after vegetation initialization
and advancement, summer/winter Dijkstra and route tracing, multi-source queries,
and malformed request/packet rejection. `scripts/test-desktop.ts` verifies the
new layers, season toggle, "Travel time from here", isochrone legend, inspector
explanation, globe/flat view switching and unchanged world fingerprint.

## Limitations

- **No sub-grid relief field:** `World` stores one scalar elevation per region;
  the neighborhood-relief term (`R_0 = 600 m`) is an uncalibrated coarse-grid
  proxy for unresolved mountain ruggedness, not measured micro-topography.
- **River crossings use drainage area, not discharge:** arid catchments with
  large contributing areas receive the same unfrozen crossing penalty as humid
  basins of equal area until `rivers-1` discharge replaces
  `river_crossing_delay_from_area`.
- **Local seasons rather than a single planetary date:** "Summer" and "Winter"
  evaluate every region in its own warm or cold season, so trans-equatorial
  journeys do not model opposite-hemisphere calendar phasing (or multi-season
  journeys lasting over a year).
- **Foot travel only:** no boats, coastal navigation, pack animals, roads,
  food/water consumption or carrying-capacity exhaustion (those belong to
  population groups and infrastructure).
- **Snow and ice heuristics:** the ocean remains impassable. The lake and
  river rules use monthly air temperature at each region and omit ice thickness,
  water depth, current and lake-wide thermal inertia. Winter snow severity uses
  `climate-normals-1` monthly normals.
