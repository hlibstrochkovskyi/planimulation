# Continuous physical initial perturbation review

October 16, 2026. Native `physical-seed-spectrum-1`, diagnostic
`fine-landscape-review-3`. This is a bounded initial-condition field and a
controlled comparison, not a selected World family or an accepted landscape fix.

## Field definition

The previous patch seed field consists of six 4 km spherical sine modes with a
1 m amplitude envelope. The new field samples the existing frozen rotated
quintic value-noise implementation in global 3D directions, combining four
positive, normalized bands:

| Correlation lattice spacing | Weight |
| ---: | ---: |
| 8,000 m | 0.125 |
| 4,000 m | 0.250 |
| 2,000 m | 0.250 |
| 1,000 m | 0.375 |

These are lattice correlation spacings, **not sinusoidal wavelengths**. Each
band has an independent `physical-seed-spectrum-1.band-N` random stream.
Frequency is radius divided by resolved physical spacing. The field depends
on a seed and physical parameters, never a target grid, camera or query order.
It has no atlas seam/pole discontinuity and no mutable sampling RNG state.

The analytical amplitude bound follows convex interpolation and normalized
weights. Sampling checks a rounding envelope rather than clipping output.
Identity records seed, settings, random pin, noise dependency and resolved
band lengths/weights/streams. Do not modify the frozen noise/RNG dependencies
in place while retaining this field pin. Bounds are radius 100–20,000 km,
amplitude 0–1,000 m, dimensionless length scale 0.01–1,000, and a nonblank seed
of at most 128 UTF-16 units. Samples require finite unit directions.

This is physical initial relief before geological preparation, not shader
texture or a claim to have simulated tectonics/erosion. It creates no ongoing
material, water, resource or climate flow. Existing subsystem streams and
geological-source geometry are unchanged.

## Controlled comparison

```sh
cargo run --locked --release --manifest-path native/Cargo.toml --example fine_landscape -- artifacts/NEW_DIRECTORY first-light 64 4800000 1 --spectrum

# A separate initial-relief magnitude control, not a new calibration:
cargo run --locked --release --manifest-path native/Cargo.toml --example fine_landscape -- artifacts/OTHER_DIRECTORY first-light 64 4800000 1 --spectrum 100
```

`--spectrum` includes the three existing preparation variants, defaults to a
1 m envelope, and accepts an optional recorded amplitude. Old default and
`--facets` paths retain their previous identities/results. The raw six-column
format is unchanged. The read-only routing observer explicitly accepts known
review versions 1–3 and rejects unknown future versions.

Geometry, initial geological bed, all uplift rates, K, D, duration and steps
remain fixed across paired initial-field controls. Only active interior seed
values change; the existing external collar heights are preserved exactly.
Synthetic collar height remains zero. Generated collar heights retain the
legacy perturbation, whose pin/parameters are recorded alongside the new field.

Equal amplitude envelopes do not mean equal realized RMS, orientation or
correlation structure. The observer records area-weighted interior means,
standard deviations and extrema for both fields. At generated 250 m sampling,
standard deviations are 0.292 m (legacy), 0.210 m (new 1 m envelope), and
20.975 m (new 100 m envelope). This is a change of initial field, not an isolated
single-frequency intervention or a literal reproduction of published data.

[Fastscapelib's mountain example](https://fastscapelib.readthedocs.io/en/latest/examples/mountain_py.html)
uses independent per-node initial perturbations and a separate sink resolver.
We preserve common physical point queries instead of reseeding when the grid
changes. Its exponent/coefficient and depression assumptions also differ;
these runs are not a reproduction or Earth calibration of that example.

## Results: the initial-field hypothesis is insufficient

First-light, 250 m sampling, facet preparation, D=1 m²/year, 4.8 Myr:

| Initial field | Synthetic closed area | Generated p95 height | Generated 1 km transect RMS slope |
| --- | ---: | ---: | ---: |
| Legacy six waves, 1 m bound | 4.57% | 6,171 m | 0.0753 |
| New spectrum, 1 m bound | 0% | 6,169 m | 0.0740 |
| New spectrum, 100 m bound | 8.98% | 6,171 m | 0.0694 |

The new spectrum changes drainage, but actual D=1 beds remain broad and smooth.
Larger initial roughness does not establish a successful mountain generator.
At D=0.01 with the 100 m spectrum, generated closed share reaches 46.61%
(9 minima), and synthetic share 47.78% (48 minima). Lower D is not selected:
its 100 m characteristic process length remains below 250 m sampling.
Do not promote a field merely because one synthetic closed-area count improves.

Current local artifacts: `physical-seed-repeat`, `physical-seed-verified-one-meter`,
`physical-seed-amplitude-100`, `physical-seed-amplitude-100-repeat`,
`physical-seed-low-diffusion-100`, and compatibility run `physical-seed-old-control`.
Earlier `physical-seed-landscape` / `physical-seed-final` records retain the same
physical 1 m experiment with preliminary diagnostic metadata. These ignored
paths are not permanent fixtures. Generated/synthetic actual-height images
were inspected; no default/UI family is changed by this review.

## Next model comparison — proposed, not implemented

The reference's depression-routing assumption is a distinct remaining difference.
Our current patch follows real bed downhill and leaves dry basins as terminals.
A separate humid-geological-prehistory comparison may instead use a potential
fully connected lake surface, without raising physical bed or creating actual
water stock. [Priority-Flood](https://rbarnes.org/sci/2014_depressions.pdf) supplies
a known minimax geometric construction; our existing depression analysis already
uses a related escape-level calculation on the global surface.

Such a candidate must have a separate pin and explicit long-term runoff/lake
assumptions. Potential storage/filled geometry is not a funded lake inventory.
Protected submerged bed, actual accepted erosion/material accounting, unchanged
initial sources and separate actual-bed versus proxy-routing diagnostics are
required. It must not replace the conservative seasonal water model, fill/breach
the actual bed silently or call every real closed basin erroneous. A successful
headless comparison still needs R3 visual/cohort review and R4 scale coupling.

## Verification scope

Field controls cover bounds, deterministic independent query order, radius/length
scaling, amplitude scaling, seam/pole continuity, seed limits and zero-amplitude
validation. An independent two-sample calculation verifies area-weighted summary
statistics. Existing preparation/graph/old-field controls remain in the focused
suite. Prior full native results predate this field; they are not claimed as
new-field verification. Concurrent owner UI work is excluded.

Final verification: 45 focused tests across eight targets, strict all-target
Clippy, formatting and scoped whitespace pass. The old path repeats all 18 raw
records and report data except timings. Both new amplitude controls also repeat
their 18 records and reports exactly apart from timings. Sources, geometry,
every uplift rate, preparation parameters and every boundary bed value match
the paired baseline. Independent decomposition checks pass on 54 physical
records (maximum discrepancy `1.37e-11 m`). Eight actual CLI refusals create no
output, and the version-3 record passes read-only routing inspection.
