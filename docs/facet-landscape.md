# Receiver-constrained facet landscape preparation

October 16, 2026. Native `frozen-facet-incision-1` and opt-in
`fine-patch-facet-landscape-1`; observer `fine-landscape-review-2`.
This connects [facet routing](facet-routing.md) to actual bed changes in the
[bounded fine patch](fine-terrain-patch.md). It is not a World/recipe family,
desktop default, higher global resolution, seasonal-water replacement or
completed Earth-like landscape milestone.

## Incision equation and explicit numerical constraint

Freeze contributing area and the reconstructed unit descent direction for one
incision stage. The direction has nonnegative derivative coefficients:

```text
d = alpha_a v_a + alpha_b v_b             alpha: m^-1
S = sum(alpha_j (h_i - h_j))              S: dimensionless slope
base = sum(alpha_j h_j(new)) / sum(alpha_j)
gamma = dt K sqrt(A_i) sum(alpha_j)
h_unconstrained = (h_i(old) + gamma base) / (1 + gamma)
h_i(new) = max(h_unconstrained, max_j h_j(new))
```

`K` is year⁻¹ for the fixed `m=.5, n=1` case, `dt` years and `A` m². Angular
area-partition weights are not substituted for the derivative coefficients.
The implementation uses stable difference/interval forms rather than summing
large absolute heights. Process downstream receivers before donors on the DAG.
Terminals do not incise. Positive coefficients and no-higher receivers imply
that the accepted donor height lies between its highest updated receiver and
its own old height. The incision stage therefore neither raises old bed nor
reverses any selected branch. Subsequent hillslope transport can change routing;
this is not a claim that the entire coupled landscape cannot develop minima.

The receiver maximum is a declared **numerical process constraint**, not the
exact solution of the unconstrained stream-power equation. It freezes the old
direction/area and can limit erosion at a large step. It does not implicitly
solve new contributing areas or facet orientations. The previous valid-facet
counterexample demonstrated why an unconstrained weighted base is insufficient.

