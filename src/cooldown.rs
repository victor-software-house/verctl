//! What cargo's publish-age cooldown refuses, and the setting that turns the
//! cooldown off for one run.
//!
//! `semver` and `release` both read cargo's error for the crates the cooldown
//! refused, so the two checks parse it one way.

use std::collections::BTreeSet;

/// The setting that lets cargo resolve a release younger than the cooldown.
pub(crate) const COOLDOWN_OFF: (&str, &str) = ("CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE", "allow");

/// The crates whose release the publish-age cooldown rejected, from cargo's
/// error in `output`, sorted and without repeats.
///
/// Cargo 1.100 reports one as `failed to select a version for the
/// requirement` naming the crate, and a `version ... is too new (published
/// ...)` line follows within three lines. Another resolution error, which has
/// no such line, names no crate.
pub(crate) fn cooldown_rejections(output: &str) -> Vec<String> {
    const REQUIREMENT: &str = "failed to select a version for the requirement `";
    const TOO_NEW: &str = " is too new (published ";
    let plain = strip_ansi(output);
    let lines: Vec<&str> = plain.lines().collect();
    let mut names = BTreeSet::new();
    for (index, line) in lines.iter().enumerate() {
        let Some((_, rest)) = line.split_once(REQUIREMENT) else {
            continue;
        };
        let Some((name, _)) = rest.split_once(" = ") else {
            continue;
        };
        let too_new = lines
            .iter()
            .skip(index + 1)
            .filter(|next| !next.trim().is_empty())
            .take(3)
            .any(|next| next.contains(TOO_NEW));
        if too_new {
            names.insert(name.to_owned());
        }
    }
    names.into_iter().collect()
}

/// `text` without its ANSI escape sequences: `ESC [`, then everything up to a
/// final byte in `@` to `~`.
fn strip_ansi(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(next) = chars.next() {
        if next == '\x1b' && chars.next_if_eq(&'[').is_some() {
            for skipped in chars.by_ref() {
                if ('@'..='~').contains(&skipped) {
                    break;
                }
            }
        } else {
            plain.push(next);
        }
    }
    plain
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::cooldown_rejections;

    const CTL_TEXT: &str = indoc! {"
        error: `cargo metadata` exited with an error:     Updating crates.io index
        error: failed to select a version for the requirement `ctl-text = \"=0.7.4\"`
          version 0.7.4 is too new (published 36 hours ago, minimum age 3 days)
        location searched: crates.io index
        required by package `ctl-core v0.7.4 (/home/runner/work/ctl-core/ctl-core/ctl-core)`
        help: to preserve the min-publish-age, downgrade the requirement to \"0.7.0\"
        help: to use too-new packages anyways, re-resolve with `CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow`
    "};

    const YAMLED: &str = indoc! {"
        error: `cargo metadata` exited with an error:     Updating crates.io index
        error: failed to select a version for the requirement `yamled = \"=0.0.13\"`
          version 0.0.13 is too new (published 39 hours ago, minimum age 3 days)
        location searched: crates.io index
        required by package `ctl-core v0.7.4 (/home/runner/work/ctl-core/ctl-core/ctl-core)`
        help: to preserve the min-publish-age, downgrade the requirement to \"0.0.12\"
        help: to use too-new packages anyways, re-resolve with `CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow`
    "};

    const QCTL: &str = indoc! {"
        error: `cargo metadata` exited with an error:     Updating crates.io index
        error: failed to select a version for the requirement `qctl = \"=0.7.0\"`
          version 0.7.0 is too new (published 3 days ago, minimum age 3 days)
        location searched: crates.io index
        required by package `qctl v0.7.1 (/home/runner/work/qctl/qctl)`
        help: to preserve the min-publish-age, downgrade the requirement to \"0.6.0\"
        help: to use too-new packages anyways, re-resolve with `CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow`
    "};

    #[test]
    fn the_cooldown_names_the_crate_it_rejected() {
        assert_eq!(cooldown_rejections(CTL_TEXT), ["ctl-text"]);
        assert_eq!(cooldown_rejections(QCTL), ["qctl"]);
    }

    #[test]
    fn several_rejections_are_sorted_and_listed_once() {
        let output = format!("{YAMLED}{CTL_TEXT}{YAMLED}");
        assert_eq!(cooldown_rejections(&output), ["ctl-text", "yamled"]);
    }

    #[test]
    fn ansi_colour_codes_do_not_hide_a_rejection() {
        let output = indoc! {"
            \u{1b}[1m\u{1b}[31merror\u{1b}[0m: failed to select a version for the requirement `ctl-text = \"=0.7.4\"`
            \u{1b}[1m\u{1b}[33m  version 0.7.4 is too new (published 36 hours ago, minimum age 3 days)\u{1b}[0m
        "};
        assert_eq!(cooldown_rejections(output), ["ctl-text"]);
    }

    #[test]
    fn another_resolution_error_names_no_crate() {
        let output = indoc! {"
            error: failed to select a version for the requirement `demo = \"^9.0.0\"`
            candidate versions found which didn't match: 0.1.0
            location searched: crates.io index
            required by package `ctl-core v0.7.4 (/work/ctl-core)`
            help: version 0.9.0 is too new (published 1 hour ago, minimum age 3 days)
        "};
        assert_eq!(cooldown_rejections(output), Vec::<String>::new());
        assert_eq!(
            cooldown_rejections("error: could not compile `demo`"),
            Vec::<String>::new()
        );
    }
}
