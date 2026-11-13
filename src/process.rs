use std::error::Error;
use std::fmt;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::ExitStatus;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};

use crate::registry_token;

/// How long a driver command may run.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_mins(1);
/// How many bytes a driver command may print before it is killed.
pub const DEFAULT_OUTPUT_LIMIT: usize = 8 * 1024 * 1024;

/// Run an argv with stdin piped and stdout captured.
///
/// `duct` owns the pipe lifetime so a streaming child cannot deadlock
/// the parent the way a sequential `write_all` + `wait_with_output` can.
/// stdout/stderr are read incrementally and killed at `output_limit`.
///
/// # Errors
///
/// Returns an error in the same cases as [`filter_limited`], called with the
/// default timeout and output limit.
pub fn filter(argv: &[String], stdin: &str, env: &[(&str, &str)]) -> Result<String> {
    filter_limited(argv, stdin, env, DEFAULT_TIMEOUT, DEFAULT_OUTPUT_LIMIT)
}

/// Run an argv with no stdin. Used by `release`.
///
/// # Errors
///
/// Returns an error in the same cases as [`filter_limited`], called with the
/// given `timeout` and the default output limit.
pub fn run_limited(argv: &[String], env: &[(&str, &str)], timeout: Duration) -> Result<String> {
    filter_limited(argv, "", env, timeout, DEFAULT_OUTPUT_LIMIT)
}

/// The most stderr [`run_inherit`] keeps for its error message.
const STDERR_TAIL: usize = 64 * 1024;

/// Stream the command's stdout and stderr to this process's stderr.
/// Actions then show cargo/bun as they run, and stdout holds only verctl's
/// own report, so `--format json` stays parseable.
/// The error keeps the last 64 KiB of the streamed output.
/// A caller such as `release` reads it to recognise an already-published
/// version.
///
/// # Errors
///
/// Returns an error when:
///
/// - `argv` is empty.
/// - The stderr pipe cannot be created.
/// - The command cannot be spawned or waited on.
/// - The command runs longer than `timeout`.
/// - The command exits unsuccessfully.
///
/// A non-zero exit puts the joined argv and the stderr tail in the error.
pub fn run_inherit(argv: &[String], timeout: Duration) -> Result<()> {
    run_inherit_in(argv, None, timeout)
}

/// [`run_inherit`] in `dir`, or in this process's working directory when
/// `dir` is `None`.
///
/// # Errors
///
/// Returns an error under the conditions of [`run_inherit`].
pub fn run_inherit_in(argv: &[String], dir: Option<&Path>, timeout: Duration) -> Result<()> {
    inherit(argv, dir, &[], timeout)
}

/// [`run_inherit`], plus `env` set on the command.
/// Only a command that needs a secret takes this path, such as a publisher
/// that reads its registry token.
///
/// # Errors
///
/// Returns an error under the conditions of [`run_inherit`].
pub fn run_inherit_env(argv: &[String], env: &[(&str, &str)], timeout: Duration) -> Result<()> {
    inherit(argv, None, env, timeout)
}

/// The variables that carry GitHub credentials.
/// A publisher, a dependency build script, and an asset build all run as
/// children of a job that holds a token able to write contents.
/// None of them inherits one.
const GITHUB_CREDENTIALS: [&str; 2] = ["GITHUB_TOKEN", "GH_TOKEN"];

/// `argv[0]` with its arguments and `env`, without the variables a registry
/// token arrives in ([`registry_token::SOURCES`]), the owner's token
/// ([`registry_token::OWNER`]), or a GitHub credential
/// ([`GITHUB_CREDENTIALS`]), so no command verctl starts inherits one.
///
/// A command that needs one gets it back through `env` under the variable it
/// reads. `env` is set inside the removals, so a variable named in both stays
/// set. duct applies the outermost call first, which makes the innermost win.
fn command(argv: &[String], env: &[(&str, &str)]) -> duct::Expression {
    let expression = env.iter().fold(
        duct::cmd(&argv[0], &argv[1..]),
        |expression, (key, value)| expression.env(key, value),
    );
    registry_token::SOURCES
        .iter()
        .chain(&[registry_token::OWNER])
        .chain(&GITHUB_CREDENTIALS)
        .fold(expression, |expression, variable| {
            expression.env_remove(variable)
        })
}

