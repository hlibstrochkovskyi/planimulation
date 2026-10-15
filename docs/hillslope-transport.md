# Bounded linear hillslope transport

October 14, 2026. Native primitive `linear-hillslope-transfer-1`. Subsequently
integrated into the bounded [fine-patch review](fine-terrain-patch.md), not
desktop generation, global sediment stocks or the seasonal clock.

## Model and units

[Theodoratos et al., section 2.1](https://esurf.copernicus.org/articles/6/779/2018/)
describe linear slope-proportional transport together with uplift and incision.
The linear law approximates gentle soil-mantled hillslopes; it does not describe
all steep mountain processes. We independently discretize a linear exchange,
not import an external implementation or claim calibrated Earth coefficients.

For a shared face between cells a and b, request one signed volume:

```text
q[a -> b] = dt * D * face_length / center_distance * (old_h[a] - old_h[b])
```

`D` is m²/year, `dt` years, both lengths meters, elevations meters and q m³.
Each active-active edge debits and credits the same requested volume, once.
Convert net input to height with each cell's own reference area; unequal areas
must not receive equal height changes. This is reference-prism transport, not
a density-resolved bedrock/soil stratigraphy.

The caller supplies unique canonical undirected edges, areas and fixed-height
boundary flags. A fixed boundary is an explicit external reservoir, **not an
inferred lake or ocean**. Fixed-fixed edges do not belong to the active model.
Positive signed boundary exchange adds material to the active bed; negative
exports it. This primitive does not certify geometric orthogonality or construct
the mesh. A supplied two-point face transmissivity must be justified separately.

## Stability, numerical accounting and refusals

All edge requests use the old bed. Before updating, require at every active cell:

```text
CFL_i = dt * D * sum(face_length / center_distance) / area_i <= 1
```

The real-arithmetic update is then a convex combination of old local heights.
The implementation also verifies the actual output against old local extrema.
Unstable steps refuse even on constant fields; no clipping repairs an unstable
step. There is no adaptive driver or implicit solve in this primitive.

Requests and accepted floating-point changes are different records. Return
requested per-edge/per-node volumes, actual height changes and actual node
prisms, signed/gross external requests, gross throughput and rounding observations.
The accepted net bed prism must reconcile with requested net boundary exchange
at `1e-11` relative to gross exchange. A separate gross-boundary-scale gate
prevents large internal flow from concealing a smaller lost external exchange.
The initial absolute bed prism never supplies the tolerance scale.

Balanced subprecision internal requests can leave the bed unchanged; they are
explicitly unresolved, with absolute node rounding recorded. Requested edge
throughput is **not accepted inventory**. A coupled caller must not use it as
an accepted sediment debit or hide unresolved changes. Unrepresentable nonzero
net external exchange refuses. No mass correction or stock-repair adjustment is
performed. Lost nonzero node-net changes incident to requested external transfer
also refuse, even when opposite lost boundary exchanges cancel globally. All
failures preserve caller arrays.

Work is bounded at 262144 nodes and four times that many unique edges. Positive
finite areas/lengths, finite fields/settings, canonical indices, duplicate refusal,
finite products and actual outputs are checked. Zero diffusivity returns an
exact no-op only after shape/topology/input validation. The kernel consumes no
random stream and advances no simulation clock.

## Verification and next integration

Tests cover independently known dyadic two-node/unequal-area transfers; fixed
boundary input/export signs; constant/zero controls; CFL-at-one behavior and
unstable refusals; closed deterministic conservation/local extrema; observed
subprecision no-ops versus lost external requests; malformed/overflow/workload
refusals; manufactured planar/quadratic grid interiors; and second-order spatial
refinement of a sine heat operator. Temporal refinement tests compare a closed
pair with its independent exponential solution.

The previous full suite (800 native / 96 Node) predates this new primitive.
Its focused tests and strict checks are qualified separately; do not infer a
complete coupled landscape from them. The subsequent bounded
authoritative [fine patch](fine-terrain-patch.md) records declared
geometry, boundaries, process scales and actual-bed views. Global sedimentary
pending/active stocks and climate/water coupling remain untouched.

Current verification: ten kernel tests pass, along with seventeen source/adapter/
old-family controls (27 focused tests total). The physical square-grid fixture
uses its actual declared spacing for both face and center lengths. Strict
all-target Clippy, formatting and whitespace pass. The owner's renderer files
retain their hashes and are excluded from this increment.
