---
verctl: minor
verctl:group: changed
---

`verctl publish` is now `verctl release`, with the same flags, variables, and behaviour.  
`verctl assets` is now `verctl release asset <target>`.  
It reads the target from `VERCTL_TARGET`, builds the tarball, and uploads it.  
`--dry-run` builds it and uploads nothing, and `--vendored <name>` builds and uploads a vendored set.  
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