fn inherit(
    argv: &[String],
    dir: Option<&Path>,
    env: &[(&str, &str)],
    timeout: Duration,
) -> Result<()> {
    ensure!(!argv.is_empty(), "driver argv is empty");
    let (stderr_r, stderr_w) = os_pipe::pipe().context("command stderr pipe")?;
    let mut expression = command(argv, env);
    if let Some(dir) = dir {
        expression = expression.dir(dir);
    }
    // duct applies the outer redirect first, so stdout joins the pipe.
    let handle = expression
        .stdout_to_stderr()
        .stderr_file(stderr_w)
        .unchecked()
        .start()
        .context("spawn command")?;
    let tail = thread::spawn(move || stream_tail(stderr_r));
    let waited = match handle.wait_timeout(timeout) {
        Ok(output) => output.is_some(),
        Err(error) => {
            let _ = handle.kill();
            let _ = handle.wait();
            let _ = tail.join();
            return Err(error).context("command");
        }
    };
    if !waited {
        let _ = handle.kill();
        let _ = handle.wait();
        let _ = tail.join();
        bail!("command timed out after {timeout:?}");
    }
    #[expect(
        clippy::map_err_ignore,
        reason = "a thread join error is a panic payload (`Box<dyn Any>`), not an `Error`, so it cannot be a source"
    )]
    let stderr = tail
        .join()
        .map_err(|_| anyhow::anyhow!("stderr reader panicked"))?;
    let status = handle
        .try_wait()
        .context("command")?
        .context("command exited without a status")?
        .status;
    if !status.success() {
        bail!(
            "command failed: {}: {}",
            argv.join(" "),
            String::from_utf8_lossy(&stderr).trim()
        );
    }
    Ok(())
}

/// Copies `reader` to this process's stderr and returns its last
/// [`STDERR_TAIL`] bytes.
fn stream_tail(mut reader: impl Read) -> Vec<u8> {
    let mut tail = Vec::new();
    let mut chunk = [0_u8; 8192];
    let mut stderr = io::stderr();
    loop {
        match reader.read(&mut chunk) {
            Ok(0) | Err(_) => return tail,
            Ok(read) => {
                let _ = stderr.write_all(&chunk[..read]);
                tail.extend_from_slice(&chunk[..read]);
                if tail.len() > STDERR_TAIL {
                    tail.drain(..tail.len() - STDERR_TAIL);
                }
            }
        }
    }
}

/// `["cargo", "publish"]` without a `.into()` on every word.
#[must_use]
pub fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

/// [`filter`] with an explicit timeout and output limit.
///
/// # Errors
///
/// Returns an error when:
///
/// - `argv` is empty.
/// - The pipes cannot be created.
/// - The command cannot be spawned or waited on.
/// - The command runs longer than `timeout`.
/// - The command prints more than `output_limit` bytes on either stream.
/// - The command exits unsuccessfully.
/// - Its stdout is not UTF-8.
///
/// An unsuccessful exit puts the argv, status, and trimmed stderr in the
/// error.
pub fn filter_limited(
    argv: &[String],
    stdin: &str,
    env: &[(&str, &str)],
    timeout: Duration,
    output_limit: usize,
) -> Result<String> {
    capture(argv, stdin, env, None, timeout, output_limit)
}

/// Run an argv in `dir` with `env` set, no stdin, and stdout captured.
/// A registry `query` runs this way in the package directory, where the
/// registry config of the publisher is in effect.
///
/// # Errors
///
/// Returns an error when:
///
/// - `argv` is empty.
/// - The command cannot be spawned.
/// - The command runs longer than `timeout`, which returns a [`TimedOut`].
/// - Its output passes [`DEFAULT_OUTPUT_LIMIT`].
/// - Its stdout is not UTF-8.
/// - The command exits unsuccessfully, which returns an [`ExitFailure`].
///
/// A caller can downcast an [`ExitFailure`] to tell an answer apart from a
/// failure to run.
pub fn run_in(
    argv: &[String],
    dir: &Path,
    env: &[(&str, &str)],
    timeout: Duration,
) -> Result<String> {
    capture(argv, "", env, Some(dir), timeout, DEFAULT_OUTPUT_LIMIT)
}

/// A command that ran to completion and exited unsuccessfully.
#[derive(Debug)]
pub struct ExitFailure {
    /// The argv, joined with spaces.
    pub command: String,
    /// How it exited.
    pub status: ExitStatus,
    /// Its stderr, trimmed.
    pub stderr: String,
}

impl fmt::Display for ExitFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            command,
            status,
            stderr,
        } = self;
        if stderr.is_empty() {
            write!(f, "driver command failed: {command}: {status}")
        } else {
            write!(f, "driver command failed: {command}: {status}: {stderr}")
        }
    }
}

