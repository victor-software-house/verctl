use anyhow::{Context, Result, bail, ensure};
use std::io::{self, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_mins(1);
pub const DEFAULT_OUTPUT_LIMIT: usize = 8 * 1024 * 1024;

/// Run an argv with stdin piped and stdout captured.
///
/// `duct` owns the pipe lifetime so a streaming child cannot deadlock
/// the parent the way a sequential `write_all` + `wait_with_output` can.
/// stdout/stderr are read incrementally and killed at `output_limit`.
pub fn filter(argv: &[String], stdin: &str, env: &[(&str, &str)]) -> Result<String> {
    filter_limited(argv, stdin, env, DEFAULT_TIMEOUT, DEFAULT_OUTPUT_LIMIT)
}

/// Run an argv with no stdin. Used by `publish`.
pub fn run_limited(argv: &[String], env: &[(&str, &str)], timeout: Duration) -> Result<String> {
    filter_limited(argv, "", env, timeout, DEFAULT_OUTPUT_LIMIT)
}

/// `["cargo", "publish"]` without a `.into()` on every word.
#[must_use]
pub fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
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
    let (stdout_r, stdout_w) = os_pipe::pipe().context("driver stdout pipe")?;
    let (stderr_r, stderr_w) = os_pipe::pipe().context("driver stderr pipe")?;
    let handle = Arc::new(
        expr.stdin_bytes(stdin.as_bytes().to_vec())
            .stdout_file(stdout_w)
            .stderr_file(stderr_w)
            .unchecked()
            .start()
            .context("spawn driver command")?,
    );
    let mut guard = ChildGuard {
        handle: Arc::clone(&handle),
        reaped: false,
    };
    let exceeded = Arc::new(AtomicBool::new(false));
    let stdout_h = spawn_capped(
        stdout_r,
        output_limit,
        Arc::clone(&handle),
        Arc::clone(&exceeded),
    );
    let stderr_h = spawn_capped(
        stderr_r,
        output_limit,
        Arc::clone(&handle),
        Arc::clone(&exceeded),
    );

    let waited = match handle.wait_timeout(timeout) {
        Ok(output) => output.is_some(),
        Err(error) => {
            let _ = handle.kill();
            let _ = handle.wait();
            let _ = take_capped(stdout_h);
            let _ = take_capped(stderr_h);
            guard.reaped = true;
            return Err(error).context("driver command");
        }
    };
    if !waited {
        let _ = handle.kill();
        let _ = handle.wait();
        let _ = take_capped(stdout_h);
        let _ = take_capped(stderr_h);
        guard.reaped = true;
        bail!("driver command timed out after {timeout:?}");
    }

    let stdout = take_capped(stdout_h)?;
    let stderr = take_capped(stderr_h)?;
    guard.reaped = true;

    if exceeded.load(Ordering::SeqCst) {
        bail!("driver command output exceeded {output_limit} bytes");
    }
    let status = handle
        .try_wait()
        .context("driver command")?
        .context("driver exited without a status")?
        .status;
    if !status.success() {
        bail!(
            "driver command failed: {}",
            String::from_utf8_lossy(&stderr)
        );
    }
    String::from_utf8(stdout).context("driver stdout is not UTF-8")
}

struct ChildGuard {
    handle: Arc<duct::Handle>,
    reaped: bool,
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.handle.kill();
            let _ = self.handle.try_wait();
        }
    }
}

enum CapRead {
    Bytes(Vec<u8>),
    Exceeded,
}

fn spawn_capped(
    reader: os_pipe::PipeReader,
    limit: usize,
    handle: Arc<duct::Handle>,
    exceeded: Arc<AtomicBool>,
) -> JoinHandle<io::Result<CapRead>> {
    thread::spawn(move || {
        let result = read_capped(reader, limit);
        if matches!(result, Ok(CapRead::Exceeded)) {
            exceeded.store(true, Ordering::SeqCst);
            let _ = handle.kill();
        }
        result
    })
}

fn read_capped(mut reader: os_pipe::PipeReader, limit: usize) -> io::Result<CapRead> {
    let mut buf = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let n = reader.read(&mut chunk)?;
        if n == 0 {
            return Ok(CapRead::Bytes(buf));
        }
        if buf.len().saturating_add(n) > limit {
            return Ok(CapRead::Exceeded);
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

fn take_capped(handle: JoinHandle<io::Result<CapRead>>) -> Result<Vec<u8>> {
    match handle.join() {
        Ok(Ok(CapRead::Bytes(bytes))) => Ok(bytes),
        Ok(Ok(CapRead::Exceeded)) => bail!("driver command output exceeded the cap"),
        Ok(Err(error)) => Err(error).context("read driver output"),
        Err(_) => bail!("driver output reader panicked"),
    }
}

#[cfg(test)]
mod tests {
    use super::{filter, filter_limited};
    use indoc::indoc;
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
    fn filter_rejects_oversized_stdout_without_buffering_it() {
        let script = indoc! {"
            import sys
            while True:
                sys.stdout.buffer.write(b'x' * 65536)
                sys.stdout.buffer.flush()
        "};
        let start = Instant::now();
        let error = filter_limited(
            &["python3".into(), "-c".into(), script.into()],
            "",
            &[],
            Duration::from_secs(10),
            4096,
        )
        .expect_err("limit");
        let message = format!("{error:#}");
        assert!(message.contains("exceeded"), "{message}");
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "cap waited for the firehose instead of killing: {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn filter_reports_nonzero_status() {
        let error = filter(&["false".into()], "", &[]).expect_err("fail");
        let message = format!("{error:#}");
        assert!(message.contains("failed"), "{message}");
    }
}
