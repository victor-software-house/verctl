# Design

## Context

See [proposal.md](proposal.md) for why, and the specs for what:
[`publish-options`](specs/publish-options/spec.md), [`release-only`](specs/release-only/spec.md), and [`workspace-publish`](specs/workspace-publish/spec.md).

Today a publisher is a flat record of command fragments ([`PublisherSpec`][publisher]): `argv`, `config_argv`, `registry_argv`, `default_argv`.
A package picks one by name, and the code that knows cargo sits behind `name == "cargo"`.
Three functions each ask again whether anything was overridden: `resolve`, `registry_check`, and `workspace_members`.
A cargo setting therefore has no field to live in, and nothing separates what a repository wrote from what verctl inferred.

## Goals / Non-Goals

**Goals:**

1. A setting that belongs to one publisher is an option of that publisher, typed and checked at load.
2. A repository writes only what differs from what its files already state.
3. One function resolves how a package is published, and every command reads its result.
4. No caller changes its config.

**Non-Goals:**

1. Removing `publishers`, `publisher`, or `publish` as a list. That is the breaking minor in [release-state-machine][rsm].
2. Options for the asset build. The asset jobs keep compiling from the version commit, with the mbx cache `release.yml` has; [VER-119][queue] holds the rest.
3. Steps, declared ecosystems, and the Lockfile block of [building-blocks][bb].
4. Options for bun. Its publish moves to verctl's own registry client in [built-in-ecosystems][bie], which brings its options.

## Prior art

Read on 2026-11-10 (UTC), from source where a link names a file.

- **mise backends** ([docs][mise-backends], [`tool_version_options.rs`][mise-options], [`backend/cargo.rs`][mise-cargo])
  - A tool is `backend:name`. A short name resolves to a backend in a documented order, and `mise tool <name> --backend` prints the result.
  - One `Backend` trait has one implementation per backend, and each declares the option keys it accepts.
  - A tool's options are two parts flattened into one entry: `CoreToolOptions`, shared by every backend, and the backend's own keys.
    `CargoOptions` is a typed view over those keys, one accessor per option, and the accessor holds the default.
  - An entry with no options is one string. Every resolved option records where it came from.
- **mise workspace graph** ([docs][mise-graph])
  - One provider per stack parses the root manifest and names each project `cargo:<name>`.
  - `[monorepo.projects."<id>"]` overrides one inferred field at a time, and `mise tasks graph --explain` names the file behind each value.
- **cargo-dist** ([`config/v1/mod.rs`][dist-v1], [`layer.rs`][dist-layer], [`builds/mod.rs`][dist-builds])
  - A `…Layer` type is what a file holds, every field optional. A `…Config` type is complete, and the rest of the program reads only that.
  - A tool's settings nest under the tool: `builds.cargo`, `builds.generic`.
  - `BoolOr<T>` lets one key be `false` or a mapping.
  - Its own header calls the half-resolved `…Inheritable` types a mistake and says defaults belong at the end, after every layer.
- **release-plz** ([`config.rs`][rplz-config], [docs][release-plz])
  - Cargo only, and the file is optional: packages come from `cargo metadata`.
  - Cargo flags are flat keys with a prefix: `publish_no_verify`, `publish_features`.
  - Every struct refuses unknown fields.
- **cargo-release** ([reference][cargo-release]): `verify = false` is the key for `--no-verify`.
- **Nx** ([inferred tasks][nx-inferred]): a plugin reads the tool's own config file, and the project's own file overrides what was inferred.
  `nx show project` prints where each setting came from.
- **GoReleaser** ([prebuilt builder][goreleaser-prebuilt]): `builder: prebuilt` is written by hand, with its options in a mapping named after the builder.

What they share, and what this design takes:

1. The tool is inferred from files that already exist. A repository names it only to choose another.
2. Options belong to the tool that reads them, and a wrong key is an error that names the tool.
3. What the file holds and what the program uses are separate types, and defaults are applied once.
4. One key takes a short form or a mapping.
5. The result can be shown with where it came from.

## The model

Four small types, each with one job.

