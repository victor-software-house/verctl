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

fn one_line(text: &str) -> String {
    text.trim_end().to_owned()
}

#[test]
fn internal_pr_has_no_byline() {
    let rendered = render_release(
        &ReleaseInput {
            summary: "Restore mise.toml".into(),
            continuations: vec![],
            pull_request: Some(pr(false)),
            commit: Some(Commit {
                short: "96fb0bc".into(),
                url: "https://github.com/org/repo/commit/96fb0bc".into(),
            }),
        }
        .with_author_filter(&["ext".into()]),
    )
    .expect("render");
    assert_eq!(
        rendered,
        one_line(indoc! {"
            - Restore mise.toml ([#12](https://github.com/org/repo/pull/12)).
        "})
    );
}

#[test]
fn external_pr_has_byline() {
    let rendered = render_release(
        &ReleaseInput {
            summary: "Restore mise.toml".into(),
            continuations: vec![],
            pull_request: Some(pr(true)),
            commit: None,
        }
        .with_author_filter(&["owner".into()]),
    )
    .expect("render");
    assert_eq!(
        rendered,
        one_line(indoc! {"
            - Restore mise.toml ([#12](https://github.com/org/repo/pull/12) by [@ext](https://github.com/ext)).
        "})
    );
}

#[test]
fn commit_fallback_when_no_pr() {
    let rendered = render_release(&ReleaseInput {
        summary: "Restore mise.toml".into(),
        continuations: vec![],
        pull_request: None,
        commit: Some(Commit {
            short: "96fb0bc".into(),
            url: "https://github.com/org/repo/commit/96fb0bc".into(),
        }),
    })
    .expect("render");
    assert_eq!(
        rendered,
        one_line(indoc! {"
            - Restore mise.toml ([`96fb0bc`](https://github.com/org/repo/commit/96fb0bc)).
        "})
    );
}

#[test]
fn bare_period_when_no_link_and_no_terminal() {
    let rendered = render_release(&ReleaseInput {
        summary: "Restore mise.toml".into(),
        continuations: vec!["More detail.".into(), String::new()],
        pull_request: None,
        commit: None,
    })
    .expect("render");
    assert_eq!(
        rendered,
        one_line(indoc! {"
            - Restore mise.toml.
              More detail.
        "})
    );
}

#[test]
fn no_extra_period_when_summary_has_terminal() {
    let rendered = render_release(&ReleaseInput {
        summary: "Restore mise.toml.".into(),
        continuations: vec![],
        pull_request: None,
        commit: None,
    })
    .expect("render");
    assert_eq!(
        rendered,
        one_line(indoc! {"
            - Restore mise.toml.
        "})
    );
}

#[test]
fn pr_wins_over_commit() {
    let rendered = render_release(&ReleaseInput {
        summary: "Restore mise.toml".into(),
        continuations: vec![],
        pull_request: Some(pr(false)),
        commit: Some(Commit {
            short: "96fb0bc".into(),
            url: "https://github.com/org/repo/commit/96fb0bc".into(),
        }),
    })
    .expect("render");
    assert!(rendered.contains("#12"));
    assert!(!rendered.contains("96fb0bc"));
}

#[test]
fn question_and_bang_are_terminal() {
    for summary in ["Ship it?", "Ship it!"] {
        let rendered = render_release(&ReleaseInput {
            summary: summary.into(),
            continuations: vec![],
            pull_request: None,
            commit: None,
        })
        .expect("render");
        assert_eq!(rendered, format!("- {summary}"));
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

#[test]
fn empty_login_is_not_external() {
    let rendered = render_release(
        &ReleaseInput {
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
        }
        .with_author_filter(&["owner".into()]),
    )
    .expect("render");
    assert_eq!(
        rendered,
        one_line(indoc! {"
            - Restore mise.toml ([#12](https://github.com/org/repo/pull/12)).
        "})
    );
}

#[test]
fn dependency_list() {
    let rendered = render_dependencies(&[
        Dependency {
            name: "left".into(),
            new_version: "1.2.3".into(),
        },
        Dependency {
            name: "right".into(),
            new_version: "4.5.6".into(),
        },
    ])
    .expect("render");
    assert_eq!(
        rendered,
        one_line(indoc! {"
            - Updated dependencies:
              - left@1.2.3
              - right@4.5.6
        "})
    );
}
