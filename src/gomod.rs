//! A Go module's version, kept as a `// vX.Y.Z` comment at the end of its
//! `module` line.
//!
//! Go itself records no version in `go.mod`: a release is a tag. The comment
//! gives verctl a tracked line to read and bump, and Go ignores it. The
//! [go.mod reference] gives a module-line comment meaning only when it starts
//! with `Deprecated:`, which this format never writes. A notice directly above
//! the line is read together with that comment, so such a file is refused.
//!
//! [go.mod reference]: https://go.dev/ref/mod#go-mod-file-module-deprecation

use anyhow::{Result, anyhow, bail};
use semver::Version;

/// The version a module with no comment has: a new package's.
const UNRELEASED: &str = "0.0.0";

/// The parts of the `module` line this format reads and rewrites.
struct ModuleLine<'a> {
    /// The module path, without quotes.
    path: &'a str,
    /// Byte offset just after the path, where the comment goes.
    path_end: usize,
    /// Byte offset of the line ending, or of the end of the file.
    line_end: usize,
    /// The comment's text after `//`, trimmed.
    comment: Option<&'a str>,
    /// Whether a `Deprecated:` notice sits in the comment lines directly above.
    deprecated: bool,
}

/// The version on the `module` line, `0.0.0` when it carries no comment.
///
/// # Errors
///
/// Returns an error when the file has no `module` line, uses a `module`
/// block, or carries a comment there that is not `vX.Y.Z`.
pub(crate) fn read(raw: &str) -> Result<String> {
    let line = module_line(raw)?;
    match line.comment {
        None => Ok(UNRELEASED.to_owned()),
        Some(comment) => version_in(comment),
    }
}

/// `raw` with the `module` line's comment set to `// v<new_version>`, every
/// other byte kept.
///
/// # Errors
///
/// Fails on the same conditions as [`read`], and when a `Deprecated:` notice
/// sits directly above the module line, as [`refuse_deprecated`] explains.
pub(crate) fn write(raw: &str, new_version: &str) -> Result<String> {
    let line = module_line(raw)?;
    refuse_deprecated(&line)?;
    if let Some(comment) = line.comment {
        version_in(comment)?;
    }
    Ok(format!(
        "{} // v{new_version}{}",
        &raw[..line.path_end],
        &raw[line.line_end..]
    ))
}

/// Refuse a version whose major the module path does not carry.
///
/// From v2 on, the Go toolchain resolves a module only at a path that ends in
/// `/v<major>`, and below v2 the path carries no such suffix
/// ([major version suffixes]). A tag that breaks this rule is one no `go get`
/// can fetch, so the bump stops and names the path the release needs.
/// `gopkg.in` paths spell the major as `.vN` and are left to their owner.
///
/// [major version suffixes]: https://go.dev/ref/mod#major-version-suffixes
///
/// # Errors
///
/// Returns an error when the file cannot be read as [`read`] describes, when
/// `new_version` is not semver, when its major conflicts with the path, or
/// when [`refuse_deprecated`] refuses the file.
pub(crate) fn accepts(raw: &str, new_version: &str) -> Result<()> {
    let line = module_line(raw)?;
    refuse_deprecated(&line)?;
    if line.path.starts_with("gopkg.in/") {
        return Ok(());
    }
    let major = Version::parse(new_version)?.major;
    let (base, suffix) = split_major(line.path);
    let wanted = (major >= 2).then_some(major);
    if suffix == wanted {
        return Ok(());
    }
    let needed = match wanted {
        Some(major) => format!("{base}/v{major}"),
        None => base.to_owned(),
    };
    bail!(
        "go.mod declares module {}, but version {new_version} needs module {needed}: Go resolves a module from v2 on only at a path ending in /v<major>; change the module path in the same pull request",
        line.path
    )
}

