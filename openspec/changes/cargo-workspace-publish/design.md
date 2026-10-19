# Design

## Context

`verctl publish` runs one upload command per package, then tags and opens the
Release. Since [#162](https://github.com/victor-software-house/verctl/pull/162)
a stock cargo package bound for crates.io is checked against the sparse index
before and after its upload. A package with an overridden command has no
check, so its "already published" comes from stderr.

A workspace whose crates share `workspace.package.version` declares one
package at the root manifest. The stock cargo publisher cannot serve it: the
root of a virtual workspace names no package, so the index check fails with
"names no package", and the repository overrides the upload with
`cargo publish --workspace`, which loses the check.

## 1. The mode

A package is a workspace package when all of these hold:

1. Its publisher is the stock `cargo` one, named or inferred from
   `Cargo.toml`.
2. Neither the package (`publish`) nor `publishers.cargo` overrides the upload
   command, and neither declares a `query`.
3. Its `path` is a virtual manifest: a `Cargo.toml` with a `[workspace]` table
   and no `[package]` table.

Override: declare `publish` on the package, which runs that one command as
today. Disable path: the same, or point `path` at a member manifest. No key
turns the mode on; a virtual manifest under the stock publisher fails
today, so no configuration that works changes behaviour.

Alternative considered: a new `workspace: true` key. Lost: the manifest
already says the package is a workspace, a key would be a second statement of
it, and every other value of the key would be an error.

## 2. Members

verctl runs `cargo metadata --format-version 1 --no-deps --manifest-path
<path>` and reads `workspace_members` and `packages`. Cargo resolves member
globs, `version.workspace`, and `publish.workspace`, so verctl reads none of
them itself.

1. A member whose `publish` is `[]` (`publish = false`) is never published.
2. Every other member is uploaded at the version cargo reports for it.
3. Its check is the stock one for its own target: the crates.io index when
   the package's `registry`, the member's `publish` list, and Cargo's
   `registry.default` all agree on crates.io, else no check, as for any
   cargo package today.

Alternative considered: parse `[workspace].members` and each member manifest
with `toml_edit`. Lost: it re-implements globbing and inheritance that Cargo
owns. Every job that publishes already has cargo for the upload.

## 3. Order

A member waits for every other published member it depends on through a
dependency that counts: normal and build dependencies, target-specific ones
included, and a development dependency that declares a version, which
`cargo publish` keeps. A path-only development dependency does not count,
since `cargo publish` strips it. Ties keep `workspace_members` order.

A cycle among dependencies that count fails before any upload and names it,
because cargo cannot publish crates with cyclic versioned development
dependencies ([rust-lang/cargo#4242](https://github.com/rust-lang/cargo/issues/4242)).

Alternative considered: `cargo publish --workspace` orders the crates
itself. Lost: it is the command that cannot resume (proposal, item 1).

## 4. Upload, skip, and wait

Each member runs `cargo publish --locked --manifest-path <path> --package
<member>`, plus `--registry` when the package sets one, under the existing
10-minute timeout. With a check, the member goes through the same steps as a
single package: a listed version is skipped and reported `already`, and an
uploaded one must appear in the index within the existing backoff.

Each member is one report row, so the run shows which crates it uploaded and
which it skipped.

## 5. Rate limit

crates.io answers a publish over its limit with status 429 and a message that
says `Please try again after <time>`. When an upload fails that way, verctl
stops and exits non-zero with an error that names the member and that time,
and says that a rerun then skips every member the index lists. The tag and
Release loop never runs, so no tag exists until every member is listed.

verctl does not sleep until the retry time. A first release of 7 new crates
waits 20 minutes after the burst, which would hold a runner idle, and the
next run picks up where this one stopped. Override: none needed; the
operator reruns the publish. Alternative considered: retry after the given
time inside the run. Lost: it bills an idle runner and pushes the run past
any job timeout for larger workspaces.

## 6. Completeness and docs

`pending` counts a workspace package complete when every published member's
version is listed (or, for a member with no check, when the tag exists), plus
the tag and Release as for any package. `verctl docs` waits for each published
member on crates.io that has a library target.

## 7. What stays

The package's version, changelog, tag, and Release do not change: they belong
to the package, not to the members. The upload command the plan shows for the
package stays the stock one; the report rows show the members.
