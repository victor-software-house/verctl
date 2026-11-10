# Design

## Context

The stack is guessed from the manifest file name in four places.
The lockfile refresh is decided in three.
The only extension point is `prepare.after`, whose effect is unknown.
This change builds a release from blocks with one contract each, and makes
steps the extension unit.

This change is one part of [release-state-machine][rsm]. The cargo and bun
providers are [built-in-ecosystems][bie].

## 1. Building blocks

### 1.1 The blocks

A release is built from these blocks. Each has one contract: its inputs, what
it may change, and the fact verctl checks after it runs.

| Block | Does | May change | verctl checks after |
|:--|:--|:--|:--|
| Manifest | read and write a package's version; list its workspace dependencies; say whether it publishes | the manifest | the version reads back |
| Lockfile | bring a lockfile into agreement with the manifests by running the tool that owns it | the declared lockfile only | the parser reads agreement |
| Artifact | produce the file a registry receives, and inspect it | a scratch directory | the inspection passes |
| Registry | query "is `p@v` published", publish an artifact, wait until visible, authenticate | the registry | the query lists `p@v` |
| Forge | the Version PR, tags, Releases, assets | GitHub | each object reads back |
| Served files | render templates at the versions on HEAD | the rendered files | each file equals its render |
| Binary asset | build a native binary for one target | a scratch directory | the archive exists |
| Step | run a mise task or an argv at a named point | only its declared `writes` | its `check`, when declared |

### 1.2 Providers

An ecosystem is a set of providers for the Manifest, Lockfile, Artifact, and
Registry blocks.

1. **Built-in `cargo` and `bun`.** Selected by the manifest file name.
2. **Declared ecosystems.** A repository names one under `ecosystems` and gives
   each block a step. A package selects it with `ecosystem: <name>`.
3. **Per-package override.** Any package may replace one block of its
   ecosystem with a step and keep the rest built in.
   An example is a cargo crate that publishes through a mise task while
   verctl still bumps its version and refreshes `Cargo.lock`.

So a built-in is a default, never a ceiling.

### 1.3 Steps: the extension unit

A step is named once under `steps` and listed by name where it applies.

```yaml
steps:
  version-file:
    task: version:write-file        # a mise task, run with `mise run --raw`
    writes: [src/version.rs]        # the only paths it may change
  smoke:
    run: [bun, run, smoke]          # or an argv
  announce:
    task: release:announce
    check: [bun, run, announce:sent]  # postcondition; exit 0 means done
```

A step listed in the top-level `at` runs once per run; a step listed in
`packages[].at` runs once for that package. Inputs reach a step as
environment variables: `VERCTL_PACKAGE`, `VERCTL_VERSION`,
`VERCTL_PREVIOUS_VERSION`, `VERCTL_TAG`, `VERCTL_RELEASE_COMMIT`, and
`VERCTL_PLAN`, the path of the plan as JSON; a run-level step gets only
`VERCTL_PLAN`. The outcome is the exit status. verctl then checks what the step
was allowed to do.

Steps attach at these points:

| Point | When | Contract |
|:--|:--|:--|
| `version` | after manifests and lockfiles are written, before served files render | may change only its `writes`; its changes ride the Version PR |
| `verify` | before any release side effect | may change nothing; a failure stops the release |
| `publish` | replaces a package's built-in publish | the registry query must list `p@v` afterwards |
| `released` | after `finalize` | must declare `check`, so a rerun skips a step that already ran |

Steps replace `prepare.after`, `prepare.stage`, and `prepare.stage_ignored`,
and they answer the request for publish-time steps in queue row
[VER-034][ver-034]. The difference from
the old hooks is that every step has a declared effect verctl checks, and
nothing it does is only printed.

Alternatives considered:

1. **No hooks** (the first draft). Lost: a stack the built-ins do not cover
   would have no way to release.
2. **Free-form hooks** (today's `prepare.after` plus `stage` globs). Lost: the
   effect is unknown, so `stage` and `stage_ignored` had to be invented to
   admit it, and the per-package `after` only printed.
3. **A plugin interface** (WASM or a library ABI). Lost: heavy to build and to
   keep stable; a mise task is already a portable extension unit with its own
   tools, environment, and dependency graph.

## 2. Which steps were a symptom

Before this design, steps existed only as `prepare.after`, and the six known
uses all refreshed `Cargo.lock`. The Lockfile block now does that, so those six
repositories declare no step at all. Steps remain for what no built-in covers.
That is generated version files, smoke tests before publish, announcements
after a release, and every block of a stack verctl does not ship.

[rsm]: ../release-state-machine/design.md
[bie]: ../built-in-ecosystems/design.md
[ver-034]: ../../../tasks.yaml
