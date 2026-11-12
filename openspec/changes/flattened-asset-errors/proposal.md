# Asset target problems on their own line

## Motivation

The targets under `assets` sit in a map that serde flattens into `Assets` ([`src/config.rs`][config]).
garde names a target's field through the Rust field, as `assets.targets.linux-x64.files`.
The file has no `targets` key, so the message names a path the file does not hold.
ctl-core then stops at the last key that resolves and places the problem on the `assets:` line.

ctl-core 0.7.3 lets a caller declare a flattened field with `Input::flattened` ([ctl-core PR 284][ctl-core#284]).
verctl pins ctl-core 0.7.0, so it cannot declare one yet.

Queue row: [VER-125][ver-125].

## Changes

1. verctl takes ctl-core 0.7.3.
2. `Config` loads `.ctl/ver.yaml` with `assets.targets` declared as a flattened field.
   A target's problem then names `assets.linux-x64.files` and lands on the line of that key.
3. Every other problem prints and lands as before.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `asset-contents`: where a target's load problem is placed and what path it names.

## Impact

1. Code: `Cargo.toml`, `Cargo.lock`, `src/config.rs`, tests.
2. Bundle: the instructions and the operator skill state no field path for these problems, so neither changes.
3. Callers: none must change.
4. Release: a patch.

[config]: ../../../src/config.rs
[ctl-core#284]: https://github.com/victor-software-house/ctl-core/pull/284
[ver-125]: ../../../tasks.yaml
