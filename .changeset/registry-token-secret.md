---
verctl: minor
verctl:group: changed
---

The reusable `release.yml` reads the registry token only from the `registry-token` secret.  
It ignores the secret that `registry-token-env` names, so a caller never passes `secrets: inherit`.  
`registry-token-env` names only the variable the publisher reads the token from.

A caller that kept its token in an environment called the workflow with `environment: release` and `secrets: inherit`.  
It replaces `secrets: inherit` with its `secrets:` map plus `registry-token: ${{ secrets.CARGO_REGISTRY_TOKEN }}`.  
The publish job runs in that environment and receives the environment's value.  
The App secrets in the same map reach the Version PR job as well.
