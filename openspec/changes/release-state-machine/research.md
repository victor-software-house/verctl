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

## 4. Identifying a release pull request

1. GitHub's update endpoint for a pull request accepts `title`, `body`,
   `base`, `state`, and `maintainer_can_modify`, and nothing else
   ([REST description][gh-openapi]). The head branch and head repository are
   fixed for the pull request's life; the base is not.
2. Changesets keeps one Version PR per base branch on `changeset-release/<base>`
   ([run.ts][cs-branch]).
3. release-please names its branch per base and per component,
   `release-please--branches--<base>--components--<component>`, and
   `separate-pull-requests` gives one pull request per component
   ([branch-name.ts][rp-branch]).

## 5. pnpm 12

pnpm 12 is written in Rust. Read at [v12.12.1][pn], with its official binary
run in a scratch workspace.

1. **Native Changesets-compatible versioning.** `pnpm change` and
   `pnpm version -r` read `.changeset/*.md`, plan direct bumps, dependents,
   fixed groups, and per-package release lanes, and record consumed intents in
   `.changeset/ledger.yaml` keyed by `package@version`
   ([lib.rs][pn-versioning], [pnpm/rfcs#18][pn-rfc]). When changelogs are kept
   in the registry instead of the repository, an intent file is deleted only
   once the registry confirms the published version carries its section
   ([apply.rs][pn-apply]).
2. **Lockfile.** `pnpm-lock.yaml` records a workspace dependency as
   `specifier: workspace:*` with `version: link:../a` and does not store a
   workspace package's own version, so a `workspace:*` bump leaves the file
   unchanged. An explicit range such as `workspace:^0.1.0` does change it;
   `pnpm install --frozen-lockfile` then fails with "dependency is mismatched",
   and `pnpm install --lockfile-only` rewrites that line. Every write goes
   through the lockfile crate's serializer ([save_lockfile.rs][pn-lock]).
3. **Publish.** pnpm resolves `workspace:` from the dependency's current
   `package.json` and `catalog:` from `pnpm-workspace.yaml`, never from the
   lockfile. It sends `PUT /<name>` with the version manifest and the tarball
   under `_attachments`, performs the npm OIDC exchange per package, signs
   provenance with sigstore when the repository and package are public, skips
   versions the packument already lists, and waits until the registry serves
   the new version.
4. **Reuse.** The packing, publish, lockfile, and versioning crates are MIT but
   declare `publish = false` ([publish/Cargo.toml][pn-publish-toml]); none is on
   crates.io.

## 6. npm registry publishing

1. **Bun** packs with its own file rules, always includes a root CHANGELOG,
   takes `workspace:` and `catalog:` versions from `bun.lock`, and sends
   `PUT /<name>` with Bearer or Basic auth and an OTP retry. It has no OIDC, no
   provenance, and text-only output ([publish_command.rs][bun-publish]).
2. **npm** sends the same `PUT`, or `POST /-/stage/package/<name>` for a staged
   publish; attaches a sigstore bundle as `<name>-<version>.sigstore` for
   provenance, using Fulcio and Rekor; and exchanges a GitHub OIDC token at
   `POST /-/npm/v1/oidc/token/exchange/package/<name>`
   ([publish.js][npm-publish], [oidc.js][npm-oidc]).
3. **aube**, an MIT Rust package manager on crates.io (2.7.0, 2026-10-07),
   implements publish, the OIDC exchange, and provenance inside a CLI command
   with private helpers ([publish.rs][aube-publish]). Its `aube-registry` crate
   reads packuments and writes dist-tags and access, with no publish. Only its
   provenance generator is a public function.
4. **Rust crates** that are maintained: `sigstore-sign` 0.14.0, `tar`, `flate2`,
   `ignore`, `reqwest`. No crate reproduces npm's file-selection rules.

## 7. mise

Read at jdx/mise [v2026.10.8][mise], with scratch runs of mise in an isolated
environment.

1. `mise run <task> [args]` returns the failed task's exit status and prints no
   JSON. When a loaded config sets `redactions`, task output gains escape codes
   and stdin is disconnected; `--raw` avoids both.
2. A task's `#MISE tools` are installed before it runs, and `depends` and
   structured `run` entries build the task graph.
3. `mise.lock` records, per platform, the asset URL and checksum, and
   `provenance = "github-attestations"` when the release has attestations.
   `mise lock <tool>@<version>` rewrites the pin and the lock without
   installing. It refuses a version that has no release yet, and on a tool with
   no earlier entry it can exit 0 with only the version recorded, so
   `mise install --locked` is the completeness check.
4. `[deps.*]` providers are experimental and have no cargo provider.
5. The `github:` backend picks an asset by os and arch tokens in its name,
   verifies the GitHub asset digest or a checksum file, and checks GitHub
   artifact attestations by default on public GitHub.
6. mise has no versioning, changelog, or release pull request model.

## 8. What verctl adopts

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
13. A Version PR identified by its head repository, head branch, and base,
    the facts a pull request cannot change, with one Version PR per base
    (Changesets).
14. A registry client that sends the documented npm wire format: the `PUT`
    with `_attachments`, staged publish, the per-package OIDC exchange, and a
    sigstore provenance bundle (npm, pnpm 12, aube).
15. Skip versions the packument lists, then wait until the registry serves the
    new version (pnpm 12, bun-release).
16. mise for installing and verifying release binaries, for running extension
    tasks, and for `mise.lock`, moved only by `mise lock` after the release
    exists and checked by `mise install --locked`.

## 9. What verctl rejects

1. Labels, event payloads, or a versions manifest file as release state.
2. Tagging before publish.
3. Matching registry error text to detect "already published".
4. `cargo publish --workspace` as the only publish path.
5. Deciding to release from "this commit changed the version" or "fragments
   exist". Both lose a release once another commit lands.
6. `--frozen-lockfile` as proof that a Version PR is consistent.
7. Commit-message version inference, registry tarball diffs, layered config
   files, release-level actions forced to run once per package, blind publish
   retries, and fixed sleeps.
8. One Version PR per package: dependents span pull requests, and each one
   rewrites the shared lockfile, so every merge conflicts the others.
9. A release branch named after the version: the version changes while the
   pull request is open, and its head branch cannot be renamed.
10. Delegating Cargo or Bun lockfile sync to mise `[deps]`: experimental, and no
    cargo provider.
11. A committed ledger of consumed fragments (pnpm 12): in the Version PR the
    fragment deletion is the record, `check` reads it from the diff, and the
    committed CHANGELOG keeps the prose if a publish fails.
12. Shelling out to `bun publish` (no provenance, staging, OIDC, or typed
    results), and porting npm's file-selection rules (no maintained crate, and
    rules that drift from Bun's).


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
[gh-openapi]: https://github.com/github/rest-api-description/blob/58b1e0c00b39b9e46c24dc4a0ba6a2669c90e9c1/descriptions/api.github.com/api.github.com.json
[cs-branch]: https://github.com/changesets/action/blob/615034bb14e5d240e559f941ce9769beb5e14fb0/src/run.ts#L359
[rp-branch]: https://github.com/googleapis/release-please/blob/fa67225b42d8133331a1b48e89640b60e9ca6a23/src/util/branch-name.ts#L79
[pn]: https://github.com/pnpm/pnpm/tree/65f29a55286fafa57dbf2770e8d2304df0091cfd
[pn-versioning]: https://github.com/pnpm/pnpm/blob/65f29a55286fafa57dbf2770e8d2304df0091cfd/pnpm/crates/versioning/src/lib.rs#L1-L12
[pn-rfc]: https://github.com/pnpm/rfcs/pull/18
[pn-apply]: https://github.com/pnpm/pnpm/blob/65f29a55286fafa57dbf2770e8d2304df0091cfd/pnpm/crates/versioning/src/apply.rs#L33-L46
[pn-lock]: https://github.com/pnpm/pnpm/blob/65f29a55286fafa57dbf2770e8d2304df0091cfd/pnpm/crates/lockfile/src/save_lockfile.rs#L92-L115
[pn-publish-toml]: https://github.com/pnpm/pnpm/blob/65f29a55286fafa57dbf2770e8d2304df0091cfd/pnpm/crates/publish/Cargo.toml#L5
[bun-publish]: https://github.com/oven-sh/bun/blob/c6da4a4d3010e5553438c60f6bd76d981976867c/src/runtime/cli/publish_command.rs#L842-L865
[npm-publish]: https://github.com/npm/cli/blob/c276cadf785c1018d82dd287fe7742567055efb9/workspaces/libnpmpublish/lib/publish.js
[npm-oidc]: https://github.com/npm/cli/blob/c276cadf785c1018d82dd287fe7742567055efb9/lib/utils/oidc.js
[aube-publish]: https://github.com/aubepkg/aube/blob/1443128fdb279a63dd3b57a79e20930efa94db4d/crates/aube/src/commands/publish.rs
[mise]: https://github.com/jdx/mise/tree/b671d356aaf33b87e48e7469a77abb40c3553c68
