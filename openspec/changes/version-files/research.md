# Research: versioning any stack, in one repository or a monorepo

Queue row: VER-083.
Read on 2026-11-09 from source at the commits linked below, not from blog posts.
The five changes that follow from it are listed at the end.

## 1. What we version today, and what we cannot

1. verctl reads a version from a manifest it knows by file name (`Cargo.toml`, `package.json`), or from a declared `toml`, `json`, or command driver ([`src/config.rs`][config]).
2. A repository with no manifest cannot be a package.
   The templates repository has 75 tags, `v0.0.1` to `v0.0.75`, each made by hand, two of them lightweight, and no Release.
   The actions repository has no tag at all: its consumers pin a commit with a `# main, <date>` comment, and five consumers sampled on 2026-11-09 pinned four different commits.
3. A package is listed by hand in `.ctl/ver.yaml`.
   A Cargo workspace is read through `cargo metadata` ([`src/members.rs`][members]); a Bun or pnpm workspace is not read at all.

## 2. Where each tool keeps the version

| Tool | Version lives in | Discovery | Groups | Tag | Go |
|:--|:--|:--|:--|:--|:--|
| [Nx release][nx-actions] | the stack plugin's manifest, a tag, or the registry (`currentVersionResolver`) | project graph | fixed or independent | `v{version}`, `{projectName}@{version}` | no plugin in tree |
| [Knope][knope-files] | each package's own files, any of 13 formats, or a regex over any file | declared, Cargo auto-detected | per package | `v…`, `name/v…` | `// vX.Y.Z` on the `module` line, else the tag under the module directory |
| [release-please][rp-manifest] | one root `.release-please-manifest.json`, plus each package's files | declared per path | `linked-versions` | `component-vX` | optional `version.go` |
| [release-plz][rplz-next] | `Cargo.toml` | `cargo metadata` | per package | `v…`, `pkg-v…` | none |
| [Changesets][cs-version] | `package.json` | manypkg | `fixed`, `linked` | `name@version` | none |

Two models exist.
Knope, Nx, release-plz, and Changesets keep each version in the package's own file.
release-please keeps every version in one root file as well, which can drift from the package files.

[Knope reads Go][knope-go] from a comment on the `module` line of `go.mod`, and falls back to the newest tag whose prefix is the module directory.
On a bump to v2 or later it rewrites the module path, and it tags `v{version}` or `{dir}/v{version}`, the form the Go toolchain reads.

## 3. How monorepo tools find their packages

1. [Turborepo][turbo-globs] reads the package manager's own declaration: `pnpm-workspace.yaml` for pnpm, `package.json` `workspaces` for npm, Yarn, and Bun.
   Each glob gets the manifest name appended; `!` moves a glob to the exclusions; `node_modules` and `.git` are always excluded.
   Each hit is resolved to its real path and refused outside the repository.
2. Turborepo [identifies a package by its manifest name][turbo-identity], and a duplicate name is an error that names both paths.
   Its Cargo, uv, and Go support reuses the same glob engine, and full Cargo discovery shells out to `cargo metadata`, because Cargo owns its membership rules.
3. [mise][mise-roots] declares projects once, in `[monorepo].config_roots`, as relative paths or one-level globs.
   It refuses absolute paths, `..`, `**`, and any match outside the root, and it has deprecated its old walk of every directory in favour of that declaration.
4. [Bun][bun-ws] reads `workspaces` as an array or as an object with `packages`, with `!` negation.
   [pnpm][pnpm-find] reads `packages` from `pnpm-workspace.yaml`, ignores `node_modules`, and sorts by path.
5. [manypkg][manypkg-tool], which Changesets uses, detects the tool (Yarn, pnpm, npm, Bun, Lerna, Rush, or a single root package) and lists its packages; a package with no `name` fails the listing.

The common rule: discovery is declared once at the root, as the stack's own declaration or as globs, never a scan of every manifest on disk.
A package is named by its manifest, and a directory that holds a manifest is the unit.

## 4. Internal dependency ranges and publish order

