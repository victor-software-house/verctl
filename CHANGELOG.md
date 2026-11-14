# Changelog

## verctl 0.8.6-rc.5

### Fixed

- `verctl release --dry-run` and `release bootstrap --dry-run` take the path of a real release through every read and stop before the first write: no draft, upload or tag ([#403](https://github.com/victor-software-house/verctl/pull/403)).

  - A refusal the release would raise is the dry run's error too, with the same message: a tag at another commit, a published Release at another commit, a changed workflow, or an off-branch HEAD.
  - A failed release proof stops the dry run there, listing the proof rows beside the packages still to release.
  - Its rows come from the real plan: each upload it would make, each version the registry already lists as `already`, and each tag as `would create`, `would create draft` or `would complete`.
  - It needs `GITHUB_TOKEN` for the tag and Release reads, and fails without it naming the variable. It needs no registry or owner token.

  A real run drops the `registry` notices of each crate it created, and `release bootstrap` drops the `trust` notice of each crate whose entry it added.
  A dry run keeps those notices as the plan, and a plain `release --new` keeps the `trust` notice as the prompt to run `release trust`.
- Two version PRs prepared in one process in the same clock tick get two scratch directories, where the second had failed with `File exists` ([#405](https://github.com/victor-software-house/verctl/pull/405)).
- The `semver` check treats a Breaking section of any release in the pre-release train above the last stable baseline as a `patch!` mark ([#408](https://github.com/victor-software-house/verctl/pull/408)).

## verctl 0.8.6-rc.4

### Breaking

- `verctl check registry` fails a never-published standalone package whose version is not `0.0.0`, naming the crate, its version, and `0.0.0`, where it passed before.
  A workspace member is exempt, since it first publishes at the group's version.
  The release proof runs `registry`, so `release` stops on it too ([#395](https://github.com/victor-software-house/verctl/pull/395)).

  `verctl release trust [CRATE]...` lists each crate's trusted publisher entries and adds this repository's entry only when none matches, with an owner's `VERCTL_OWNER_TOKEN`.
  `--dry-run` lists what it would add and sends no write.
- `verctl release --new` (`VERCTL_NEW`) lets a release create each crate crates.io lacks with `VERCTL_REGISTRY_TOKEN`, while trusted publishing publishes every other crate, and the reusable release workflow takes a `publish-new` input that runs it.
  `verctl status --new` plans the same.
  `--bootstrap` and the `bootstrap` input are gone, replaced by `--new` and `publish-new` ([#402](https://github.com/victor-software-house/verctl/pull/402)).

  `verctl release bootstrap [CRATE]...` is now `release --new` followed by `release trust` for each crate that run created.
  It takes every option `release` takes, and needs `VERCTL_REGISTRY_TOKEN` and `VERCTL_OWNER_TOKEN`.
  It no longer has its own upload, tag or rate-limit wait: the release's proofs, draft Release and stop-and-rerun behaviour apply.
- `verctl check registry` fails a pull request or a merge group while the default branch holds a version whose release is incomplete, naming each `<name>@<version>`, where it passed before.
  It needs `GITHUB_TOKEN` on those events, and on any other event the pending release is a notice.
  Before it decides, it waits for a run of the `release_workflow` file at the default branch's tip, up to the new `--release-wait <SECONDS>` (`VERCTL_RELEASE_WAIT`, 1800 when omitted), and fails naming the run and the limit when the run outlasts it.
  `--crates-io-index` (`VERCTL_CRATES_IO_INDEX`) now sets the sparse index of that search for `check` as it does for `release` and `status` ([#398](https://github.com/victor-software-house/verctl/pull/398)).

  A pull request labelled `verctl: unfreeze` passes the freeze, and no other proof of `registry`.
  A merge group reads the label of the pull request its head ref names.
  `release` and `release --dry-run` run `registry` without the freeze.

  `verctl status` names the enforcement tier of the default branch's rules, `merge queue`, `rulesets without a queue` or `neither`, and shows it as unknown, with the reason, without a token or when GitHub refuses the read.
  Its JSON report gains `tier`.

### Changed

- The release freeze waits for a release run in any status but `completed`, so a status GitHub adds later no longer lets a pull request past a running release ([#400](https://github.com/victor-software-house/verctl/pull/400)).

## verctl 0.8.6-rc.3

### Breaking

- `verctl check trust` reads the owner's token from `VERCTL_OWNER_TOKEN` in place of `VERCTL_REGISTRY_TOKEN`.
  The publish token and the owner's token then never share a variable: set the new one where `check trust` runs.
  No command verctl starts inherits it ([#391](https://github.com/victor-software-house/verctl/pull/391)).

  `verctl release` runs the registry checks before its first upload: once something is left to release, and before the draft, it runs each check the new `release_checks` key names, `[registry, trust]` when omitted.
  `trust` runs only when `VERCTL_OWNER_TOKEN` is set, and the report lists it as skipped otherwise.

  A failed check stops the run naming each failure, with nothing published, and `release_checks: []` turns the proof off.
  `release --dry-run` prints the plan and runs the same checks, and fails when one does.

  `--crates-io-api` (`VERCTL_CRATES_IO_API`) points `check` and `release` at another crates.io API, such as its staging instance.
  `--crates-io-index` (`VERCTL_CRATES_IO_INDEX`) does the same for the sparse index `release` and `status` read.

### Changed

- `verctl check registry` proves, with no token, that each crates.io crate the default branch holds exists and lists every login the new `owners` key names.
  It also reports whether the crate's newest version came through trusted publishing from the repository.
  A crate only the pull request adds passes with a notice that it needs `verctl release bootstrap` once it lands ([#385](https://github.com/victor-software-house/verctl/pull/385)).

  `verctl check trust` reads an owner's token from `VERCTL_REGISTRY_TOKEN`.
  It proves that a trusted publisher entry names the repository and the workflow file the new `release_workflow` key names, `release.yml` when omitted.
  `owners` sits at the top level of `.ctl/ver.yaml`, and a package's own `owners` replaces it for that package.

## verctl 0.8.6-rc.2

### Breaking

- The reusable release workflow loses its `docs` job and its `docs` input, and the `verctl docs` verb is removed with no hidden one left behind.
  verctl waits for no docs.rs build: docs.rs builds every crates.io release on its own, and a repository checks its docs build in pull request CI.
  A caller that passes `docs:` to the workflow, or runs `verctl docs`, drops it ([#382](https://github.com/victor-software-house/verctl/pull/382)).
- `verctl check [CHECK]...` runs the checks it names, `fragments`, `served` and `versions`, each once, and the report lists each check that ran.
  Bare `check` runs `fragments` and `served`, and `--all` (`-a`) runs every check.
  `check --versions` is removed with no hidden flag: run `check versions`.
  The hidden verbs `prepare`, `pin`, `publish` and `assets`, and the hidden `release --plan`, are removed too, and each now fails as an unknown command or flag ([#379](https://github.com/victor-software-house/verctl/pull/379)).
- `verctl release` and `verctl status` take HEAD as the release reference.
  A run releases, from HEAD, every version HEAD's manifests hold whose release is incomplete, whichever commit set it.
  A version HEAD no longer holds stays unreleased, `status --github-output` writes HEAD as the only commit, and a release no longer refuses a skipped version.
  The `waiting` list leaves the `release` report, and `declared` in the `--github-output` file is the comma-separated asset names HEAD's Release holds ([#370](https://github.com/victor-software-house/verctl/pull/370)).
- `verctl release` creates the tag last: it creates a draft Release at HEAD with no tag, uploads the declared tarballs to it from `--assets-dir`, publishes to the registries, and then publishes the draft, which creates the tag.
  A failed upload leaves no tag, and a rerun resumes, skipping every version a registry lists.
  `release asset` builds its tarball and uploads nothing, and loses `--tag`.
  The reusable workflow builds the tarballs before the release job and drops the `finalize` job.
  A hand-written workflow passes the tarballs to `actions/publish` through its new `assets-dir` input, and `actions/asset` loses its `token` and `tag` inputs.
  Each row of the plan's `assets` matrix names its tarball as `asset`, so a job hands the file on without reading verctl's output ([#377](https://github.com/victor-software-house/verctl/pull/377)).

## verctl 0.8.6-rc.1

- `verctl semver` runs `cargo semver-checks` once more past the publish-age cooldown when every crate the cooldown rejected has a crates.io owner that `CARGO_TRUSTED_OWNERS` lists.
  A check blocked by any other crate fails as before, and its error names the rejected crates ([#367](https://github.com/victor-software-house/verctl/pull/367)).

## verctl 0.8.5

### Changed

- The built-in `darwin-arm64` target builds on `xcode-27`, GitHub's macOS 27 image, where it used `macos-latest`.
  GitHub serves that image as a public preview, with weekly updates and no Actions SLA, until a GA `macos-27` label exists.
  The binary's deployment target stays Rust's default for `aarch64-apple-darwin`, so it still runs on older macOS releases ([#363](https://github.com/victor-software-house/verctl/pull/363)).

## verctl 0.8.4

### Breaking

- A Release with a declared asset target stays a draft until every asset is uploaded, so a repository can turn on GitHub immutable Releases.  
  `verctl release` creates the tag, then the draft, and a later `verctl release` at that commit publishes it once every asset is uploaded.  
  It reports a draft that still lacks assets as a warning, `draft <url> lacks <names>`, and exits zero.  
  A draft is incomplete.  
  A published Release that lacks a declared asset stays pending for that target only, unless GitHub holds it immutable: it then counts complete, and `status` and `release` warn that a later version must ship the asset.  
  `--skip-assets` also publishes an existing draft without its assets ([#359](https://github.com/victor-software-house/verctl/pull/359)).

  The reusable `release.yml` gains a `finalize` job, which runs `release --releases-only` after the assets.  
  Its `plan` job needs `contents: write` to list drafts, so a caller grants `contents: write` to the whole called workflow.  
  The plan hands each leg the assets it expects at that commit as `VERCTL_DECLARED_ASSETS`, which `release` reads as `--declared-assets`.

  A hand-written workflow that runs `actions/publish` and then `actions/asset` must run `verctl release` again at the version commit after its uploads, which publishes the draft.  
  Without that run the Release stays a draft.  
  Draft publishing covers asset targets.  
  A vendored set's Release is still created published, so immutable Releases are not yet safe for a repository with vendored sets.

### Changed

- `verctl release asset` takes `--prebuilt <path>` (`VERCTL_PREBUILT`), which packs a binary built earlier as the tarball's one entry and runs neither `prepare` nor `build`.
  A target that packs `files` under `dir` refuses it, and it does not combine with `--vendored`.
  In verctl's own release, the asset job reuses the release build when the version commit is the run's commit, and an older version commit still compiles from its own tree ([#361](https://github.com/victor-software-house/verctl/pull/361)).

  The default GitHub-hosted Ubuntu machines move to `ubuntu-26.04` and `ubuntu-26.04-arm`: the `runner` input of `release.yml`, the built-in `linux-x64` target, and the `ci` default.
  A Linux tarball built on the defaults needs the newer glibc of Ubuntu 26.04.

### Fixed

- The publish, asset, and docs jobs of the reusable `release.yml` install the verctl the plan job checked from HEAD's mise lock with `mise install --locked`, and no longer set `MISE_LOCKED` or `MISE_LOCKFILE`.
  A caller whose `mise.toml` sets `locked = true` in `[settings]` and `[tool_config]` releases again, where the install failed with `locked mode requires lockfile to be enabled` ([#358](https://github.com/victor-software-house/verctl/pull/358)).

## verctl 0.8.3

### Fixed

- A Version PR opens when its changelog is longer than GitHub's 65,536-character body limit.
  The body keeps every whole line that fits, drops a heading left at the end, and ends with a line that points at the changelog files in the PR's diff ([#351](https://github.com/victor-software-house/verctl/pull/351)).

## verctl 0.8.2

### Fixed

- A problem in an asset target names the path `.ctl/ver.yaml` holds, such as `assets.linux-x64.files`.  
  It lands on the line of that key instead of the `assets:` line ([#348](https://github.com/victor-software-house/verctl/pull/348)).
- A new release builds its asset tarballs again.
  0.8.1 read the targets of the version commit's config from an `assets.targets` key, but targets sit directly under `assets`, so a release that held no asset yet was checked for none and released without tarballs.
  The next release run on the default branch builds the tarballs a release still lacks, 0.8.1's among them ([#349](https://github.com/victor-software-house/verctl/pull/349)).

## verctl 0.8.1

### Breaking

- The library API changes, and the `verctl` command does not.
  `config::PackageSpec::publish_argv` becomes `PackageSpec::publish`, which holds publisher options or the whole upload command.
  `fragment::PackageBump` gains `breaking`, and `semver_check::Decision` gains `BreakingPatch`.
  `pending::pending`, `publish::run`, and `publish::run_pending` take the path of the loaded config after the config ([#328](https://github.com/victor-software-house/verctl/pull/328)).

### Added

- A fragment may bump a package by `patch!`: a patch that breaks the package's API on purpose.
  While the fragment is pending, `verctl semver` skips the package, and the changelog lists the entry under `### Breaking`.
  The Version PR consumes the fragment, so the next release is checked again ([#328](https://github.com/victor-software-house/verctl/pull/328)).
- A package's `publish` key now also takes a mapping of options for its publisher.
  `publish: {verify: false}` makes cargo upload with `--no-verify` and keeps the crates.io check and the member-by-member workspace upload.
  An option the publisher does not have fails the load.
  A `Cargo.toml` with `publish = false` or `[]`, and a `package.json` with `"private": true`, now release on GitHub only with no `publisher: none` ([#321](https://github.com/victor-software-house/verctl/pull/321)).
- A package that sets `prerelease: <tag>` in `.ctl/ver.yaml` releases candidates, such as `0.8.2-rc.1`, then `0.8.2-rc.2`.  
  A candidate is published and released as a prerelease: the stock bun upload adds `--tag <tag>`, and its GitHub Release never becomes the latest.  
  Pins move to a candidate, because `{version}` and a tool pin's `?ref=v` match a version verctl writes, candidates included.  
  Removing the key makes the next Version PR write the stable version, and its CHANGELOG section lists every change the candidates carried ([#343](https://github.com/victor-software-house/verctl/pull/343)).

### Changed

- release.yml's asset jobs cache compiled objects with mbx between releases.  
  A tarball build then compiles only what changed since the last release on that runner.  
  A repository with no `Cargo.toml` skips the step ([#320](https://github.com/victor-software-house/verctl/pull/320)).

### Fixed

- Adding an asset target leaves earlier releases complete.
  verctl checks a release only for the targets `.ctl/ver.yaml` declared at its own version commit.
  A release made before a target existed therefore stays complete, and the Version PR opens ([#330](https://github.com/victor-software-house/verctl/pull/330)).
- The publish, asset, and docs jobs of the reusable `release.yml` run the verctl the plan job checked at HEAD.
  A pending version commit that pins an older verctl, such as 0.7.4 with no `release` verb, still releases ([#332](https://github.com/victor-software-house/verctl/pull/332)).

## verctl 0.8.0

### Added

- An `assets` target may list its own `files`, which replace `assets.files` for that target alone.  
  A release whose platforms ship different files, such as a tool built only on Linux, packs each list from the shared `assets.dir`.  
  A target without `files` packs the shared list, as before.  
  The load names the target when its `files` comes without `assets.dir`, is empty, or sits beside `assets.binary` ([#313](https://github.com/victor-software-house/verctl/pull/313)).
- A package whose `path` is `go.mod` needs no driver.  
  Its version is a `// vX.Y.Z` comment at the end of the `module` line.  
  A module with no such comment starts at `0.0.0` ([#259](https://github.com/victor-software-house/verctl/pull/259)).

  `version` refuses a bump to v2 or later until the module path ends in `/v<major>`, because Go reads such a module only there.  
  A dry run refuses too.

  With no publisher, a Go module releases on GitHub only, as `publisher: none` does, since Go fetches a module from its tag.
- Each release carries a `linux_arm64` tarball for `aarch64-unknown-linux-gnu`.  
  GitHub's arm64 Linux runner builds it natively ([#273](https://github.com/victor-software-house/verctl/pull/273)).
- `marker: true` on a package makes any file a package, such as one reusable workflow.  
  Its version is the file's one marker line: `# version: 1.2.0`, `// version: 1.2.0`, or `<!-- version: 1.2.0 -->`.  
  `version` rewrites only that version.  
  A file with no marker line, or two, fails and names the line numbers.  
  A marker package takes no driver, and with no publisher it releases on GitHub only ([#261](https://github.com/victor-software-house/verctl/pull/261)).
- A package whose `path` is `pyproject.toml` needs no driver.  
  `version` rewrites `project.version` and keeps every other byte.  
  When the repository holds a `uv.lock`, `version` names `uv lock` as the follow-up.  
  A `pyproject.toml` that lists `version` in `project.dynamic` fails with the fixes, because its build backend sets the version.  
  verctl has no stock Python publisher, so such a package names its `publisher`, or `none` to release on GitHub only ([#257](https://github.com/victor-software-house/verctl/pull/257)).

  A `pyproject.toml` package fails when `.ctl/ver.yaml` loads unless it names a `publisher`, a `publish` command, or a `publishers` entry of its own.  
  That holds for a package with its own driver too.
- A package whose `path` is `VERSION` needs no driver: the file holds one version and a newline, and anything else in it fails.  
  A missing `VERSION` file reads as `0.0.0`, so a new package needs no file before its first release.  
  `check` leaves the file missing, and `version` writes it ([#260](https://github.com/victor-software-house/verctl/pull/260)).

  A `VERSION` path whose directory is missing fails the plan before `version` writes anything.  
  With no publisher, a `VERSION` package releases on GitHub only, as `publisher: none` does.

### Changed

- Upgrading from 0.7 takes these steps, each described in its own entry below: ([#314](https://github.com/victor-software-house/verctl/pull/314)).

  1. Finish every pending release, then move the verctl pin and the `release.yml` ref to 0.8.0 together.
  2. Call the renamed verbs: `prepare` is `version`, `pin` is `render`, `publish` is `release`, and `assets` is `release asset <target>`, with no `--build` or `--upload`.
  3. Move the Version PR title and commit message from `VERCTL_PR_TITLE`, `VERCTL_COMMIT_MESSAGE`, and the `title` and `commit-message` inputs to `proposal.title` and `proposal.commit_message`, and remove those inputs from the caller.
  4. Delete `prepare.version_label`, and match a rule that read the `verctl:version` label on the head branch `verctl/release/<base>`.
  5. Replace `secrets: inherit` with a `secrets:` map that passes `registry-token`.
  6. Close each `patterns.match` with an open side of `{version}`.
  7. Read `after` where the `version` report held `next`.
  8. Name a publisher for each `pyproject.toml` package.
  9. Fix any misspelled key under `packages`, `pins`, or `patterns`, which now fails the load.
- A publisher or query argument with a `{placeholder}` that has no value fails and names the placeholder.  
  Before, verctl left that argument off the command line.  
  A typo such as `{registr}` fails this way, and so does `{registry}` on a package with no registry.  
  Braces that are not a placeholder name, such as a JSON argument, pass through ([#274](https://github.com/victor-software-house/verctl/pull/274)).
- `verctl prepare` is now `verctl version`, with the same flags, variables, and behaviour.  
  `verctl pin` is now `verctl render`, which also renders the served-file templates.  
  `render` takes `--config` and `--dry-run` only: `pin` shared `--plan` and `--skip-assets` with `publish` and ignored both.  
  Its human rows are `file` and `files` where `pin` printed `pin` and `pins`.  
  The old verbs are hidden and exit 2 with an error that names the new one, so `mise run ver prepare --pr` becomes `mise run ver version --pr`.  
  The reusable `release.yml` and `actions/version-pr` call the new verbs ([#275](https://github.com/victor-software-house/verctl/pull/275)).

  The JSON and the human view of `version` list follow-up commands under `after`, a key that was `next`.

  In the library, `verctl::cli::Command::Prepare` is `Command::Version`, `PrepareArgs` is `VersionArgs`, and `Command::Pin` is `Command::Render`.  
  The `prepare` and `pins` keys in `.ctl/ver.yaml` keep their names.
- verctl builds on ctl-core 0.7.0, which renders through its own terminal engine.  
  It reads `.ctl/ver.yaml` and fragment frontmatter through vsh-serde-saphyr 0.0.3 and yamled 0.0.12.  
  The help of `--color` and its values, `--no-color`, and the `pretty` format takes the colour wording of ctl-core ([#267](https://github.com/victor-software-house/verctl/pull/267)).
- `verctl instructions` and the bundled skill are split into headed sections, each sized to read on its own.  
  Both open with the release workflow in four steps and a table that maps each `.ctl/ver.yaml` key to its section ([#314](https://github.com/victor-software-house/verctl/pull/314)).

  The instructions, the skill, and the README now say that the release workflow's `plan`, `publish`, `docs`, and `propose` jobs run on its `runner` input.  
  Before, they named jobs the workflow does not have.
- Loading `.ctl/ver.yaml` now refuses a pattern whose `match` leaves either side of `{version}` open, naming the pattern and the side.  
  Before, `version: {version}` against `version: 0.1.1-rc1` rewrote the numeric head and kept `-rc1` ([#281](https://github.com/victor-software-house/verctl/pull/281)).

  A side's literal is the text between `{version}` and the pattern's edge, on that side.  
  A side is bounded when its literal holds at least one character that is not a digit, `.`, `-`, or `+`.  
  Under `whole_line: true`, the line start or end bounds the side, unless the literal on that side is made only of those four characters.

  The character next to `{version}` must not be a digit, or a `.` with a digit beyond it.  
  `name-{version}.tar.gz` loads; `verctl@{version}.`, `v{version}-`, and `v{version}.5x` do not.

  To fix a refused pattern, add the text that follows the version to `match`, such as a closing quote.  
  For a line the pattern owns, set `whole_line: true`.

  A `tool` pin now fails, and writes nothing, on an include of its own repository whose ref starts with a version and goes on.  
  Before, `?ref=v1.2.3-rc1` became `?ref=v<new>-rc1`.  
  A ref that is not a version, such as a branch, stays put as before.

  A `tool` pin rewrites only the version of its `[tools]` entry.  
  An inline table such as `{ version = "1.2.3", os = ["linux"] }`, a `[tools.<name>]` table, and the comments and spacing around the value keep every other byte.  
  Before, the whole entry became a bare version string, which dropped its options and comments, so `check` then failed the file on every run.
- `verctl check --versions` fails a pull request that is not the Version PR in two more cases ([#288](https://github.com/victor-software-house/verctl/pull/288)).

  Each case is a change the pull request makes:

  - It adds a CHANGELOG section for a version.  
    A heading names its version last, as in `## demo 1.2.0` or `## 1.2.0`.  
    A version the CHANGELOG held at the merge-base is not added.  
    Nor is one the release branch held at its merge-base with HEAD.  
    Fixing the name in a heading therefore passes, and so does merging the release branch.
  - It changes whether a package publishes and adds no fragment that bumps that package above `none`.  
    Whether a package publishes comes from its `publisher`, the members a workspace package uploads, a Cargo `publish` key (inherited from the workspace root with `publish.workspace`), and `private` in `package.json`.  
    A setting the release branch already holds at its merge-base with HEAD passes, as a released version does.

  For a pull request with either problem, `check` looks up on GitHub whether it is the Version PR, as for one that changes a version.  
  A merge queue run skips both rules.

  `members::members` reads the workspace with `cargo metadata --offline`, so it never reaches a registry.  
  In the library, `pull_request::problems` is new.  
  It returns `pull_request::Findings`: the problems, and a notice for each rule skipped because the config at the merge-base does not load.
- The reusable `release.yml` reads the registry token only from the `registry-token` secret.  
  It ignores the secret that `registry-token-env` names, so a caller never passes `secrets: inherit`.  
  `registry-token-env` names only the variable the publisher reads the token from ([#272](https://github.com/victor-software-house/verctl/pull/272)).

  A caller that kept its token in an environment called the workflow with `environment: release` and `secrets: inherit`.  
  It replaces `secrets: inherit` with its `secrets:` map plus `registry-token: ${{ secrets.CARGO_REGISTRY_TOKEN }}`.  
  The publish job runs in that environment and receives the environment's value.  
  The App secrets in the same map reach the Version PR job as well.
- `verctl publish` is now `verctl release`, with the same flags and variables, except `--plan`, which moved to `status --github-output`.  
  Its JSON report drops `pending`, which `status` holds as `unreleased`, and gains `allowed`.  
  `verctl assets` is now `verctl release asset <target>`.  
  It reads the target from `VERCTL_TARGET`, builds the tarball, and uploads it.  
  `--dry-run` builds the tarball and uploads nothing, written before or after `asset`, and `--vendored <name>` builds and uploads a vendored set ([#279](https://github.com/victor-software-house/verctl/pull/279)).

  The old verbs are hidden and exit 2 with an error that names the new one.  
  `--build`, `--upload`, and their variables are gone.

  `verctl status` now holds the release plan.  
  Its report shows the asset plan and lists each version commit with an incomplete release under `unreleased`.  
  It reads `.ctl/ver.yaml`, so it fails with no config or an invalid one, and it takes `--config` and `--skip-assets`.  
  With a token it reads GitHub and runs each package's registry `query`.

  Without `GITHUB_TOKEN` or `GH_TOKEN` it does not ask GitHub: JSON holds `"unreleased": null` and `unreleased_unchecked` holds the reason.

  Some lookups cannot run: in a shallow clone, with no GitHub `origin`, or with no answer from GitHub or a registry.  
  No answer means a failed connection, a timeout, a 5xx, or a spent rate limit.  
  `status` reports these the same way.  
  Any other error, such as a release tag on the wrong commit or a 401 from a bad token, fails `status`.

  `status --github-output <file>` fetches the history the plan needs.  
  It writes `has_assets`, `tag`, `matrix`, `commits`, `pending`, and `assets`, the outputs `assets --github-output` and `publish --plan` wrote.  

  `status --vendored <name>` plans a vendored set and nothing else.  
  With `--github-output` it writes only `has_assets`, `tag`, and `matrix`, and it fetches no history, reads no token, and looks up no release.  
  A config with no packages, such as one that only vendors, has no package assets to plan, so `status` works there.

  `verctl release --plan`, or `VERCTL_PLAN` set for it, exits 2 and names `verctl status --github-output`.  
  Without `--github-output`, `status` never fetches.

  The reusable `release.yml`, `actions/publish`, and `actions/asset` call the new verbs; their job ids, inputs, and the `actions/publish` path keep their names.  
  In the library, `Command::Publish` and `Command::Assets` are `Command::Release`, with `ReleaseArgs`, `ReleaseStep::Asset`, and `AssetArgs`, and `pin`'s arguments are `RenderArgs`.
- `verctl release` now refuses a run whose `.github/workflows/` tree differs from the default branch's tip, before any upload, tag, or Release.  
  Actions takes the workflows from `GITHUB_SHA`.  
  The tree compared is the one at that commit when the variable names a commit in the repository, and the one at HEAD otherwise ([#277](https://github.com/victor-software-house/verctl/pull/277)).

  In the reusable workflow HEAD is the version commit, so a workflow change merged after a Version PR does not block its release.

  A run from an old commit executes the workflows as they were then, and could publish to the registry and then fail to tag.  
  `--allow-workflow-change` (`VERCTL_ALLOW_WORKFLOW_CHANGE`) lets the run go on.

  `--allow-off-default-branch` (`VERCTL_ALLOW_OFF_DEFAULT_BRANCH`) lets `verctl release` run from a commit the default branch does not contain.  
  A check that cannot run still refuses.

  Before, `release` skipped the default-branch proof when it could not run.  
  The error names the cause: no git repository, no HEAD commit, or no `origin` remote.  
  A run that uses either flag lists what it went past under `allowed`, as a row in the human view and as `allowed: [{check, reason}]` in JSON.

  `verctl release` now refuses a package with a skipped version, because publishing past it would skip that release for good.  
  No flag allows it.

  A version v is skipped when three things hold in the first-parent range from HEAD:

  - some commit in the range has v;
  - v is below HEAD's version;
  - no newer commit in the range has a version strictly below v.

  The range ends before a released version, `0.0.0`, a revision with no version, or a version that is not semver, and otherwise at the root.  

  A version is released when the registry lists it for every checked upload of the package.  
  A package with an unchecked upload, or with none, has it released once its tag exists, because `release` creates that tag after the upload.

  For a workspace, only these members count at an earlier version:

  - a member on the package's version;
  - a member that crates.io lists at or below that version.

  The other members joined later or move on their own.  
  With no member left, the tag decides.

  An abandoned version, one that a later downward change superseded, is not listed.  
  `verctl status --github-output` lists each skipped version once, at its oldest commit in the range, oldest first.  
  The workflow then releases it before the newer one.

  The reusable `release.yml` takes two boolean inputs, `allow-workflow-change` and `allow-off-default-branch`, default `false`, and passes them to verctl through `env:`.

  In the library, `publish::run_pending` takes an `Allow` argument, and the report it returns lists `allowed`.
- `verctl release` reads the registry token from `VERCTL_REGISTRY_TOKEN` or `VERCTL_TRUSTED_PUBLISHING_TOKEN`, and fails when both are set.  
  It gives the token to each publisher and its registry `query` under `--registry-token-env` (`VERCTL_REGISTRY_TOKEN_ENV`, default `CARGO_REGISTRY_TOKEN`).  
  The queries that decide whether a release is complete or a version was skipped get it too.  
  No other command it starts gets either variable, and no flag takes the token ([#306](https://github.com/victor-software-house/verctl/pull/306)).

  In the library, `publish::run_pending` takes an `Option<&RegistryToken>`, and `pending::pending` takes one for those queries.

  No command verctl starts inherits `GITHUB_TOKEN` or `GH_TOKEN` either: not a publisher, not a build script it runs, not `assets.prepare` or `assets.build`, and not `prepare.after`.  
  Before, each ran with the job's token, which can write contents.  
  verctl keeps the token for its own GitHub calls, and its own `git push` takes it through a private variable.  

  A publisher that reads `GITHUB_TOKEN` gets the registry token there when `registry-token-env` is `GITHUB_TOKEN`.  
  A bunfig for GitHub Packages with `token = "$GITHUB_TOKEN"` is one.

  The reusable `release.yml` sets these in its publish step and runs `mise run ver release`.  
  Its registry-token inputs and secrets keep their names.  
  Only its `plan` job checks that the pinned verctl is 0.8.0 or later.

  A pending version commit that pins an older verctl fails on the `release` verb.  
  A merged commit's pin cannot move, so finish every pending release before moving the workflow ref to 0.8.0.

  To release such a commit later, check it out and run `verctl release` from a 0.8.0 install there.

  Every job of `release.yml` now sets up mise with `victor-software-house/actions/mise`, which downloads tools through the organization's release mirror.  
  Callers already grant `id-token: write` for the publish job, and that grant now serves the mirror too.  
  A job without it downloads from GitHub and logs a notice.

  Every job of `release.yml` has a `timeout-minutes`: 10 for `plan` and `propose`, 30 for `publish`, 45 for `assets`, and 75 for `docs`.  
  A publish that runs longer than 30 minutes is cancelled.
- The library's `Driver` enum gains a `Text` variant, which reads `go.mod`, `VERSION`, and marker-line files.  
  Every public enum that can grow is now `#[non_exhaustive]`, so adding a variant later is not an API break.  
  The exceptions are `Bump`, the command list (`cli::Command`, `cli::ReleaseStep`, and `cli::Removed`), and the report states the binary prints: `semver_check::Decision`, `publish::Outcome`, and `docsrs::DocsStatus`.  
  The command line does not change ([#262](https://github.com/victor-software-house/verctl/pull/262)).
- `verctl check` now renders every template and rewrites every `pins` entry in memory, at the versions the manifests hold, with or without pending fragments.  
  It fails on each served file that differs from its template.  
  It also fails on each pin that names another version, or whose pattern matches nothing.  
  One run names every such file ([#276](https://github.com/victor-software-house/verctl/pull/276)).

  A pull request that edits a served file by hand, or changes a template without rendering it, fails.

  `verctl render` renders every template after it rewrites the pins.  
  So it writes exactly what `check` compares, and it is the fix for a failed check.  
  `check` also reads `.ctl/ver.yaml` when no fragment is pending.

  A file that a template renders and a `pins` entry also names has two owners.  
  `verctl version`, `check`, and `render` refuse it before writing anything, naming the file, the template, and the `pins` entry.

  In the library, `templates::plan` and `templates::write` take the `pins` as an argument, so they can name that conflict.
- An unknown key under a `packages` entry, a `pins` entry, or a `patterns` entry now fails the load.  
  Before, verctl dropped it: `publsher: none` on a Cargo package published it to crates.io.  
  The error names the key and, for a package, the package.  
  The driver fields a package may carry inline, such as `format` and `keys`, count as its keys ([#315](https://github.com/victor-software-house/verctl/pull/315)).

  Fix a misspelled key before moving to 0.8.0, since a config that loaded before can fail now.
- `verctl check --versions` on a Version PR replays the version step on the merge-base and fails each change the replay does not make ([#286](https://github.com/victor-software-house/verctl/pull/286)).

  The error lists every difference.  
  Each one is of these kinds:

  - a path the version step does not write, such as a source file;
  - a changed symlink, compared by its target, and a submodule whose commit changed;
  - a manifest key that differs from the planned version and its pins, such as an added dependency;
  - a CHANGELOG without the new version's section, or with older text rewritten;
  - a fragment the version step consumes that is still present;
  - a served file that differs from its template's render;
  - a lockfile entry other than a workspace version the plan moves, and a lockfile that does not record each moved version.

  A changed `yarn.lock` or `bun.lockb` fails, since verctl does not read them, and so does a changed `pnpm-lock.yaml`, `poetry.lock`, or `pdm.lock`.  
  A path that `prepare.stage` names at the merge-base may change, so a staged `yarn.lock` or `bun.lockb` passes.  
  A lockfile verctl parses, such as `Cargo.lock`, stays checked even when staged.

  The plan, the pins, and `prepare.stage` come from `.ctl/ver.yaml` at the merge-base, so a Version PR cannot widen what it may change.

  The replay runs no lockfile owner and no `prepare.after` command, and makes no network call.  
  For a Cargo workspace it reads members with `cargo metadata --no-deps --offline`.

  When the config at the merge-base does not load, `check --versions` skips the rules that need it and does not fail.  
  That happens in the pull request that removes a key this verctl refuses, such as `prepare.version_label`.  
  The report then carries one notice per skipped rule, naming the rule and the load error.  
  JSON holds the notices under `notices`.

  The config on HEAD must still load.

  Earlier releases exempted the Version PR branch whatever it changed.

  In the library, `versions::report` and `versions::require` take a `content::Inputs` naming the config file and the fragment directory.  
  `content::check` and `content::removes_fragment` are new, with `git::MergeBase`, `git::read_optional`, and `members::versions`.  
  `content::check` returns the notices of the rules it skipped, and `versions::VersionReport` gains a `notices` field.
- The Version PR title and commit message move into `.ctl/ver.yaml`.  
  `proposal.title` replaces `VERCTL_PR_TITLE` and the `title` input of the reusable `release.yml` and `actions/version-pr`.  
  `proposal.commit_message` replaces `VERCTL_COMMIT_MESSAGE` and their `commit-message` input.  
  verctl ignores both variables, and both inputs are gone, so a caller that still passes `title` or `commit-message` fails until it removes them.  
  Unset or empty, the title is `chore(release): version packages` and the commit message is the title, as before ([#274](https://github.com/victor-software-house/verctl/pull/274)).

  `prepare.version_label` is removed, and the Version PR gets no `verctl:version` label.  
  A rule that matched that label matches the head branch `verctl/release/<base>` instead.  
  A config that still sets the key fails to load, and the error names the key to delete.  
  Any other unknown key under `prepare` fails too.

  The reusable `release.yml` runs the verctl the caller pins.  
  Its `plan` job fails first when that verctl is older than 0.8.0.  
  Move the verctl pin and the workflow ref together.

  In the library, `VersionPr::from_env` is `VersionPr::new`, which takes the `Proposal` and the root.  
  `release::open_or_update_pr` takes no label argument.
- `verctl version` runs the command that owns each bumped manifest's lockfile and commits that lockfile without a `prepare.stage` entry ([#282](https://github.com/victor-software-house/verctl/pull/282)).

  These commands own the lockfiles:

  - `cargo update --workspace` beside `Cargo.lock`;
  - `uv lock` beside `uv.lock`;
  - the package manager's install beside a JavaScript lockfile.

  A package's own `after` runs as well; earlier releases only listed these commands.  
  `prepare.after` runs after them, and a `prepare.after` that repeats an owner's command in the same directory runs once.  
  The report's `after` lists each command that ran.

  The next Version PR consumes a fragment that bumps nothing, such as one holding only `demo: none`.  
  Earlier releases left it pending.

  In the library, `prepare::apply_plan` returns no follow-up commands.  
  `prepare::steps` and `prepare::run_steps` list and run them, and `detect::owner` names a manifest's lockfile owner, its directory, and its lockfile.

### Fixed

- Every GitHub API call goes to `GITHUB_API_URL` when it is set, as GitHub Actions sets it, and to api.github.com otherwise.  
  Before, only `check --versions` read it, to check who opened the Version PR.  
  So a run on GitHub Enterprise Server sent its other calls to api.github.com.  
  History fetches still use `https://github.com/` ([#294](https://github.com/victor-software-house/verctl/pull/294)).
- `release`, `render`, `release asset`, and `check --versions` read a package's version the way `version` does.  
  On a failed read, the error names the package and its file.  
  `check --versions` fails on a declared package whose file is missing, where it left the package out of the report ([#262](https://github.com/victor-software-house/verctl/pull/262)).

## verctl 0.7.4

### Fixed

- A fragment's later lines stay inside its changelog entry: every line after the first is indented two spaces under the bullet, and blank lines stay blank. A second paragraph, a nested list, or a code fence used to start at column 0, outside the entry, in CHANGELOG.md and in the GitHub Release body. The instructions and skill also describe the 0.7.3 rule for a release tag on a merged ancestor ([#263](https://github.com/victor-software-house/verctl/pull/263)).

## verctl 0.7.3

### Added

- `assets.dir` and `assets.files` in `.ctl/ver.yaml` pack several files into each package asset tarball, such as a daemon beside its client, a service unit, and a licence. `verctl assets --build` checks that each listed path exists, then packs `tar czf <asset> -C <dir> <files>` with the same placeholders as `binary`. Without them a tarball holds the one binary, as before ([#239](https://github.com/victor-software-house/verctl/pull/239)).
- The reusable release workflow takes an `environment` input for its `publish` job, so a registry token can live only in a GitHub environment that admits the default branch. In an environment, with no `registry-token` secret, `publish` reads the secret named by `registry-token-env`, such as `CARGO_REGISTRY_TOKEN`, which the caller passes with `secrets: inherit`; trusted publishing skips that read. Without `environment` nothing changes ([#241](https://github.com/victor-software-house/verctl/pull/241)).

### Fixed

- `verctl publish --plan` accepts a release tag on a commit that HEAD holds only through a merge, when the package's version there equals its version at HEAD. A forkctl append publish leaves the tagged commit only on a merge's second parent, and the plan failed there with "not a first-parent commit". A tag on a commit HEAD does not contain, or at another version, still fails ([#249](https://github.com/victor-software-house/verctl/pull/249)).

## verctl 0.7.2

### Added

- `vendored.<name>` in `.ctl/ver.yaml` declares release assets built from someone else's source at its own version, and `verctl assets --vendored <name>` (`VERCTL_VENDORED`) plans, builds, and uploads them. `--upload` creates the set's Release when it is missing, never marked latest, and attaches each archive with a `.sha256` written from the digest GitHub reports. A config that vendors may declare no packages. `assets` without `--vendored` is unchanged ([#232](https://github.com/victor-software-house/verctl/pull/232)).

### Changed

- The reusable `release.yml` installs mise 2026.10.25 when a caller passes no `mise-version`, the version verctl's own `mise.toml` requires ([#233](https://github.com/victor-software-house/verctl/pull/233)).
- verctl no longer arms auto-merge on the Version PR: a release starts only when a person merges it. `prepare` drops `--auto-merge` and `VERCTL_AUTO_MERGE`, and the reusable `release.yml` and `actions/version-pr` drop their `auto-merge` input, so a caller that still passes `auto-merge` must remove it when it moves to this release ([#230](https://github.com/victor-software-house/verctl/pull/230)).

## verctl 0.7.1

- The release workflow's asset jobs install tools one at a time. On macOS runners rustup failed to rename a component download when two Rust toolchains installed at once, which left a release without its darwin-arm64 asset ([#223](https://github.com/victor-software-house/verctl/pull/223)).

## verctl 0.7.0

- `check --versions` skips in a merge queue run (a `merge_group` event). Each entry's pull request already compared its own versions, and the queue run of the Version PR, or of any entry behind it, carries the new versions, so without the skip the check would fail every release that lands through a queue.

The library's public `versions::Skip` enum gains a `MergeQueue` variant, so a crate that matches on it exhaustively needs a new arm ([#220](https://github.com/victor-software-house/verctl/pull/220)).

## verctl 0.6.0

- The reusable release workflow publishes to crates.io through trusted publishing with `crates-io-trusted-publishing: true`, so a caller keeps no registry token. Its publish job now requests `id-token: write`, which every caller grants in the same change that moves its pin, because GitHub checks a called job's permissions when the run starts ([#218](https://github.com/victor-software-house/verctl/pull/218)).

## verctl 0.5.5

- The release binary no longer carries octocrab's unused JWT crypto, which RUSTSEC-2026-0387 flagged in `crypto-bigint`. verctl authenticates with a personal token only ([#212](https://github.com/victor-software-house/verctl/pull/212)).

## verctl 0.5.4

- `verctl publish --plan` and `verctl publish` leave out a package at `0.0.0`, before any registry or GitHub lookup, `publish` asks no changelog section of it, and it gets no tag, so a new package beside a released one no longer breaks a shared `v{version}` tag. A crate that adopted the release workflow at `0.0.0` had its placeholder planned as a release, so every push to its default branch ran the publish job on the commit where `0.0.0` first appeared, and the Version PR job never ran ([#204](https://github.com/victor-software-house/verctl/pull/204)).

## verctl 0.5.3

- `verctl semver` leaves out a crate with no library target, or a proc-macro, which cargo-semver-checks does not check. It ran `cargo semver-checks --package <crate>` on a binary-only crate, which cargo-semver-checks refuses, so a workspace package with a binary member failed ([#197](https://github.com/victor-software-house/verctl/pull/197)).

## verctl 0.5.2

- The shared verify runs the version guard from ctl-core's Rust tasks at 0.6.27, through this checkout's `ver` task, in place of a local copy of the same check ([#196](https://github.com/victor-software-house/verctl/pull/196)).

## verctl 0.5.1

- `verctl check --versions` works in the depth-1 pull request checkout of `actions/checkout`. On Actions it first fetches `VERCTL_DEFAULT_BRANCH`, the event's default branch, and `GITHUB_BASE_REF` with their whole history when the checkout lacks them or is shallow, HEAD's parents included, so the merge-base can be computed. A full clone that has them fetches nothing ([#194](https://github.com/victor-software-house/verctl/pull/194)).

## verctl 0.5.0

- `verctl semver` checks the API of each crates.io crate whose next release is a patch with `cargo semver-checks --release-type minor`, so only a break fails. It skips a crate whose package has a pending minor fragment or whose minor the Version PR already moved, and fails on a crate at 1.0 or later ([#183](https://github.com/victor-software-house/verctl/pull/183)).
- `check --versions` knows a Version PR by its head repository, its head branch `verctl/release/<base>`, and its current base, read through the API, and no longer reads the `verctl:version` label. The Version PR's first CI run passes without a `labeled` trigger; when a version differs, the CI job needs `GITHUB_TOKEN` with `pull-requests: read`, and a pull request that changes no version reads no API. A version the default branch carries at its merge-base with HEAD is not a hand edit, so merging the default branch into another branch passes. `Skip::from_env` takes no config and returns a `Result`, `report_with` and `require_with` take the default-branch candidates, `VersionRow` gains `released`, and the payload label readers are removed ([#188](https://github.com/victor-software-house/verctl/pull/188)).

## verctl 0.4.12

- The reusable `release.yml` takes `mise-ignored-config-paths`: colon-separated mise config paths, relative to the repository root, that every job but `propose` ignores, so the release jobs skip tools only the repository's own tasks need ([#184](https://github.com/victor-software-house/verctl/pull/184)).

## verctl 0.4.11

- `publish` creates the tag and GitHub Release of a package whose uploads all have a registry check, or that has no upload, before its uploads, so a retry after a failed upload creates no tag that GitHub would refuse for a later workflow change. `docs` waits only for crates the crates.io index lists ([#182](https://github.com/victor-software-house/verctl/pull/182)).

## verctl 0.4.10

- `verctl publish` now works in a shallow clone. With `VERCTL_DEFAULT_BRANCH` set, it fetches the whole history of that branch before looking for the commit that set each version, so a depth-1 checkout no longer fails with "the clone is shallow". Without the variable, the error now names it. When GitHub refuses to create a Release with 403, the error now says why: the release commit's `.github/workflows` differs from the default branch, so the Releases API also needs `workflows: write`, which `GITHUB_TOKEN` cannot hold. The operator instructions now say to adopt the release workflow only when no release is pending ([#180](https://github.com/victor-software-house/verctl/pull/180)).

## verctl 0.4.9

- An asset upload that GitHub refuses because the release already has a complete asset of that name now reports `already` instead of failing. Two release runs for the same commit both build every asset, so the run that uploads second used to fail after a release that was in fact complete. The duplicate check read the error's text, which never contained GitHub's `already_exists` code; it now reads the code from the response. When the other run is still sending the asset, the upload waits up to a minute for it to finish. An asset whose earlier upload broke off still fails, with a message to delete it and rerun ([#177](https://github.com/victor-software-house/verctl/pull/177)).
- The Version PR action loads again. A colon followed by a space in its `title` input description made the file invalid YAML, so GitHub failed every workflow that used the action at "Set up job", from v0.4.6 on ([#178](https://github.com/victor-software-house/verctl/pull/178)).

## verctl 0.4.8

- A cargo package at a virtual workspace manifest publishes each workspace member that publishes, dependencies first, with `cargo publish --package <member>` under its own crates.io index check. A rerun after a partial release uploads only the members still missing, and a registry rate limit stops the run before any tag, naming the crate and the time to retry. Before this, such a repository overrode the upload with `cargo publish --workspace`, which cannot resume, and a rerun could tag a release whose later crates were never uploaded ([#174](https://github.com/victor-software-house/verctl/pull/174)).

## verctl 0.4.7

- `verctl docs` keeps waiting while docs.rs builds a crate. docs.rs reports `doc_status: false` both while a build runs and after it failed, and `docs` used to fail the release on the first such answer. A crate now fails only when the timeout runs out while docs.rs still reports no documentation ([#171](https://github.com/victor-software-house/verctl/pull/171)).

## verctl 0.4.6

- The docs.rs page builds again and has a real landing page: a quick start, how a release flows through the modules, and the crate's features. docs.rs renders with `--cap-lints warn`, so a lint that only its newer nightly fires no longer fails the build. Every fallible public function documents when it returns an error ([#160](https://github.com/victor-software-house/verctl/pull/160)).
- verctl reads `.ctl/ver.yaml` and fragment frontmatter with ctl-core 0.6.23, which moves to serde-saphyr 2.0. An error inside an aliased YAML value now keeps the parser's own wording and points at that value, followed by where the anchor is defined ([#167](https://github.com/victor-software-house/verctl/pull/167)).
- `prepare --pr` no longer guesses `main` for the Version PR base. It targets `GITHUB_REF_NAME`, then `VERCTL_DEFAULT_BRANCH`, then the branch `origin/HEAD` names, and fails before writing anything when none is set. An empty `VERCTL_PR_TITLE` or `VERCTL_COMMIT_MESSAGE` now counts as unset. The `version-pr` action defaults `title` and `commit-message` to empty, so verctl's own default applies; a set `title` without a `commit-message` now becomes the commit message too ([#155](https://github.com/victor-software-house/verctl/pull/155)).
- New `verctl docs` waits for docs.rs to build each crates.io library crate whose release tag names HEAD, and fails naming any crate docs.rs failed to build or had not built within `--timeout` minutes (default 60). verctl 0.4.5 shipped without a docs.rs build and nothing noticed; the release workflow's `docs` job now runs this after every publish ([#165](https://github.com/victor-software-house/verctl/pull/165)).
- The crate documentation builds on docs.rs again. The public `process::run_inherit` docs linked to a private constant, which `private_intra_doc_links = "deny"` turns into an error under rustdoc, so every docs.rs build since 0.3.14 failed ([#158](https://github.com/victor-software-house/verctl/pull/158)).
- Every long option now reads `VERCTL_<LONG>` from the environment, through ctl-core 0.6.21: `VERCTL_CONFIG`, `VERCTL_DRY_RUN`, `VERCTL_FORMAT`, and the rest. A flag on the command line wins over its variable, an empty variable is unset, and `--help` names each variable. The `version-pr`, `publish`, and `asset` actions set these variables and run the bare verb, and their `config` input defaults to empty, which means `.ctl/ver.yaml` ([#159](https://github.com/victor-software-house/verctl/pull/159)).
- `publish` asks the registry before it runs a publisher, and again after it. A stock cargo package bound for crates.io is read from the crates.io sparse index; any other package runs its recipe's new `query` command (stock bun: `bun pm view {name}@{version} version`). A listed version is reported `already` and its publisher does not run, so a rerun after a partial failure no longer depends on the registry's wording. A publisher that exits zero while the registry never lists the version now fails. Set `query` on a package or a `publishers` entry for another registry, or `query: []` to keep the old stderr match ([#162](https://github.com/victor-software-house/verctl/pull/162)).
- The Version PR now comes from `verctl/release/<base>`, where `<base>` is the branch it targets, instead of `version-packages`. `check --versions` exempts that branch locally, and no longer exempts `version-packages`. `prepare --pr --auto-merge` (`VERCTL_AUTO_MERGE`) enables the squash auto-merge that `actions/version-pr` used to run from a shell script. Before anything is written, it refuses when no ruleset on the base requires a status check, or when its token equals `VERCTL_JOB_TOKEN`. The action now sets both variables and runs `verctl prepare --pr`. A workflow that publishes when the Version PR merges must accept the new head branch. Close an open `version-packages` pull request after upgrading ([#161](https://github.com/victor-software-house/verctl/pull/161)).
- verctl ships a reusable release workflow, `.github/workflows/release.yml`, called from one thin caller on push to the default branch and on dispatch. It plans every incomplete version commit, publishes each in history order, builds the assets each Release lacks, waits for docs.rs, and opens or updates the Version PR. Every job is safe to rerun, and every input is optional with verctl's default. It replaces the copied `version-pr.yml` and `publish.yml`; migrate by adding the caller, deleting both, and closing the open `version-packages` pull request ([#166](https://github.com/victor-software-house/verctl/pull/166)).
- `publish` now acts on version commits, the commit that set each package's version. `publish --plan` lists every version commit whose release is incomplete (registry, tag, Release, and declared assets unless `--skip-assets`), oldest first, and writes `commits`, `pending`, and `assets` to `$GITHUB_OUTPUT`. `publish` releases only the packages whose version commit is HEAD and fails before any side effect on any other commit, so a later commit can no longer tag a version that was never published. When every release is complete, both report `nothing to release` and exit zero, with `"outcome": "nothing-to-release"` in JSON ([#164](https://github.com/victor-software-house/verctl/pull/164)).

## verctl 0.4.5

### Changed

- verctl reads and writes git through `gix` instead of `git2`, so it builds without OpenSSL or Perl. `prepare --pr` now pushes with the `git` binary, which must be on PATH: the token reaches it through a credential helper that reads the push's environment, never argv or a URL, and the push skips repository hooks. Inside a git hook, verctl ignores `GIT_INDEX_FILE`, `GIT_WORK_TREE`, and `GIT_DIR` and never writes the index ([#146](https://github.com/victor-software-house/verctl/pull/146)).

## verctl 0.4.4

### Fixed

- The served `ver` mise task now runs `raw`. mise relayed its output and appended an SGR reset to every line whenever mise's own colour was on, as under `CLICOLOR_FORCE=1`, so `mise run ver status --format json | jq` received an ESC byte verctl never wrote ([#142](https://github.com/victor-software-house/verctl/pull/142)).

## verctl 0.4.3

### Fixed

- A parse error under `--format json` is now a JSON error even when the bad argument comes before the flag, as in `verctl prepare --bogus --format json`. It printed as text, coloured when `CLICOLOR_FORCE` or `FORCE_COLOR` was set, to a caller that asked for JSON. JSON output stays plain whatever the colour environment says ([#138](https://github.com/victor-software-house/verctl/pull/138)).

## verctl 0.4.2

### Added

- `publisher: none` releases a package on GitHub only: `publish` creates its tag and Release, and `assets` its tarballs, with no registry upload. A private binary, such as a Cargo package with `publish = false`, no longer needs a stand-in publish command. A `registry` or `publish` on the same package fails the load ([#134](https://github.com/victor-software-house/verctl/pull/134)).

## verctl 0.4.1

### Fixed

- A changelog entry whose fragment reached the default branch through a merge commit now links the pull request that added it on the merged branch, not the merge. In 0.4.0 both entries linked the pull request that merged `next` ([#130](https://github.com/victor-software-house/verctl/pull/130)).

## verctl 0.4.0

- A fragment may set `verctl:group` to `added`, `changed`, or `fixed`. Once one fragment in a release section names a group, the section lists its entries under `### Added`, `### Changed`, and `### Fixed` headings, and a fragment without the key goes under `### Changed`. A section where no fragment names a group renders as before. `Fragment` has a new public field, `group` ([#126](https://github.com/victor-software-house/verctl/pull/126)).
- `.ctl/ver.yaml` accepts `changelog.internal_authors`, the maintainers and Apps a changelog never credits. With a token, an entry credits its pull request author only when that login is not listed, the author is a person, and the pull request comes from a fork. `Config` and every config struct are now `#[non_exhaustive]`, so later keys are additive ([#126](https://github.com/victor-software-house/verctl/pull/126)).

## verctl 0.3.16

- A changelog entry whose summary ends with a period now puts its pull request or commit reference before a single closing period, `- Summary ([#12](…)).`, instead of `- Summary. ([#12](…)).` ([#120](https://github.com/victor-software-house/verctl/pull/120)).

## verctl 0.3.15

- Each changelog entry now links the pull request that merged its fragment, read from the trailing `(#N)` of the squash commit that added it, or that commit when the subject names none. No entry carries an author byline. ([#117](https://github.com/victor-software-house/verctl/pull/117)).

## verctl 0.3.14

- Raise `rust-version` to 1.99.0, the pinned stable the shared template builds, lints, and tests on. A build with an older compiler stays on the previous release.

## verctl 0.3.13

- `actions/version-pr` takes `auto-merge: true` to merge the Version PR once the base branch's required checks pass. It refuses a base branch with no required status check and the default `GITHUB_TOKEN`.

## verctl 0.3.12

- A failed driver command now names its argv and exit status, for example `driver command failed: false: exit status: 1`, so a command killed by a signal can be told apart from one that exited non-zero.

## verctl 0.3.11

- `prepare` and `check` refuse a `minor` or `major` fragment on a package at `0.0.0`, including its pre-releases and builds, so a new package releases `0.0.1` first. Setting `first_minor: true` on a package in `.ctl/ver.yaml` lets the operator open it at `0.1.0`. `check` now plans the pending fragments against `.ctl/ver.yaml`, so it also fails on a package the declarations do not name.

## verctl 0.3.10

- State that a new package starts at `0.0.0` and its first fragment is a `patch`, so its first release is `0.0.1`; moving to `0.1.0` or `1.0.0` is a separate operator decision. The example fragments show `patch`, and a `minor` fragment on a `0.0.0` package is a stop condition.

## verctl 0.3.9

- Read config and changeset fragments through ctl-core 0.6.9. A parse error in a value reached through an alias names where that value is defined, counted from the file.

## verctl 0.3.8

- A pattern's `occurrences` bound with a count that is not a whole number, such as `{exactly: -1}`, names the accepted arities again instead of repeating the parser's wording.

## verctl 0.3.7

- Adopt ctl-core 0.6.7: a config or fragment parse problem no longer repeats a position counted from the frontmatter instead of the file.

## verctl 0.3.6

- Read `.ctl/ver.yaml` and fragment frontmatter through ctl-core's declared input, so a problem in either names its file and line.

## verctl 0.3.5

- Publish keeps an existing release whose packages were all published by an earlier run, even when its tag names that earlier commit.

## verctl 0.3.4

- `publish` recognises a version that is already on the registry again: commands that stream their output now keep the end of stderr for the error, which the already-published check reads.

## verctl 0.3.3

- Let the environment pick each style the binary leaves open: `CTL_CORE_RECORD_STYLE`, `CTL_CORE_LIST_STYLE`, `CTL_CORE_ROW_SEPARATION`, and `CTL_CORE_JSON_LAYOUT`. An unaccepted value is ignored with a warning that names the accepted values.

## verctl 0.3.2

- Adopt ctl-core 0.6.3: borderless records, bold package names, indented JSON, and an 80-column fallback width in piped output.

## verctl 0.3.1

- Render the operator skill and installed instructions from ctl-core Surface fragments, keeping Clap as the only command inventory and the shared mounted invocation as the only no-`--` rule.

## verctl 0.3.0

- Route every command through ctl-core's typed App and semantic Document presentation kernel, so pretty, colorless, JSON, help, errors, and exit codes share one report model.

## verctl 0.2.4

- The operator skill template wraps Jinja examples in `raw` so `prepare` can render it.
- Served `ver` task mounts the ctl-core Usage spec. `mise run ver status` needs no `--`.

## verctl 0.2.3

- `verctl publish` fetches the one missing default-branch ref with its history, not at depth 1, so the ancestry proof can walk parents. It rewrites `origin/HEAD` only from `VERCTL_DEFAULT_BRANCH`, never from `GITHUB_BASE_REF`.

## verctl 0.2.2

- `verctl publish` fetches `origin/<default>` at depth 1 only when that ref is missing. GitHub git HTTPS authenticates as `x-access-token`; the token is never in the URL and never sent as Bearer. The publish action no longer shells out to `git fetch`.

## verctl 0.2.1

- `actions/publish` fetches the default branch at depth 1 before pointing `origin/HEAD` at it, so a shallow checkout of the merge SHA still has that ref.

## verctl 0.2.0

- Declare release tag names with `tags.template` (default `v{version}`). A template with `{name}` creates one tag and one Release per package, each filled from that package's CHANGELOG. Without `{name}`, differing versions refuse instead of guessing the first package.
- Pin each GitHub Release tag to the published HEAD commit. After create, and when a Release already exists, the tag is re-read and the run fails if it names another commit.

## verctl 0.1.3

- `docs/served-files.md` §4 no longer promises that a prerelease always fails the
release, because it does not. The condition is exact: a match ending in
literal text has a right-hand boundary, so `verctl@{version} today` reads
`verctl@1.0.0-rc.1 today` as no match at all. A match ending *at* the
placeholder has none, and rewrites the numeric head, leaving the tail.

The doc now says that, and says what to do about it — put the following text
into `match`, or use `whole_line` for a line the match owns. It also records
why nothing can be inferred: `download/v{version}` against
`download/v1.2.3-linux-x64.tar.gz` is correct and pins 1.2.3, so a rule
refusing every trailing `-` would stop real releases.

## verctl 0.1.2

- The bundled skill now says which release its text describes.
`skills/verctl/SKILL.md` carries `version:` in its frontmatter, kept current
by a `patterns` pin, so the Version PR moves it onto the commit the tag names
along with every other version site.

A skill is prose, not a generated file, so it takes a pattern rather than a
template — a template would claim to own the whole document. The copy vendored
into the global skill set tracks `latest-release`, so what a consumer loads
now names the binary they actually have.

A pattern can now say `whole_line: true`, meaning the match owns its line
rather than sitting inside one. Unsaid, nothing changes: an inline spelling
like `github:org/tool@1.2.3` still matches anywhere. Saying it beats naming
the text next door — a match that spells out its neighbour breaks when the
neighbour moves — and it decides two cases the version alphabet cannot: the
same words inside a sentence are not the line, and a line ending in
`0.1.1-rc1` does not end where the version does, so it fails to match and the
declared arity stops the release.

## verctl 0.1.1

- `tasks/ver/ver` is now served from `.ctl/templates/ver.jinja` and runs
`verctl` from PATH rather than `"$(mise where …)/verctl"`.

`mise where <tool>` resolves a version from the surrounding config, not from
the task's own `#MISE tools` pin, so the pin guaranteed the install and never
the selection. Measured on one machine with 0.0.1 through 0.1.0 installed and
a task pinned to 0.0.4 while the config said 0.1.0: `exec verctl` ran 0.0.4
and `exec "$(mise where …)/verctl"` ran 0.1.0. mise already puts the task's
pinned tool first on PATH, so the plain exec is both simpler and correct.

The two halves ship together on purpose. The committed file said `0.0.1`
while the crate was at `0.1.0`, and only `mise where` was hiding that — so
removing it alone would have frozen `mise run ver` on 0.0.1. Making the file
a template is what keeps the pin current: the Version PR renders it onto the
commit the tag names, so the tag installs itself.
- A template in the source tree that git does not track now fails the run,
naming the file, instead of rendering nothing and letting the release serve
the stale hand-authored version of that file. Adding a served file and
getting silence was the one place this lane failed open, and it did it once
per repo, at the moment somebody was learning the feature.

Only tracked templates still render, for the reason they always did:
`prepare` stages what it writes, so a template git does not carry would put
a served file on the tag with no source beside it, and `check` would have
nothing to compare against on a fresh clone. What changed is the answer to
one that is present anyway — commit it or delete it.

Two things stay silent, both deliberately. A template *outside* the source
tree is not verctl's to render, which is what keeps this crate's own
changelog templates out of the released set; it was never a claim to serve
anything. And a tree with no repository has no index to not carry a file,
and cannot serve anything by tag, so it renders nothing without a word.

An ignored template is left alone rather than refused: ignoring a file is a
repo saying it is not part of what it publishes, which is the same statement
the index makes. A symlink in the source tree is not followed either, so a
directory outside the repository cannot have its templates counted as this
one's.

## verctl 0.1.0

- A repo now declares everything in `.ctl/ver.yaml`, and served-file templates
live in `.ctl/templates/`. `verctl.toml` is not read at all — no fallback, no
deprecation window. `.ctl/` is the directory every ctl CLI shares, so a repo
that uses two of them still gains one entry at its root, and templates sit in
one place because a template already declares its own target and nothing in one
is verctl's except who renders it.

YAML, not TOML, so every file a repo writes for a ctl CLI is the language
`tasks.yaml` already is. The sections are unchanged and so is every rule across
them: a pattern no pin lists still fails the load, and a job or asset target
naming an undeclared machine still fails with the declared names. The one
spelling that moved is an arity with a bound, now `{exactly: 2}` and
`{at_least: 2}`.

Every `path` in the file is relative to the directory that holds `.ctl/`, not
to the directory the file sits in, so `-c crates/foo/.ctl/ver.yaml` still
governs `crates/foo`. Complaints name sections the way a repo writes them:
`prepare.stage`, `patterns.install`, `not declared in runners`.

The retired `[assets].targets` shim is gone with it. Its own removal condition
was that every consumer had bumped past the release that replaced it, and this
is that bump.

## verctl 0.0.4

- The cargo driver's lockfile follow-up is `cargo update --workspace`, not
`cargo generate-lockfile`. A bump makes one line of `Cargo.lock` stale;
`generate-lockfile` re-resolved all 138 packages and moved a dependency the
bump never touched.

Lockfile detection now stops at the repository root for cargo and JavaScript
alike. Both branches walk one shared scope, so a `Cargo.lock` or `bun.lock`
in some unrelated parent directory can no longer claim a manifest that is not
part of it.
- `prepare` rewrites `[[pins]]` and puts them on the Version PR commit. The
tag names that commit, so the released tree carries pins that name the
release; a pin rewritten after publish can never reach the tree a consumer
fetches by tag or `?ref=`. Publish pushes nothing but the tag.

A pin that has to name an already-published tarball — the one a repo's own
release lane installs — is not a `[[pins]]` entry, and this repo's
`mise.release.toml` says so.
- A repo declares the files it serves instead of scripting them. A template in
`.verctl/templates/` says inside itself where it goes — `path`, `name`, and
`executable` are top-level Jinja exports, parsed into one schema and validated
at the boundary, so nothing sits beside the file it generates and no manifest
lists templates. Committing one adds a served file; only tracked templates
render, so scratch work renders nowhere. `prepare` writes them onto the Version
PR commit, which is the commit the tag names.

For a file that has to be hand-authored, `[[pins]]` now rewrites any format. A
version spelling is named once as `[patterns.<id>]` and listed by every file
that carries it, so two files that say the same thing say it in one place and
which file carries a spelling is written down rather than implied by where a
table sits. Each pattern declares how often its file must say it — `once`,
`many`, `never`, `{ exactly = N }`, `{ at_least = N }` — and both too few and
too many stop the release, as does a name nothing declares, a name listed
twice, and a declared pattern no file lists.

`verctl.toml` is now held to the same standard by the same validators, so the
file a repo writes fails the way a template does — naming the field, in words
that say what to change. A config that names no package, a runner with no
label, and an empty `[templates].suffix` are rejected at load rather than
carried into a release.

Two corrections that come with it: a project in a subdirectory of a repository
now serves its own templates instead of none, and templates render from every
package's version — the manifests as they read now, with the release's bumps
over them — so a served file may mention a package this release did not bump.

The contract is `docs/served-files.md`, and verctl serves its own
`examples/mise.toml` through it.

## verctl 0.0.3

- `prepare --pr` commits dirty paths matching `[prepare].stage` from the same git-status walk as the unexpected-dirty check, including deletions. `[prepare].stage_ignored` opts into gitignored matches.
- Runners are configuration. A machine is declared once as `[runners.NAME]` with
`labels`, the literal GitHub label list — every label at once, so one entry is
one machine however many labels it carries. Jobs name machines: `[ci.NAME]`
takes `runners = [...]` and gets one check per machine, `[assets.NAME]` takes a
single `runner` because one tarball is one machine. A name resolves against
`[runners]` or fails listing what is declared; only labels reach `runs-on:`, so
verctl never invents a label and a label can never pass as a machine name.
Omitting `[ci]` runs one `verify` job on `ubuntu-latest`, so a repo that says
nothing behaves as before. New `verctl ci` writes the matrix a `plan` job hands
to `runs-on:`.

`verctl instructions` now states which jobs take a runner and why: verctl
decides how many jobs there are, the workflow file decides where a fixed job
runs. `[ci]` and `[assets]` exist because only the repo knows how many verify
checks or tarballs it wants. `plan`, `crate`, `pin`, and `prepare` are always
one job each, so their `runs-on` stays a literal in the workflow you own. There
is no `[release]` runner table and none is planned.

`$GITHUB_OUTPUT` is appended to rather than truncated, so a plan step no
longer drops assignments an earlier step in the same step file made.

Breaking for anyone who wrote an asset target. `[assets] targets = [...]` is
retired for one `[assets.NAME]` table per target, and naming it is an error
that states the migration:

```toml
# before
[assets]
targets = ["linux-x64"]

# after — built-in name, so nothing else is needed
[assets.linux-x64]

# after — with a machine of your own
[runners.big]
labels = ["self-hosted", "linux", "x64"]

[assets.linux-x64]
runner = "big"
```

An unknown target name with a partial record now fails instead of building
against an empty target triple. `{runner}` is no longer expanded in `[assets]`
`prepare` / `build` / `binary` and has no replacement, since a label list has
no single rendering. A workflow copied from `examples/workflows/publish.yml`
renames `matrix.runner` to `matrix.labels` in the same step as the bump.
- `check --versions` treats the GitHub event label `verctl:version` as the Version PR. `prepare --pr` applies that label. Not `GITHUB_HEAD_REF`.

## verctl 0.0.2

- Actions run the released tarball, mise splits dev/release/ci, publish is exact-SHA plus a matching changelog heading, `check --versions` blocks hand edits, and `verctl pin` rewrites collocated github:verctl refs after the tarball exists.

## verctl 0.0.1

- First release. Version PR via git2 and octocrab, ctl-core chassis.

