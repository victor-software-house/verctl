---
verctl: minor
---

`.ctl/ver.yaml` accepts `changelog.internal_authors`, the maintainers and Apps a changelog never credits. With a token, an entry credits its pull request author only when that login is not listed, the author is a person, and the pull request comes from a fork. `Config` and every config struct are now `#[non_exhaustive]`, so later keys are additive.
