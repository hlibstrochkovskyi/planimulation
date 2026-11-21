# Foot traversal cost and travel time: `traversal-1`

November 21, 2026. First movement increment, preparing the natural world for
population groups ("Movement takes time and food", [DESIGN.md](../DESIGN.md)).
`traversal-1` is a read-only analytical model computed on demand in the native
core (about 13 ms to build both seasonal cost graphs and 1–2 ms per Dijkstra
query at 40,962 regions). It is not part of the world recipe or fingerprint and
never alters terrain, water, climate or vegetation fields.

## Purpose

Before population groups can migrate, forage across regions or establish trade
routes, the environment needs a reproducible, explainable movement cost over
the region graph. `traversal-1` assigns directed edge travel times in days for
local summer and local winter, and solves deterministic least-time travel fields
and routes from one or more origin regions.

## Model

All resolved parameters live in `traversal::Settings` (mirrored in
`src/core/traversal.ts` and validated across protocol 9). No randomness is used;
the model is purely deterministic.

**Edge speed from slope.** Each directed neighbor edge $u \to v$ of spherical
center distance $d_{uv}$ (meters) uses Tobler's hiking function (Tobler 1993)
on surface height $h$ (`water.level_meters` on frozen lakes, `terrain.elevation`
on dry land):

$$v_{\text{tobler}}(s_{uv}) = \max\!\left(0.05,\; 6 \exp\!\bigl(-3.5 \, |s_{uv} + 0.05|\bigr)\right) \text{ km/h}, \qquad s_{uv} = \frac{h(v) - h(u)}{d_{uv}}$$

Flat ground ($s = 0$) yields $5.04\text{ km/h}$; the peak ($6.0\text{ km/h}$)
occurs at a gentle $5\%$ downhill grade ($s = -0.05$), and uphill travel is
strictly slower than equal-magnitude downhill travel.

**Sub-grid ruggedness proxy.** `World` stores one scalar bed elevation per
region and no sub-grid relief or roughness statistic. At subdivision 6
(~111 km spacing), direct center-to-center slopes rarely exceed $0.04$, under
which Tobler's function alone slows mountain belts by less than 15%. To reflect
unresolved valleys, passes and switchbacks, each dry region's mean absolute
surface-height difference to its neighbors, $\overline{\Delta h}_i$ (with wet
neighbors clamped to `water.level_meters` so ocean trenches do not create false
coastal cliffs), gives a coarse-grid ruggedness multiplier:

$$m_{\text{rugged}}(i) = \frac{1}{1 + \overline{\Delta h}_i / R_0}, \qquad R_0 = 600\text{ m}$$

Frozen lakes have $\overline{\Delta h}_i = 0$ ($m_{\text{rugged}} = 1$). This
neighborhood-relief term is a coarse-grid proxy, not measured micro-topography.

**Land cover multipliers.** Off-path terrain coefficients follow cross-country
foot-travel literature (Soule & Goldman 1972; Pandolf et al. 1977; Herzog 2014;
US Army FM 21-18). Each dry region's cover factor separates climate stress
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
| Ice sheet / glacier ($w = 0$) | 0.30 | 0.00 | 1.51 | 12.1 | Crevasses, firn, extreme cold and no forage |
| Deep-winter frozen inland lake | — | — | 3.78* | 30.2* | Lake ice ($m_{\text{cover}} = 0.75$) before winter snow factor |

\*Before multiplying by $m_{\text{snow}}$ (typically $0.45\text{--}0.65$, yielding
13–20 km/day on frozen lakes).

**Seasons and snow.** `Summer` represents each region's local warm season (no
seasonal snowpack); `Winter` represents each region's local cold season. From
`climate-normals-1` monthly temperature and precipitation normals, let $n_{<0}$
be the count of months below $0\,^\circ\text{C}$ and $P_{<0}$ (mm) the
precipitation falling during those months. The winter snow severity index is

