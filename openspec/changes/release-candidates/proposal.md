# Release candidates that never move latest

## Motivation

verctl writes only stable versions.
`bump::apply` clears the prerelease of every version it writes ([`src/bump.rs`][bump]).
A breaking minor such as 0.8.0 therefore reaches every consumer at once, with no way to try it first.

0.8.0 changes `.ctl/ver.yaml`, the commands, and the release workflow ([release-state-machine][rsm]).
19 repositories call `release.yml`.
The first run of a new publish path is the riskiest run, and today that run is the stable release.

The tools around a release already keep a prerelease away from consumers who did not ask for one:

1. Cargo never resolves `=0.7.0` or `^0.7` to a prerelease, and `cargo install` takes one only with `--version` ([Cargo reference][cargo-pre]).
2. mise skips a prerelease for `latest` and for a prefix such as `verctl@0.8` unless a tool sets `prerelease = true` ([mise settings][mise-pre]).
3. GitHub's latest Release skips a Release marked prerelease ([GitHub releases][gh-latest]).
4. npm 11 refuses to publish a prerelease without `--tag` ([npm/cli#7910][npm-tag]), and `bun publish` gives it `latest` unless told otherwise ([bun publish][bun-tag]).

So a candidate only has to be versioned, published, and marked as one.
Nothing else in the chain needs to change.

Queue row: [VER-117][ver-117].

## Changes

1. A package may set `prerelease: rc` in `.ctl/ver.yaml`.
   While it is set, each Version PR writes a candidate: `0.8.0-rc.1`, then `0.8.0-rc.2`.
2. Removing the key makes the next Version PR write the stable version, `0.8.0`, even when no fragment is pending.
   Its CHANGELOG section lists every change from every candidate before it.
3. A candidate's GitHub Release is marked prerelease, with `make_latest: false`.
4. The cargo publisher uploads a candidate as it uploads any version.
   The bun publisher adds `--tag rc`, so the npm `latest` tag stays on the stable version.
5. A pin pattern and a tool pin read and write a whole version, prerelease included.
   A pinned file may then name `0.8.0-rc.1`, and the next Version PR moves it.

Nothing is removed, and no caller must change.

## Capabilities

### New Capabilities

- `release-candidates`: entering and leaving candidate versions, how a candidate version is computed, how it is published and released, and the stable release that follows.

### Modified Capabilities

- `pin-patterns`: a pin reads and writes a prerelease version, where it used to stop the release.

## Impact

1. Code, with tests for each:
   - `src/config.rs`: the `prerelease` field;
   - `src/bump.rs`: candidate versions;
   - `src/prepare.rs` and `src/pending.rs`: the stable release with no new fragment;
   - `src/release.rs` and `src/git.rs`: fragments a candidate consumed;
   - `src/github.rs`: prerelease Releases;
   - `src/publisher.rs`: the bun tag;
   - `src/pins.rs`: the version pattern.
2. Bundle: `verctl instructions`, the operator skill, `README.md`, `examples/ver.yaml`, through their templates.
3. Callers: none must change. A caller that wants to try a candidate pins its exact version, such as `verctl = "0.8.0-rc.1"` in `mise.toml`.
4. Release: a patch. Every change is an addition.
   The first use is 0.8.0 itself: verctl ships `0.8.0-rc.1` to its callers before `0.8.0`.

[bump]: ../../../src/bump.rs
[rsm]: ../release-state-machine/proposal.md
[cargo-pre]: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#pre-releases
[mise-pre]: https://mise.jdx.dev/configuration/settings.html#prereleases
[gh-latest]: https://docs.github.com/en/rest/releases/releases#get-the-latest-release
[npm-tag]: https://github.com/npm/cli/pull/7910
[bun-tag]: https://bun.com/docs/pm/cli/publish#tag
[ver-117]: ../../../tasks.yaml
