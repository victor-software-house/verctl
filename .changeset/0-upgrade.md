---
verctl: minor
verctl:group: changed
---

Upgrading from 0.7 takes these steps, each described in its own entry below:

1. Finish every pending release, then move the verctl pin and the `release.yml` ref to 0.8.0 together.
2. Call the renamed verbs: `prepare` is `version`, `pin` is `render`, `publish` is `release`, and `assets` is `release asset <target>`, with no `--build` or `--upload`.
3. Move the Version PR title and commit message from `VERCTL_PR_TITLE`, `VERCTL_COMMIT_MESSAGE`, and the `title` and `commit-message` inputs to `proposal.title` and `proposal.commit_message`, and remove those inputs from the caller.
4. Delete `prepare.version_label`, and match a rule that read the `verctl:version` label on the head branch `verctl/release/<base>`.
5. Replace `secrets: inherit` with a `secrets:` map that passes `registry-token`.
6. Close each `patterns.match` with an open side of `{version}`.
7. Read `after` where the `version` report held `next`.
8. Name a publisher for each `pyproject.toml` package.
9. Fix any misspelled key under `packages`, `pins`, or `patterns`, which now fails the load.