$$S_i = \min\!\left(1,\; \frac{n_{<0}}{6}\right)\left(0.35 + 0.65 \min\!\left(1,\; \frac{P_{<0}}{300\text{ mm}}\right)\right), \qquad m_{\text{snow}}(i, \text{Winter}) = \frac{1}{1 + 1.5\, S_i}$$

Regions with $n_{<0} = 0$ have $m_{\text{snow}} = 1$; deep snowy winters
($S_i = 1$) slow travel to $0.40\times$ summer pace (matching Pandolf et al.
1977 snow-depth penalties of roughly 2–2.5×).

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
  walking along the drainage link (`receivers[u] == v || receivers[v] == u`),
  and drops to $0$ in deep winter when the river freezes solid. The function is
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

Movement costs compute automatically after vegetation starts and refresh when
vegetation advances or resets.

- **Layers (Movement group):**
  - **Travel cost**: hours per 100 km on a logarithmic scale (≤20 to ≥160
    h/100 km) for the selected season, over relief shading and bathymetric water
    (deep-winter frozen inland lakes appear passable in winter).
  - **Travel time**: stepped isochrone bands (`≤ 1 d`, `1–3 d`, `3–7 d`,
    `7–14 d`, `14–30 d`, `30–60 d`, `60–120 d`, `> 120 d`, plus muted slate for
    unreachable landmasses).
- **Season toggle:** Summer / Winter bar shown for both Movement layers, with a
  "Clear origin" button when an origin is active.
- **Inspector and route overlay:** Clicking any passable region enables
  **"Travel time from here"**; Overview rows and the Factors **Why this travel
  cost?** breakdown report slope/ruggedness, cover source (`vegetation-2` vs
  biome fallback), winter snow, river proxy, and—for any target region—travel
  time, route length and average pace. The least-time route between origin and
  inspected region is drawn as a polyline on both the 2D flat map (splitting
  cleanly across the antimeridian seam) and the 3D globe (following vertical
  exaggeration). Display changes never modify model data.

`node --import tsx scripts/traversal-review.ts [OUTPUT_DIR] [seed]` captures
the layers, season contrast, inspector breakdown and flat/globe route overlays.

## Measurements

Default new-world recipe (`multiscale-geography-4`, 40,962 regions, year-0
`vegetation-2` cover), `cargo run --release --example traversal_report`:

### Widest landmass end-to-end crossings

| Seed | Largest landmass | Great-circle span | Summer time / route / pace | Winter time / route / pace | Frozen winter lakes / river cells | Setup / Dijkstra |
| --- | --- | ---: | --- | --- | :---: | --- |
| `first-light` | 3,881 regions (48.74 M km²) | 10,160 km | 422.6 d · 11,061 km (**26.2 km/d**) | 715.6 d · 11,074 km (**15.5 km/d**) | 5 / 597 | 13.2 ms / 0.8–1.2 ms |
| `5c151f46` | 8,198 regions (101.58 M km²) | 18,111 km | 923.2 d · 22,873 km (**24.8 km/d**) | 1,491.4 d · 22,816 km (**15.3 km/d**) | 44 / 644 | 12.0 ms / 1.1 ms |
| `4836187e` | 7,291 regions (90.63 M km²) | 19,623 km | 973.5 d · 26,443 km (**27.2 km/d**) | 1,445.7 d · 27,302 km (**18.9 km/d**) | 25 / 99 | 12.3 ms / 1.3–1.4 ms |

Historical comparison: sustained overland foot travel by groups (pre-modern
armies, trade caravans, foraging bands with camp and rest) typically averages
**20–30 km/day** in fair seasons and drops toward **10–18 km/day** in winter
snow. Summer continental crossings across all three seeds average
**24.8–27.2 km/day**, and winter crossings **15.3–18.9 km/day**. On `5c151f46`,
the winter route is 57 km shorter than the summer route because deep-winter
frozen lakes and rivers open northern shortcuts even though snow slows total
duration by 62%.

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
  travel in both seasons;
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
- **No sea ice or dynamic snowpack:** main oceans stay impassable in winter,
  and winter snow severity comes from `climate-normals-1` monthly normals rather
  than a dynamic snow balance.
