use anyhow::{Context, Result, bail, ensure};
use std::time::Duration;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
pub const DEFAULT_OUTPUT_LIMIT: usize = 8 * 1024 * 1024;

/// Run an argv with stdin piped and stdout captured.
///
/// `duct` owns the pipe lifetime so a streaming child cannot deadlock
/// the parent the way a sequential `write_all` + `wait_with_output` can.
pub fn filter(argv: &[String], stdin: &str, env: &[(&str, &str)]) -> Result<String> {
    filter_limited(argv, stdin, env, DEFAULT_TIMEOUT, DEFAULT_OUTPUT_LIMIT)
}

pub fn filter_limited(
    argv: &[String],
    stdin: &str,
    env: &[(&str, &str)],
    timeout: Duration,
    output_limit: usize,
) -> Result<String> {
    ensure!(!argv.is_empty(), "driver argv is empty");
    let mut expr = duct::cmd(&argv[0], &argv[1..]);
    for (key, value) in env {
        expr = expr.env(key, value);
    }
    let handle = expr
        .stdin_bytes(stdin.as_bytes().to_vec())
        .stdout_capture()
        .stderr_capture()
        .unchecked()
        .start()
        .context("spawn driver command")?;
    if handle
        .wait_timeout(timeout)
        .context("driver command")?
        .is_none()
    {
        handle.kill().context("kill timed-out driver")?;
        let _ = handle.wait();
        bail!("driver command timed out after {timeout:?}");
    }
    let output = handle.into_output().context("driver command")?;
    ensure!(
        output.stdout.len() <= output_limit && output.stderr.len() <= output_limit,
        "driver command output exceeded {output_limit} bytes"
    );
    if !output.status.success() {
        bail!(
            "driver command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8(output.stdout).context("driver stdout is not UTF-8")
}

#[cfg(test)]
mod tests {
    use super::{filter, filter_limited};
    use std::time::{Duration, Instant};

    #[test]
    fn filter_returns_stdout() {
        let out = filter(&["tr".into(), "-d".into(), "\n".into()], "1.2.3\n", &[]).expect("filter");
        assert_eq!(out, "1.2.3");
    }

    #[test]
    fn filter_times_out_and_kills() {
        let start = Instant::now();
        let error = filter_limited(
            &["sleep".into(), "30".into()],
            "",
            &[],
            Duration::from_millis(200),
            1024,
        )
        .expect_err("timeout");
        let message = format!("{error:#}");
        assert!(message.contains("timed out"), "{message}");
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "kill did not return promptly: {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn filter_rejects_oversized_stdout() {
        let script = "import sys; sys.stdout.buffer.write(b'x' * 8192)";
        let error = filter_limited(
            &["python3".into(), "-c".into(), script.into()],
            "",
            &[],
            Duration::from_secs(5),
            1024,
        )
        .expect_err("limit");
        let message = format!("{error:#}");
        assert!(message.contains("exceeded"), "{message}");
    }

    #[test]
    fn filter_reports_nonzero_status() {
        let error = filter(&["false".into()], "", &[]).expect_err("fail");
        let message = format!("{error:#}");
        assert!(message.contains("failed"), "{message}");
    }
}
