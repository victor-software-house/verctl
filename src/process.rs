use anyhow::{Context, Result, bail, ensure};

/// Run an argv with stdin piped and stdout captured.
///
/// `duct` owns the pipe lifetime so a streaming child cannot deadlock
/// the parent the way a sequential `write_all` + `wait_with_output` can.
pub fn filter(argv: &[String], stdin: &str, env: &[(&str, &str)]) -> Result<String> {
    ensure!(!argv.is_empty(), "driver argv is empty");
    let mut expr = duct::cmd(&argv[0], &argv[1..]);
    for (key, value) in env {
        expr = expr.env(key, value);
    }
    let output = expr
        .stdin_bytes(stdin.as_bytes().to_vec())
        .stdout_capture()
        .stderr_capture()
        .unchecked()
        .run()
        .context("driver command")?;
    if !output.status.success() {
        bail!(
            "driver command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8(output.stdout).context("driver stdout is not UTF-8")
}
