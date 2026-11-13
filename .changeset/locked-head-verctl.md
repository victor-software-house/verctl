---
verctl: patch
verctl:group: fixed
---

The publish, asset, and docs jobs of the reusable `release.yml` install the verctl the plan job checked from HEAD's mise lock with `mise install --locked`, and no longer set `MISE_LOCKED` or `MISE_LOCKFILE`.
A caller whose `mise.toml` sets `locked = true` in `[settings]` and `[tool_config]` releases again, where the install failed with `locked mode requires lockfile to be enabled`.
