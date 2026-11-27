# Proposal

## Motivation

The templates repository sets `CARGO_REGISTRY_GLOBAL_MIN_PUBLISH_AGE = "3 days"` for every Rust repository.
`verctl release` publishes a workspace one member at a time, and `cargo publish` packages each member against the registry.
A member that depends on a sibling released a few seconds earlier in the same run is then refused: cargo calls the sibling too new for the cooldown.
The ctl-core 0.8.0-rc.2 release stopped this way at `ctl-layout`, after `ctl-input` and `ctl-text` were already on crates.io ([run][run]).

## Changes

1. When a workspace member's `cargo publish` fails and cargo names as too new only crates that are members of the same workspace, verctl runs that upload once more with `CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow`, and that run decides.
2. Any other cooldown rejection fails as today, and the error names the crates the cooldown refused and that they are not members.
3. The cooldown parser and the setting that semver already uses move to one module both share.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `workspace-publish`: a workspace upload past the cooldown for its own members.

## Impact

1. Code: `src/publisher.rs` or the release loop that runs uploads, `src/semver_check.rs`, a new shared cooldown module.
2. Release: a `verctl: patch` changeset.
3. ctl-core moves its verctl pin to the release, then reruns its Release workflow, which resumes the partial 0.8.0-rc.2.

[run]: https://github.com/victor-software-house/ctl-core/actions/runs/46012926347
