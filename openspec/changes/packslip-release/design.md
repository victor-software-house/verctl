# Design

## Context

See [proposal.md](proposal.md) for why, and [`release-packslip`](specs/release-packslip/spec.md) for what.

The `publish` job downloads the tarballs the asset jobs built into `$RUNNER_TEMP/assets`, then `verctl release` uploads them to the draft Release and publishes it, which creates the tag.
On a rerun the asset jobs build only the tarballs the draft lacks, so the directory can contain a subset.

The [packslip action][action], pinned at [v1.11.5][action-tag] by commit, attests each file, writes and signs `packslip.sigstore.json`, and with `upload: false` leaves the upload to the caller.
It reads a [release.toml][release-toml] through its `manifest` input and takes the project, version, tag and commit from its other inputs, which replace the file's values.

## Decisions

### 1. The manifest is packslip's own file

A repository commits a packslip release.toml, `.ctl/packslip.toml` by convention, and names it in `.ctl/ver.yaml`:

```yaml
packslip: .ctl/packslip.toml
```

The file contains what does not change per release, such as this repository's:

```toml
bin = ["verctl"]

[[resource]]
kind = "completion"
exec = ["verctl", "completion", "{shell}"]
shells = ["bash", "zsh", "fish"]

[[resource]]
kind = "skill"
name = "verctl"
repo = "skills/verctl"
```

verctl checks only that the path is tracked, as it does for a template.
packslip validates the file when it signs.

Alternative: a `packslip` schema in `.ctl/ver.yaml` that verctl turns into flags.
It lost because every packslip key would need verctl code, and verctl would copy a schema packslip already owns.
The cost is that a mistake in the file shows only in the `publish` job; the draft then stays incomplete, and a rerun after the fix finishes it.

Disable path: omit `packslip`.

### 2. Platforms come from the files

packslip reads the os and architecture from each file name, so `verctl_0.9.0_linux_arm64.tar.gz` and `verctl_0.9.0_macos_arm64.tar.gz` need nothing more.
It takes the C library from a `musl` or `gnu` word in the name ([C library][c-library]).
Without one, it reads the executables `bin` names: a glibc build records `gnu`, and a static build records none, which fits glibc and musl hosts.

verctl therefore adds no platform field, and a new target is a declaration only.

Two targets for one os and architecture, such as a glibc and a musl build, would render the same name.
A target may set `tarball`, its file name template, with `{bin}`, `{version}`, `{os}` and `{arch}`, where `{os}` renders `macos` for `darwin` as in the default name.
With the `musl` word in its name, the musl build below records `musl`, so a glibc host takes `linux-x64` and a musl host takes `linux-x64-musl`:

```yaml
runners:
  x64:
    labels: [ubuntu-26.04]

assets:
  linux-x64: {}
  linux-x64-musl:
    runner: x64
    os: linux
    arch: x64
    triple: x86_64-unknown-linux-musl
    tarball: "{bin}_{version}_linux_x64_musl.tar.gz"
```

A musl build also overrides `prepare` and `build` for its C toolchain, as any target may.
The default stays `{bin}_{version}_{os}_{arch}.tar.gz`, so no existing name changes.
Two targets that render the same name fail the load.

Alternative: `os`, `arch` and `libc` per target in packslip's spelling, written into the manifest.
It lost because verctl would keep a table in step with packslip's vocabulary, and the libc a file claims could differ from what it loads.

### 3. The bundle covers every tarball on the Release

The bundle must cover every tarball of the Release, including those an earlier run uploaded and this run did not build.
`plan` outputs `uploaded`, the declared tarballs already uploaded to the draft, and none when there is no draft yet.

The `publish` job passes `uploaded` to the action's `download` input, which runs `gh release download`.
gh finds a draft by its pending tag when the token has push access ([gh draft lookup][gh-draft]).
It signs the tarballs this run built and the ones it downloaded, and writes `packslip.sigstore.json` into `VERCTL_ASSETS_DIR`.
`verctl release` uploads the bundle from there.

Alternative: a `verctl release download` verb that fills the directory from the draft.
It lost because it repeats a draft lookup gh already does.

Alternative: rebuild every tarball on each rerun.
It lost because a rebuilt tarball's digest differs from the one already on the draft, and an uploaded asset is not replaced.

With `packslip` set, `packslip.sigstore.json` is a declared asset of the Release that carries the tarballs, so `pending` counts its draft incomplete until the bundle is uploaded.

### 4. verctl's reusable workflow signs

The signing identity is `victor-software-house/verctl/.github/workflows/release.yml`.
packslip accepts a reusable workflow of another repository when the certificate's source repository is the project's ([forge identity][forge]).
mise pins that signer for each project on first use; a project that later releases from another workflow is a signer change its consumers accept with `mise packslip forget`.

Alternative: each caller signs in its own workflow.
It lost because every repository would carry the same steps.

### 5. Build provenance is on from the first release

The action attests each tarball, which needs `attestations: write` on the `publish` job.
GitHub checks a called job's permissions when the run starts, so a caller grants it in the same change that moves its pin to the verctl release adding this job.

Alternative: `attest: false`.
It lost because mise refuses a later release that drops provenance a consumer has seen, so starting without it would block adding it later.

### 6. Completions run the installed binary

A ctl-core CLI prints its registration script with `<bin> completion <shell>` for bash, zsh and fish.
mise runs the `exec` entry on the first TAB, caches the output per version ([completion sources][mise-exec]), and the script calls the binary by name through PATH, which mise points at the active version.
A static file would need a generation step in each asset job.

### 7. Skills are declared, and linking them is the consumer's choice

The `skill` entry is a `repo` source pinned to the release commit, so mise fetches it into the install directory and nowhere else.
Linking it into an agent's skill directory with `mise skills sync` is each consumer's choice.

### 8. Placement beside `assets`

`packslip` sits at the top level beside `assets` today.
The [release-state-machine](../release-state-machine/design.md) migration moves both into `packages[]`.

## Risks

1. A consumer needs a mise with the `packslip:` backend; a private repository also needs mise's GitHub sign-in.
2. A release that fails between upload and publish leaves a draft without its bundle; the next run downloads the tarballs and signs again.

[action]: https://github.com/jdx/packslip/blob/9d5b1c469ba90d26b6dbd0a0162e84a95da524d5/action.yml
[action-tag]: https://github.com/jdx/packslip/tree/9d5b1c469ba90d26b6dbd0a0162e84a95da524d5
[release-toml]: https://github.com/jdx/packslip/blob/9223893b5a001b60a28610d88dac519a34ee9b07/content/docs/release-toml.md
[c-library]: https://github.com/jdx/packslip/blob/9223893b5a001b60a28610d88dac519a34ee9b07/content/docs/release-toml.md#c-library
[gh-draft]: https://github.com/cli/cli/blob/aec38089147ae38264668843fa6126022c56a0e4/pkg/cmd/release/shared/fetch.go#L182-L237
[forge]: https://github.com/jdx/packslip/blob/9223893b5a001b60a28610d88dac519a34ee9b07/src/forge.rs#L1-L19
[mise-exec]: https://github.com/jdx/mise/blob/8e9f47c9c7166f3f6d92df0b7724ab2dbf16381b/src/packslip.rs#L1725-L1760
