//! The publish action must not fetch. `verctl publish` owns that, with tests.
//! The Version PR action leaves the title and commit message to verctl.
//! Every action passes settings as `VERCTL_*` variables and runs the bare verb.

const ACTION: &str = include_str!("../actions/publish/action.yml");
const VERSION_PR: &str = include_str!("../actions/version-pr/action.yml");
const ASSET: &str = include_str!("../actions/asset/action.yml");

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

/// An empty title or commit message reaches verctl as an empty variable,
/// which it reads as unset, so one place owns the default.
#[test]
fn version_pr_action_leaves_the_title_to_verctl() {
    assert!(
        !VERSION_PR.contains(r#"default: "chore(release): version packages""#),
        "{VERSION_PR}"
    );
}

/// verctl reads every flag from `VERCTL_<LONG>`, so no action rebuilds argv in
/// bash or restates verctl's default config path.
#[test]
fn actions_set_variables_and_run_the_bare_verb() {
    for (action, verb) in [
        (VERSION_PR, "run: exec verctl prepare --pr\n"),
        (ACTION, "run: exec verctl publish\n"),
        (ASSET, "run: exec verctl assets --upload\n"),
    ] {
        assert!(action.contains(verb), "{action}");
        assert!(!action.contains("extra=("), "{action}");
        assert!(!action.contains("-c \""), "{action}");
        assert!(!action.contains("default: .ctl/ver.yaml"), "{action}");
    }
}
