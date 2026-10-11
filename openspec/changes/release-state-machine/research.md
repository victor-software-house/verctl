# Prior art

Retrieved 2026-10-11 (UTC). Every claim below was read in code at the pinned
commit, in official documentation, in the issue itself, in registry data, or in
a run of an official release binary. Nothing here is inferred.

## 1. Sources and real usage

| Tool | Pinned at | Usage, 2026-10-11 |
|:--|:--|:--|
| [Changesets][cs] CLI 3.0.3, [action][ca] 2.1.2 | `c73949b`, `615034b` | `@changesets/cli` 5,638,613 npm downloads in the week to 2026-10-10 |
| [release-please][rp] 17.11.2, [action][rpa] 5.0.0 | `fa67225`, `0b6b3fc` | 213,991 npm downloads that week; action dependents page lists 26,612 repositories under its current name |
| [semantic-release][sr] 25.0.9 | `04c1923` | 3,043,852 npm downloads that week |
| [release-plz][rplz] 0.3.171 | `14b0aac` | 45,712 crates.io downloads in 90 days; action dependents page prints 1,704, which GitHub labels approximate |
| [cargo-release][crel] 1.1.6 | `dc81299` | 16,581 in 90 days; 545,322 all time |
| [cargo-workspaces][cws] 0.4.2 | `4165711` | 21,017 in 90 days |
| [cargo-smart-release][csr] 0.21.14 | `0623b29` | 636 in 90 days |
| [knope][knope] 0.23.0 | `c952077` | 1,759 crates.io downloads in 90 days |
| [goreleaser][gor] 2.18.3 | `1942d44` | 52,526 downloads of the v2.18.3 release assets |
| [Cargo][cargo] 1.99.0, current stable since 2026-10-01 | `1ef25ea` | |
| [Bun][bun] 1.4.3 | `c6da4a4` | |
| [bun-release][br] 0.0.3 | `149886d` | the operator's own library |

## 2. How each tool models a release

