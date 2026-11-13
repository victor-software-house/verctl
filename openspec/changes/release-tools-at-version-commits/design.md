# Design

## Context

See [proposal.md](proposal.md) for why, and [`release-workflow-tools`](specs/release-workflow-tools/spec.md) for what.

The plan job runs at HEAD and lists each version commit whose release is incomplete.

For one such commit, the publish job:

1. checks out the commit and installs its tools with `mise install --locked`;
2. installs the plan's verctl from HEAD's lock;
3. runs `mise exec -- "${VERCTL_BIN:-verctl}" release`.

The release verb uploads each package through its publisher: the built-in `cargo` or `bun` upload, or a command the config names.
Afterwards, the docs job checks out the same commit, installs the same tools, and waits for docs.rs to build each released crate.

## Goals / Non-Goals

**Goals:**

1. A version commit releases when its lock can install the tools the release runs, whatever else its config names.
2. Every install still comes from a lock, and no checksum check is switched off.
3. A caller can name the tools, or keep today's whole install.

**Non-Goals:**

1. Building an old commit with HEAD's toolchain. The publisher's tools still come from the version commit's own lock.
2. Changing the asset jobs. Their recipes run tools verctl cannot list.
3. Changing which commit a release publishes from. The job keeps checking out the version commit, as today.
4. Changing which pending version commits a run releases. Both questions go to the operator as their own change.

## Decisions

### 1. The plan job lists the publisher tools for each version commit

`status --github-output` writes a `release-tools` map from version commit to a space-separated tool list.
Each package adds its publisher's tools: `rust` for the built-in cargo publisher, and `bun` for the built-in bun publisher.
A package whose publisher is a custom command adds `all`, because verctl cannot know what the command runs.
A list that holds `all` installs the whole config.

Alternatives considered:

1. **Keep the whole install.** It lost because one tool without a checksum in an old lock blocks every release after it, as in shrub.
2. **Set `MISE_LOCKED_REQUIRE_CHECKSUM=false` at a version commit.** It lost because mise would then install whatever an unpinned URL serves.
3. **Take a missing entry from HEAD's lock.** It lost because the job would write a merged lock into the version commit's checkout, and it still fails for a tool whose version HEAD no longer pins.

### 2. The publish job installs that list, and mise installs nothing more

The publish job runs `mise install --locked <tools>` with the list for its commit, and runs the release with `MISE_EXEC_AUTO_INSTALL=false`.
When the version commit's rust entry sets `mr_boxington`, cargo runs through mbx, so the job adds `mr-boxington` to a list that holds `rust`.
The job reads that option from the version commit's config with mise, never by parsing the file.

Alternatives considered:

1. **Let `mise exec` install what the command needs.** It lost because `mise exec` installs every missing tool of the config, which is the whole install again.

### 3. The docs job installs no tool

The docs job only polls docs.rs through verctl, so it skips `mise install` at the version commit.
It still installs the plan's verctl from HEAD's lock.

### 4. `release-tools` overrides the list

The input takes a space-separated list of mise tools, used for every version commit in the run.
`all` restores today's whole install, and the default is empty, which takes the plan's list.
This is the public override and the disable path the derived list needs.

## Risks / Trade-offs

1. **A build script runs a tool the publisher does not.** A cargo package whose `build.rs` runs `protoc` fails at `cargo publish` with the tool missing. The caller adds it through `release-tools`, and the failure names the missing command.
2. **The publisher's own tool lacks a checksum in the old lock.** The install still fails, with the tool named, and this change offers no way past it.
