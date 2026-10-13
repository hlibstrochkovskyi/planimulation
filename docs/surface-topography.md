# Affine-bed storage and hydrostatic interface baseline

October 13, 2026. This implements the next bounded geometry/rest/barrier laboratory after the [non-affine planar gate](surface-non-affine-evolution.md). It is a separately named first-order head/storage hypothesis, not an extension, replacement or fallback of the flat-pressure operator. No production equations, seasonal selectors, model/save pins or desktop behavior change.

## Storage before flux

Each owner contains a monotone affine bed with known endpoint elevations. Its physical state remains the accepted compensated integrated depth stock. For positive mean depth `m`, bed span `d` and minimum bed `bmin`, infer a constant hydraulic head by its offset above `bmin`:

```text
d == 0:        offset = m
0 < m < d/2:  offset = sqrt(2*d*m)
m >= d/2:     offset = m + d/2
```

An independent triangle/trapezoid reintegration must reproduce the supplied mean within the separate unchanged `256*epsilon` relative moment limit. Scaled square-root arithmetic handles otherwise unrepresentable products; invalid/unrepresentable geometry or positive quantities refuse. Zero stock remains `Dry`, without a fabricated physical head. Width converts owned volume to a mean; the storage inverse itself is width-independent. No owner is fitted or corrected to the reconstructed head.

Interior faces use the larger adjacent **endpoint** bed as crest, relative head differences divided by width, and donor depth above that crest. The flux is `-sign(g)*min(cap*abs(g), hFace^(5/3)*sqrt(abs(g))/n)`. Exact zero drive, a dry donor or nonpositive over-crest depth gives zero before positive-branch arithmetic. Both flux branches must be finite and positive otherwise; NaNs/overflow cannot disappear through `min/max`. One computed signed face amount enters the existing closed paired transaction, with complete old-stock outgoing budgets and persistent high/low owners. Incoming water cannot finance the same step; no request is resized.

For numerical ordering only, `Dry` uses its minimum bed with zero head offset. This matches the positive-storage limit of the chosen local closure. A face-bed surrogate would introduce a discontinuity on a sloping receiver. Neither convention identifies a physical dry water level. See the [primary-source review and scope distinctions](surface-topography-source-review.md): the published method uses center beds, regularized gradients and implicit integration, whereas this endpoint/hypsometry/explicit-cap adaptation does not inherit its theorem.

One constant head per owner assumes local hydrostatic equilibration. A monotone affine bed has no unresolved internal ridge; endpoint data cannot encode a hidden interior barrier or independently filled subcell pools. The laboratory rejects unsupported input shapes and is not a general terrain representation.

## Fixed controls and results

All owners have width 1 m. Work is bounded by 128 owners, 4,000 steps and 192,000 cell-step updates per trajectory. The repeated closed cohort has 28 completed trajectories, 32,000 accepted steps and 136,000 cell-step updates. Four independent frozen interface steps and one deliberately refused trajectory remain separate.

- Twenty-four exact-representable rest cases combine shoreline bowl, unequal pools separated by a dry ridge, fully wet alternating bed, discontinuous bed, uniform water and dry terrain with reflection and datum `0/2^40 m`. All complete 1,000 steps with exactly zero computed flux and bit-identical high/low owners. No boundary exchange occurs.
- Four frozen interface controls retain below-crest, exact-crest, next-binary64-above-crest and ordinary above-crest states. The first two transfer zero; the latter two transfer positive water without a tolerance band. The next-float flux is approximately `3.03315e-33 m²/s`, with its small withdrawal retained in the donor's low component.
- Rising/falling receiver controls compare zero stock against means `1e-12/1e-24 m`. Flux differences decrease from `4.47214e-4/1.68550e-5` to `4.47244e-10/1.68541e-11 m²/s`, respectively. These check the declared dry ordering limit, not shoreline or variable-bed PDE accuracy.
- Three fully wet, constant-bed two-owner controls use cap `.01 m²/s`, time `1 s` and steps `.001/.0005/.00025 s`. Every active computed face stays capped. An independent discrete ODE, `delta(t)=.5*exp(-.02*t)`, gives maximum depth errors `4.90106e-8/2.45051e-8/1.22525e-8 m`: approximately halving under time refinement. This is not a continuum or spatial-order oracle.
- A complete-old-donor overdraw refuses its first requested step, retains zero accepted time, records `candidateCommitted=false` and preserves all owners. Computed-face diagnostics include an attempted refused face; they are not claimed committed transfers.

The non-dyadic equal-head initialization is intentionally not bit-identical after evolution: its independently rounded stocks reconstruct slightly different heads, producing maximum computed flux `5.55112e-15 m²/s`. Its low components retain the actual exchanges. It completes 1,000 steps without snapping or a gradient tolerance. Thus exact dyadic rest controls do **not** prove unconditional bit-exact well-balancing for arbitrary rounded geometry/stocks. All completed trajectories retain local, step-global and cumulative ledgers below the unchanged arithmetic limit.

The six existing flat-pressure coverage controls remain separate. Uniform water is accepted; peak, trough, mixed plateau, isolated wet owner and resting-bowl depth patterns retain explicit reconstruction refusals with inputs unchanged. The new geometric baseline does not silently rescue those trials or qualify general pressure reconstruction at extrema.

## Decision and reproduction

This closes the bounded **affine storage/rest/interface-blocking compatibility gate** for the new baseline. Moving variable-bed accuracy, general pressure/head reconstruction, intracell transients, spherical assembly, seasonal coupling and calibration remain open. Frozen over-crest direction is not calibrated overtopping discharge or a dam-break qualification. Next establish a stock-consistent bed-aware moving reconstruction/reference before selecting any production extension; do not promote the low-order baseline merely because rest tests pass.

```bash
cargo run --locked --manifest-path native/Cargo.toml --example surface_topography_report -- artifacts/new-topography.json
cargo test --locked --release --manifest-path native/Cargo.toml --lib hypsometry
cargo test --locked --release --manifest-path native/Cargo.toml --lib topography
```

The CLI computes the complete bounded report before creating a new output and refuses overwrite/extra arguments. Three refusal checks preserve existing output and leave the invalid extra-argument path absent. The [validation record](data/surface-topography-validation.json) retains resolved inputs, outcomes, limitations, validation scope and fingerprints. Complete reports remain ignored local artifacts.

Full `npm test` passes 637 native tests across 118 targets, with the same one pre-existing ignored test, and all eighteen Node files (88 tests), including typecheck and a rebuilt native release executable. Optimized regression passes 182 library, twelve regional-soil and five regional-surface tests (199 total). That optimized regression precedes the last test-only assertions and fixed-array chunk iteration; the final three optimized topography tests and strict all-target Clippy include them. Six hypsometry tests, formatting and whitespace checks pass. Physical/reference arithmetic does not change between those validation phases.

The 109,281-byte report repeats exactly and matches optimized output, SHA-256 `3ef45f4303b45da063f9dc3dafea95e457bf119bd3b388023b852b27065ca348`. Rebuilt non-affine output preserves `637bbf678092f9fca352c9de5c1548d3baf5a4e0eb979cdb3f2c352912577bba`. Owner renderer edits remain untouched and excluded. No GUI run or production performance claim follows.