impl Error for ExitFailure {}

/// A command that ran longer than its timeout and was killed.
#[derive(Debug)]
pub struct TimedOut {
    /// How long it was allowed to run.
    pub timeout: Duration,
}

impl fmt::Display for TimedOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "driver command timed out after {:?}", self.timeout)
    }
}

impl Error for TimedOut {}

fn capture(
    argv: &[String],
    stdin: &str,
    env: &[(&str, &str)],
    dir: Option<&Path>,
    timeout: Duration,
    output_limit: usize,
) -> Result<String> {
    ensure!(!argv.is_empty(), "driver argv is empty");
    let mut expr = command(argv, env);
    if let Some(dir) = dir {
        expr = expr.dir(dir);
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
        return Err(TimedOut { timeout }.into());
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
        return Err(ExitFailure {
            command: argv.join(" "),
            status,
            stderr: String::from_utf8_lossy(&stderr).trim().to_owned(),
        }
        .into());
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
    use std::time::{Duration, Instant};

    use super::{argv, command, filter, filter_limited, run_inherit};

    #[test]
    fn a_failed_command_reports_its_stderr() {
        let error = run_inherit(
            &[
                "sh".into(),
                "-c".into(),
                "echo 'crate x@0.0.1 already exists on crates.io index' >&2; exit 1".into(),
            ],
            Duration::from_secs(5),
        )
        .expect_err("failure");
        let message = format!("{error:#}");
        assert!(
            message.contains("already exists on crates.io index"),
            "{message}"
        );
    }

    #[test]
    fn a_command_writes_its_stdout_to_stderr() {
        let error = run_inherit(
            &[
                "sh".into(),
                "-c".into(),
                "echo 'lint failed: function_missing'; exit 1".into(),
            ],
            Duration::from_secs(5),
        )
        .expect_err("failure");
        let message = format!("{error:#}");
        assert!(message.contains("function_missing"), "{message}");
    }

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
        let start = Instant::now();
        let error = filter_limited(
            &[
                "sh".into(),
                "-c".into(),
                format!("while :; do echo {}; done", "x".repeat(64)),
            ],
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
        assert_eq!(
            format!("{error:#}"),
            "driver command failed: false: exit status: 1"
        );
    }

    #[test]
    fn filter_reports_the_stderr_of_a_failed_command() {
        let error = filter(
            &["sh".into(), "-c".into(), "echo boom >&2; exit 3".into()],
            "",
            &[],
        )
        .expect_err("fail");
        assert_eq!(
            format!("{error:#}"),
            "driver command failed: sh -c echo boom >&2; exit 3: exit status: 3: boom"
        );
    }

    /// What a child prints for the credential variables.
    /// `inherited` is set outermost, as this process's environment is, and
    /// `given` is what verctl hands the child.
    fn seen_by_child(inherited: &[(&str, &str)], given: &[(&str, &str)]) -> String {
        let script = r#"printf '[%s][%s][%s][%s]' "$GITHUB_TOKEN" "$GH_TOKEN" "$VERCTL_REGISTRY_TOKEN" "$VERCTL_OWNER_TOKEN""#;
        let mut expression = command(&argv(&["sh", "-c", script]), given);
        for (key, value) in inherited {
            expression = expression.env(key, value);
        }
        let output = expression.stdout_capture().run().expect("run");
        String::from_utf8(output.stdout).expect("UTF-8")
    }

    #[test]
    fn no_child_inherits_a_github_credential_or_a_registry_token() {
        let inherited = [
            ("GITHUB_TOKEN", "github"),
            ("GH_TOKEN", "gh"),
            ("VERCTL_REGISTRY_TOKEN", "registry"),
            ("VERCTL_OWNER_TOKEN", "owner"),
        ];
        assert_eq!(seen_by_child(&inherited, &[]), "[][][][]");
    }

    /// A publisher whose `--registry-token-env` names `GITHUB_TOKEN`, as a
    /// bunfig for GitHub Packages may read it, gets the registry token there
    /// and nothing else.
    #[test]
    fn a_credential_a_command_is_given_stays_set() {
        let inherited = [("GITHUB_TOKEN", "github"), ("GH_TOKEN", "gh")];
        assert_eq!(
            seen_by_child(&inherited, &[("GITHUB_TOKEN", "given")]),
            "[given][][][]"
        );
    }
}
