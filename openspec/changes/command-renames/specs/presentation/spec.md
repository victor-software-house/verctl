## MODIFIED Requirements

### Requirement: Records render borderless

verctl SHALL render key and value summaries with ctl-core's default record
style: no frame and right-aligned keys.

#### Scenario: Publish dry run

- **WHEN** `verctl release --dry-run` runs for one cargo crate at version 0.0.1
- **THEN** the summary lines read `release  would create v0.0.1` and `dry-run  nothing published`
- **AND** they carry no box-drawing characters
