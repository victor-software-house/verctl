//! The stop record a failed `release` run writes into its draft Release.
//!
//! When a registry upload fails, the run appends one HTML comment line to the
//! draft's body, so the rendered page does not show it:
//!
//! ```text
//! <!-- verctl:stop {"package":"shrub-process","version":"0.7.0","cause":"429 Too Many Requests","retry_after":"Fri, 13 Nov 2026 19:00:00 GMT"} -->
//! ```
//!
//! `status` reads it from the draft it already looks up. Anyone who can edit
//! the draft can edit the record, so it is display only: no decision reads
//! it, and what is live always comes from the registry. Publishing the draft
//! writes the notes alone, which drops the record.

use anyhow::{Context, Result};
use ctl_core::formatdoc;
use serde::{Deserialize, Serialize};

/// What opens a stop record's line.
const OPEN: &str = "<!-- verctl:stop ";

/// What closes a stop record's line.
const CLOSE: &str = " -->";

/// Why the last `release` run stopped, as it reported it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Stop {
    /// The package whose upload failed, such as a workspace member.
    pub package: String,
    /// The version it was uploading.
    pub version: String,
    /// What stopped it: `429 Too Many Requests` for a registry rate limit,
    /// or the first line of the error otherwise.
    pub cause: String,
    /// The time the registry gave for the next attempt, as it wrote it.
    /// `None` when it gave none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<String>,
}

impl Stop {
    /// A stop of `package` at `version` for `cause`.
    #[must_use]
    pub fn new(package: &str, version: &str, cause: &str, retry_after: Option<String>) -> Self {
        Self {
            package: package.to_owned(),
            version: version.to_owned(),
            cause: cause.to_owned(),
            retry_after,
        }
    }
}

/// The stop record a Release body holds.
///
/// Not `#[non_exhaustive]`: the binary prints each state, and an exhaustive
/// match makes a new variant fail the build until it is printed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Record {
    /// The body holds no record.
    #[default]
    Absent,
    /// The record parsed.
    Read(Stop),
    /// A `verctl:stop` comment is there but does not parse. It is never
    /// trusted in part.
    Unreadable,
}

/// The stop record in `body`: the first `verctl:stop` line.
#[must_use]
pub fn read(body: &str) -> Record {
    let Some(line) = body.lines().map(str::trim).find(|line| is_record(line)) else {
        return Record::Absent;
    };
    line.strip_prefix(OPEN)
        .and_then(|rest| rest.strip_suffix(CLOSE))
        .and_then(|json| serde_json::from_str(json).ok())
        .map_or(Record::Unreadable, Record::Read)
}

/// `body` with its stop records replaced by one for `stop`, on its own line
/// after the notes.
///
/// `-->` inside a value is written as `--\u003e`, which JSON reads back the
/// same, so a cause cannot end the comment early.
///
/// # Errors
///
/// Returns an error when `stop` cannot be encoded as JSON.
pub fn write(body: &str, stop: &Stop) -> Result<String> {
    let json = serde_json::to_string(stop)
        .context("encode the stop record")?
        .replace("-->", "--\\u003e");
    let notes = body
        .lines()
        .filter(|line| !is_record(line.trim()))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(formatdoc! {"
        {notes}

        {OPEN}{json}{CLOSE}
        ",
        notes = notes.trim_end(),
    })
}

fn is_record(line: &str) -> bool {
    line.starts_with(OPEN.trim_end())
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::{Record, Stop, read, write};

    fn limited() -> Stop {
        Stop::new(
            "shrub-process",
            "0.7.0",
            "429 Too Many Requests",
            Some("Fri, 13 Nov 2026 19:00:00 GMT".to_owned()),
        )
    }

    #[test]
    fn a_record_reads_back_as_written() {
        let notes = indoc! {"
            ## shrub 0.7.0

            - Notes.
        "};
        let body = write(notes, &limited()).unwrap();
        assert_eq!(
            body,
            indoc! {r#"
                ## shrub 0.7.0

                - Notes.

                <!-- verctl:stop {"package":"shrub-process","version":"0.7.0","cause":"429 Too Many Requests","retry_after":"Fri, 13 Nov 2026 19:00:00 GMT"} -->
            "#}
        );
        assert_eq!(read(&body), Record::Read(limited()));
    }

    #[test]
    fn an_unknown_field_is_unreadable() {
        let body = indoc! {r#"
            - Notes.

            <!-- verctl:stop {"package":"a","version":"1.0.0","cause":"x","extra":true} -->
        "#};
        assert_eq!(read(body), Record::Unreadable);
    }

    #[test]
    fn a_comment_that_is_not_json_is_unreadable() {
        assert_eq!(read("<!-- verctl:stop not json -->"), Record::Unreadable);
        assert_eq!(read("<!-- verctl:stop"), Record::Unreadable);
    }

    #[test]
    fn a_body_with_no_record_has_none() {
        let body = indoc! {"
            ## demo 1.0.0

            <!-- a comment -->
        "};
        assert_eq!(read(body), Record::Absent);
    }

    #[test]
    fn writing_a_record_twice_keeps_one() {
        let first = write("- Notes.", &Stop::new("a", "1.0.0", "exit 1", None)).unwrap();
        let second = write(&first, &limited()).unwrap();
        assert_eq!(second.matches("verctl:stop").count(), 1, "{second}");
        assert!(second.starts_with("- Notes."), "{second}");
        assert_eq!(read(&second), Record::Read(limited()));
    }

    #[test]
    fn a_cause_cannot_close_the_comment() {
        let stop = Stop::new("a", "1.0.0", "saw --> here", None);
        let body = write("", &stop).unwrap();
        assert_eq!(body.matches("-->").count(), 1, "{body}");
        assert_eq!(read(&body), Record::Read(stop));
    }
}
