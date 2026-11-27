# Publish a signed packslip with each Release that has assets

## Motivation

A consumer installs our CLIs through mise's `github:` backend, which picks a tarball by the os and arch words in its name and checks it against GitHub's asset digest.
No file records which tarball fits a host, which executables it contains, or where its completions and skill come from.

A [packslip][spec] is a manifest the release job signs with its own OIDC identity.
It lists each artifact with its digest, platform and executables, and the resources that ship beside them.
mise's [`packslip:` backend][mise-packslip] verifies it, picks the artifact for the host, records a lock entry per platform, and serves version-matched completions and skills ([resources][mise-resources]).

verctl releases every ctl CLI and should publish a packslip for each, with no per-repository workflow code and no new verctl code for a new platform.

Queue row: [VER-145][ver-145].

## Changes

1. `.ctl/ver.yaml` gains `packslip`, the path of a committed packslip [release.toml][release-toml] that contains what does not change per release: executables and resources.
   Omitted, nothing changes.
2. With `packslip` set, the bundle `packslip.sigstore.json` is a declared asset of the Release that carries the tarballs, so a draft without it stays incomplete and a rerun finishes it.
3. `plan` outputs `uploaded`, the declared tarballs already uploaded to the draft. The reusable workflow's `publish` job runs the pinned `jdx/packslip` action over this run's tarballs and `uploaded`, with the manifest, `upload: false` and build provenance on, and `verctl release` uploads the bundle with the tarballs before it publishes the draft.
4. An asset target may set `tarball`, its file name template, so two targets for one os and arch, such as a glibc and a musl build, get distinct names.
   packslip reads each platform from the name and the libc from the executables, so verctl declares no platform table.
5. The `publish` job requests `attestations: write`.
   Each caller grants it when it takes the release that adds this job.

## Capabilities

### New Capabilities

- `release-packslip`: a Release with declared assets can carry a signed packslip built from a committed manifest.

### Modified Capabilities

None.

## Impact

1. Code: `src/config.rs`, `src/assets.rs`, `src/pending.rs`, `src/publish.rs`, the plan output, tests.
2. Workflow: `.github/workflows/release.yml` `plan` outputs and `publish` steps; `self-release.yml` grants `attestations: write`.
3. This repository: `.ctl/packslip.toml` with the verctl executable, its completions and its skill.
4. Docs: `src/instructions.md` and its template, the operator skill, `docs/served-files.md` if a served file names it, and `examples/ver.yaml`.
5. Callers: the templates repository's caller workflow grants `attestations: write` and renders `.ctl/packslip.toml` for a CLI project; that change follows this one.

[spec]: https://github.com/jdx/packslip/blob/9223893b5a001b60a28610d88dac519a34ee9b07/docs/spec/packslip.md
[mise-packslip]: https://github.com/jdx/mise/blob/8e9f47c9c7166f3f6d92df0b7724ab2dbf16381b/docs/dev-tools/backends/packslip.md
[mise-resources]: https://github.com/jdx/mise/blob/8e9f47c9c7166f3f6d92df0b7724ab2dbf16381b/docs/dev-tools/packslip-resources.md
[release-toml]: https://github.com/jdx/packslip/blob/9223893b5a001b60a28610d88dac519a34ee9b07/content/docs/release-toml.md
[ver-145]: ../../../tasks.yaml
