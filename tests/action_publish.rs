//! The publish action must not fetch. `verctl publish` owns that, with tests.

#![allow(missing_docs)]

const ACTION: &str = include_str!("../actions/publish/action.yml");

#[test]
fn publish_action_does_not_fetch() {
    assert!(
        !ACTION.contains("git fetch"),
        "fetch belongs in verctl publish, not untested bash: {ACTION}"
    );
}

#[test]
fn publish_action_does_not_use_bearer() {
    let lower = ACTION.to_ascii_lowercase();
    assert!(
        !lower.contains("bearer"),
        "GitHub git HTTPS does not accept Bearer: {ACTION}"
    );
}

#[test]
fn publish_action_passes_the_default_branch() {
    assert!(ACTION.contains("VERCTL_DEFAULT_BRANCH"), "{ACTION}");
}
