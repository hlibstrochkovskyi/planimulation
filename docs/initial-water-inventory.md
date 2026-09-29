# Generated initial-water inventory: milestone C3j

Status: implemented read-only transfer, `initial-water-inventory-1`, September 29, 2026. This transfers the already generated B4 water state into exclusive C2a basin stocks. It does **not** advance water, choose rainfall, initialize C3i's dynamic checkpoint, or change desktop rendering or the wire protocol.

## Contract

The input is a matching `basins-1` surface, bed, basin hierarchy and generated initial water state. The importer verifies the hierarchy against the supplied bed; it does not rerun water fitting or modify the global level, depths, body labels or resolved total volume. The report records the source model version, global initial level, initial-volume ledger, ordered active stocks, each stock's level and body ID, and the largest body's ID. The full recipe accompanies the standalone JSON report so the static geometry can be regenerated. The inventory alone is not a complete world save.

At a given initial level, each local minimum has an active leaf stock unless a higher merged branch is already wet. A parent becomes active only when the level is **strictly above** its connection threshold. At exact equality, sill regions still have zero depth and positive-depth water bodies remain separate; their child stocks are retained at capacity. Each wet region contributes `area × depth` to exactly one active branch. No parent capacity is added to a child volume. Dry leaves may have zero stock. This is a faithful initial-state representation, not yet the C3i frontier convention for an advancing interval.

Reconstruction checks the canonical active frontier, each stock and global volume budget, regional depths, positive-depth graph components, stock/body labels and the largest body. It rejects incompatible geometry, unsupported versions and corrupted inventory metadata. No epsilon is added to cross a sill. JSON decoding uses `serde_json`'s `float_roundtrip` option so serialized stock levels and volumes replay bit-exactly in the native process.

The transfer and audit each rebuild the basin analysis for static-geometry compatibility. This is deliberate one-time validation, not a per-tick algorithm. It uses iterative tree walks and one region scan for stock assignment; it does not build C3i's dense branch-membership matrix or duplicated per-branch storage curves.

## Developer report

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example initial_water_inventory -- docs/scenarios/spill-connections.json
```

Use `-` for stdin. Input is capped at 32 KiB; malformed or incompatible recipes fail before JSON output. The report's `scope` explicitly excludes water movement. A generated world at subdivision 5 or 6 can be transferred even though the existing dynamic laboratory still rejects that size. Transfer success therefore does not imply transport readiness.

## Validation and next gate

Tests cover dry, partial, fully covered and weighted nested-bowl worlds; exact sill equality; separate and merged bodies; corrupted stock volume, initial-volume ledger, branch, body label and version; mixed bed/hierarchy; deterministic generated worlds at four resolutions plus 12 seeds × 3 coverage settings; and bit-exact serialized inventory replay. Reconstruction is compared against the generator's independent water-body flood and regional depths, not just the importer's own stock sum.

Validation on September 29, 2026: `npm test`, Rust Clippy across all targets with warnings denied, and `npm run test:desktop` passed. The desktop fingerprint remained `2e66ac09`. The standalone recipe report produced 162 regions, four positive-depth water bodies and five active stocks; the same recipe at subdivision 6 produced 40,962 regions, 19 bodies and 93 active stocks. These are deterministic smoke observations, not a dynamic performance benchmark or a claim that C3i can run on those worlds.

Next, reconcile exact-sill child stocks with a dynamic frontier, add a distinct **initial** volume ledger to dynamic checkpoints rather than misreporting pre-existing water as input, and measure storage/numeric behavior on generated-world scales. The limited desktop water demonstration remains the visible acceptance target. No generated-world headless advance is claimed here.