| Type | Module | Job |
|:--|:--|:--|
| `Publish` | `config` | What a package wrote under `publish`: `Options(map)` or `Command(argv)`. Read by a visitor, so a wrong shape is reported in the file's own words |
| `CargoPublish` | `publisher` | The cargo publisher's options, complete: `verify: bool`. `CargoPublish::read` is the one place that names cargo's option keys, their types, and their defaults |
| `Publisher` | `publisher` | How one package is published, resolved: the enum below |
| `opt_out` | `opt_out` | Whether a manifest opts out of publishing, as words an error can repeat. It holds the Cargo and npm readers that `check` already used |

```rust
pub enum Publisher {
    /// A tag and a Release, with no upload.
    ReleaseOnly,
    /// The built-in cargo upload: the crates.io index decides "published",
    /// and a virtual workspace uploads member by member.
    Cargo { recipe: PublisherSpec, options: CargoPublish },
    /// The built-in bun upload, checked by its stock query.
    Bun { recipe: PublisherSpec },
    /// A command the repository wrote: a `publish` list, or a `publishers`
    /// entry with its own `argv`. Checked only by a `query` it declares.
    Command { name: String, recipe: PublisherSpec },
}
```

`Publisher::of(config, package, root)` is the only public constructor.
`Publisher::of_in` runs the same resolution on a merge-base copy, and the load check runs steps 1 to 4 through the same private function.
`resolve`, `registry_check`, `uploads`, `workspace_members`, and the `check` that asks whether a package publishes each match on its result.
The three "was it overridden" tests and the `name == "cargo"` branch are gone, because the variant already says it.

Resolution order, first match wins:

1. `publisher: none` gives `ReleaseOnly`.
2. A `publisher` the package names gives that publisher.
3. Otherwise the manifest file name gives `cargo` or `bun`, and a version source no registry reads gives `ReleaseOnly`, as today.
4. A built-in publisher whose upload command is replaced, by a `publish` list or by `publishers.<name>.argv`, is `Command`.
5. A built-in publisher that was inferred in step 3, and whose manifest opts out, is `ReleaseOnly`.

## Decisions

### 1. Options are a mapping under `publish`, typed by the resolved publisher

```yaml
packages:
  - name: verctl
    path: Cargo.toml
    publish:
      verify: false
```

mise does the same: the backend is resolved first, and the entry's keys are read as that backend's options.

Alternatives:

1. **A key named after the publisher**, `publish: {cargo: {verify: false}}`, as GoReleaser and cargo-dist nest.
   Lost: the publisher is already known from `Cargo.toml`, so the key repeats it, and a second way to name a publisher would sit beside `publisher:`.
2. **A flat key on the package**, `verify: false` or `publish_no_verify: true`, as release-plz has.
   Lost: release-plz is cargo only. Here the key would sit on every package, and the package struct would grow one field per tool.
3. **Extra arguments**, `publishers.cargo.extra_argv: [--no-verify]`.
   Lost: verctl would not know what the arguments mean.
   It could not show them, check them, or keep them apart from arguments that change the registry.
4. **Reusing `default_argv`.** It appends when no registry is named, which is the usual case.
   Lost: it exists for a registry default (`--access public`), and it would stop applying the day a package names a registry.

Override: a `publish` list replaces the whole command.
Disable path: omit `publish`.

### 2. Unknown options fail the load, in the publisher's words

`Config::parse` checks each `publish` mapping against the publisher the package resolves to from the file alone (steps 1 to 4 above).
The messages:

- `package "web" publishes with bun, which has no publish options; remove publish.verify`
- `package "demo": cargo has no publish option "allow_dirty"; its options are: verify`
- `package "demo": publish.verify is true or false`
- `package "demo": publishers.cargo replaces the upload command, which takes no options; remove publish, or the argv`

Alternative: derive `Deserialize` with `deny_unknown_fields` on `CargoPublish` inside an untagged enum.
Lost: serde reports "data did not match any variant", naming a type nobody wrote.

### 3. `verify` is the option name

It is the word cargo's own documentation uses for the step, and cargo-release uses the same key.
The value is positive, so the default reads as the safe case: `verify: true`.

Alternative: `no_verify: true`, the flag's spelling.
Lost: a negated boolean, and `no_verify: false` reads twice.

### 4. A manifest that opts out is read where the publisher resolves

