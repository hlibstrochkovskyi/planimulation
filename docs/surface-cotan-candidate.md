# Cotangent surface-conductance candidate

October 9, 2026: implemented the separately pinned headless `regional-surface-cotan-candidate-1` operator candidate. It reuses the paired surface-transfer stage with geometric weak-form edge conductances and unchanged spherical stock areas. **No seasonal session, checkpoint, protocol, default or desktop mode selects this candidate.** The [old spatial discrepancy](surface-spatial-verification.md) remains reproducible under its existing pins.

## Decision and scope

The preceding study found weak spatial refinement and a persistent discrepancy between two-point exchanges and exact continuum flux through nonorthogonal barycentric boundaries. The next candidate must improve the continuum operator while retaining explicit inventory ownership, reproducibility and a usable positivity bound. The following alternatives are implementation choices, not new product principles:

| Approach | Advantage | Additional gate |
| --- | --- | --- |
| Corrected multipoint finite-volume flux | Can approximate flux through the existing physical boundary using reconstructed gradients | Conservative nonorthogonal corrections, dry/sill treatment and positivity need a separate reviewed formulation |
| Orthogonal physical dual | Aligns boundary conormals and center differences | Changes physical areas/boundaries and downstream generation, water fitting, climate geometry and persistence; needs a separately versioned world |
| Positive cotangent stiffness with lumped stock areas | Sparse symmetric geometric operator, unchanged owners/adjacency and explicit positive-weight bound | Exchanges have weak-form edge meaning, not literal existing dual-face discharge; nonlinear and seasonal qualification still required |

The third option is implemented as the first bounded candidate. This selection does not rule out the other approaches, promise accuracy on arbitrary meshes, or make cotangent edge histories exact measurements of flux through the current dual boundary.

## Implemented geometric operator

For each native triangular face, use its unit-center positions to form the straight chordal triangle. For an edge shared by two triangles:

```text
w_ij = 0.5 * (cot(alpha_ij) + cot(beta_ij))
cot(angle(u,v)) = dot(u,v) / |cross(u,v)|
area_i * dh_i/dt = D * sum_j(w_ij * (h_j - h_i))
```

