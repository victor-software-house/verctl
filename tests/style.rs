use std::fs;
use std::path::Path;

#[test]
fn concat_macro_is_forbidden() {
    let needle = format!("{}{}", "concat", "!");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut hits = Vec::new();
    for dir in ["src", "tests"] {
        collect_hits(&root.join(dir), &needle, &mut hits);
    }
    assert!(
        hits.is_empty(),
        "use indoc for multiline strings, not {needle}:\n{}",
        hits.join("\n")
    );
}

fn collect_hits(dir: &Path, needle: &str, hits: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_hits(&path, needle, hits);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        for (index, line) in source.lines().enumerate() {
            if line.contains(needle) {
                hits.push(format!("{}:{}:{line}", path.display(), index + 1));
            }
        }
    }
}
