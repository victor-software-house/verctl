---
verctl: patch
verctl:group: fixed
---

Adding an asset target leaves earlier releases complete.
A release owes only the targets `.ctl/ver.yaml` declared at its own version commit.
A release made before a target existed therefore stays complete, and the Version PR opens.
