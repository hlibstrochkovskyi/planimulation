# Resolution-aware water levels: milestone C3p

Status: implemented as an **opt-in headless experiment**, `seeded-multi-entry-network-5`, September 29, 2026. The generated-world default remains v4, and v1–v4 checkpoints retain their prior inverse and replay semantics. No desktop water simulation or climate forcing is enabled.

## Problem and rule

C3o identified a 50,000,000 m³ stock in `profile-03` that v4 rejected because reconstructing volume from its computed absolute `f64` water elevation differed by about 0.00963 m³. The fixed relative reconstruction tolerance was 0.005 m³ there, while one representable elevation step corresponds to roughly 0.02 m³. Rejecting such a stock is a false precision demand on the **displayed/derived level**, not evidence that the authoritative volume is absent.

V5 first applies the unmodified v4 test. If that fails, it examines the computed level and its immediate representable neighbors. A candidate is accepted only when the requested volume lies between the reconstructed volumes at the candidate's adjacent representable levels. Of those candidates, v5 chooses the nearest reconstructed volume, and accepts it only if its absolute error is at most `1e-9 × requested volume`. The checkpoint's stock is **never rounded to the derived level**. Zero-reconstructing levels, genuinely unresolved tiny volumes, and out-of-interval branch levels remain rejected. This is a bounded precision policy for the current `f64` geometry, not a claim of physical accuracy. The event budget, initial/external ledgers, and threshold rules are unchanged.

To exercise v5 without changing the saved C3o fixtures, pipe an explicit version into the observational probe:

```sh
jq '.experimentVersion="seeded-multi-entry-network-5"' docs/scenarios/seeded-forcing-small-input.json \
  | cargo run --release --locked --manifest-path native/Cargo.toml --example seeded_forcing_probe -- -
```

The probe accepts only this explicit override; omitted `experimentVersion` still selects the previous size-dependent default. Complete v5 checkpoints record their experiment version and restore with the same rule.

## Fixed-seed comparison

The same 20 `profile-00`–`profile-19` generated worlds and 100 cyclic intervals from [C3o](repeated-forcing-probe.md) were rerun, changing only the experiment version. With the smaller input fixture, v4 completed 13/20 and rejected seven at the level-representation check; v5 completed **20/20**. The seven previously rejected seeds were `profile-03`, `profile-04`, `profile-13`, `profile-15`, `profile-16`, `profile-17`, and `profile-18`. Every accepted run includes exact serialized checkpoint continuation after interval 50.

With the large input fixture, v5 still completed **12/20**. The same eight seeds rejected the unchanged `Concurrent event budget does not balance.` check: `profile-03`, `profile-10`, `profile-11`, `profile-13`, `profile-15`, `profile-16`, `profile-18`, and `profile-19`, after respectively 7, 83, 42, 3, 1, 82, 19, and 3 completed intervals. Rejections remained atomic. These are fixed engineering smoke samples, not a probability estimate or general reliability result.

A directed single-column test proves v4 still rejects a 50-million-m³ reconstruction that v5 accepts within its representable-level bound, while v5 still rejects a 0.0001 m³ stock at that scale. A generated `profile-03` test checks the original v4 atomic rejection, v5 acceptance, and exact v5 checkpoint replay.

## Remaining gate

V5 only addresses the level inverse. The internal concurrent-event residual and sub-stock-precision input require their own accounting designs. In particular, increasing the event-budget tolerance or silently discarding a fractional remainder would hide rather than resolve an imbalance. Dynamic desktop water, precipitation-derived forcing, drying/splitting, and long-running reliability remain out of scope. The next narrow investigation is the retained `profile-15` event witness from C3o, including represented-stock differences and a checkpoint-safe remainder policy.
