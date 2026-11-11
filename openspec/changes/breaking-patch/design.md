# Design

## Context

See [proposal.md](proposal.md) for why, and the specs for what:
[`breaking-patch`](specs/breaking-patch/spec.md), [`semver-check`](specs/semver-check/spec.md), and [`changelog-groups`](specs/changelog-groups/spec.md).

1. verctl parses fragments itself ([`src/fragment.rs`][fragment]).
   Frontmatter maps each package to a `Bump`, and the `verctl:group` key already sits beside the packages.
2. `verctl semver` checks a crate with `--release-type minor` when its pending release is a patch, and skips it when a fragment bumps the minor or the version already moved ([`src/semver_check.rs`][semver]).
3. The changelog groups entries under `### Added`, `### Changed`, and `### Fixed` once any fragment in the section names a group ([`release::group_entries`][release]).

## Goals / Non-Goals

**Goals:**

1. The exception lives as long as one release, and nothing remains to delete after it.
2. It names one package, so a fragment that bumps two packages excuses only the one it marks.
3. The release changelog lists the break.

**Non-Goals:**

1. Excusing some lints and keeping others. cargo-semver-checks reads lint levels only from `Cargo.toml` ([README][csc-levels]), so a per-lint exception would bring the table back.
2. A mark for a stack without an API check. Only cargo has one today.

## Prior art

Read on 2026-11-10 (UTC).

- **cargo-semver-checks lint levels** ([README][csc-levels]): `[package.metadata.cargo-semver-checks.lints]` sets each lint to `deny`, `warn`, or `allow`.
  It applies to every run until the table is edited again, which is why [#235][pr-235] existed.
- **Conventional Commits** ([1.0.0][cc]): `!` after the type, as in `feat!:`, marks a breaking change, and tools render those commits under a breaking heading.
- **Upstream drift exception** in another repository of ours: a patch changeset holds the package, the reason, and the task that removes it.
  The gate reads the pending changesets, skips only while that changeset is pending, and the changelog template lists it under its own heading.

What this design takes:

1. The exception rides on a fragment, as the drift exception does.
2. The mark is `!` beside the bump, as Conventional Commits writes it beside the type.
3. The changelog lists the break under its own heading.

## Decisions

### 1. The mark is the bump value `patch!`

```yaml
---
verctl: patch!
verctl:group: changed
---

`PackageSpec.publish` replaces `PackageSpec.publish_argv`.
```

The fragment body is the reason, and it becomes the changelog entry.

Alternatives considered:

1. **A `Cargo.toml` lint table.** It lost because it outlives the release and needs a second pull request to remove.
2. **A frontmatter key such as `verctl:semver: allow`.** It lost because a key covers the whole fragment, and a fragment may bump several packages.
   `patch!` sits on the one package it excuses.
3. **A JSON comment in the body**, as the drift exception writes it. It lost because verctl owns the frontmatter parser, and frontmatter is where a bump lives.
4. **A flag or variable on `verctl semver`.** It lost because CI runs the gate on every pull request, so a flag would excuse every later change until someone removed it.

Disable: delete the `!`.
Override: none is needed, because the mark is the override.

### 2. `PackageBump` gains `breaking: bool`

`Bump::parse` reads `patch!` as `Bump::Patch` with `breaking: true`.
`Bump` keeps its order, so every reader that takes the largest bump stays unchanged.

Alternative considered: a `Bump::BreakingPatch` variant.
It lost because `Bump` is ordered, and every `max()` over bumps would need to place it.

`minor!`, `major!`, and `none!` fail the parse:
"`!` goes only on patch: a minor or major already allows an API break, and none releases nothing".

### 3. `verctl semver` skips a marked patch

A crate is skipped with `Decision::BreakingPatch` when both hold:

1. its package's largest pending bump is a patch;
2. a pending fragment bumps the package by `patch!`.

A pending minor or major still reads `skipped: minor fragment`, because that release is no patch.

The Version PR consumes the fragment, yet it still runs the Semver step with the manifest one patch above crates.io.

There the mark is read from the version's CHANGELOG section: a `### Breaking` heading skips the crate the same way.
The heading comes from `group_entries`, not from the template, so a custom template cannot drop it.
Once crates.io holds the version, the section does not count.

The row label is `skipped: patch! fragment`.

### 4. The changelog lists the entry under `### Breaking`

In the section of a package the fragment marks `patch!`, the entry goes under `### Breaking`, the first heading, whatever its `verctl:group` holds.
Grouped rendering then applies to that section, so the other entries go under their groups as today.
In the section of another package the same fragment bumps, the entry renders as any other.

`### Breaking` is decided in `group_entries`, not in `Group`.
`verctl:group: breaking` stays an unknown group, so the heading appears only with the mark.

## Risks / Trade-offs

1. **A whole-crate skip excuses more than the one break.**
   Mitigation: the mark lasts one release, and the reviewer reads the fragment that sets it.
2. **Changesets cannot read `patch!`.**
   It already cannot read `verctl:group`, and verctl is the only reader of `.changeset/` in its callers.
3. **This change breaks verctl's library.** `Decision` gains a variant and `PackageBump` a field.
   Its own fragment marks `verctl: patch!`, so the feature ships through itself.

## Migration Plan

1. Ship in the next patch, with the publish options change, under one `verctl: patch!` fragment.
2. The release after it checks verctl's API again.

Rollback: delete the `!` from the fragment before the Version PR merges.

[fragment]: ../../../src/fragment.rs
[semver]: ../../../src/semver_check.rs
[release]: ../../../src/release.rs
[pr-235]: https://github.com/victor-software-house/verctl/pull/235
[csc-levels]: https://github.com/obi1kenobi/cargo-semver-checks/blob/49125b52e7633e60358f7ecd75bff2948181bc73/README.md
[cc]: https://www.conventionalcommits.org/en/v1.0.0/
