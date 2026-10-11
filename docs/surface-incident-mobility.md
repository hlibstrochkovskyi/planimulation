# Incident-triangle mobility comparison

October 11, 2026: `regional-surface-flat-incident-mobility-candidate-1` is a standalone gradient-side coefficient assembly ablation of the [flat centered-depth stage](surface-centered-depth.md). It retains the fixed dry/wet witnesses, edge depth and independent references. No seasonal selector, save format, desktop contract or default selects it. Accuracy results are recorded below only after completed runs; implementing a candidate does not qualify it.

## Question and mathematical basis

The preceding stage uses the arithmetic mean of two incident triangle gradient magnitudes inside a nonlinear coefficient. Evaluate whether computing the two capped mobilities separately, then combining their geometric contributions, improves the remaining spatial discrepancy.

[Bobenko and Springborn, section 1](https://arxiv.org/pdf/math/0503219#page=2) derive the cotangent operator by summing triangle Dirichlet energies. Each opposite angle contributes `c_T=0.5*cot(angle_T)` to the shared-edge weight. Our variable-coefficient extension follows from multiplying each local stiffness contribution by its coefficient. This extension is a project derivation, not a wet/dry algorithm supplied by that paper.

[Santillana and Dawson's publisher abstract](https://link.springer.com/article/10.1007/s10596-009-9131-4) studies the doubly nonlinear diffusive-wave equation with Galerkin methods and emphasizes physically consistent assumptions and applicability limits. Only the abstract was accessible here; no claim to reproduce their algorithm, regularization or error theorem is made. Searches also surfaced a Basilisk author sandbox, but its full page was unavailable; its implementation was not used as verified evidence.

For a flat closed-land edge `ij`, keep the existing chordal P1 gradient in each triangle and the same frozen arithmetic edge depth:

```text
h_face = (h_i+h_j)/2
beta_T = min(Dmax, h_face^(5/3)/(n*sqrt(|grad_T h|)))
w_ij = c_left+c_right
beta_edge = (c_left*beta_left+c_right*beta_right)/w_ij
request = rho*dt*beta_edge*w_ij*abs(h_i-h_j)
```

Cap each term before combining; capping an averaged uncapped coefficient is different. Fully dry edge depth has explicitly zero mobility, positive depth with zero gradient uses the capped limit, and equal incident coefficients are returned unchanged. No slope floor, wet film, threshold, fitted coefficient or stock repair is introduced.

This changes both the order of the nonlinear operation and its geometric weighting. It does **not** isolate gradient sampling locations alone. Edge depth remains shared by both triangles: this is **not standard elementwise nonlinear P1 assembly**, which would integrate a depth-dependent coefficient over each triangle. No quadrature or such integration is silently claimed.

## Accounting and admissibility

Individual local half-cotangents must be strictly positive and finite, not merely their assembled sum. Mixed signed terms can destroy the mobility cap bound despite a positive summed weight; preparation refuses them without clipping or flipping. The inherited full-gradient branch retains its assembled-only geometric contract. All native levels 0–6 are checked against the stronger precondition.

With positive local terms, `0<=beta_edge<=Dmax`. Retain the same `0.45*min(area/(Dmax*sum w))` outgoing-stock bound, actual spherical stock areas, canonical transfer order, frozen leading depths, paired owners, directed histories and numerical retention. Edge histories remain weak-form numerical adjacency exchanges, not literal dual-boundary discharge. The wrapper admits only flat terrestrial geometry and no body owners, reusing the centered branch's preflight and work limit. This is not a general rollback guarantee for arbitrary downstream in-place failures; reports own their state and publish only after success.

## Verification and reproduction

Directed controls check independent two-triangle coefficient arithmetic, unequal geometric weights, per-term caps, dry/zero-cap limits, unchanged fully capped completed transfers, exact rest, common datum, frozen one-ring arrivals, repeated output, malformed-owner refusal and the stronger local positivity precondition.

The dry report keeps eighteen candidate trajectories and two matched finest centered-depth controls. The wet report keeps thirty-six candidate trajectories and four matched finest centered-depth controls. Native initial stocks, all reference controls, radii, roughness, caps, durations, axes, meshes and caller cadences are unchanged. All arrays and numerical accounting remain retained. Threshold crossings remain graph-edge proxies, not exact PDE support; initialization and hemisphere-footprint discrepancies are not corrected.

```bash
cargo run --release --locked --manifest-path native/Cargo.toml --example surface_incident_report -- artifacts/new-incident-dry.json --dry
cargo run --release --locked --manifest-path native/Cargo.toml --example surface_incident_report -- artifacts/new-incident-wet.json --wet
cargo test --locked --manifest-path native/Cargo.toml --lib surface_incident
```

The CLI requires a new output path and exactly one mode; dry/wet pins are `surface-flat-incident-front-report-1` and `surface-flat-incident-wet-report-1`. Complete ignored reports are not simulation checkpoints. No terrain/coastal flow, generated-world seasonal continuation, physical calibration or ecology qualification follows from this ablation alone.

## Smooth wet result

On 2,562 regions at the unchanged finest cadence:

| Profile / axis | Previous centered RMS, micrometres | Incident RMS, micrometres |
| --- | ---: | ---: |
| P1 / x | 2.525 | 2.308 |
| P1 / tilted | 2.517 | 2.381 |
| P2 / x | 9.414 | 8.402 |
| P2 / tilted | 9.402 | 9.349 |

Finest measured reductions are 0.568–10.750%. Coarse P1/x instead increases by 0.276%; this is not universal improvement. All eight spatial comparisons decrease, with final ratios 0.254–0.300. All twelve time-difference ratios remain near one half (0.499734–0.499900); the last difference reaches 0.440% of endpoint RMS discrepancy. Finest errors are 0.389–0.402% of the reference's actual depth change for P1 and 0.711–0.791% for P2, not normalized by the standing water depth.

Last reference spatial/time RMS differences reach 4.750%/0.1028% of candidate discrepancy. In particular, the smallest 0.568% measured change is smaller than the corresponding reference spatial-refinement indicator. It is an observed same-reference comparison, not strong evidence of superiority relative to the exact continuum solution. These refinement indicators are not rigorous error bounds.

All sixteen reference controls, thirty-six initial stock/sample arrays and four complete matched centered trajectories are exactly unchanged. Maximum candidate local/global relative inventory residuals are `1.593e-18`/`2.489e-17`. The complete 43,184,193-byte report repeats byte-for-byte. The rebuilt 43,215,413-byte original full-gradient wet report retains its historic hash. These checks do not qualify dry-front or terrain flow.

## Dry-front result and decision

At the finest unchanged cadence:

| Regions | Incident RMS, x / tilted, mm | Incident maximum 1 cm proxy error, x / tilted, m |
| ---: | ---: | ---: |
| 642 | 24.544 / 14.879 | 129.59 / 146.46 |
| 2,562 | 13.517 / 8.644 | 63.02 / 69.73 |
| 10,242 | 6.708 / 4.788 | 25.70 / 30.88 |

The preceding centered stage's finest RMS errors were 6.825/4.834 mm: reductions of only 1.706%/0.9645%. Its maximum 1 cm proxy errors were 25.29/30.75 m, so the new stage **worsens** them by 1.592%/0.4340%. Finest 1 mm errors also increase from 46.63/52.20 to 46.76/52.38 m; 5 cm errors increase from 5.85/8.99 to 6.34/9.09 m. All eighteen finest-cadence threshold/mesh/orientation comparisons worsen relative to the centered baseline. A slightly smaller domain-wide RMS does not establish better front placement.

All four candidate spatial RMS comparisons and all threshold maximum errors still decrease with mesh refinement. Final RMS ratios are 0.4963/0.5539. Time-difference ratios are 0.499644–0.499940, with the last difference at most 0.168% of endpoint RMS discrepancy. Last reference spatial/time RMS differences reach 2.582%/2.390% of candidate discrepancy; the unchanged final reference contour differences remain below 0.3 m. These are uncertainty indicators, not rigorous bounds. The reference's 1 cm contour advances 152.48 m; the remaining 25–31 m native error is not explained away by time-step refinement.

All sixteen planar/spherical reference controls, eighteen initial stock/sample arrays and two complete matched centered trajectories are exactly unchanged. Maximum candidate local/global inventory residuals are `4.597e-16`/`1.266e-17`. Finest initially dry-half inventory proxy discrepancies are +8.394%/-0.8763%; inclusive equatorial vertex footprints and point initialization remain uncorrected. This is not an exact geometric hemisphere mass comparison. Some 5 cm crossings remain behind the reference; the failure is not a claim that every reconstructed contour point is ahead.

**Do not select this variant as the dry-front remedy or promote it to a seasonal model.** The experiment distinguishes a small smooth RMS benefit from worse measured front placement. It shows that replacing the averaged-slope denominator with positive locally weighted mobilities, while retaining edge depth, does not resolve this witness. It does not prove that every gradient reconstruction is equivalent or that sharp-front reconstruction is irrelevant.

The next proposed numerical step is a consistent elementwise coefficient/flux formulation, including positive depth quadrature and its dry limit, rather than another fitted averaging factor. This is **not implemented** by the current stage. First verify local energy, cap and outgoing-stock bounds; then reuse the unchanged evolving dry/wet references. Terrain lake-at-rest, disconnected-water and barrier controls remain independent prerequisites before any seasonal extension. Further annual endurance or changing roughness to minimize this one error is not a substitute.

## Retained evidence and final validation

The [scalar record](data/surface-incident-validation.json) retains all sixty new/matched native trajectories, all thirty-two reference controls, independent spatial/time comparisons, previous-stage comparisons, numerical retention and source/report fingerprints. Full owner/history arrays remain in the ignored reports. The 79,542,597-byte dry and 43,184,193-byte wet reports both repeat byte-for-byte. All six complete centered controls and all initial/reference arrays are unchanged; rebuilt original wet and legacy seasonal reports retain their historic hashes. A complete historic dry report was not rerun in this increment; its preceding validation remains recorded rather than relabelled as a new result.

Candidate dry/wet attempted-defer counts total 67,676,681/101,459; including matched controls they total 92,042,530/134,361. Maximum single requests are `1.338e-11`/`7.663e-8` kg, with maximum per-run summed attempt counters `5.272e-8`/`8.178e-5` kg. Deferred requests leave donors unchanged and can repeat; these are not completed exchanges or lost water. No accounting tolerance is widened.

Ten new directed tests pass. Full `npm test` passes 576 native tests across 107 targets and all 18 Node test files, with the same one pre-existing ignored annual-snow test. Release regression passes 121 library, twelve regional-soil and five regional-surface tests. Final all-target Clippy with warnings denied, formatting, whitespace and six CLI argument/overwrite refusals pass. Invalid output remains absent and existing outputs unchanged. The owner's renderer files are untouched and excluded; no GUI rerun or product promotion is claimed.
