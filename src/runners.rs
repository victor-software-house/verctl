//! The label rules shared by `[[ci.jobs]]` and `[[assets.targets]]`.
//!
//! Both surfaces spell a runner the same way — `id` names the job, `runs_on`
//! is the literal GitHub label list. Nothing here resolves a label: verctl
//! cannot know which machines carry which labels, so it passes them through
//! and a wrong label fails the way every wrong label fails, by queueing.

use anyhow::{Result, bail, ensure};
use std::collections::BTreeSet;

/// `runs_on` for one row. `None` (field omitted) takes `default`.
pub fn labels(
    kind: &str,
    id: &str,
    runs_on: Option<&Vec<String>>,
    default: &[&str],
) -> Result<Vec<String>> {
    let Some(labels) = runs_on else {
        return Ok(default.iter().map(|label| (*label).to_owned()).collect());
    };
    ensure!(
        !labels.is_empty(),
        "{kind} {id:?}: runs_on must name at least one label"
    );
    ensure!(
        labels.iter().all(|label| !label.trim().is_empty()),
        "{kind} {id:?}: runs_on has an empty label"
    );
    Ok(labels.clone())
}

/// Reject a repeated id: two jobs with one name, one overwriting the other.
pub fn unique<'a>(kind: &str, ids: impl IntoIterator<Item = &'a str>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            bail!("duplicate {kind} {id:?}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{labels, unique};

    #[test]
    fn omitted_takes_the_default() {
        let resolved = labels("ci job", "verify", None, &["ubuntu-latest"]).unwrap();
        assert_eq!(resolved, ["ubuntu-latest"]);
    }

    #[test]
    fn a_written_list_is_passed_through_verbatim() {
        let written = vec!["self-hosted".to_owned(), "linux".to_owned()];
        let resolved = labels("ci job", "verify", Some(&written), &["ubuntu-latest"]).unwrap();
        assert_eq!(resolved, ["self-hosted", "linux"]);
    }

    #[test]
    fn an_empty_list_is_not_a_default() {
        let err = labels("ci job", "verify", Some(&Vec::new()), &["ubuntu-latest"]).unwrap_err();
        assert!(format!("{err:#}").contains("at least one label"), "{err:#}");
    }

    #[test]
    fn a_blank_label_is_rejected() {
        let written = vec!["  ".to_owned()];
        let err = labels("asset target", "linux-x64", Some(&written), &[]).unwrap_err();
        assert!(format!("{err:#}").contains("empty label"), "{err:#}");
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        assert!(unique("ci job", ["verify", "lint"]).is_ok());
        let err = unique("ci job", ["verify", "verify"]).unwrap_err();
        assert!(format!("{err:#}").contains("duplicate ci job"), "{err:#}");
    }
}
