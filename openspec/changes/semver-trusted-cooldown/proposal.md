# Semver checks past the publish-age cooldown for trusted crates

## Motivation

The templates repository's v0.0.102 sets `CARGO_REGISTRY_GLOBAL_MIN_PUBLISH_AGE = "3 days"` for every Rust repository.
`verctl semver` runs `cargo semver-checks`, which resolves a fresh placeholder project with no `Cargo.lock`.  
The cooldown then applies to every crate that project resolves.

Each release of one of our crates then fails `verctl semver` for three days, in every repository that depends on it or checks it:

- ctl-core's checks failed on its own crates `ctl-text`, `ctl-layout` and `ctl-term` 0.7.4, published 36 hours before, and on `yamled` 0.0.13;
- qctl's check failed on its own baseline, `qctl` 0.7.0.

Cargo has no per-crate exemption from the cooldown (RFC 3923 defers an exclude list).
The repositories already name the crates.io logins they trust in `CARGO_TRUSTED_OWNERS`, which only the `update:own` task reads today.

Queue row: [VER-139][ver-139].

## Changes

1. When `cargo semver-checks` fails and cargo names a requirement as too new for the cooldown, `verctl semver` reads the crates.io owners of each crate so named.
2. When every such crate has an owner that `CARGO_TRUSTED_OWNERS` lists, `verctl semver` runs the check once more with `CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow`, and that run decides.
3. Otherwise the check fails as today, and the error names the crates the cooldown rejected and why no retry ran.

## Release

A patch: a check that failed only on trusted crates now passes.

[ver-139]: ../../../tasks.yaml