| Tool | Where state lives | Lockfile | Tag timing | "Already published" | Resume after a partial failure |
|:--|:--|:--|:--|:--|:--|
| Changesets | fragments, then the Version PR, then the npm versions list | not touched | after publish, only for tags it created in that run | registry versions list | a crash after tagging never creates the GitHub Release ([run.ts][cs-run]) |
| release-please | PR label `autorelease: pending` / `tagged` plus a versions manifest file ([manifest.ts][rp-labels]) | text edit of Cargo.lock and package-lock.json | creates the GitHub Release; does not publish | not checked | `release_created` is true only on the run that created the release, so a failed publish is never retried ([index.ts][rpa-out], [#1163][rpa-1163]) |
| semantic-release | tags plus git notes | `npm version` updates package-lock.json | tag pushed before publish ([index.js][sr-tag]) | not checked | a re-run after a failed publish exits green and publishes nothing ([#3178][sr-3178]) |
| release-plz | the registry, tags, the release PR branch | `cargo update --workspace` ([update/mod.rs][rplz-lock]) | after publish | registry pre-check, then stderr match | publish succeeded but tag failed: a re-run skips the package and never tags it ([release.rs][rplz-skip], [#2578][rplz-2578]) |
| cargo-release | none; a local run, dry-run by default | `cargo metadata` ([cargo.rs][crel-lock]) | after publish | full `release` aborts; the `publish` step skips | run the step commands by hand |
| cargo-workspaces | tags and the index | `cargo update -w` | before publish | skips | n/a, tags come first |
| knope | manifest version against the latest tag ([releases/mod.rs][knope-pending]) | text edit of Cargo.lock ([cargo_lock.rs][knope-lock]) | the forge release creates it | no publish step | a failed draft upload: the re-run creates another release and uploads every asset again ([create_release.rs][knope-draft]) |
| goreleaser | HEAD must sit exactly on an existing tag | none | never creates tags | updates an existing release found by tag | an existing asset fails unless replacement is configured |
| bun-release | fragments, packument, remote tag, GitHub Release | `bun update --lockfile-only` in the version script | after publish, checked by peeled SHA | packument | skips without error when HEAD's parent has the same version, so a later commit cancels a pending release ([git.ts][br-gate]) |

## 3. Platform facts the design depends on

1. **Cargo multi-package publish is not resumable.** Cargo 1.90 stabilized
   `cargo publish --workspace`, and the changelog says it is non-atomic. It
   checks every selected package first and fails if any version already exists
   ([cargo_publish.rs][cargo-verify]). An idempotent mode is still an open
   request ([cargo#13397][cargo-13397]).
2. **Cargo waits for the index.** Since Cargo 1.66, `cargo publish` blocks until
   the index lists the new version. Index visibility does not guarantee that a
   dependent can resolve it yet ([release-plz#2946][rplz-2946]).
3. **`cargo update --workspace` is the documented lockfile refresh** after
   version edits. Editing Cargo.lock as text is exact when the bumped crate name
   appears once in the graph, and wrong when two versions of that name are in
   the graph: references written as `"name version"` go stale and `--locked`
   then refuses ([encode.rs][cargo-encode], [lockfile.rs][cargo-lockfile]).
4. **bun.lock decides published dependency versions.** It records a `version`
   for every non-root workspace package ([bun.lock.rs][bun-lock]), and `bun pm
   pack` and `bun publish` replace `workspace:` with the version from the
   lockfile, not from package.json ([pack_command.rs][bun-pack]). A run with
   Bun 1.4.0, 1.4.2, and 1.4.3 confirmed it: after a version bump, a pack
   without a lockfile refresh shipped the old dependency version.
5. **`bun install --frozen-lockfile` does not catch a stale workspace version.**
   It exited 0 on all three versions after the bump.
6. **`bun install --lockfile-only` refreshes workspace versions from Bun 1.4.1**
   ([oven-sh/bun#41302][bun-41302]). On 1.4.0 it left the lockfile stale.
7. **Changesets publishes Bun workspaces through npm.** Its publish tool is
   pnpm, yarn berry, or npm ([getPublishTool.ts][cs-tool]), so `workspace:` and
   `catalog:` ranges ship raw. Bun support is an open, unmerged pull request
   ([changesets#1789][cs-1789]).
8. **Bun has no OIDC exchange, no provenance, and no JSON output for
   `publish`** ([oven-sh/bun#22423][bun-22423] and [#15601][bun-15601], both
   open). bun-release exchanges a GitHub OIDC token for an npm token itself,
   one package name per exchange.
9. **npm trusted publishing cannot create a package**, and a new trusted
   publisher configuration expires after 48 hours unless a publish goes through
   it. Direct publishing through OIDC is opt-in for configurations created after
   2026-09-03. 2FA-bypass tokens lose direct publishing around January 2027
   (GitHub Changelog, 2026-07-08, 2026-09-03, 2026-10-02).
10. **crates.io trusted publishing** needs `id-token: write`; the exchange
    returns a token valid for 30 minutes. The first publish of a crate still
    needs an API token.
11. **A pull request opened with `GITHUB_TOKEN` gets its CI runs held for
    approval**, and `labeled` events from that token start no run at all. A
    GitHub App token avoids both ([GitHub docs][gh-token]).
12. **Release assets report `state`, `size`, and a `sha256` digest** in the
    REST API, and a release reports whether it is a draft and whether it is
    immutable. Read from this repository's v0.3.12 release.

## 4. What verctl adopts

1. Changesets fragments and bump arithmetic as the only source of version
   intent. Every tool that infers versions from commits or registry diffs
   answers a different question.
2. The registry as the record of "published", queried before each publish:
   the crates.io sparse index, the npm packument.
3. Publish, tag, GitHub Release, and each asset decided from their own
   existence, every run. Never mark one done because another exists. This is
   the gap in release-plz, Changesets, release-please, and semantic-release.
4. Tag after publish, at the exact release commit, checked by peeled SHA; a tag
   on another commit is a conflict (bun-release, goreleaser).
5. A release with assets starts as a draft and is published once every asset is
   present (goreleaser, knope), and an existing release is found by tag and
   updated, never duplicated.
6. Publish order from the dependency graph over normal and build dependencies,
   excluding dev dependencies (cargo-release, release-plz).
7. Dependents whose requirement no longer admits the new version get a patch
   bump, repeated until nothing changes (release-plz, Changesets).
8. The package manager refreshes the lockfile on the Version PR: `cargo update
   --workspace`, `bun install --lockfile-only` on Bun 1.4.1 or later.
9. For Bun: pack once, inspect the packed package.json, then publish that
   tarball (Changesets 3 `pack` and `publish --from-pack-dir`).
10. One OIDC exchange per package where the registry supports it, done by
    verctl, because Bun cannot.
11. A versioned, machine-readable plan with dependency chunks (Changesets 3
    `publish-plan`).
12. "Do not open a new Version PR while a merged one is unreleased"
    (release-please).

## 5. What verctl rejects

1. Labels, event payloads, or a versions manifest file as release state.
2. Tagging before publish.
3. Matching registry error text to detect "already published".
4. `cargo publish --workspace` as the only publish path.
5. Deciding to release from "this commit changed the version" or "fragments
   exist". Both lose a release once another commit lands.
6. `--frozen-lockfile` as proof that a Version PR is consistent.
7. Commit-message version inference, registry tarball diffs, layered config
   files, per-package hooks, blind publish retries, and fixed sleeps.

[cs]: https://github.com/changesets/changesets/tree/c73949ba7b3160a4aa5729223335c190de1528f8
[ca]: https://github.com/changesets/action/tree/615034bb14e5d240e559f941ce9769beb5e14fb0
[cs-run]: https://github.com/changesets/action/blob/615034bb14e5d240e559f941ce9769beb5e14fb0/src/run.ts#L207-L245
[cs-tool]: https://github.com/changesets/changesets/blob/c73949ba7b3160a4aa5729223335c190de1528f8/packages/cli/src/commands/publish/getPublishTool.ts#L40-L56
[cs-1789]: https://github.com/changesets/changesets/pull/1789
[rp]: https://github.com/googleapis/release-please/tree/fa67225b42d8133331a1b48e89640b60e9ca6a23
[rpa]: https://github.com/googleapis/release-please-action/tree/0b6b3fc0186a2f7118bfd88fab9ea481e1839504
[rp-labels]: https://github.com/googleapis/release-please/blob/fa67225b42d8133331a1b48e89640b60e9ca6a23/src/manifest.ts#L289-L292
[rpa-out]: https://github.com/googleapis/release-please-action/blob/0b6b3fc0186a2f7118bfd88fab9ea481e1839504/src/index.ts#L186-L216
[rpa-1163]: https://github.com/googleapis/release-please-action/issues/1163
[sr]: https://github.com/semantic-release/semantic-release/tree/04c1923646eb801cf6f0f90455cf00b78b94fdb7
[sr-tag]: https://github.com/semantic-release/semantic-release/blob/04c1923646eb801cf6f0f90455cf00b78b94fdb7/index.js#L204-L215
[sr-3178]: https://github.com/semantic-release/semantic-release/issues/3178
[rplz]: https://github.com/release-plz/release-plz/tree/14b0aacfca28ffc998781886d163c1ef55e9774a
[rplz-lock]: https://github.com/release-plz/release-plz/blob/14b0aacfca28ffc998781886d163c1ef55e9774a/crates/release_plz_core/src/command/update/mod.rs#L139-L158
[rplz-skip]: https://github.com/release-plz/release-plz/blob/14b0aacfca28ffc998781886d163c1ef55e9774a/crates/release_plz_core/src/command/release.rs#L713-L766
[rplz-2578]: https://github.com/release-plz/release-plz/issues/2578
[rplz-2946]: https://github.com/release-plz/release-plz/issues/2946
[crel]: https://github.com/crate-ci/cargo-release/tree/dc812993455c219b290ddcda36a786da17910563
[crel-lock]: https://github.com/crate-ci/cargo-release/blob/dc812993455c219b290ddcda36a786da17910563/src/ops/cargo.rs#L468-L474
[cws]: https://github.com/pksunkara/cargo-workspaces/tree/41657116ce4648a7e1a537a5907adf7ed26f1023
[csr]: https://github.com/GitoxideLabs/cargo-smart-release/tree/0623b29bfed963976272a55016b4030f0fa2f895
[knope]: https://github.com/knope-dev/knope/tree/c95207772898f344e160e3434c93f7c9f5fb0362
[knope-pending]: https://github.com/knope-dev/knope/blob/c95207772898f344e160e3434c93f7c9f5fb0362/crates/knope/src/step/releases/mod.rs#L241-L270
[knope-lock]: https://github.com/knope-dev/knope/blob/c95207772898f344e160e3434c93f7c9f5fb0362/crates/knope-versioning/src/versioned_file/cargo_lock.rs#L52-L72
[knope-draft]: https://github.com/knope-dev/knope/blob/c95207772898f344e160e3434c93f7c9f5fb0362/crates/knope/src/integrations/github/create_release.rs#L26-L117
[gor]: https://github.com/goreleaser/goreleaser/tree/1942d44355492f44cd3774c57bd3c27fe544dfe9
[cargo]: https://github.com/rust-lang/cargo/tree/1ef25ea3d391374fe3bed4dcbe24f0d3be3ab8f9
[cargo-verify]: https://github.com/rust-lang/cargo/blob/1ef25ea3d391374fe3bed4dcbe24f0d3be3ab8f9/src/ops/registry/cargo_publish.rs#L450-L483
[cargo-13397]: https://github.com/rust-lang/cargo/issues/13397
[cargo-encode]: https://github.com/rust-lang/cargo/blob/1ef25ea3d391374fe3bed4dcbe24f0d3be3ab8f9/src/resolver/encode.rs#L574-L660
[cargo-lockfile]: https://github.com/rust-lang/cargo/blob/1ef25ea3d391374fe3bed4dcbe24f0d3be3ab8f9/src/ops/lockfile.rs#L45-L69
[bun]: https://github.com/oven-sh/bun/tree/c6da4a4d3010e5553438c60f6bd76d981976867c
[bun-lock]: https://github.com/oven-sh/bun/blob/c6da4a4d3010e5553438c60f6bd76d981976867c/src/install/lockfile/bun.lock.rs#L1286-L1290
[bun-pack]: https://github.com/oven-sh/bun/blob/c6da4a4d3010e5553438c60f6bd76d981976867c/src/runtime/cli/pack_command.rs#L3387-L3396
[br]: https://github.com/victor-software-house/bun-release/tree/149886d2cb57d80b565ea4fdbcb92922ea4acd51
[br-gate]: https://github.com/victor-software-house/bun-release/blob/149886d2cb57d80b565ea4fdbcb92922ea4acd51/src/release/git.ts#L4-L12
[gh-token]: https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow
[bun-41302]: https://github.com/oven-sh/bun/pull/41302
[bun-22423]: https://github.com/oven-sh/bun/issues/22423
[bun-15601]: https://github.com/oven-sh/bun/issues/15601
