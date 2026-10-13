# Tasks

- [ ] 1.1 `publisher: none` resolves to an empty upload command, and `publish` skips it; proof: `dry_run_lists_a_release_only_package` on the build host
- [ ] 1.2 A registry or publish beside `none` fails the load; proof: `a_release_only_package_names_no_registry` on the build host
- [ ] 1.3 Only `none` uploads nothing: an empty command and `publishers.none` fail; proof: `only_none_renders_an_empty_command` and `a_release_only_package_names_no_registry` on the build host
- [ ] 1.4 Operator docs name the mode; proof: `cargo test operator_docs` on the build host