/// Refuse a module whose deprecation notice sits directly above its line.
///
/// Go reads a notice from the comment lines directly above the `module`
/// line and its trailing comment together, joined by newlines
/// ([`parseDirectiveComment`]). A `// vX.Y.Z` comment there would end the
/// notice every `go get` user sees, so the bump stops instead.
///
/// [`parseDirectiveComment`]: https://github.com/golang/mod/blob/b5136e5ba156da612a1efc6e49b55988a134a5e0/modfile/rule.go#L899
///
/// # Errors
///
/// Returns an error when `line` has a `Deprecated:` notice above it.
fn refuse_deprecated(line: &ModuleLine<'_>) -> Result<()> {
    if line.deprecated {
        bail!(
            "go.mod has a `Deprecated:` comment directly above its module line, and Go would read the `// vX.Y.Z` comment as part of that notice; keep this module's version in a VERSION file, or declare a driver"
        );
    }
    Ok(())
}

/// Whether a comment line in the block directly above the module line starts
/// with `Deprecated:`. A blank line ends that block, as it does for Go.
fn deprecated_above(above: &str) -> bool {
    above
        .lines()
        .rev()
        .map(str::trim)
        .map_while(|line| line.strip_prefix("//"))
        .any(|text| text.trim_start().starts_with("Deprecated:"))
}

fn version_in(comment: &str) -> Result<String> {
    if let Some(version) = comment.strip_prefix('v')
        && Version::parse(version).is_ok()
    {
        return Ok(version.to_owned());
    }
    bail!(
        "the go.mod module line carries the comment {comment:?}, which is not a version; write `// vX.Y.Z` there, or declare a driver"
    )
}

/// The path without a `/vN` major suffix, and that major when it is 2 or more.
fn split_major(path: &str) -> (&str, Option<u64>) {
    if let Some((base, last)) = path.rsplit_once('/')
        && let Some(digits) = last.strip_prefix('v')
        && let Ok(major) = digits.parse::<u64>()
        && major >= 2
        && !digits.starts_with('0')
    {
        return (base, Some(major));
    }
    (path, None)
}

fn module_line(raw: &str) -> Result<ModuleLine<'_>> {
    let mut start = 0;
    for line in raw.split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        let line_end = start + content.len();
        let indent = content.len() - content.trim_start().len();
        if let Some(rest) = content.trim_start().strip_prefix("module")
            && rest.starts_with([' ', '\t', '"', '`', '('])
        {
            let rest_start = start + indent + "module".len();
            let deprecated = deprecated_above(&raw[..start]);
            return parse_module(raw, rest_start, line_end, deprecated);
        }
        start += line.len();
    }
    bail!("go.mod has no module line")
}

fn parse_module(
    raw: &str,
    rest_start: usize,
    line_end: usize,
    deprecated: bool,
) -> Result<ModuleLine<'_>> {
    let rest = &raw[rest_start..line_end];
    let lead = rest.len() - rest.trim_start().len();
    let token = &rest[lead..];
    let (path, token_len) = match token.chars().next() {
        Some('(') => bail!("go.mod declares its module in a block; declare a driver"),
        Some(quote @ ('"' | '`')) => {
            let close = token[1..]
                .find(quote)
                .map(|at| at + 1)
                .ok_or_else(|| anyhow!("go.mod module path has no closing quote"))?;
            (&token[1..close], close + 1)
        }
        Some(_) | None => {
            let len = path_len(token);
            (&token[..len], len)
        }
    };
    if path.is_empty() {
        bail!("go.mod module line names no path");
    }
    let path_end = rest_start + lead + token_len;
    let tail = raw[path_end..line_end].trim();
    let comment = match tail.strip_prefix("//") {
        Some(comment) => Some(comment.trim()),
        None if tail.is_empty() => None,
        None => bail!("go.mod module line has {tail:?} after its path"),
    };
    Ok(ModuleLine {
        path,
        path_end,
        line_end,
        comment,
        deprecated,
    })
}

