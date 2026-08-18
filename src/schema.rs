//! Validators every declared schema shares, so a repo's TOML and a template's
//! own exports are held to the same rules and complain the same way.
//!
//! Each one is a `#[garde(custom(…))]` rule: it runs at the boundary, on a
//! fully parsed value, and garde names the field that failed. They are generic
//! over the context so the same rule works in a schema validated against
//! nothing and in one validated against the whole config.

use std::path::{Component, Path};

/// Somewhere a repo may write: relative, and never upward out of the tree.
pub fn inside_the_repo<C>(path: &Path, _: &C) -> garde::Result {
    if path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(garde::Error::new("must stay inside the repository"));
    }
    Ok(())
}

/// A list a schema needs something in, complaining in the repo's own words:
/// `at_least_one("label")` says "must declare at least one label".
pub fn at_least_one<T, C>(noun: &'static str) -> impl Fn(&[T], &C) -> garde::Result {
    move |items, _| {
        if items.is_empty() {
            return Err(garde::Error::new(format!(
                "must declare at least one {noun}"
            )));
        }
        Ok(())
    }
}

/// A value a schema cannot do without, told what it is for, so the complaint
/// reads like a sentence: "cannot be empty — it is what marks a template".
pub fn cannot_be_empty<C>(because: &'static str) -> impl Fn(&str, &C) -> garde::Result {
    move |value, _| {
        if value.is_empty() {
            return Err(garde::Error::new(format!("cannot be empty — {because}")));
        }
        Ok(())
    }
}

/// One file name, so a name cannot smuggle in the path it was separated from.
pub fn one_file_name<C>(name: &str, _: &C) -> garde::Result {
    let path = Path::new(name);
    if path.file_name().is_none_or(|only| only != name) {
        return Err(garde::Error::new("must be one file name"));
    }
    Ok(())
}
