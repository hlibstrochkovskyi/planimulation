# Moving affine-bed reconstruction comparison

October 13–14, 2026. This adds an independently checked moving-front reference and
a separately named `AffineHeadSupportedMinmodPenaltyOne` candidate after the
[hydrostatic storage/rest baseline](surface-topography.md). It changes no
selected seasonal model, physical transaction, checkpoint pin or desktop mode.
Working simulation features and this numerical laboratory remain distinct.

## Exact reference and ownership

The [primary-source review](surface-moving-bed-source-review.md) distinguishes
published numerical methods from the project's exact traveling solution. With
`bed=-S*x`, `S=.001`, `c=.1 m/s`, `n=.04` and `A=(n*c)^2`, use

```text
h(x,t) = H(x-c*t)
H' = S - A*H^(-4/3)
q = c*H
0 < H < D = (A/S)^(3/4)
```

Direct substitution verifies source-free continuity and original uncapped
Manning flow. Height and flux both vanish at the advancing dry front. Exterior
flux is prescribed on a bounded strip, not a closed planetary experiment.
The actual reference coefficient remains below the declared cap of `100 m²/s`.

Reference-only integrated means and interval boundary exchanges use independent
positive series. For `u=(H/D)^(1/3)` and `L=D/S`:

```text
distance = 3*L * integral[0..u] v^6/(1-v^4) dv
volume primitive = 3*D*L * integral[0..u] v^9/(1-v^4) dv
```

The branch is explicitly bounded by `u<=.9`. Adjacent primitive differences
are evaluated as positive interval series, using `log1p/expm1`, rather than
subtracting independently rounded large primitives. Root inversions retain
their own residual and work bounds. Initially dry exact means are exactly zero.

The Python verifier independently evaluates elementary antiderivatives,
bracketed Newton inverses and direct rational-integrand quadrature at 80 and
100 decimal digits. Cancellation in those elementary expressions led to a
declared 68/88-digit reference residual, not a change to physical solver bounds.
The extra `.0625 m` controls fail the initial **reference-only** `256*epsilon`
relative probe: two rounded inverse endpoints amplify error in a narrower
interval. Maximum observed reference error is `1.61e-15 m`, or `8.25e-14`
relative. The verifier retains that failed probe and separately applies a
declared `512*epsilon` reference limit. This is a reported oracle limitation,
not a physical correction: solver moment and ledger bounds remain unchanged
at `256*epsilon`. It does not establish arbitrary-width oracle accuracy.

Short-interval reference probes retain inverse-endpoint conditioning: at a
requested `1e-10 s`, relative boundary-amount error is about `2.22e-4`, although
absolute error is only about `4.36e-17 m²`. These probes are not evolved or
qualified timesteps. Actual transactions own the rounded `q*dt` amount; the
report independently compares their counters with whole-interval reference
exchange. No invisible boundary correction is applied.

## Named candidate, not a fallback

Each accepted owner retains its original compensated integrated stock. Initial
hypsometry supplies a zero-order head offset above the actual bed minimum.
For wet owners, a minmod limiter uses neighboring numerical head differences,
including the declared minimum-bed ordering surrogate of a dry neighbor. A dry
owner still has no inferred physical head or slope. Boundary slopes are zero.

Limit the half-cell head variation to the owner's zero-order storage offset.
This declared geometric support bound makes an owner's slope tend to zero with
its positive stock. Merely requiring all neighbors to be wet had introduced a
finite dry-to-tiny-positive flux discontinuity and was rejected.

Fit an affine head against the effective bed `bed-slope*(x-center)` by the
existing clipped affine storage inverse. Retain its anchor and positive offset
separately through moment, gradient and crest differences: collapsing them
first had erased small partial-cell water and correctly triggered a refusal.
Independent endpoint triangle/trapezoid integration retains the unchanged
`256*epsilon` moment bound. Positive trace averaging preserves representable
subnormal flat means and avoids overflow of a large positive sum.

The shared interface uses average reconstructed slope plus penalty-one head
jump and the donor's reconstructed depth above the actual endpoint crest. In
fully wet cells the slope terms cancel algebraically, leaving a center-head
secant. The improvement is donor conveyance and partial-wetting storage, not a
new higher-order gradient theorem. The original capped Manning branches and
complete old-stock donor budgets remain; no incoming same-step credit, film,
head snapping, resized transfer or hidden operator substitution is introduced.

## Controlled comparison and limits