`Cargo.toml` `publish = false` or `[]`, and `package.json` `"private": true`, give `ReleaseOnly` at step 5.
The readers already exist in `src/pull_request.rs`, where `check` compares whether a package publishes at the merge base and at HEAD.
They move to their own module, `opt_out`, which `Publisher::of` calls.
`check` calls the same resolution, through `Publisher::of_in` for the merge-base copy, so the two cannot diverge.

Today such a package fails its release: `cargo publish` refuses a crate with `publish = false`, and `bun publish` refuses a private package.
So no working release changes.

Alternative: keep requiring `publisher: none`.
Lost: 15 callers write it beside a manifest that already states it.
A caller that forgets it gets a failed release after the Version PR merged.

Override: name the `publisher`, or write a `publish` list; either is used as written.
Disable path: none in `.ctl/ver.yaml`. The manifest is where the stack's own tool reads the setting.

The check of options against the publisher (decision 2) runs at load, before any manifest is read.
A `publish` mapping or a `registry` beside a manifest that opts out is therefore caught at step 5, when the plan is built, with:
`package "demo" does not publish: Cargo.toml sets publish = false; remove its publish and registry, or the manifest key`.

### 5. The plan shows options that differ from the default

`via` reads `cargo, no verify`.
Options at their default add nothing, so every existing plan reads as before.

Alternative: a column or JSON field per option with its source, as `mise tasks graph --explain` prints.
Lost for now: one option exists. The label is the place the plan already names the publisher, and a field per option can be added without changing it.

### 6. The old keys stay

`publishers`, `publisher`, `noun`, `query`, and `publish` as a list are read as today.
[release-state-machine][rsm] removes them together, in the breaking minor.
`Publisher::Command` is where they all land, so that removal deletes one variant's inputs and leaves the others alone.

Alternative: remove them now.
Lost: a breaking minor for 19 callers, which the operator batches.

## Risks / Trade-offs

1. [A released verctl reads `publish` as a list only] → verctl's own release runs the verctl built in the same run.
   Its `publish` mapping is therefore read by code that knows the mapping.
   A caller sets a `publish` mapping only after it pins a verctl that has this change; an older pin fails the load, and names the key.
2. [`--no-verify` uploads a crate nobody built] → the option is off unless written.
   The instructions name when to set it.
   CI must build the packed crate on every commit the default branch receives, such as a `cargo package` step the merge queue requires.
3. [The manifest opt-out reads a file at plan time] → the same readers already run in `check`, inside the repository tree, without following a symlink.
4. [`Publisher::of` replaces three code paths at once] → the existing tests in `src/publisher.rs` cover each path and must pass unchanged.
   The one exception is the root argument that `resolve` now takes in place of a manifest path.

## Migration Plan

Additive.
A repository that writes nothing new gets the same commands as before.
The one exception only repairs a failure: a package whose manifest opts out no longer fails at `cargo publish`.
It ships as a patch.

[publisher]: ../../../src/publisher.rs
[queue]: ../../../tasks.yaml
[rsm]: ../release-state-machine/design.md
[bb]: ../building-blocks/design.md
[bie]: ../built-in-ecosystems/design.md
[mise-backends]: https://mise.jdx.dev/dev-tools/backends/
[mise-options]: https://github.com/jdx/mise/blob/main/src/toolset/tool_version_options.rs
[mise-cargo]: https://github.com/jdx/mise/blob/main/src/backend/cargo.rs
[mise-graph]: https://mise.jdx.dev/tasks/workspace-graph.html
[dist-v1]: https://github.com/axodotdev/cargo-dist/blob/main/cargo-dist/src/config/v1/mod.rs
[dist-layer]: https://github.com/axodotdev/cargo-dist/blob/main/cargo-dist/src/config/v1/layer.rs
[dist-builds]: https://github.com/axodotdev/cargo-dist/blob/main/cargo-dist/src/config/v1/builds/mod.rs
[rplz-config]: https://github.com/release-plz/release-plz/blob/main/crates/release_plz/src/config.rs
[release-plz]: https://release-plz.dev/docs/config
[cargo-release]: https://github.com/crate-ci/cargo-release/blob/master/docs/reference.md
[nx-inferred]: https://nx.dev/docs/features/run-tasks#inferred-tasks
[goreleaser-prebuilt]: https://goreleaser.com/customization/builds/builders/prebuilt/
