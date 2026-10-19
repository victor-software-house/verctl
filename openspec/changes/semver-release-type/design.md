# Design

## Context

cargo-semver-checks 0.51.0 maps `--release-type minor` to the level Minor,
and `supports_requirement(Minor, Major)` is false, so at that level it runs
exactly the lints whose required update is major: the API breaks. The type
is applied as given, with no 0.x remapping, so it behaves the same at 0.0.z
and 0.y.z. Without a type it infers one from the version change, and when
the version equals the baseline it assumes the smallest next release Cargo
allows: major at 0.0.z, minor at 0.y.z.

## 1. Packages and crates

Fragments name verctl packages; cargo-semver-checks takes crates. verctl
already maps one to the other for publishing: `publisher::uploads` gives one
upload per crate, a member per publishing member of a virtual workspace
package, each with its registry check. `semver` takes the uploads whose
check is the crates.io index, so it checks exactly the crates `publish`
would upload to crates.io.

## 2. The decision

For each crate, in order:

1. Version at 1.0 or later: fail the run (section 5).
2. No release on crates.io at or below the version: skip, `unpublished`.
3. A pending fragment bumps the package `minor` or `major`: skip.
4. The manifest's major.minor differs from the baseline: skip. This covers
   the Version PR and the window before publish, when the fragments are
   gone and the bump is in the manifest.
5. Otherwise run the check.

Alternative considered: decide from the fragments alone. It lost because
the Version PR consumes them, so a minor release would be checked as a
patch on its own Version PR and fail on the break it ships.

The baseline is chosen as cargo-semver-checks chooses it: the newest
release at or below the version that is neither yanked nor a pre-release.
The check is passed `--baseline-version` with that release, so the decision
and the check read the same version.

## 3. Invocation

One `cargo semver-checks --manifest-path <manifest> --package <crate>
--baseline-version <baseline> --release-type minor` per crate, with a
30-minute timeout. Every crate is checked before the run fails, so one
report names every break.

Override: a `minor` fragment for the package skips its crates, which is the
intended way to ship a break. Disable path: do not run the verb; the shared
task is the only caller.

## 4. ctl-core's shared task

ctl-core's `tasks/rust.toml` `semver` task changes, in a ctl-core change
after this release, from `cargo semver-checks` to `mise run ver semver`. A
repository that includes it and runs `semver` serves a `ver` task:

1. A consumer includes
   `git::https://github.com/victor-software-house/verctl.git//tasks/ver?ref=<tag>`,
   which runs the verctl it pins.
2. verctl serves its own `ver` task (`cargo run --`), so it checks with the
   checkout under test.
3. ctl-core and qctl, which serve no `ver` task today, add the include in
   the pull request that takes the ctl-core release.

The repository still pins `cargo-semver-checks`; verctl runs it from PATH.

## 5. 1.0 and later

No family crate is at 1.0. At 1.0 a break needs a major release, so the
rule above would let a minor release ship a break against Cargo's rules.
Reaching 1.0 is the operator's decision, and that decision revisits the
rule; until then `semver` fails on such a crate instead of guessing.
