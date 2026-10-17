---
verctl: patch
---

verctl ships a reusable release workflow, `.github/workflows/release.yml`, called from one thin caller on push to the default branch and on dispatch. It plans every incomplete version commit, publishes each in history order, builds the assets each Release lacks, waits for docs.rs, and opens or updates the Version PR. Every job is safe to rerun, and every input is optional with verctl's default. It replaces the copied `version-pr.yml` and `publish.yml`; migrate by adding the caller, deleting both, and closing the open `version-packages` pull request.
