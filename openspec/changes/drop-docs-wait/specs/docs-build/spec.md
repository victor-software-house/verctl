## REMOVED Requirements

### Requirement: verctl checks the docs.rs build of a released crate

**Reason**: verctl waits for no docs.rs build; the pull request's `doc` task checks the build before the release.
**Migration**: Drop `verctl docs` from any workflow. A failed docs.rs build shows on docs.rs, and a crate owner rebuilds it from crates.io.

### Requirement: The docs.rs build is not part of a complete release

**Reason**: The `docs` job is gone, so no job runs after `publish` to check docs.rs.
**Migration**: Drop the `docs` input from a caller of the reusable workflow.
