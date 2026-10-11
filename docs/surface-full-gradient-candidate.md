# Full-gradient surface mobility candidate

October 11, 2026: implemented the explicitly selected **standalone** `regional-surface-full-gradient-candidate-1` stage. It changes nonlinear mobility, not spherical physical areas, adjacency, paired stock ownership or the rendering model. No existing seasonal selector, checkpoint, protocol or default can select it. The [old nonlinear witness](surface-constitutive-screen.md) remains reproducible. This increment improves bounded smooth weak work; it is not a general nonlinear evolution or dry-front accuracy qualification.

## Established models and the selected approximation

The literature provides useful models, not a drop-in planetary simulator:

| Model | What it supplies | Relevance and limits here |
| --- | --- | --- |
| [HEC-RAS diffusive wave](https://www.hec.usace.army.mil/confluence/hecras/beta/technical-reference/hydraulic-equations/diffusive-wave-equation) | Gravity/friction balance with full water-surface gradient; implicit solution machinery | Chosen physical verification hypothesis for gradually varied friction-dominated flow. Omits inertia, wind and Coriolis; not dam-break or ocean-wave physics |
| [LISFLOOD-FP ACC](https://gmd.copernicus.org/articles/16/2391/2023/index.html) | Local-inertial flow on uniform/non-uniform grids, including GPU implementations | Potential reference if retaining local acceleration becomes necessary; changes state/equations rather than repairing this one coefficient |
| [ANUGA](https://github.com/GeoscienceAustralia/anuga_core) | Finite-volume shallow-water solver with wetting/drying capability | Useful future hydrodynamic reference; adapting a full momentum solver is a different scope from the current slow water-cycle stage |

The selected law approximates hydraulic radius by depth and uses our explicitly declared mobility cap:

```text
eta = bed + h
q = -beta * grad(eta)
beta = min(Dmax, h^(5/3) / (n * sqrt(|grad(eta)|)))
dh/dt = div(beta * grad(eta)) + sources
```

The cap is not a coefficient supplied or calibrated by HEC-RAS. No external solver code is copied or dependency installed. Explicit paired transfers remain an implementation choice, not HEC-RAS's implicit algorithm. Considering an implicit method later may address cadence cost but requires its own conservation, positivity and reproducibility gates. A more complete PDE is not automatically a better fit for century-scale coarse planetary simulation.

## Implemented reconstruction and conservative stage

Use the same straight chordal triangles as the [cotangent conductances](surface-cotan-candidate.md). A P1 scalar field has a constant tangent gradient per triangle, as described in [Crane's triangle-gradient lecture, slide 85](https://brickisland.net/DDGFall2017/wp-content/uploads/2017/11/CMU_DDG_Fall2017_12_ConformalGeometry.pdf#page=86). Our independently implemented dual basis solves:

```text
u = s1 - s0, v = s2 - s0
G1.(R*u)=1, G1.(R*v)=0
G2.(R*u)=0, G2.(R*v)=1
g_triangle = G1 * ((bed1-bed0)+(h1-h0))
           + G2 * ((bed2-bed0)+(h2-h0))
slope_edge = (|g_triangle_left| + |g_triangle_right|)/2
```

Relative differences avoid losing shallow water by first adding it to a large absolute datum. Averaging magnitudes, not vectors, avoids cancellation of opposing gradients. This **edge coefficient averaging is our candidate choice**, not a claim to reproduce a standard elementwise variable-coefficient FEM assembly. It is not a fitted scalar correction. Chordal tangent geometry differs from exact spherical tangent geometry at finite resolution.

Preparation owns its immutable cotangent layout and canonical edge-to-two-triangle stencils. Shape/index/finite-geometry failures and nonpositive assembled cotangent weights reject. Supported native levels 0–6 are checked. No edge clipping, triangle flipping or arbitrary imported-mesh support is claimed. Gradient slopes are recomputed from frozen leading depths each substep; arrivals cannot be spent in the same frozen stage. The old branch does not allocate these stencils or slopes.

Replace only the old directional denominator by the full reconstructed magnitude. Keep source-side over-crest depth, barrier/drive clipping, prescribed reference-body heads, canonical transfers, finite donors, numerical retention and histories. Mobility stays between zero and Dmax (zero slope takes the capped limit). Consequently the existing positive-weight bound `dt <= 0.45*min(area/(Dmax*sum weights))` still bounds total outgoing requests by frozen liquid stock in exact arithmetic: drive is no larger than source depth. This is a positivity argument, not an error estimate or a maximum principle on arbitrary variable beds.

Edge histories remain **numerical weak-form adjacency exchanges**, not literal barycentric-boundary discharge. Dry terrestrial vertices use their bed as head; an adjacent high dry bed can affect a reconstructed slope. Sharp-front/shoreline reconstruction and the inherited crest law therefore need separate accuracy tests, even though directed dry-arrival and barrier controls pass.

## Fixed-input screen and measured result

The report independently selects this candidate. Its 128 cases cover two positive flat-bed profiles `h=1+0.1*P_l(axis.s)` metres (`l=1,2`), four orientations, four meshes (162–10,242 regions), two caps and two one-substep caller intervals. The y-axis is an intentional native-mesh symmetry control, not an independent random orientation. Every case starts afresh with point samples times actual spherical areas, without mass rescaling.

Reuse the independent continuum integral and completed-history/stock-work identity of the old screen. For `p=axis.s`, the general integral uses `|grad h|=(A/R)*|P'_l(p)|*sqrt(1-p^2)` and `I=2*pi*A^2*integral beta*(P'_l(p))^2*(1-p^2) dp`. Exact fully capped works are `8*pi*Dmax*A^2/3` and `24*pi*Dmax*A^2/5`. Composite Simpson 16,384/32,768 controls refuse differences above 1e-7; 65,536-interval tests check further refinement. These references do not sum native edges or integrate a substitute graph law.

Signed nonlinear weak-work discrepancies at 0.01 s, cap 1e6 m²/s:

| Regions | First profile, x axis | First profile, tilted axis | Second profile, x axis | Second profile, tilted axis |
| ---: | ---: | ---: | ---: | ---: |
| 162 | -0.122058% | -0.118231% | -1.017687% | -1.057935% |
| 642 | +0.427610% | +0.435128% | +0.385032% | +0.388588% |
| 2,562 | +0.337270% | +0.341468% | +0.422940% | +0.429337% |
| 10,242 | +0.199464% | +0.201539% | +0.269518% | +0.273520% |

The old first-profile finest-grid discrepancies are +11.744705%/+10.861773%: matched reduction is about 59/54 times. All four orientations' finest candidate discrepancies are below 0.274%, and all eight nonlinear profile/orientation groups improve on the last mesh refinement. **Not every coarse-grid comparison decreases**: geometric and upwind-coefficient errors can cancel. Four meshes and two smooth weak functionals do not establish general PDE convergence or a universal accuracy bound. Source-side depth remains an uncalibrated finite-grid approximation. Caller-step halving checks frozen rate arithmetic, not evolving temporal convergence.

The low-cap first-profile completed arrays match the old stage exactly in a directed test; both capped profiles show approximately fourfold work-error reduction per refinement. Exact lake-at-rest/common-datum controls, capped/nonlinear frozen dry arrivals, blocked crests, finite-body receipts, zero cap and pre-transfer work-bound refusal provide directed safety coverage. These do not compare a moving wet/dry interface to a physical reference.

The [retained scalar record](data/surface-full-gradient-validation.json) fingerprints complete ignored reports, records every case and distinguishes regression success from physical qualification. Maximum local/global relative inventory residuals are `1.394e-23`/`2.533e-17`; maximum independent work-identity difference is `6.614e-16`. They measure accounting, not physical accuracy. Numerical deferrals, where present, remain reported attempted requests with the donor unchanged; they are not lost stocks.

The final complete 266,129,916-byte candidate report repeats byte-for-byte. The old 66,531,115-byte constitutive witness and 5,272,305-byte twelve-case seasonal report (three funded active-flow cases) remain byte-identical to their historic artifacts. Scalar `caseColumns` defines each retained row's field mapping; only complete initial/final owners and transfer arrays are omitted from that compact record. Maximum single/summed deferred requests are `2.179e-10`/`5.324e-9` kg, with at most 61 requests in a case. Source fingerprints and environment versions are retained for reproduction.

Validation: full `npm test` passes 545 native tests across 103 targets and all 18 Node files, with the same one pre-existing ignored annual-snow test. Release regression passes all 90 library, twelve regional-soil and five legacy regional-surface tests; final release checks rerun all nine constitutive and six gradient tests. Warnings-denied all-target Clippy, formatting and whitespace checks pass. CLI missing/extra arguments and overwrite controls reject without creating invalid output or altering existing report hashes. No GUI rerun is claimed for this headless-only increment; the owner's two renderer edits remain untouched and excluded.

## Reproduction and next gate

```bash
cargo run --release --locked --manifest-path native/Cargo.toml --example surface_full_gradient_report -- artifacts/new-full-gradient.json
cargo test --locked --manifest-path native/Cargo.toml --lib surface_gradient
cargo test --locked --manifest-path native/Cargo.toml --lib surface_constitutive
```

The standalone CLI requires one new output path and refuses overwrite. The existing `surface_constitutive_report` still runs the old witness. Full arrays remain in ignored `artifacts/`, not save/checkpoint formats. Existing seasonal generations keep their old pinned law; no desktop promotion or GUI rerun is claimed.

Next: verify evolving nonlinear smooth/sharp-head and wet/dry-front behavior with independently specified physical references and space/time controls. Then consider separately pinned seasonal selection/persistence. Fixed coastal heads, soil/cold calibration, generated-world validation and ecology readiness remain additional gates. No milestone C/D completion follows from this frozen screen.
