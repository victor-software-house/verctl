# Design

## Context

See [proposal.md](proposal.md) for why, and the specs for what:
[`release-candidates`](specs/release-candidates/spec.md) and [`pin-patterns`](specs/pin-patterns/spec.md).

Today every version verctl writes is stable.

1. `bump::apply` clears `version.pre` after every bump ([`src/bump.rs`][bump]).
2. A Version PR deletes the fragments it consumed ([`release::consume_fragments`][release]), so a later Version PR cannot see them.
3. `ensure_release` creates a package's Release with GitHub's defaults ([`src/github.rs`][github]).
   A new published Release that is not a prerelease becomes the repository's latest.
4. The pin pattern's version is `[0-9]+(?:\.[0-9]+)*` ([`src/pins.rs`][pins]).
   A prerelease where a version goes fails the release, by design, so a numeric head is never rewritten with its tail left behind.
5. The crates.io baseline for a semver check is already the newest stable version at or below the current one ([`src/registry.rs`][registry]).

## Goals / Non-Goals

**Goals:**

1. One line in `.ctl/ver.yaml` starts candidates, and removing it ends them.
2. Each candidate is versioned, published, tagged, and released by the same path a stable release takes.
3. No consumer receives a candidate without naming its exact version.
4. The stable release after candidates lists every change they carried, rendered by the same template.
5. Everything else is derived from the manifest version and the branch history, as [derived-release-state][drs] requires.

**Non-Goals:**

1. Candidates released from a second branch while the default branch moves on.
2. A candidate for one group member alone. When [release-groups][groups] lands, `prerelease` belongs to the group.
3. Floating tags. When `tags.floating` lands, a candidate moves none.
4. A prerelease on a version that is not semver, such as a PEP 440 `0.8.0rc1`.
   A driver whose manifest cannot hold the candidate fails the plan, as `Driver::accepts` already makes it.

## Prior art

Read on 2026-11-10 (UTC), from source where a link names a file.

- **Changesets pre mode** ([docs][cs-pre], [`PreState`][cs-prestate])
  - `changeset pre enter <tag>` writes `.changeset/pre.json`: the mode, the tag, each package's version on entry, and the fragment ids already versioned.
  - Versions are `1.0.0-beta.0`, then `beta.1`.
  - Fragment files stay on disk until `pre exit`, so the stable `changeset version` sees them all.
  - The npm dist-tag is the pre tag. A package's first version still gets `latest`.
- **cargo-release** ([reference][cargo-release])
  - `cargo release rc` moves `1.0.0` to `1.0.1-rc.1`, and `rc.1` to `rc.2`.
  - `cargo release release` strips the prerelease. `minor` and `major` bump from the prerelease's base.
  - Its replacements skip a prerelease unless one sets `prerelease = true`.
- **semantic-release** ([pre-release branches][sr-pre])
  - A branch with `prerelease: true` releases `2.0.0-beta.1` from that branch, to a channel named after it.
  - The default branch receives the stable release once the branch merges back.
- **release-plz** ([config][rplz-config])
  - `git_release_type: auto` marks a Release prerelease when the version has a prerelease.
  - It keeps whatever suffix `Cargo.toml` carries and has no way to enter or leave one.
- **GoReleaser** ([release][gr-release])
  - `prerelease: auto` marks a Release prerelease when the tag has one. `make_latest` is a separate setting.
- **node-semver** ([`inc`][semver-inc])
  - `inc('1.2.0-rc.1', 'minor')` gives `1.2.0`: a prerelease whose base already reaches the bump keeps its base.
  - Changesets versions through it.
- **Consumers** ([Cargo][cargo-pre], [mise][mise-pre], [GitHub][gh-latest], [npm][npm-tag], [bun][bun-tag])
  - Cargo, mise, and GitHub's latest Release all skip a prerelease unless asked.
  - npm 11 refuses a prerelease without `--tag`. `bun publish` gives it `latest` unless told otherwise.

What this design takes:

1. The mode and its tag come from one setting, as in Changesets.
2. The base of a candidate follows node-semver's `inc`, and the count starts at 1, as in cargo-release.
3. The Release is marked by the version alone, as release-plz and GoReleaser do.
4. The npm dist-tag is the prerelease tag, as in Changesets.

## Decisions

### 1. The setting is `prerelease` on a package in `.ctl/ver.yaml`

```yaml
packages:
  - name: verctl
    path: Cargo.toml
    prerelease: rc
```

`Option<Prerelease>` on `PackageSpec`.
When omitted, the package releases stable versions.
`Prerelease` is a validated newtype over the tag.

Alternatives considered:

1. **`.changeset/pre.json`.** It lost because it is a state file.
   It records each version on entry and each fragment already versioned, and verctl derives both from history.
   [derived-release-state][drs] rules out a state file.
