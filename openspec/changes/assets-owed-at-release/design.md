# Design

## Context

See [proposal.md](proposal.md) for why, and [`idempotent-publish`](specs/idempotent-publish/spec.md) for what.

`pending` reads `assets::plan(config, root)` once, at HEAD, and gives its matrix to every release it inspects ([`src/pending.rs`][pending]):

1. the release at each package's version commit, through `state_of`;
2. each earlier unreleased version that `git::unreleased_below` finds, with every target missing.

A target added after a release therefore reopens that release.

## Goals / Non-Goals

**Goals:**

1. A release owes what its own commit declared.
2. Nothing new parses for a release that already has every target.

**Non-Goals:**

1. Backfilling a new target into an old release on purpose. That would need the old commit's build, and the target did not exist there.
2. Which verctl the publish job runs at an old commit. The reusable workflow's version floor is its own change.

## Decisions

### 1. Owed targets are HEAD's targets that the release commit also declared

`owed(root, targets, assets, sha)` returns `targets` unchanged when the Release already holds every asset.
Otherwise it reads `.ctl/ver.yaml` at `sha` and keeps the targets whose id that config declares under `assets.targets`.

The intersection keeps HEAD's matrix, which holds the runner, triple, and file name each job uses.
The old commit decides only which ids it owed.

Alternatives considered:

1. **Build the matrix from the old config.** It lost because the asset jobs run from HEAD's workflow and HEAD's runner records.
   An id the old config declared and HEAD dropped has no runner left to build it.
2. **Compare release dates with the commit that added a target.** It lost because a date is not a fact of the release, and a rerun on another day would change the answer.

### 2. Read only the target ids, and only for a release that lacks a target

A complete release keeps today's path, and its old config is never read.

For an incomplete release, the old revision is read for its `assets.targets` ids and nothing else.
The read ignores unknown keys and runs no validation.
A key that a later verctl renamed or removed must not stop an old release from being judged.
Only YAML that does not parse fails, naming the commit.

Alternative considered: parse the old revision with the current `Config`.
It lost because the current schema refuses unknown keys, so any later rename would fail every incomplete release it touched.

### 3. The old config is the file this run loaded

`pending` takes the path of the config the run loaded, as `release` and `status` resolve it from `-c`.
The old revision is read at that path, so a nested config such as `crates/demo/.ctl/ver.yaml` or a renamed one is found where it lives.

### 4. No config at the commit owes nothing

A commit without `.ctl/ver.yaml` predates the file, so no target was declared there.

## Risks / Trade-offs

1. **A target dropped from HEAD stops counting for old releases.** That was already true.
2. **A shallow clone may lack the version commit.** `version_commit` already needs that commit and fails with its own message first.

[pending]: ../../../src/pending.rs