/// The length of an unquoted path: up to whitespace or a `//` comment, since
/// a single `/` is part of the path.
fn path_len(token: &str) -> usize {
    let space = token.find(char::is_whitespace).unwrap_or(token.len());
    let comment = token.find("//").unwrap_or(token.len());
    space.min(comment)
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::{accepts, read, write};

    const MODULE: &str = indoc! {"
        // The demo module.
        module example.com/demo // v0.4.0

        go 1.25

        require golang.org/x/text v0.20.0 // indirect
    "};

    #[test]
    fn the_comment_is_the_version() {
        assert_eq!(read(MODULE).unwrap(), "0.4.0");
        let next = write(MODULE, "0.4.1").unwrap();
        assert_eq!(next, MODULE.replace("// v0.4.0", "// v0.4.1"));
        assert_eq!(read(&next).unwrap(), "0.4.1");
    }

    #[test]
    fn no_comment_is_unreleased() {
        let raw = indoc! {"
            module example.com/demo

            go 1.25
        "};
        assert_eq!(read(raw).unwrap(), "0.0.0");
        assert_eq!(
            write(raw, "0.0.1").unwrap(),
            indoc! {"
                module example.com/demo // v0.0.1

                go 1.25
            "}
        );
    }

    #[test]
    fn a_quoted_path_and_crlf_are_kept() {
        let raw = "module \"example.com/demo\" // v1.0.0\r\ngo 1.25\r\n";
        assert_eq!(read(raw).unwrap(), "1.0.0");
        assert_eq!(
            write(raw, "1.0.1").unwrap(),
            "module \"example.com/demo\" // v1.0.1\r\ngo 1.25\r\n"
        );
    }

    #[test]
    fn another_comment_is_not_a_version() {
        let raw = "module example.com/demo // Deprecated: use example.com/next\n";
        let error = read(raw).unwrap_err();
        assert!(format!("{error:#}").contains("Deprecated"), "{error:#}");
        assert!(write(raw, "1.0.0").is_err());
    }

    #[test]
    fn a_deprecation_notice_directly_above_refuses_a_version() {
        let raw = indoc! {"
            // Deprecated: use example.com/next.
            module example.com/demo
        "};
        assert_eq!(read(raw).unwrap(), "0.0.0");
        let error = write(raw, "0.0.1").unwrap_err();
        assert!(format!("{error:#}").contains("Deprecated:"), "{error:#}");
        assert!(accepts(raw, "0.0.1").is_err());
        let apart = indoc! {"
            // Deprecated: use example.com/next.

            module example.com/demo
        "};
        assert_eq!(
            write(apart, "0.0.1").unwrap(),
            apart.replace("demo\n", "demo // v0.0.1\n")
        );
    }

    #[test]
    fn the_major_matches_the_path() {
        let v1 = "module example.com/demo // v1.4.0\n";
        accepts(v1, "1.5.0").unwrap();
        let error = accepts(v1, "2.0.0").unwrap_err();
        assert!(
            format!("{error:#}").contains("needs module example.com/demo/v2"),
            "{error:#}"
        );
        let v2 = "module example.com/demo/v2 // v2.1.0\n";
        accepts(v2, "2.1.1").unwrap();
        let error = accepts(v2, "3.0.0").unwrap_err();
        assert!(
            format!("{error:#}").contains("needs module example.com/demo/v3"),
            "{error:#}"
        );
        let error = accepts(v2, "1.9.0").unwrap_err();
        assert!(
            format!("{error:#}").contains("needs module example.com/demo:"),
            "{error:#}"
        );
        accepts("module gopkg.in/yaml.v3 // v3.0.1\n", "3.0.2").unwrap();
    }

    #[test]
    fn a_file_without_a_module_line_fails() {
        assert!(read("go 1.25\n").is_err());
        let block = indoc! {"
            module (
                example.com/demo
            )
        "};
        assert!(read(block).is_err());
    }
}
