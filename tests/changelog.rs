use indoc::indoc;
use verctl::changelog::{
    Commit, Dependency, PullRequest, ReleaseInput, render_dependencies, render_release,
};

fn pr(external: bool) -> PullRequest {
    PullRequest {
        number: 12,
        url: "https://github.com/org/repo/pull/12".into(),
        user: Some("ext".into()),
        user_url: Some("https://github.com/ext".into()),
        external_author: external,
    }
}

fn commit() -> Commit {
    Commit {
        short: "96fb0bc".into(),
        url: "https://github.com/org/repo/commit/96fb0bc".into(),
    }
}

fn trimmed(text: &str) -> String {
    text.trim_end().to_owned()
}

struct ReleaseCase {
    name: &'static str,
    input: ReleaseInput,
    authors: &'static [&'static str],
    expected: &'static str,
}

#[test]
#[allow(clippy::too_many_lines)]
fn release_template_varies_by_data() {
    let cases = [
        ReleaseCase {
            name: "internal pr, no byline",
            input: ReleaseInput {
                summary: "Restore mise.toml".into(),
                continuations: vec![],
                pull_request: Some(pr(false)),
                commit: Some(commit()),
            },
            authors: &["ext"],
            expected: "- Restore mise.toml ([#12](https://github.com/org/repo/pull/12)).",
        },
        ReleaseCase {
            name: "external pr, byline",
            input: ReleaseInput {
                summary: "Restore mise.toml".into(),
                continuations: vec![],
                pull_request: Some(pr(true)),
                commit: None,
            },
            authors: &["owner"],
            expected: "- Restore mise.toml ([#12](https://github.com/org/repo/pull/12) by [@ext](https://github.com/ext)).",
        },
        ReleaseCase {
            name: "commit when there is no pr",
            input: ReleaseInput {
                summary: "Restore mise.toml".into(),
                continuations: vec![],
                pull_request: None,
                commit: Some(commit()),
            },
            authors: &[],
            expected: "- Restore mise.toml ([`96fb0bc`](https://github.com/org/repo/commit/96fb0bc)).",
        },
        ReleaseCase {
            name: "bare period plus continuation",
            input: ReleaseInput {
                summary: "Restore mise.toml".into(),
                continuations: vec!["More detail.".into(), String::new()],
                pull_request: None,
                commit: None,
            },
            authors: &[],
            expected: "- Restore mise.toml.\n  More detail.",
        },
        ReleaseCase {
            name: "summary already terminal",
            input: ReleaseInput {
                summary: "Restore mise.toml.".into(),
                continuations: vec![],
                pull_request: None,
                commit: None,
            },
            authors: &[],
            expected: "- Restore mise.toml.",
        },
        ReleaseCase {
            name: "question is terminal",
            input: ReleaseInput {
                summary: "Ship it?".into(),
                continuations: vec![],
                pull_request: None,
                commit: None,
            },
            authors: &[],
            expected: "- Ship it?",
        },
        ReleaseCase {
            name: "bang is terminal",
            input: ReleaseInput {
                summary: "Ship it!".into(),
                continuations: vec![],
                pull_request: None,
                commit: None,
            },
            authors: &[],
            expected: "- Ship it!",
        },
        ReleaseCase {
            name: "empty login is not external",
            input: ReleaseInput {
                summary: "Restore mise.toml".into(),
                continuations: vec![],
                pull_request: Some(PullRequest {
                    number: 12,
                    url: "https://github.com/org/repo/pull/12".into(),
                    user: None,
                    user_url: None,
                    external_author: true,
                }),
                commit: None,
            },
            authors: &["owner"],
            expected: "- Restore mise.toml ([#12](https://github.com/org/repo/pull/12)).",
        },
        ReleaseCase {
            name: "pr wins over commit",
            input: ReleaseInput {
                summary: "Restore mise.toml".into(),
                continuations: vec![],
                pull_request: Some(pr(false)),
                commit: Some(commit()),
            },
            authors: &[],
            expected: "- Restore mise.toml ([#12](https://github.com/org/repo/pull/12)).",
        },
    ];

    for case in cases {
        let authors: Vec<String> = case.authors.iter().map(|name| (*name).to_owned()).collect();
        let input = if authors.is_empty() {
            case.input
        } else {
            case.input.with_author_filter(&authors)
        };
        let rendered = render_release(&input).expect(case.name);
        assert_eq!(trimmed(&rendered), case.expected, "{}", case.name);
    }
}

#[test]
fn override_template_is_used() {
    let rendered = verctl::changelog::render_release_template(
        "{{ summary }}\n",
        &ReleaseInput {
            summary: "plain".into(),
            continuations: vec![],
            pull_request: None,
            commit: None,
        },
    )
    .expect("render");
    assert_eq!(rendered, "plain");
}

#[test]
fn invalid_template_fails() {
    let error = verctl::changelog::render_release_template(
        "{% if %}",
        &ReleaseInput {
            summary: "x".into(),
            continuations: vec![],
            pull_request: None,
            commit: None,
        },
    )
    .expect_err("bad template");
    assert!(format!("{error:#}").contains("parse"), "{error:#}");
}

struct DepsCase {
    name: &'static str,
    deps: &'static [(&'static str, &'static str)],
    expected: &'static str,
}

#[test]
fn dependency_template_varies_by_data() {
    let cases = [
        DepsCase {
            name: "two crates",
            deps: &[("left", "1.2.3"), ("right", "4.5.6")],
            expected: indoc! {"
                - Updated dependencies:
                  - left@1.2.3
                  - right@4.5.6
            "},
        },
        DepsCase {
            name: "one crate",
            deps: &[("only", "0.1.0")],
            expected: indoc! {"
                - Updated dependencies:
                  - only@0.1.0
            "},
        },
        DepsCase {
            name: "empty list",
            deps: &[],
            expected: "- Updated dependencies:\n",
        },
    ];
    for case in cases {
        let deps: Vec<Dependency> = case
            .deps
            .iter()
            .map(|(name, version)| Dependency {
                name: (*name).to_owned(),
                new_version: (*version).to_owned(),
            })
            .collect();
        let rendered = render_dependencies(&deps).expect(case.name);
        assert_eq!(trimmed(&rendered), trimmed(case.expected), "{}", case.name);
    }
}