1. [Bun][bun-pack] and [pnpm][pnpm-export] rewrite `workspace:` and `catalog:` specs only in the packed manifest, never in the source tree.
2. [pnpm 11][pnpm-versioning] reads Changesets fragments natively and bumps a dependent only when the new version leaves the dependent's materialized range (`^V`, `~V`, `V`).
   Its `enforceWorkspaceProtocol` refuses a release whose internal production dependencies do not use `workspace:`, so ranges never need rewriting at release time.
3. [Changesets][cs-version] leaves `workspace:*`, `workspace:^`, and `workspace:~` untouched and rewrites only an explicit range, keeping the prefix.
4. [pnpm][pnpm-sort] and Changesets publish in topological order of the internal dependency graph, restricted to the released set, and report a cycle.
5. `bun pm version` edits one `package.json` and never `bun.lock`, which reads workspace versions from the manifests.

## 5. Releases that cannot move

1. A GitHub [immutable release][gh-immutable], generally available since 2025-10-28, locks its tag to one commit and refuses any asset change once published.
   A tag without a Release is not covered ([GitHub, actions releases][gh-action-releases]).
2. A tag [ruleset][gh-rulesets] can restrict updates and deletions of matching tags.
3. GitHub recommends a full commit SHA as the only immutable reference to a reusable workflow ([GitHub, reuse workflows][gh-reuse]).
   The organization policy that requires SHA pins covers actions, not reusable workflows ([GitHub, organization policy][gh-org-policy]).
4. [Copier][copier-vcs] picks the highest tag that parses as PEP 440 and skips prereleases; `v0.0.75` parses as `0.0.75`.
5. [Renovate][renovate-gha] bumps a SHA pin that carries a trailing version comment (`@<sha> # v1.2.3`), for actions and reusable workflows.
   A regex custom manager can bump other references, such as a mise `git::…?ref=<sha>` include, untested here.
6. [mise][mise-git] caches a remote task include by URL and ref, so a moving tag stays stale; a new SHA is a new key.

## 6. The model these changes build

The operator ruled on 2026-11-09 that versioning is fine-grained, with defaults that keep a simple repository simple, so that complexity is opt-in.
A consumer moves a pin only when the thing it pins changed.
The model is five small primitives that compose:

| Primitive | Holds | Default |
|:--|:--|:--|
| Version source | one tracked file and where the version sits in it: a manifest key, a `VERSION` file, or a marker line | chosen by file name |
| Package | a name, a version source, and the files it owns | one package per repository; owned files are the version source's directory |
| Discovery | where packages come from: a workspace file, root globs, or an explicit list | the explicit list, as today |
| Group | packages that share one version | none: each package moves on its own |
| Tag | the tag name of a package or group | `v{version}` for one package, `{name}@{version}` for several |

A package that changed gets a fragment and a new version; a package that did not keeps its version and its tag, so its pins stay where they are.

## 7. The changes

| Change | Decides |
|:--|:--|
| [`version-files`](proposal.md) | the version lives in a tracked file: the stack's manifest, `go.mod`, or a `VERSION` file when nothing else declares one |
| [`package-discovery`](../package-discovery/proposal.md) | packages come from the stack's own workspace declaration and root globs, named by their manifest |
| [`release-groups`](../release-groups/proposal.md) | fixed and independent groups, and one tag per package in a monorepo |
| [`dependent-ranges`](../dependent-ranges/proposal.md) | internal dependency ranges and publish order |
| [`release-immutability`](../release-immutability/proposal.md) | annotated tags, immutable Releases, and how consumers' pins follow a release |

