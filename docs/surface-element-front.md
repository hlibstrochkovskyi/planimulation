# Element-integrated moving dry front

October 12, 2026: this standalone report applies the existing [positive elementwise mobility](surface-element-mobility.md) to the fixed [moving dry-cap witness](surface-dry-front-verification.md). It does not change the physical law, independent reference, seasonal model, save schema, desktop selector or defaults. A completed accounting check is not an accuracy qualification.

## Fixed problem and comparisons

Keep `R=1000 m`, Manning roughness `n=0.04`, mobility cap `1000 m²/s`, duration `300 s` and initial depth `h=p_+^(3/7)`. Both original axes, actual spherical storage areas, point-sampled initial stocks, closed land, compensated paired transfers, native audit tolerances and threshold definitions are unchanged. No water film, slope floor, dry cutoff, stock repair or inventory fitting is added. Histories remain weak-form adjacency exchanges, not literal dual-boundary discharge.

The independent reference retains all eight planar exact traveling-wave controls and eight spherical band controls. The selected spherical endpoint is the original 4096-band, 0.125-second solve. Neither the reference mesh nor its initial averages are fitted to this candidate. Reference spatial/time differences remain uncertainty indicators, not rigorous error bounds.

The main cohort uses quadrature subdivision `q=0` (six positive nodes on an uncapped triangle), both orientations, 642/2562/10242 regions, and the original three caller cadences. This gives eighteen trajectories, with independent same-mesh time differences and finest-cadence spatial comparisons. The operator integrates the pointwise capped mobility over affine P1 depth in each triangle before assembling its positive local stiffness contributions; it does not use a shared edge depth.

Six further finest-cadence trajectories compare `q=1` against `q=0` on levels 3 and 5, and `q=2` against `q=1` on level 3. They retain complete owner/history arrays and initialization. No level-4 quadrature check or fine-mesh `q=2` solve is implied. These bounded comparisons cannot establish a universal quadrature error bound or certify a final production quadrature setting.

Two complete finest-mesh, finest-cadence incident-mobility trajectories provide matched old-stage controls. Reported old-to-new comparisons distinguish domain-wide RMS from each threshold's minimum/maximum signed contour error and maximum absolute contour error. A difference of contour-error extrema is **not** a pointwise maximum contour displacement: edge crossings are not matched physical boundary points. The 1 mm, 1 cm and 5 cm contours are unchanged adjacency-edge proxies, not exact PDE support or reconstructed flooded geometry.

## Reproduction and cost

```bash
cargo run --release --locked --manifest-path native/Cargo.toml --example surface_element_front_report -- artifacts/new-element-front.json
cargo test --release --locked --manifest-path native/Cargo.toml --lib surface_front
```

The CLI requires exactly one new output path, refuses overwrite and publishes only after the entire report succeeds. `surface-flat-element-front-report-1` is an observation report, not a checkpoint. Full arrays belong in ignored `artifacts/`; the retained scalar record must identify every case, independent comparison and source/report fingerprint.

Each candidate row records an upper bound `steps * triangles * 12 * 4^q` for scalar coefficient-node evaluations. A closed triangular sphere has `triangles=2*regions-4`. The factor twelve allows two uncapped pieces after a cap intersection; whole-dry, constant and fully capped pieces use fewer nodes. This is a conservative work bound, **not measured execution counts**. Preparation, stock auditing and reference costs are excluded from it. Wall-clock measurements are kept separately from deterministic report data, and a mixed-cohort timing must not be presented as an isolated kernel speed comparison.

## External implementation review

