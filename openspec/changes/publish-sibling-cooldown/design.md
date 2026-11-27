# Design

## Context

`semver` already retries once past the cooldown when every refused crate has a trusted owner ([semver_check.rs][semver]).
`release` runs one `cargo publish --locked --package <member>` per workspace member, in dependency order ([publisher.rs][publisher]).

## Decisions

### 1. Retry only for the workspace's own members

A refused crate that is a member of the workspace being published was uploaded by this run or an earlier one of the same release, so it is ours.
The retry needs no owner lookup and no `CARGO_TRUSTED_OWNERS`.
`--locked` keeps every other dependency at the version the lock holds, so the second run resolves nothing new beyond those members.

Alternatives considered:

1. Always publish with the cooldown off. It drops the cooldown for a publish whose refusal names a third-party crate.
2. Reuse the trusted-owner retry of `semver`. It needs a network lookup and a variable for crates the run itself published.

### 2. One shared cooldown module

`cooldown_rejections` and `COOLDOWN_OFF` move out of `semver_check.rs` into a module both callers import, so the two checks parse cargo's error one way.

## Risks

1. Cargo can reword its error; the parser's tests pin the wording of cargo 1.100, as they do today.

[semver]: https://github.com/victor-software-house/verctl/blob/9da2f0e4a9ba0208db8f773e217744918b83561a/src/semver_check.rs
[publisher]: https://github.com/victor-software-house/verctl/blob/9da2f0e4a9ba0208db8f773e217744918b83561a/src/publisher.rs