The opposite-angle cotangents are the standard piecewise-linear triangular stiffness weights. Their energy derivation and the dependence of positive weights on mesh geometry are given by [Bobenko and Springborn](https://arxiv.org/pdf/math/0503219). The associated diagonal area normalization is also described in [CMU's cotan-Laplace assignment](https://brickisland.net/ddg-web/assignments/assignment3/index.html). Our implementation does not add the assignment's diagonal regularization: a diagonal sink/source would alter closed-water accounting.

The candidate retains the **actual existing spherical areas**, not the planar chordal one-third triangle areas of a standard P1 mass lump. This hybrid choice preserves the world's physical inventory/forcing footprints. Chordal-versus-spherical and point-versus-cell-average errors remain part of the measured discretization; standard planar FEM properties are not asserted to prove every property of this spherical hybrid.

Preparation assembles in deterministic native triangle order and assigns weights to the same sorted physical adjacency. It requires exactly two incident triangles and strictly positive finite assembled edge weights. Degenerate triangles, graph mismatches and zero/negative assembled weights reject; no clipping, triangulation flip or fitted scalar correction is used. Tests check all native subdivisions 0–6, but this is not support for arbitrary imported triangulations.

The unchanged paired stage uses w in place of `width / distance`; this internal solver coefficient is not a changed `Surface` boundary or stored physical area. For general heads it still applies the inherited crest barrier, capped Manning-inspired mobility, frozen leading depths, canonical ordering, finite-donor transfers and numerical-retention policy. Only the verified flat-bed capped regime is linear diffusion; edge-dependent uncapped mobility is not thereby a validated nonlinear P1 discretization.

The explicit step bound is recomputed from the new weights:

```text
dt <= 0.45 * min_i(area_i / (D_max * sum_j(w_ij)))
```

In the capped flat regime, positive weights and this bound give convex explicit depth updates in exact arithmetic. The same transfer routine retains normalized high/low owners, paired directed histories, grant-relative checks and deferred-request diagnostics. The old equation, state machine and checkpoint pins are unchanged.

## Verification controls

The report uses the same radius, density, background depth, amplitude, roughness, diffusivity, physical meshes and 36,000 s endpoint as the retained surface study. Initial masses are point samples times unchanged physical areas, without global rescaling. Two independent continuous solution families are used:

```text
h_l(s,t) = H + A exp(-l(l+1) D t/R²) P_l(dot(axis,s))
P_1(p) = p
P_2(p) = (3p² - 1)/2
```

The eigenvalues follow the [spherical-harmonic derivation](https://www.math.utoronto.ca/courses/mat394h1/20139/L30.html). The second mode tests a different decay rate and spatial pattern; a coefficient tuned for the first mode is not introduced.

Forty-eight candidate cases cover two degrees, two orientations, four meshes (162–10,242 regions) and three common time steps. The finest-grid bound of **both** operators selects the common cadence; smaller steps halve it twice. A separate uniform candidate control and two finest-grid first-harmonic legacy controls bring the full report to 51 cases. The legacy controls use exactly the same initial problem, physical area, parameters, endpoint and finest time step as their candidate partners. Existing report pins and output remain separate.

Every step checks normalized finite owners, positivity and depth bounds that certify capped mobility. Every 32 steps and at the endpoint, independent local directed-history and global inventory audits use the existing tolerances. Full final liquid pairs, every directed history pair, center errors, numerical diagnostics and common-time comparisons are retained. The second harmonic's exact initial segment integral uses the existing `2 R sin(angle/2)` weight; an independent Simpson integration checks that formula. These diagnostic temporary transfers do not modify the evolving initial state.

The exact continuum face-flux diagnostic remains visible. **A convergent weak-form nodal solution does not imply convergence of these edge histories to the individual barycentric dual-face integrals.** They must be interpreted as conservative numerical adjacency exchanges. This distinction must survive any future inspector/river-discharge contract.

## Measured bounded result

The [retained scalar evidence](data/surface-cotan-validation.json) records all 51 cases, source hashes and the full ignored reports. Both orientations give effectively the same depth errors in this symmetric test. At the common finest cadence (9,800 steps, 3.673469387755102 s), the first orientation has:

| Regions | First harmonic RMS error (mm) | Second harmonic RMS error (mm) |
| ---: | ---: | ---: |
| 162 | 0.948921 | 3.566018 |
| 642 | 0.232855 | 0.893870 |
| 2,562 | 0.057178 | 0.221680 |
| 10,242 | 0.014131 | 0.054718 |

All twelve adjacent-mesh comparisons decrease. Observed orders based on square-root region count range from 2.0097 to 2.0406. The other common cadences use 2,450 and 4,900 steps (14.693877551020408 and 7.346938775510204 s). Successive temporal differences approximately halve: their ratios range from 0.49997976 to 0.49999287. The final time-refinement difference is at most 1.566% of the corresponding fine-run analytic RMS error. This separates the measured spatial trend from the remaining first-order temporal contribution; it is not a universal error bound or a claim about generated nonlinear worlds.

The two matched legacy controls have approximately 3.634180 mm RMS error versus 0.014131 mm for the candidate: about a 257-fold reduction on the finest grid at the **same** endpoint and time step. No coefficient was fitted to either harmonic. Full reports repeat byte-for-byte in the same build; the old 17-case report also remains byte-for-byte unchanged.

Maximum independent local and global relative inventory residuals across the candidate cases are respectively 2.930e-18 and 2.840e-17. The uniform control's maximum depth error is 1.047e-14 m. It is not bitwise motionless: rounded initial mass-to-depth conversion produces 46.788 kg of cumulative numerical exchange against approximately 1.257e16 kg of stock. The inherited retention policy remains active, with up to 1,693,017 deferred requests in a case, a largest single deferred request of 3.023e-6 kg and a largest cumulative deferred-request counter of 1.633 kg. These counters count attempted requests, including repeated attempts; they are not lost stock or completed flow. No production tolerance is widened.

The literal barycentric face-flux diagnostic is **not** passed: finest-grid relative L1 discrepancies are 14.01–14.77%. This is consistent with the explicitly different weak-form edge meaning, not evidence for calibrated discharge. The completed gate is bounded smooth capped depth diffusion on the unchanged sphere. Seasonal coupling, nonlinear sharp heads and dry-front propagation remain unqualified.

Validation: `npm test` passes 521 native tests and 18 Node test files, with one pre-existing ignored native test. Release checks pass 74 library tests, seven regional-soil tests and five legacy regional-surface tests. All-target Clippy with warnings denied, formatting and whitespace checks pass. CLI invalid-argument and no-overwrite controls reject without altering the retained output. No GUI rerun is claimed for this headless-only increment; the owner's separate renderer edits are untouched and excluded from the commit.

## Reproduction

```bash
cargo run --release --locked --manifest-path native/Cargo.toml --example surface_cotan_report -- artifacts/new-surface-cotan.json
cargo test --locked --manifest-path native/Cargo.toml --lib surface_cotan
cargo test --locked --manifest-path native/Cargo.toml --lib surface_verification
cargo test --release --locked --manifest-path native/Cargo.toml --lib --test regional_soil --test regional_surface_flow
```

The CLI accepts one new output path, refuses overwrite with exclusive final creation, and aborts rather than publishing a numerically incomplete cohort. Full reports belong in ignored `artifacts/`; they are not seasonal checkpoints. Same-build repeatability is checked separately from numerical accuracy.

## Promotion gates

This increment implements a candidate operator, not a new seasonal model. Before seasonal integration, preserve the old discrepancy and provide explicit new setting/model pins, immutable prepared conductances, complete restore validation and same-build replay. The new graph-history meaning must be explicit. Then qualify finite reference-body contacts, sharp/nonlinear heads, dry-front arrival, soil/climate coupling, generated active-flow cadence controls and longer runs. Existing prescribed reference heads, cold/soil closures and mobility calibration remain separate approximations. No milestone C/D completion, calibrated river discharge or ecology readiness is claimed.