The preceding source review found a closely related [Basilisk author-sandbox diffusive-wave implementation](https://basilisk.fr/sandbox/nlemoine/SWE/diffwave/diffwave.c). It uses Manning mobility with full slope magnitude, an implicit diffusion solve, slope regularization and an explicit excess-water compensation mechanism after depth correction. Those choices differ from our capped law and per-stage ownership contract; none is silently imported here. This is an author sandbox, not a certified production replacement. The [Landlab OverlandFlow source](https://landlab.readthedocs.io/en/latest/_modules/landlab/components/overland_flow/generate_overland_flow_deAlmeida.html) defaults to a thin initial water layer and contains depth correction in its steep-slope mode, so its defaults cannot serve as a no-film reference for this witness.

The [ANUGA validation suite](https://anuga.readthedocs.io/en/stable/reference/validation.html) usefully separates analytical, laboratory and behaviour-only checks. Its shallow-water dam-break solutions do not solve our inertia-free equation and are not substituted for the existing exact traveling-wave certificate. The review motivates explicit front measurements and independent references, not a claim that another library validates this implementation.

## Result and decision

All 24 candidate trajectories, two matched incident trajectories and sixteen independent reference controls complete. All candidate initial owner arrays/reference samples are exactly unchanged, and both complete matched incident trajectories agree with the retained baseline. The [scalar validation record](data/surface-element-front-validation.json) retains every candidate case, contour extrema, reference diagnostics, independent comparisons, numerical retention and source/report fingerprints. The complete report is 95,137,928 bytes with SHA-256 `c0ebc9d0a9249360f940ae04d1eb8e64a60a292b927f62018f6259ce43d59651`.

At 10,242 regions and the finest caller cadence:

| Metric, x / tilted | Previous incident stage | Element-integrated q=0 |
| --- | ---: | ---: |
| Area-weighted depth RMS, mm | 6.708 / 4.788 | 5.619 / 4.297 |
| Maximum 1 mm contour-proxy error, m | 46.76 / 52.38 | 47.51 / 54.74 |
| Maximum 1 cm contour-proxy error, m | 25.70 / 30.88 | 30.24 / 33.86 |
| Maximum 5 cm contour-proxy error, m | 6.34 / 9.09 | 9.56 / 11.73 |

Depth RMS improves by 16.242%/10.244%, but all eighteen finest-cadence threshold/mesh/orientation maximum-contour errors **worsen** relative to the retained incident stage. The finest 1 cm maximum error increases by 4.539/2.975 m. Lower RMS is not evidence of a better front. The independent reference's 1 cm contour advances 152.48 m; remaining native discrepancy is not a reference-refinement-scale effect. Some 5 cm crossings remain behind the reference, so this is not a claim that every contour point lies ahead.

All four adjacent-mesh RMS comparisons improve; last ratios are 0.4700/0.5420. Every measured threshold maximum error also decreases with spatial refinement. Successive time-difference ratios are 0.499680–0.499932; the last time difference reaches only 0.1633% of endpoint RMS error. Last reference spatial/time RMS differences reach 2.876%/2.853% of candidate discrepancy. These are fixed-witness refinement indicators, not universal convergence orders or rigorous uncertainty bounds.

Quadrature refinement cannot explain this measured old-to-new difference: finest `q=0 → 1` endpoint RMS differences are 0.243/0.270 micrometres, and changes in maximum 1 cm contour error are +1.324/+0.773 mm. On level 3, `q=1 → 2` endpoint differences are 0.0832/0.0725 micrometres, about 8.1–8.3% of the preceding quadrature difference. The finest mesh lacks a `q=2` solve, so no general quadrature certificate follows. Measured millimetre-scale extrema changes are not pointwise contour-displacement bounds. The per-case finest work upper bounds are 2,787,901,440 (`q=0`) and 11,151,605,760 (`q=1`) coefficient-node evaluations; actual counts and isolated wall-clock performance were not measured.

Maximum native local/global inventory residuals are `4.369e-16`/`1.249e-17` under unchanged tolerances. Candidate trajectories record 89,806,197 attempted numerical deferrals, maximum single request `1.204e-11 kg` and maximum per-case summed attempt counter `5.158e-8 kg`. Requests leave donors unchanged and can repeat; they are not lost mass or completed exchanges. The initially dry-half inventory proxy still differs from the reference by +8.831%/-0.4842% on the finest main runs. Point initialization and inclusive equatorial footprints are unchanged, not corrected or diagnosed away.

**Do not select this candidate as the dry-front remedy or promote it to a seasonal model.** This is a negative front-remedy result despite better smooth frozen work and lower evolving RMS. It does not reject all element methods or identify the exact cause of the remaining contour error.

The [subsequent observation audit](surface-front-observation.md) now measures the unchanged graph-edge contour observable on independently sampled reference depths. Its substantial mesh/observation contribution does not qualify this evolving state or change the retained negative remedy decision. Original contour results remain retained. Smooth evolving wet comparisons and terrain/rest/disconnected-water/barrier checks are not replaced by that control.

## Validation and reproduction limits

Two new directed tests cover exact repeat of small `q=0/1/2` trajectories, unchanged initialization/reference inputs, conservative dry arrivals, deterministic work bounds and invalid quadrature/stage refusal. Full `npm test` passes 589 native tests across 109 targets and all eighteen Node files, with the same one pre-existing ignored annual-snow test. Optimized library regression passes 134 tests. Strict all-target Clippy with warnings denied, formatting, whitespace and three CLI argument/overwrite refusals pass; invalid output remains absent and the existing report unchanged.

The complete rebuilt 79,542,597-byte incident report retains SHA-256 `c872e47fe5b1132ad2b4949debb153dc07dcd574619808f7aaf539c692e35535`. The **new complete cohort was not run twice** in this increment: exact replay evidence here consists of small directed candidate repeats, unchanged complete matched controls and the rebuilt historic report. Do not label its one complete run as a full-report repeat. No GUI rerun or wall-clock benchmark is claimed. Owner renderer files remain byte-identical to their pre-existing state and excluded from these changes.

## Remaining gates

Moving-front accuracy, quadrature sensitivity and cost must be interpreted together. A smooth frozen-work improvement alone cannot select this operator. Smooth evolving wet profiles remain a separate regression/accuracy task. Terrain lake-at-rest, disconnected water and barriers precede any variable-bed extension. Soil/cold coupling, finite coastal heads, seasonal integration, generated-world qualification and physical calibration are not closed by this report.
