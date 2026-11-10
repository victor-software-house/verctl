---
verctl: minor
verctl:group: changed
---

The Version PR title and commit message move into `.ctl/ver.yaml`.  
`proposal.title` replaces `VERCTL_PR_TITLE` and the `title` input of the reusable `release.yml` and `actions/version-pr`.  
`proposal.commit_message` replaces `VERCTL_COMMIT_MESSAGE` and their `commit-message` input.  
verctl ignores both variables, and both inputs are gone, so a caller that still passes `title` or `commit-message` fails until it removes them.  
Unset or empty, the title is `chore(release): version packages` and the commit message is the title, as before.

`prepare.version_label` is removed, and the Version PR gets no `verctl:version` label.  
A rule that matched that label matches the head branch `verctl/release/<base>` instead.  
A config that still sets the key fails to load, and the error names the key to delete.  
Any other unknown key under `prepare` fails too.

The reusable `release.yml` runs the verctl the caller pins.  
Its `plan` job fails first when that verctl is older than 0.8.0.  
Move the verctl pin and the workflow ref together.

In the library, `VersionPr::from_env` is `VersionPr::new`, which takes the `Proposal` and the root.  
`release::open_or_update_pr` takes no label argument.
