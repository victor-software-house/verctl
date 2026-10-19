# Check the API at the release type the pending release allows

## Why

The operator set the version rule on 2026-10-19: a release is a patch by
default, and a real API break bumps the minor.

The shared `semver` task runs `cargo semver-checks` with no release type.
cargo-semver-checks then infers the type from the version change, and on a
0.0.z crate between releases it assumes a major release and runs none of
its lints: a consumer at 0.0.2 reported "0 checks: 0 pass, 260 skip". Under
Cargo's rules every 0.0.z release may break, so nothing enforces the rule
there. On a 0.y.z crate it assumes a minor release, which happens to match
the rule only while no minor fragment is pending.

Queue row: VER-075.
A patch: it adds a verb and changes no existing behaviour.

## What changes

1. A new `verctl semver` verb checks each crates.io crate of each package:
   1. It skips the crate when a pending fragment bumps its package's minor
      or major.
   2. It skips the crate when its manifest's major.minor differs from the
      crates.io baseline, because the Version PR already moved it.
   3. It skips a crate with no release on crates.io.
   4. It leaves out a crate with no library target, which has no API.
      cargo-semver-checks fails when asked to check only such a crate.
   5. Otherwise it runs `cargo semver-checks --release-type minor` against
      that baseline, which fails only on a break.
2. A crate at 1.0 or later fails the run. No family crate is at 1.0, and
   reaching 1.0 is the operator's decision, which revisits the rule.
3. ctl-core's shared `semver` task calls the verb in a later ctl-core change
   (design section 4).

## Impact

1. `src/semver_check.rs` (new), `src/registry.rs`, `src/cli.rs`,
   `src/main.rs`, `src/presentation.rs`, `src/lib.rs`.
2. `src/instructions.md`, `.ctl/operator/instructions.md.jinja`,
   `skills/verctl/SKILL.md`, `.ctl/templates/SKILL.md.jinja`.