Both named methods receive identical initial stocks, bed, clocks and exterior
exchange. The strip is `[-3.5,.5] m`, ending at `2 s`; the exact front advances
`.2 m`. Initial controls use widths `.5/.25/.125 m` and steps
`.004/.002/.001 s`, plus reflected, datum-shifted and off-grid initial-front
controls. Further `.0625 m` controls use `.002/.001 s` within the same declared
128,000 cell-step work bound per trial.

Full integrated-mean RMS, initial-dry-owner arrival and contour observations are
separate. Contours interpolate cell means at centers; their bias on independent
exact means remains visible and is never subtracted from physical errors.
Missing or nonunique crossings retain null errors. Positive numerical tails
do not identify an exact support boundary. Field comparisons cancel compensated
components before projecting to means and exclude refused trajectories.

At the original finest `.125 m` width and `.001 s` cadence, RMS decreases from
`3.03423425e-4` to `1.07109759e-4 m`, about **2.83 times**. The 1 cm contour
error decreases from `25.011` to `7.784 mm`, and the 1.5 cm contour from `10.867`
to `1.429 mm`. The thin 1 mm contour remains weaker: `106.943` to `63.330 mm`,
with `43.244 mm` exact-mean observation bias. The coarse thin contour is missing.
This is useful bounded improvement, not a claim of accurate arbitrary fronts.

At `.0625 m` and the same `.001 s` cadence, candidate RMS is `7.52968813e-5 m`
versus baseline `2.39697658e-4 m`, **3.18 times** smaller. Candidate 1 mm contour
error is `17.245 mm` versus `63.322 mm`; exact-mean observer bias is `5.895 mm`.
The 1 cm and 1.5 cm candidate errors are `3.289/.488 mm`. The front crosses
3.2 finest owners; this is stronger than the preceding 1.6-owner control, not
an unrestricted long-propagation result. All 28 moving trajectories complete,
with 39,000 accepted steps and 1,160,000 total cell-step updates. No actual
interior face uses the cap. The separate deliberate overdraw refuses before
any owner or clock is committed.

Ten candidate unit tests cover storage inversion, the cancellation witness,
positive subnormals, extrema/plateaus, three/four-owner dry continuity, crest
thresholds and overdraw. All 24 original exact-representable rest/reflection/
datum controls preserve owners bit-for-bit over 1,000 steps each. Moving
reflection and datum results retain their measured floating differences;
bit-exact moving reflection is not promised.

The [validation record](data/surface-moving-bed-validation.json) contains final
cohort outcomes, independent reference checks, command results and fingerprints.
Full `npm test` passes 655 native tests across 119 targets, with the same one
pre-existing ignored test, and all eighteen Node files (88 tests), including
type checking and native release rebuild. Optimized regression passes all 200
library tests and seventeen regional-soil/surface-flow tests. Strict all-target
Clippy, formatting, whitespace and three CLI refusal checks pass. No GUI
validation or production performance follows from this laboratory.

The 780,897-byte complete report repeats exactly and matches optimized output,
SHA-256 `b01257372b2d8e21341d03b183de0895433416d4e12230b89dddc52ca25fdefa`.
The rebuilt earlier topography report retains
`3ef45f4303b45da063f9dc3dafea95e457bf119bd3b388023b852b27065ca348`.
Owner renderer changes remain untouched and excluded from this increment.

## Remaining decision

Do not promote this candidate on conservation or this single traveling family.
Thin-front accuracy, arbitrary-phase propagation over multiple owners,
recession, interacting fronts, non-affine beds with peaks/plateaus and the
spherical full-gradient assembly remain open. Explicit donor positivity is not
a diffusion stability theorem. The next algorithm choice must use these
measured limitations rather than treating every local test as completion of
general water dynamics.

The [next non-affine-bed conveyance proposal](surface-non-affine-bed-proposal.md)
defines a finite source-free flow-over-crest check with exact stationary means.
It is not implemented and would not replace the outstanding front or spherical
requirements.

Reproduce:

```bash
cargo run --locked --manifest-path native/Cargo.toml --example surface_bed_moving_report -- artifacts/new-bed-moving.json
cargo test --locked --manifest-path native/Cargo.toml --lib bed_reference
cargo test --locked --manifest-path native/Cargo.toml --lib head_reconstruction
cargo test --locked --manifest-path native/Cargo.toml --lib bed_moving
# Use an environment containing mpmath==1.3.0.
python scripts/verify-bed-moving-reference.py artifacts/new-bed-moving.json
```
