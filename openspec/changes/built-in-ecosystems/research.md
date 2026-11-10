# Prior art: npm publishing

Retrieved 2026-10-11 (UTC).
Every claim below was read in one of these places:

1. Code at the pinned commit.
2. Official documentation.
3. The issue itself.
4. Registry data.
5. A run of an official release binary.

Nothing here is inferred.

The rest of the prior art for the release work is in
[release-state-machine](../release-state-machine/research.md).

## 1. pnpm 12

pnpm 12 is written in Rust. Read at [v12.12.1][pn], with its official binary
run in a scratch workspace.

1. **Native Changesets-compatible versioning.**
   - `pnpm change` and `pnpm version -r` read `.changeset/*.md`, plan direct
     bumps, dependents, fixed groups, and per-package release lanes, and record
     consumed intents in `.changeset/ledger.yaml` keyed by `package@version`
     ([lib.rs][pn-versioning], [pnpm/rfcs#18][pn-rfc]).
   - Changelogs can be kept in the registry instead of the repository.
   - In that case an intent file is deleted only once the registry shows the
     published version with its section ([apply.rs][pn-apply]).
2. **Lockfile.** `pnpm-lock.yaml` records a workspace dependency as
   `specifier: workspace:*` with `version: link:../a` and does not store a
   workspace package's own version, so a `workspace:*` bump leaves the file
   unchanged. An explicit range such as `workspace:^0.1.0` does change it;
   `pnpm install --frozen-lockfile` then fails with "dependency is mismatched",
   and `pnpm install --lockfile-only` rewrites that line. Every write goes
   through the lockfile crate's serializer ([save_lockfile.rs][pn-lock]).
3. **Publish.**
   - pnpm resolves `workspace:` from the dependency's current `package.json`
     and `catalog:` from `pnpm-workspace.yaml`, and takes nothing from the
     lockfile.
   - It sends `PUT /<name>` with the version manifest and the tarball under
     `_attachments`.
   - It performs the npm OIDC exchange per package.
   - It signs provenance with sigstore when the repository and package are
     public.
   - It skips versions the packument already lists, and waits until the
     registry serves the new version.
4. **Reuse.** The packing, publish, lockfile, and versioning crates are MIT but
   declare `publish = false` ([publish/Cargo.toml][pn-publish-toml]); none is on
   crates.io.

## 2. npm registry publishing

1. **Bun** packs with its own file rules and always includes a root CHANGELOG.
   It takes `workspace:` and `catalog:` versions from `bun.lock`.
   It sends
   `PUT /<name>` with Bearer or Basic auth and an OTP retry.
   It has no OIDC, no provenance, and text-only output
   ([publish_command.rs][bun-publish]).
2. **npm** sends the same `PUT`, or `POST /-/stage/package/<name>` for a staged
   publish.
   It attaches a sigstore bundle as `<name>-<version>.sigstore` for provenance,
   using Fulcio and Rekor.
   It exchanges a GitHub OIDC token at
   `POST /-/npm/v1/oidc/token/exchange/package/<name>`
   ([publish.js][npm-publish], [oidc.js][npm-oidc]).
3. **aube** is an MIT Rust package manager on crates.io (2.7.0, 2026-10-07).
   It implements publish, the OIDC exchange, and provenance inside a CLI
   command with private helpers ([publish.rs][aube-publish]).
   Its `aube-registry` crate reads packuments and writes dist-tags and access,
   with no publish.
   Only its provenance generator is a public function.
4. **Rust crates** that are maintained: `sigstore-sign` 0.14.0, `tar`, `flate2`,
   `ignore`, `reqwest`. No crate reproduces npm's file-selection rules.

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