2. **A prerelease branch**, as semantic-release has. It lost because verctl releases from one branch.
   A second one doubles the Version PR flow and needs a merge back to the default branch.
3. **A verb**, `verctl prerelease enter rc`. It lost because the change needs a pull request anyway.
   The key is one line, and a verb would write YAML a person can write.
4. **A key in fragment frontmatter.** It lost because fragments are Changesets YAML, and Changesets reads no such key.

Override and disable: the key is the whole switch.
Set it to start, change it to move to another tag, remove it to release stable.

### 2. A tag is lowercase ASCII letters

`alpha`, `beta`, `rc`, `next`, and `canary` load.
`rc1`, `RC`, `rc.1`, and `1` fail the load, through a garde rule in `src/config.rs` that names the field.

Letters only keeps three properties:

1. `<tag>.<count>` is a valid semver prerelease, and the count stays a separate numeric identifier.
2. The tag is a valid npm dist-tag, which cannot parse as a version or a range.
3. Tags compare in semver order as words do: `alpha < beta < rc`.

Alternative considered: cargo-release's fixed `alpha`, `beta`, `rc`.
It lost because npm packages use channels such as `next` and `canary`.

### 3. The next version

`bump::apply` takes the package's `prerelease` and computes the next version in two steps.

The **base** comes from the current version and the highest pending bump:

1. A stable current version bumps as today.
2. A prerelease keeps its base when the base already reaches the bump.
   A patch always does. A minor does when the base's patch is 0. A major does when its minor and patch are 0.
3. Otherwise the base bumps as a stable version would.

The **prerelease** comes from the setting:

1. With no `prerelease`, the version is the base.
2. With one, the version is `<base>-<tag>.<count>`.
   The count is the current count plus 1 when the base and the tag are unchanged, and 1 otherwise.

| Current | Pending | `prerelease` | Next |
|:--|:--|:--|:--|
| `0.7.4` | minor | `rc` | `0.8.0-rc.1` |
| `0.8.0-rc.1` | patch | `rc` | `0.8.0-rc.2` |
| `0.8.0-rc.2` | minor | `rc` | `0.8.0-rc.3` |
| `1.2.3` | patch | `rc` | `1.2.4-rc.1` |
| `1.2.4-rc.1` | minor | `rc` | `1.3.0-rc.1` |
| `1.3.0-rc.1` | major | `rc` | `2.0.0-rc.1` |
| `0.8.0-beta.2` | none | `rc` | `0.8.0-rc.1` |
| `0.8.0-rc.2` | none | omitted | `0.8.0` |
| `1.3.0-rc.2` | major | omitted | `2.0.0` |

The refusals stay: a major on 0.x, and a minor or major on `0.0.0` without `first_minor`.

A current prerelease in another shape fails the plan, naming the version and the shape verctl writes.
`0.8.0-rc1` is one: semver orders `0.8.0-rc.2` below it, so counting on would move backwards.
A `<tag>.0` from a repository that used Changesets counts on to `<tag>.1`.

A new tag must sort above the current one.
`0.8.0-rc.2` with `prerelease: beta` fails the plan, because `0.8.0-beta.1` is lower.

### 4. A package is pending without a fragment in two cases

[`pending`][pending] finds a package pending when a fragment bumps it.
Two more cases need no fragment:

1. The current version is a prerelease and `prerelease` is omitted: the stable release.
2. The current version is a prerelease with another tag than `prerelease`: the first candidate under the new tag.

Each case is a fact of the manifest and the config, so nothing records it.
To hold back the stable release, keep the key.

### 5. The stable section is rendered from every candidate's fragments

The stable Version PR renders its CHANGELOG section from two sets of fragments:

1. the fragments pending now;
2. the fragments each candidate since the last stable release consumed, for this package.

The second set comes from history:

1. verctl walks first-parent commits from `HEAD` back to the package's last stable version commit.
2. In each commit's diff against its first parent, a deleted fragment file is read from that first parent.
   The merge queue lands a Version PR as a merge commit, so its deletions show in that diff.
3. `git::added_by` gains a start commit, so a deleted fragment's origin resolves from the parent that still holds it.
   The function is `git::added_by_from`, so the public `added_by` keeps its signature.
4. Only a commit that set the package's version counts. A pull request that deletes a stray fragment adds nothing.

The first candidate under a new tag, with no fragment pending, renders from the second set alone.
`0.8.0-rc.1` after `0.8.0-beta.2` then lists what the beta candidates carried, instead of an empty section.

Every job in `release.yml` checks out with `fetch-depth: 0`.

The candidate sections stay in the CHANGELOG.
Each candidate's Release body is its own section, and the CHANGELOG is never edited by hand.

Alternatives considered:

