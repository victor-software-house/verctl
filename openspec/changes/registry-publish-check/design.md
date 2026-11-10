# Design

## Context

`verctl publish` runs each publisher and decides "already published" from six
substrings of its stderr. A rerun after a partial failure works only when the
registry's wording matches. This change queries the registry instead.

It is one of the parts [reusable-release-workflow][rrw] was split into. The
release at a version commit is [release-at-version-commit][ravc], and the
docs.rs check is [docs-build-check][docs].

## 1. The registry decides "published"

A publisher recipe gains `query`.
It is an argv, with the same placeholders plus `{name}` and `{version}`, that
exits zero and prints the version when the registry has it.
It runs in the package's directory, as the publisher does, so the registry
config the publisher reads (`bunfig.toml`, `.npmrc`) is in effect.
The stock recipes decide "published" like this:

- `cargo` whose publish target is crates.io (below):
  - verctl reads the crate's file in the crates.io sparse index,
    `https://index.crates.io/<prefix>/<name>`.
  - The prefix and the file name are built from the lowercased crate name.
  - verctl looks for a line whose `vers` equals the version as a semver
    version.
- `cargo` with any other target: the package's own `query`, or the stderr match
  when it declares none.
- `bun`: `query`: `bun pm view {name}@{version} version`
- `none`: nothing; the package publishes to no registry.

A cargo package's publish target is crates.io only when every place Cargo
reads it gives the same answer.
That is, all three of these hold:

- the package sets no `registry` in `.ctl/ver.yaml` (or sets `crates-io`)
- its manifest's `publish` field is absent or names only `crates-io`
- the `registry.default` Cargo resolves for the publisher's working directory
  is unset or `crates-io`

verctl resolves it in Cargo's documented order
([config hierarchy][cargo-config]):

1. `CARGO_REGISTRY_DEFAULT`
2. each `.cargo/config.toml` (or legacy `.cargo/config`) from the directory the
   publisher runs in up to the filesystem root
3. `$CARGO_HOME/config.toml` (or legacy `$CARGO_HOME/config`), with
   `CARGO_HOME` defaulting to `~/.cargo`

The first that sets it wins.
The stock cargo publisher runs from the repository root with
`--manifest-path`.
So for it the walk starts at the repository root and never in the package's
directory.

When a repository overrides the cargo publisher, verctl cannot determine the
directory it runs in.
So the target is unknown and the package needs `query`.

When a file it reads cannot be parsed or uses `include`, the target is unknown.
`cargo config get` would return the value directly but needs a nightly Cargo,
which a consumer's release job need not have.

Any target other than crates.io, and an unknown one, needs `query` for a
registry check.
The same holds for an overridden upload command of any publisher.
A bun package whose own argv runs `npm publish --registry …` would otherwise
be checked against the registry the stock `bun pm view` queries.

For a package with a registry check (the crates.io read or a `query`),
verctl decides first.
When the registry has the version, the package is `already` and its publisher
does not run.

Otherwise verctl runs the publisher, then decides again with a bounded backoff
(registry metadata can lag a publish by a short while).
It fails unless the registry lists the version within that bound.

A check that fails during that wait (an index timeout, a 5xx) counts as not
listed for that round, because the upload already happened.
The last round's error is reported if the bound runs out.
The bound is a constant with its reason on it.

A package with no registry check keeps today's rule.
The publish succeeds on a zero exit, or on stderr that shows the version
already exists.

The crates.io check is a direct index read.
`cargo info`, run in a Cargo workspace, can answer for a workspace member from
its local manifest and report an unpublished crate as `already`.

Running it outside the workspace lost.
Cargo then loses the checkout's registry config.
Copying that config into a temporary directory drags in parent-directory
configs, `include` targets, and relative paths.

The sparse index is public, needs no auth, and is Cargo's own source of truth
for crates.io ([sparse index][cargo-sparse]).
Every repository in scope publishes crates to crates.io;
another registry declares `query`.

The recipe is data.
The exact commands are checked against each tool's documentation when they are
written.
The reference goes in the doc comment.

- Override: a `publishers` entry or a package's own `query`, which also
  replaces the crates.io read.
- Disable: `query: []`, which falls back to today's stderr match for that
  package.

The index read and the docs.rs check ([docs-build-check][docs]) use `reqwest`,
declared directly as 0.13.5 with `default-features = false` and the `rustls`
feature.

The operator approved the dependency on 2026-10-17.
gix's HTTP transport already builds that version with rustls, so no crate
enters the build.

octocrab is no substitute: pointed at another host it would send the GitHub
token there.
Starting `curl` would make it a tool every consumer must declare.

Alternative: verctl's own clients for every registry, as
[built-in-ecosystems][bie] section 2 plans. Lost for this slice: each
authenticated registry protocol is a project, while the crates.io index read
is one unauthenticated request.

[rrw]: ../reusable-release-workflow/design.md
[ravc]: ../release-at-version-commit/design.md
[docs]: ../docs-build-check/design.md
[bie]: ../built-in-ecosystems/design.md
[cargo-sparse]: https://doc.rust-lang.org/cargo/reference/registry-index.html#sparse-protocol
[cargo-config]: https://doc.rust-lang.org/cargo/reference/config.html#hierarchical-structure