[Fastscapelib's eroder reference](https://fastscapelib.readthedocs.io/en/latest/api_cpp/eroder.html)
documents linear multiple-direction support, slope-reversal prevention and
limited-node diagnostics. This independently derived candidate constrains only
its selected receiver branches, not every possible neighboring slope. It is
not a copied implementation or a claim to reproduce that library's model.

## Accepted material versus prevented incision

Detach only `(old_height - accepted_height) * reference_area`. This actual loss
enters the existing reference-material bookkeeping reservoir, not modeled
deposition or material proven to leave the patch. The difference between the
constrained and unconstrained heights is recorded as **prevented incision**.
Its reference prism is hypothetical pressure, never an inventory credit or a
pending sediment donor. No water is created, reassigned or advanced.

Each stage reports limited-node count, per-node prevented thickness, prevented
prism and maximum local prevented thickness. Preparation retains cumulative
per-node prevented thickness and step totals. Sparse per-step records additionally
identify every limited node and its accepted/prevented incision thicknesses.
These can be summed back to the cumulative field. A subprecision numerical
no-op is reported separately from an explicit receiver-limited no-op.

Finite fields and coefficients, positive reference areas, no-higher receivers,
nonnegative normalized partitions, unique active receiver IDs and acyclic
topology are validated even at zero forcing. Work is bounded at 262,144 nodes.
Input arrays remain immutable on success and failure. Numerical validation does
not authenticate supplied gradients/contributing areas against a complete world
recipe; the coupled driver supplies them from the same physical route.

## Coupled driver and compatibility

Each geological step applies actual accepted uplift, rebuilds physical routing,
applies constrained facet incision, then uses stable paired hillslope substeps.
Cached tangent metrics do not depend on bed or camera and are constructed once
after workload preflight. The external fixed collar, real closed flats, physical
areas/distances, diffusion CFL target and separate geological clock remain as
in the existing patch experiment. The facet path has a larger declared work
estimate: `cells * steps * (6 + diffusion_substeps) <= 128,000,000`.

Actual-bed ledgers remain:

```text
net accepted bed prism = accepted uplift - actual detached material
                      + signed external hillslope exchange
```

Prevented incision does not appear on the right-hand side. Neither old global
active/pending sediment nor seasonal water stocks are touched. The shared driver
uses typed single/facet process adapters; the single-receiver pin remains
`fine-patch-landscape-1`. New optional diagnostic fields are omitted entirely
for that old path. Old physical fields and old histories/metrics remain exact.

## Reproducible comparison

```sh
cargo run --locked --release --manifest-path native/Cargo.toml --example fine_landscape -- artifacts/NEW_DIRECTORY first-light 64 4800000 1 --facets
```

The final `--facets` flag adds a third prepared variant and a fourth SVG panel:
initial bed, old incision-only, old coupled preparation, new facet preparation.
All panels use actual heights, a common elevation range and 1× slope lighting.
The default command without the flag retains review version 1. Opt-in version 2
keeps the same six f64-LE physical columns; receiver-constraint traces are
separate JSON diagnostics. The read-only `fine_routing` observer explicitly
accepts both known formats and the `facets` variant, not arbitrary future pins.

Current trace-bearing artifacts are `artifacts/facet-landscape-traced` and
`facet-landscape-traced-repeat`, with 32/128-step and low-diffusion controls in
`facet-landscape-traced-{32,128,low-diffusion}`. Earlier `facet-landscape-review`,
`facet-landscape-repeat`, `facet-landscape-temporal-{32,128}` and
`facet-landscape-low-diffusion` retain the same physical computations with
aggregate constraint diagnostics before sparse node traces were added. These
are local ignored artifacts, not checked-in fixtures or complete world saves.

## Results and limits

First-light, equal 50×50 km outer extents, 4.8 Myr, 64 steps, D=1 m²/year:

| Case | Spacing | Bed p95 (m) | Closed active area | Limited node updates |
| --- | ---: | ---: | ---: | ---: |
| Synthetic | 1,000 m | 2,242 | 0% | 0 |
| Synthetic | 500 m | 2,397 | 0% | 7 |
| Synthetic | 250 m | 2,401 | 4.57% | 289 |
| Generated | 1,000 m | 5,907 | 0% | 2 |
| Generated | 500 m | 6,108 | 0% | 158 |
| Generated | 250 m | 6,171 | 0% | 4,616 |

The generated 250 m case has about 0.184% limited active-node updates. Prevented
reference prism is 0.104% of actual detached material; the largest single local
prevented incision is 111.76 m. A small global fraction does not make this local
effect negligible. Actual-bed ledgers remain below `1.7e-16` relative in the
retained D=1 and D=0.01 controls.

With D=0.01, generated 250 m closed share changes from the old 43.33% to 28.75%,
but dry minima increase from 5 to 24. This is static routing ancestry, not lake
coverage. Sharp aligned valleys and broad unincised surfaces remain. The process
length `sqrt(D/K)=100 m` is below the finest 250 m sampling, so a sharper image
at this setting is not spatial qualification or a preferred calibration.

At D=1, 32→64→128-step bed RMS differences are 37.08→19.60 m (synthetic) and
47.73→24.60 m (generated). The generated final maximum difference is still
247.50 m. Its limited-node updates fall 6,067→4,616→1,945 and maximum prevented
incision 183.59→111.76→100.74 m. Routing/transient sensitivity remains despite
nearly stable elevation quantiles. Do not claim timestep convergence from these
three cases or from the separate fixed-graph analytic tests.

Actual new D=1 beds still have overly broad smooth forms. The D=0.01 control
still has aligned drainage and inadequately resolved process scales. Continuous
descent did not by itself produce acceptable Earth-like mountain variety.
Therefore this candidate is **not promoted** to desktop/default generation.
Next separate the retained initial perturbation spectrum and remaining spatial
representation effects under declared physical process scales; do not present
additional display noise or another global source reroll as an erosion fix.

The following [continuous initial-field review](physical-seed-spectrum.md)
holds geology, forcing and boundary bed fixed. Its 1 m and 100 m spectrum
controls change drainage but do not fix broad D=1 landforms; weak diffusion
retains problematic closed basins. Neither initial-field profile is promoted
as the missing landscape solution.

## Verification scope

Known single-link/chain solutions, an independent two-outlet result, explicit
limiter accounting, a fixed-two-outlet exponential refinement, no forcing,
terminals, subprecision observations and malformed/cyclic/overflow refusals
exercise the new primitive. Coupled tests verify actual no-increase/receiver
ordering during incision, repeatability, fixed boundaries, accepted bed/prism
decomposition, sparse/cumulative constraint reconciliation and work preflight.
Constant and enclosed-flat controls preserve old physical semantics.

Full native `cargo test --all-targets` passes 850 tests across 150 targets with
one existing ignored test. This run precedes the final sparse trace addition;
the final focused preparation/observer tests separately cover that addition.
All 40 focused tests across seven targets pass on the final trace-bearing code.
Eighteen raw records and complete trace reports repeat apart from timings;
14,940 sparse limited-node updates across 24 control rows reconcile exactly
with their cumulative prevented fields. Eight actual CLI refusals create no
output. The version-2 physical record also passes the read-only routing observer.
The earlier full 96 Node tests predate concurrent owner UI changes and are not
claimed as verification of the in-progress UI redesign. Strict all-target
Clippy, formatting, scoped whitespace, old-field compatibility and repeated
physical-record checks qualify the native increment, not the full natural-world
product or Earth calibration.
