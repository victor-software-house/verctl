---
verctl: minor
verctl:group: changed
---

The Version PR title and commit message move into `.ctl/ver.yaml`.  
`proposal.title` replaces `VERCTL_PR_TITLE` and the `title` input of the reusable `release.yml` and `actions/version-pr`.  
`proposal.commit_message` replaces `VERCTL_COMMIT_MESSAGE` and their `commit-message` input.  
verctl ignores both variables, and both inputs are gone.  
Unset or empty, the title is `chore(release): version packages` and the commit message is the title, as before.

`prepare.version_label` is removed and the Version PR gets no label, since nothing read it.  
A config that still sets the key fails to load, and the error names the key to delete.  
Any other unknown key under `prepare` fails too.

The reusable `release.yml` runs the verctl the caller pins.  
So each job fails first when that verctl is older than 0.8.0.  
Move the verctl pin and the workflow ref together.