[config]: ../../../src/config.rs
[members]: ../../../src/members.rs
[nx-actions]: https://github.com/nrwl/nx/blob/b68115cd05cd2b79c36b216f6c03dafb8266f1e0/packages/nx/src/command-line/release/version/version-actions.ts#L201
[knope-files]: https://github.com/knope-dev/knope/blob/7a906587d66e81c12d5e3e145a8536510c56f7aa/crates/knope-versioning/src/versioned_file/mod.rs#L347-L361
[knope-go]: https://github.com/knope-dev/knope/blob/7a906587d66e81c12d5e3e145a8536510c56f7aa/crates/knope-versioning/src/versioned_file/go_mod.rs#L28-L160
[rp-manifest]: https://github.com/googleapis/release-please/blob/ea20bc261acd75149f0be90890bb9c90e681eea7/src/manifest.ts#L286
[rplz-next]: https://github.com/release-plz/release-plz/blob/14771fd8fa32d124b97ddc85ca07d941b7b404f6/crates/release_plz_core/src/next_ver.rs#L96-L105
[cs-version]: https://github.com/changesets/changesets/blob/2f6995d5bbd3b1572d7a6cd897d2229bfc1ce8c1/packages/apply-release-plan/src/version-package.ts#L22-L107
[turbo-globs]: https://github.com/vercel/turborepo/blob/e9d0b1bba7a6c43aa30520a65b2fcd8538758be3/crates/turborepo-repository/src/workspaces.rs#L199-L222
[turbo-identity]: https://github.com/vercel/turborepo/blob/e9d0b1bba7a6c43aa30520a65b2fcd8538758be3/crates/turborepo-repository/src/package_graph/builder.rs#L73-L84
[mise-roots]: https://github.com/jdx/mise/blob/32c96c4d51d3abe9af0c868eda63aabe77efddb0/src/config/mod.rs#L6031-L6075
[bun-ws]: https://github.com/oven-sh/bun/blob/2596ac5d391d47804c93a42bdbe615804aa62387/src/install/lockfile/Package/WorkspaceMap.rs#L354-L432
[pnpm-find]: https://github.com/pnpm/pnpm/blob/5e52de7af190b07a6a9dfbb85a747c6800c90d92/pnpm11/workspace/projects-reader/src/findPackages.ts#L12-L38
[manypkg-tool]: https://github.com/Thinkmill/manypkg/blob/97e2ab9bc64e45af099f63f25c515f3792738cfb/packages/tools/src/Tool.ts
[bun-pack]: https://github.com/oven-sh/bun/blob/2596ac5d391d47804c93a42bdbe615804aa62387/src/runtime/cli/pack_command.rs#L2465-L2535
[pnpm-export]: https://github.com/pnpm/pnpm/blob/5e52de7af190b07a6a9dfbb85a747c6800c90d92/pnpm11/releasing/exportable-manifest/src/index.ts#L257-L337
[pnpm-versioning]: https://github.com/pnpm/pnpm/blob/5e52de7af190b07a6a9dfbb85a747c6800c90d92/pnpm11/releasing/versioning/src/participants.ts#L138-L142
[pnpm-sort]: https://github.com/pnpm/pnpm/blob/5e52de7af190b07a6a9dfbb85a747c6800c90d92/pnpm11/workspace/projects-sorter/src/index.ts#L9-L53
[gh-immutable]: https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases
[gh-action-releases]: https://docs.github.com/en/actions/how-tos/create-and-publish-actions/using-immutable-releases-and-tags-to-manage-your-actions-releases
[gh-rulesets]: https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets
[gh-reuse]: https://docs.github.com/en/actions/how-tos/reuse-automations/reuse-workflows
[gh-org-policy]: https://docs.github.com/en/organizations/managing-organization-settings/disabling-or-limiting-github-actions-for-your-organization
[copier-vcs]: https://github.com/copier-org/copier/blob/dbb2408ceefd6b32b8c6c3fb6524be1e8a1727a1/copier/_vcs.py#L165-L200
[renovate-gha]: https://github.com/renovatebot/renovate/blob/677638d3f9a476b575a5d0a3c1e9c27e437dedad/lib/modules/manager/github-actions/readme.md
[mise-git]: https://github.com/jdx/mise/blob/32c96c4d51d3abe9af0c868eda63aabe77efddb0/src/task/task_file_providers/remote_task_git.rs#L120-L127
