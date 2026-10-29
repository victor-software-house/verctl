# Keep a Release with assets a draft until its assets are uploaded

## Why

GitHub can make a repository's Releases immutable: once a Release is
published, its assets and its tag cannot change
([GitHub docs][immutable]). GitHub's advice is to create the Release as a
draft, attach every asset, then publish it.

`verctl publish` creates every Release published. Two kinds of repository
then attach assets afterwards:

1. A repository that declares `assets` targets: the `assets` job of the
   reusable release workflow uploads each tarball after `publish`.
2. A repository that builds its own files, such as a signed macOS
   installer, runs `verctl publish` and then `gh release upload`.

With immutable Releases on, both uploads are refused, and a Release ships
without its assets. verctl also has no way to name a prebuilt file as part
of a release, so nothing checks that it was attached.

Queue row: VER-079.
A minor: `github::ensure_release`, a public library function, takes the
declared asset names. Every key and flag it adds is optional, and a
repository with no assets releases as before.

It is the `release ──▶ drafted ──asset──▶ finalize ──▶ released` slice of
[VER-011](../release-state-machine/design.md) (section 1.2, decision 11),
built on today's `.ctl/ver.yaml` rather than the 0.4.0 schema.

## What changes

1. `assets.files` lists files a repository builds itself, as paths relative
   to the directory holding `.ctl/`. Each file name is an asset name.
2. When a release declares an asset, a target or a file, `verctl publish`
   creates the tag, then the Release as a draft. A later `verctl publish`
   publishes the draft once every declared asset is uploaded, and otherwise
   reports the draft and the assets it still lacks.
3. `verctl assets --upload --file <path>` uploads one declared file to the
   Release for the plan's tag, draft or published.
4. Every Release lookup sees drafts, and only an asset in state `uploaded`
   counts. A draft Release is an incomplete release.
5. The reusable release workflow gains a `finalize` job after `assets`,
   which runs `verctl publish` again at each version commit.
6. `--skip-assets` leaves every declared asset out, so the Release is
   published at once, as before.

## Impact

1. `src/github.rs`, `src/publish.rs`, `src/pending.rs`, `src/assets.rs`,
   `src/config.rs`, `src/cli.rs`, `src/main.rs`, tests.
2. `.github/workflows/release.yml`: the `plan` job reads drafts, and a new
   `finalize` job.
3. `src/instructions.md`, `skills/verctl/SKILL.md`, README, `docs/`.

[immutable]: https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/immutable-releases
