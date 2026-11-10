# Design

## Context

Cargo and Bun are the stacks verctl ships, and mise installs verctl and runs
its steps. This change states the cargo and bun providers of the blocks in
[building-blocks][bb], bun-release in Rust, and mise's part. It is one part of
[release-state-machine][rsm].

## 1. Built-in cargo

| Block | Provider |
|:--|:--|
| Manifest | `package.version`, or `workspace.package.version` when inherited; dependents from `cargo metadata --no-deps`; `publish = false` never publishes |
| Lockfile | `cargo update --workspace` when `Cargo.lock` is tracked |
| Artifact | packed by `cargo publish` itself |
| Registry | sparse index query; `cargo publish --locked -p <name>` one package at a time in dependency order; the crates.io OIDC exchange when `auth: oidc` |
| Binary asset | `cargo build --release --locked --target <triple>` |

## 2. Built-in bun, and bun-release in Rust

| Block | Provider |
|:--|:--|
| Manifest | `version`; dependents through `workspace:`; `"private": true` never publishes |
| Lockfile | `bun install --lockfile-only` when `bun.lock` is tracked; Bun 1.4.1 or later |
| Artifact | `bun pm pack`, then verctl inspects the packed `package.json`: no `workspace:` or `catalog:`, and each workspace dependency names its current manifest version |
| Registry | verctl's own npm registry client: packument query, the per-package OIDC exchange, the publish request, staged publish, dist-tags, trust configuration, sigstore provenance |

This is bun-release in Rust: its OIDC exchange, packument check, visibility
wait, tag and Release, and bootstrap all become verctl code, and the
TypeScript library retires once the bun ecosystem ships.

| Option | Owns | Gains | Costs |
|:--|:--|:--|:--|
| (a) shell out to `bun publish` | nothing past the argv | least code | no provenance, no staged publish, text-only output, no OIDC, so verctl would still need its own exchange |
| (b) `bun pm pack`, then verctl's registry client | the registry protocol and auth | Bun keeps packing, so the tarball is what `bun publish` would ship; provenance, staged publish, OIDC, and typed results become possible | verctl maintains a registry client; it must inspect the packed manifest because Bun reads workspace versions from `bun.lock` |
| (c) native packing and publishing | everything | no Bun at publish time | a port of npm's file-selection rules with no maintained crate, and rules that drift from Bun's (Bun always packs a root CHANGELOG; pnpm resolves `workspace:` from manifests, Bun from the lockfile) |

Option (b) is chosen. pnpm 12 and aube are working Rust references for the
request body, the OIDC exchange, and provenance, but neither offers them as a
library: pnpm's crates declare `publish = false`, and aube's registry crate
has no publish. verctl therefore writes its own client on `reqwest`, signs
provenance with the `sigstore-sign` crate, and follows the message format npm,
pnpm 12, and Bun all send.

## 3. mise

verctl ships through mise and runs inside mise tasks. The design uses mise for
what it owns and keeps out of what it does not.

| Concern | Owner | How |
|:--|:--|:--|
| Install and verify verctl | mise `github:` backend | asset chosen by os and arch tokens in its name; checksum from the GitHub asset digest; GitHub artifact attestations checked by default on public GitHub |
| Run an extension step | mise | `mise run --raw <task>`; the task's `#MISE tools` install first; the exit status is the outcome; `--raw` keeps stdout clean when a config sets redactions |
| Order work inside a step | mise | a step's task may `depends` on other tasks; verctl never builds a task graph |
| `mise.lock` | mise | verctl never edits it. Only `mise lock` writes it. verctl compares its parsed entries, never its bytes, because mise 2026.10.3 and 2026.10.7 write the same two `rust` entries in opposite order, and `min_version` can set only a floor, not one version for every machine |
| A repository's own tool pin | the built-in `mise-pin` step, listed under `released` | after `finalize`, it runs `mise lock <tool>@<version>` with `MISE_LOCKFILE_MODE=generate`, so a version with no release changes neither file (the default mode rewrites `mise.toml` before it fails). Then verctl reads `mise.lock` and requires a URL and checksum for every platform the lock lists. That is because `mise lock` skips a platform with no asset and still exits 0, and `mise install --locked` checks only the runner's platform |
| Package lockfiles | the package manager, called by verctl | `[deps]` is experimental and has no cargo provider, so verctl runs `cargo` and `bun` from PATH, where mise put them |
| Versions, changelogs, the Version PR, publishing | verctl | mise has no model for them |

What a release must therefore carry for mise to install and verify it: asset
names with os and arch tokens, the GitHub asset digest (GitHub computes it),
and GitHub artifact attestations, which the asset job creates. mise records
`provenance = "github-attestations"` in `mise.lock` only when a release has
them. Today's verctl releases have none, so consumers' locks carry a checksum
and no provenance. A recorded provenance is trusted as written by default: an
install checks the checksum and does not verify the attestation again, so a
hand edit to `mise.lock` can claim provenance that never existed. With
`MISE_LOCKED_VERIFY_PROVENANCE=1`, mise verifies the attestation on every
install and refuses a claim that does not verify, so the served CI workflow
sets it on `mise install --locked`.

Two mise facts bound the design. `MISE_LOCKFILE_MODE=generate` is documented
as a trial mode; if it changes, the `mise-pin` step keeps its read-back check
and stops on a partial write instead of committing it. And from mise
2026.10.8, a task granted secrets ignores `--raw` unless the task itself is
raw or interactive, so a step that needs secrets declares `raw = true`.

## 4. Risks

1. [Bun below 1.4.1 leaves the lockfile stale] → The bun ecosystem refuses an
   older Bun by version.
2. [verctl maintains an npm registry client] → It follows the documented message
   format that npm, pnpm 12, and Bun all send, and it ships behind `auth` and
   `provenance` values that patches add.
3. [`mise lock` can exit 0 with platforms missing] → The `mise-pin` step reads
   `mise.lock` back and requires a URL and checksum for every listed platform.

[rsm]: ../release-state-machine/design.md
[bb]: ../building-blocks/design.md
