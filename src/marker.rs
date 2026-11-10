//! A package's version on one marker line of a file of any type: a comment
//! opener (`#`, `//`, or `<!--`), `version:`, a space, and the version, with
//! an optional `-->` after it.
//!
//! A single file can be a package, such as one reusable workflow. Its
//! version has to live in the file, and a comment is the one place every
//! format leaves alone. A package opts in with `marker: true`: a stray
//! `# version:` line never changes how verctl reads a file nobody declared.

use anyhow::{Result, bail};
use semver::Version;

/// Comment openers a marker line may start with, longest first so `<!--`
/// is never read as text after some shorter opener.
const OPENERS: [&str; 3] = ["<!--", "//", "#"];

/// The key every marker line spells.
const KEY: &str = "version:";

/// The one marker line: where its version sits and what it says.
struct Marker<'a> {
    /// Byte range of the version text in the file.
    start: usize,
    end: usize,
    /// The version text.
    version: &'a str,
}

/// The version on the file's one marker line.
///
/// # Errors
///
/// Returns an error when the file holds no marker line, more than one, or one
/// whose value is not a version; the error names the line numbers.
pub(crate) fn read(raw: &str) -> Result<String> {
    Ok(marker(raw)?.version.to_owned())
}

/// `raw` with the marker line's version replaced and every other byte kept.
///
/// # Errors
///
/// Fails on the same conditions as [`read`].
pub(crate) fn write(raw: &str, new_version: &str) -> Result<String> {
    let found = marker(raw)?;
    Ok(format!(
        "{}{new_version}{}",
        &raw[..found.start],
        &raw[found.end..]
    ))
}

fn marker(raw: &str) -> Result<Marker<'_>> {
    let mut found = Vec::new();
    let mut offset = 0;
    for (index, line) in raw.split_inclusive('\n').enumerate() {
        if let Some(marker) = parse_line(line, offset) {
            found.push((index + 1, marker));
        }
        offset += line.len();
    }
    if found.len() > 1 {
        let lines: Vec<String> = found.iter().map(|(number, _)| number.to_string()).collect();
        bail!(
            "holds a marker line on lines {}; keep exactly one",
            lines.join(" and ")
        );
    }
    let Some((number, marker)) = found.pop() else {
        bail!("holds no marker line such as `# version: 0.1.0`");
    };
    if Version::parse(marker.version).is_err() {
        bail!(
            "line {number} holds `version: {}`, which is not a version",
            marker.version
        );
    }
    Ok(marker)
}

/// The marker on `line`, which starts at byte `offset` of the file, when the
/// line is one: optional indentation, an opener, `version:`, one space, the
/// value, and nothing after it but `-->` for an HTML comment.
fn parse_line(line: &str, offset: usize) -> Option<Marker<'_>> {
    let content = line.trim_end_matches(['\n', '\r']);
    let trimmed = content.trim_start();
    let opener = OPENERS
        .into_iter()
        .find(|opener| trimmed.starts_with(opener))?;
    let after_opener = trimmed[opener.len()..].trim_start();
    let value = after_opener.strip_prefix(KEY)?.strip_prefix(' ')?;
    let version = value.split_whitespace().next()?;
    let rest = value.strip_prefix(version)?.trim();
    let closes = rest.is_empty() || (opener == "<!--" && rest == "-->");
    if !closes {
        return None;
    }
    let start = offset + (content.len() - value.len());
    Some(Marker {
        start,
        end: start + version.len(),
        version,
    })
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::{read, write};

    #[test]
    fn each_opener_marks_the_version() {
        let cases = [
            indoc! {"
                name: CI
                # version: 1.2.0
                on: push
            "},
            indoc! {"
                // version: 1.2.0
                export const x = 1;
            "},
            indoc! {"
                <!-- version: 1.2.0 -->
                # Title
            "},
        ];
        for raw in cases {
            assert_eq!(read(raw).unwrap(), "1.2.0", "{raw}");
            let next = write(raw, "1.2.1").unwrap();
            assert_eq!(next, raw.replace("1.2.0", "1.2.1"), "{raw}");
        }
    }

    #[test]
    fn an_indented_crlf_marker_keeps_its_bytes() {
        let raw = "jobs:\r\n  # version: 0.3.0\r\n";
        assert_eq!(
            write(raw, "0.3.1").unwrap(),
            "jobs:\r\n  # version: 0.3.1\r\n"
        );
    }

    #[test]
    fn a_file_with_no_marker_fails() {
        let raw = indoc! {"
            name: CI
            on: push
        "};
        let error = read(raw).unwrap_err();
        assert!(format!("{error:#}").contains("no marker line"), "{error:#}");
    }

    #[test]
    fn two_markers_fail_and_name_both_lines() {
        let raw = indoc! {"
            # version: 1.2.0
            name: CI
            # version: 1.2.0
        "};
        let error = read(raw).unwrap_err();
        assert!(format!("{error:#}").contains("lines 1 and 3"), "{error:#}");
    }

    #[test]
    fn a_marker_that_is_not_a_version_fails() {
        let error = read("# version: latest\n").unwrap_err();
        assert!(format!("{error:#}").contains("line 1"), "{error:#}");
    }

    #[test]
    fn prose_about_a_version_is_not_a_marker() {
        let raw = indoc! {"
            # version: 1.0.0
            # The version: 2.0.0 changed things.
            # version:2.0.0
            # version: 3.0.0 and more
        "};
        assert_eq!(read(raw).unwrap(), "1.0.0");
    }

    /// A value past extra whitespace is not a marker, and a multibyte
    /// character there must not split mid-byte.
    #[test]
    fn extra_space_before_a_multibyte_value_is_not_a_marker() {
        let raw = indoc! {"
            # version: 1.0.0
            # version:  é
        "};
        assert_eq!(read(raw).unwrap(), "1.0.0");
    }
}