1. **Keep the fragment files during the mode**, as Changesets does.
   It lost because each later Version PR would need a record of the files earlier candidates versioned.
   That record is the state file decision 1 ruled out.
2. **Merge the candidate sections' text.** It lost because rendered Markdown is the template's output.
   Group headings and credits would be parsed back from it.
3. **Replace the candidate sections with the stable one.** It lost because published candidates would lose the section their Release links to.

### 6. A candidate is published by the package's publisher

1. **cargo**: unchanged. The crates.io index lookup matches the exact version, prerelease included.
2. **bun**: the upload adds `--tag <prerelease>`. A package's first version still receives `latest` from the registry.
3. **A replaced command** (`publish` list or a `publishers` entry with `argv`): unchanged. The argv is the repository's.
   npm 11 itself refuses a prerelease with no `--tag`, so a forgotten tag fails the upload.
4. **Release only**: unchanged.

The semver check keeps its baseline, the newest stable version at or below the current one.
A candidate of a patch is checked against the stable release it follows.

### 7. A candidate's Release is a prerelease

`ensure_release` takes whether the version has a prerelease.
A candidate's Release is created with `prerelease: true` and `make_latest: false`.
GitHub never makes a prerelease the latest ([create a release][gh-create]); verctl states both so the request reads the same as the result.
A stable Release keeps GitHub's defaults.
An existing Release is used as it is, as today.

### 8. A pin reads and writes a whole version

The pin pattern's version becomes `[0-9]+(?:\.[0-9]+)*(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?`.
The rule that each side of `{version}` is bounded stays, so a pattern still matches the whole token.
A tool pin's `?ref=v` value matches the same pattern.
A ref that starts with a digit and is not a whole version still fails.

Pins move on every Version PR, candidates included.
A pin names the version of the commit that holds it.
A candidate's served task and action then run the candidate's own binary, which is what a candidate is for.

Alternative considered: pins skip candidates.
It lost because the candidate would release with the previous stable binary and prove nothing about its own release path.
A per-pin switch waits until a pin needs one.

## Risks / Trade-offs

1. **A shallow clone hides the candidates' fragments.**
   Mitigation: the plan fails and names the commit it could not read, instead of rendering a short stable section.
2. **Between a candidate and the stable release, the default branch names the candidate.**
   A served README or example shows `0.8.0-rc.1` until the stable Version PR merges.
3. **`prerelease` on a stable package does nothing until a fragment lands.**
   `verctl instructions` covers this case.
4. **A command publisher may need its own tag.**
   verctl adds nothing to a replaced argv. The instructions name the flag for npm and bun.

## Migration Plan

1. Ship this change in 0.8.1, a stable patch release.
2. verctl's own package then sets `prerelease: rc`, and the next patch fragment releases `0.8.2-rc.1`.
3. One caller pins `verctl = "0.8.2-rc.1"` and runs its release workflow.
4. verctl removes the key, and the next Version PR writes `0.8.2`.

Rollback: remove the key. A published candidate stays on crates.io and GitHub, and consumers never resolve it unless they pin it.

[bump]: ../../../src/bump.rs
[release]: ../../../src/release.rs
[github]: ../../../src/github.rs
[pins]: ../../../src/pins.rs
[registry]: ../../../src/registry.rs
[pending]: ../../../src/pending.rs
[drs]: ../derived-release-state/proposal.md
[groups]: ../release-groups/proposal.md
[cs-pre]: https://github.com/changesets/changesets/blob/2f6995d5bbd3b1572d7a6cd897d2229bfc1ce8c1/docs/prereleases.md
[cs-prestate]: https://github.com/changesets/changesets/blob/2f6995d5bbd3b1572d7a6cd897d2229bfc1ce8c1/packages/types/src/index.ts
[cargo-release]: https://github.com/crate-ci/cargo-release/blob/5f271e4beb11b59f6305fbce1df7f9033ec27042/docs/reference.md
[sr-pre]: https://semantic-release.gitbook.io/semantic-release/usage/workflow-configuration#pre-release-branches
[rplz-config]: https://release-plz.dev/docs/config
[gr-release]: https://goreleaser.com/customization/release/
[semver-inc]: https://github.com/npm/node-semver/blob/b37b033b585ad5e575afeccadb426ea41bc6cea4/classes/semver.js
[cargo-pre]: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#pre-releases
[mise-pre]: https://mise.jdx.dev/configuration/settings.html#prereleases
[gh-latest]: https://docs.github.com/en/rest/releases/releases#get-the-latest-release
[gh-create]: https://docs.github.com/en/rest/releases/releases#create-a-release
[npm-tag]: https://github.com/npm/cli/pull/7910
[bun-tag]: https://bun.com/docs/pm/cli/publish#tag
