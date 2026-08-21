use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use wait_timeout::ChildExt;

use crate::model::FixtureSpec;

#[derive(Debug, Clone, Copy)]
pub struct RunLimits {
    pub timeout: Duration,
    pub max_output_bytes: usize,
    pub max_files: usize,
    pub max_file_bytes: usize,
    pub max_total_file_bytes: usize,
}

impl Default for RunLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(10),
            max_output_bytes: 1_048_576,
            max_files: 256,
            max_file_bytes: 4 * 1_048_576,
            max_total_file_bytes: 16 * 1_048_576,
        }
    }
}

pub struct RunRequest<'a> {
    pub program: &'a Path,
    pub args: &'a [String],
    pub stdin: Option<&'a str>,
    pub env: &'a BTreeMap<String, String>,
    pub fixtures: &'a [FixtureSpec],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandObservation {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    #[serde(skip)]
    pub files: BTreeMap<String, Vec<u8>>,
    #[serde(skip)]
    pub workdir: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RunFailureKind {
    Launch,
    Timeout,
    OutputLimit,
    OutputEncoding,
    Workspace,
    FileLimit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunFailure {
    pub kind: RunFailureKind,
    pub message: String,
}

pub fn run(request: RunRequest<'_>, limits: RunLimits) -> Result<CommandObservation, RunFailure> {
    let workdir = create_workspace(request.fixtures)?;
    let result = run_in_workspace(&request, limits, &workdir);
    let cleanup = fs::remove_dir_all(&workdir);

    match (result, cleanup) {
        (Ok(observation), Ok(())) => Ok(observation),
        (Ok(_), Err(error)) => Err(failure(
            RunFailureKind::Workspace,
            format!("could not remove temporary workspace: {error}"),
        )),
        (Err(error), Ok(())) => Err(error),
        (Err(mut error), Err(cleanup_error)) => {
            error.message.push_str(&format!(
                "; could not remove temporary workspace: {cleanup_error}"
            ));
            Err(error)
        }
    }
}

fn run_in_workspace(
    request: &RunRequest<'_>,
    limits: RunLimits,
    workdir: &Path,
) -> Result<CommandObservation, RunFailure> {
    let mut command = Command::new(request.program);
    command
        .args(request.args)
        .current_dir(workdir)
        .env_clear()
        .stdin(if request.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for name in [
        "PATH",
        "SystemRoot",
        "WINDIR",
        "ComSpec",
        "PATHEXT",
        "TEMP",
        "TMP",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command.envs(request.env);

    let mut child = command.spawn().map_err(|error| {
        failure(
            RunFailureKind::Launch,
            format!("could not launch {}: {error}", request.program.display()),
        )
    })?;

    let stdout = child.stdout.take().expect("stdout was configured as piped");
    let stderr = child.stderr.take().expect("stderr was configured as piped");
    let stdout_reader = read_capped(stdout, limits.max_output_bytes);
    let stderr_reader = read_capped(stderr, limits.max_output_bytes);
    let stdin_writer = request.stdin.map(|input| {
        let mut stdin = child.stdin.take().expect("stdin was configured as piped");
        let input = input.as_bytes().to_vec();
        thread::spawn(move || stdin.write_all(&input))
    });

    let status = match child.wait_timeout(limits.timeout).map_err(|error| {
        failure(
            RunFailureKind::Launch,
            format!("could not wait for command: {error}"),
        )
    })? {
        Some(status) => status,
        None => {
            child.kill().map_err(|error| {
                failure(
                    RunFailureKind::Timeout,
                    format!("command timed out and could not be terminated: {error}"),
                )
            })?;
            child.wait().map_err(|error| {
                failure(
                    RunFailureKind::Timeout,
                    format!("command timed out and could not be reaped: {error}"),
                )
            })?;
            join_input(stdin_writer)?;
            join_reader(stdout_reader)?;
            join_reader(stderr_reader)?;
            return Err(failure(
                RunFailureKind::Timeout,
                format!("command exceeded {} ms", limits.timeout.as_millis()),
            ));
        }
    };

    join_input(stdin_writer)?;
    let stdout = join_reader(stdout_reader)?;
    let stderr = join_reader(stderr_reader)?;
    if stdout.exceeded
        || stderr.exceeded
        || stdout.bytes.len().saturating_add(stderr.bytes.len()) > limits.max_output_bytes
    {
        return Err(failure(
            RunFailureKind::OutputLimit,
            format!(
                "combined stdout/stderr exceeded {} bytes",
                limits.max_output_bytes
            ),
        ));
    }

    let stdout = String::from_utf8(stdout.bytes)
        .map_err(|_| failure(RunFailureKind::OutputEncoding, "stdout is not valid UTF-8"))?;
    let stderr = String::from_utf8(stderr.bytes)
        .map_err(|_| failure(RunFailureKind::OutputEncoding, "stderr is not valid UTF-8"))?;
    let files = inventory(workdir, limits)?;

    Ok(CommandObservation {
        exit_code: status.code(),
        stdout,
        stderr,
        files,
        workdir: workdir.to_path_buf(),
    })
}

fn failure(kind: RunFailureKind, message: impl Into<String>) -> RunFailure {
    RunFailure {
        kind,
        message: message.into(),
    }
}

static WORKSPACE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_workspace(fixtures: &[FixtureSpec]) -> Result<PathBuf, RunFailure> {
    let sequence = WORKSPACE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("cmdwitness-{}-{sequence}", std::process::id()));
    fs::create_dir(&path).map_err(|error| {
        failure(
            RunFailureKind::Workspace,
            format!("could not create temporary workspace: {error}"),
        )
    })?;

    for fixture in fixtures {
        let destination = path.join(&fixture.path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                failure(
                    RunFailureKind::Workspace,
                    format!("could not create fixture directory: {error}"),
                )
            })?;
        }
        fs::write(&destination, fixture.content.as_bytes()).map_err(|error| {
            failure(
                RunFailureKind::Workspace,
                format!("could not write fixture {}: {error}", fixture.path),
            )
        })?;
        set_executable(&destination, fixture.executable)?;
    }
    Ok(path)
}

#[cfg(unix)]
fn set_executable(path: &Path, executable: bool) -> Result<(), RunFailure> {
    use std::os::unix::fs::PermissionsExt;

    if executable {
        let mut permissions = fs::metadata(path)
            .map_err(|error| failure(RunFailureKind::Workspace, error.to_string()))?
            .permissions();
        permissions.set_mode(permissions.mode() | 0o100);
        fs::set_permissions(path, permissions)
            .map_err(|error| failure(RunFailureKind::Workspace, error.to_string()))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_path: &Path, _executable: bool) -> Result<(), RunFailure> {
    Ok(())
}

struct CappedOutput {
    bytes: Vec<u8>,
    exceeded: bool,
}

fn read_capped<R>(mut reader: R, limit: usize) -> thread::JoinHandle<io::Result<CappedOutput>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut bytes = Vec::with_capacity(limit.min(65_536));
        let mut exceeded = false;
        let mut chunk = [0u8; 8192];
        loop {
            let count = reader.read(&mut chunk)?;
            if count == 0 {
                break;
            }
            let remaining = limit.saturating_sub(bytes.len());
            bytes.extend_from_slice(&chunk[..count.min(remaining)]);
            if count > remaining {
                exceeded = true;
            }
        }
        Ok(CappedOutput { bytes, exceeded })
    })
}

fn join_reader(
    handle: thread::JoinHandle<io::Result<CappedOutput>>,
) -> Result<CappedOutput, RunFailure> {
    handle
        .join()
        .map_err(|_| failure(RunFailureKind::Launch, "output reader thread panicked"))?
        .map_err(|error| {
            failure(
                RunFailureKind::Launch,
                format!("could not read output: {error}"),
            )
        })
}

fn join_input(handle: Option<thread::JoinHandle<io::Result<()>>>) -> Result<(), RunFailure> {
    let Some(handle) = handle else {
        return Ok(());
    };
    match handle
        .join()
        .map_err(|_| failure(RunFailureKind::Launch, "stdin writer thread panicked"))?
    {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(failure(
            RunFailureKind::Launch,
            format!("could not write stdin: {error}"),
        )),
    }
}

fn inventory(root: &Path, limits: RunLimits) -> Result<BTreeMap<String, Vec<u8>>, RunFailure> {
    let mut files = BTreeMap::new();
    let mut total_bytes = 0usize;
    visit_directory(root, root, limits, &mut total_bytes, &mut files)?;
    Ok(files)
}

fn visit_directory(
    root: &Path,
    directory: &Path,
    limits: RunLimits,
    total_bytes: &mut usize,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), RunFailure> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| failure(RunFailureKind::Workspace, error.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| failure(RunFailureKind::Workspace, error.to_string()))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| failure(RunFailureKind::Workspace, error.to_string()))?;
        if metadata.file_type().is_symlink() {
            return Err(failure(
                RunFailureKind::Workspace,
                format!("observed symbolic link: {}", path.display()),
            ));
        }
        if metadata.is_dir() {
            visit_directory(root, &path, limits, total_bytes, files)?;
            continue;
        }
        if !metadata.is_file() {
            return Err(failure(
                RunFailureKind::Workspace,
                format!("observed unsupported file type: {}", path.display()),
            ));
        }
        if files.len() >= limits.max_files || metadata.len() as usize > limits.max_file_bytes {
            return Err(failure(
                RunFailureKind::FileLimit,
                "observed files exceed configured limits",
            ));
        }
        *total_bytes = total_bytes.saturating_add(metadata.len() as usize);
        if *total_bytes > limits.max_total_file_bytes {
            return Err(failure(
                RunFailureKind::FileLimit,
                "observed file bytes exceed configured limits",
            ));
        }
        let relative = path
            .strip_prefix(root)
            .expect("walked paths remain under the workspace")
            .to_str()
            .ok_or_else(|| {
                failure(
                    RunFailureKind::Workspace,
                    "observed file name is not valid Unicode",
                )
            })?
            .replace('\\', "/");
        let content = fs::read(&path)
            .map_err(|error| failure(RunFailureKind::Workspace, error.to_string()))?;
        files.insert(relative, content);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::process;
    use std::thread;

    use super::*;

    #[test]
    fn helper_process() {
        let Ok(mode) = std::env::var("CMDWITNESS_TEST_HELPER") else {
            return;
        };
        match mode.as_str() {
            "capture" => {
                let mut input = String::new();
                std::io::stdin().read_to_string(&mut input).unwrap();
                print!("stdout:{input}");
                eprint!("stderr:{}", std::env::var("DEMO_VALUE").unwrap());
                std::fs::write("result.txt", "created").unwrap();
                std::io::stdout().flush().unwrap();
                std::io::stderr().flush().unwrap();
                process::exit(7);
            }
            "sleep" => thread::sleep(Duration::from_secs(2)),
            "spam" => {
                print!("{}", "x".repeat(4096));
                std::io::stdout().flush().unwrap();
            }
            _ => process::exit(99),
        }
    }

    fn helper_request<'a>(
        program: &'a Path,
        mode: &str,
        args: &'a [String],
        env: &'a mut BTreeMap<String, String>,
        fixtures: &'a [FixtureSpec],
    ) -> RunRequest<'a> {
        env.insert("CMDWITNESS_TEST_HELPER".into(), mode.into());
        RunRequest {
            program,
            args,
            stdin: Some("hello"),
            env,
            fixtures,
        }
    }

    #[test]
    fn captures_real_exit_output_environment_and_files() {
        let args = vec![
            "--exact".into(),
            "runner::tests::helper_process".into(),
            "--nocapture".into(),
        ];
        let mut env = BTreeMap::from([("DEMO_VALUE".into(), "value".into())]);
        let program = std::env::current_exe().unwrap();
        let fixtures = vec![FixtureSpec {
            path: "input.txt".into(),
            content: "original".into(),
            executable: false,
        }];

        let observation = run(
            helper_request(&program, "capture", &args, &mut env, &fixtures),
            RunLimits::default(),
        )
        .unwrap();

        assert_eq!(observation.exit_code, Some(7));
        assert!(observation.stdout.contains("stdout:hello"));
        assert!(observation.stderr.contains("stderr:value"));
        assert_eq!(observation.files["input.txt"], b"original");
        assert_eq!(observation.files["result.txt"], b"created");
        assert!(observation.workdir.is_absolute());
    }

    #[test]
    fn terminates_a_command_at_the_deadline() {
        let args = vec![
            "--exact".into(),
            "runner::tests::helper_process".into(),
            "--nocapture".into(),
        ];
        let mut env = BTreeMap::new();
        let program = std::env::current_exe().unwrap();
        let error = run(
            helper_request(&program, "sleep", &args, &mut env, &[]),
            RunLimits {
                timeout: Duration::from_millis(50),
                ..RunLimits::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.kind, RunFailureKind::Timeout);
    }

    #[test]
    fn rejects_output_beyond_the_configured_cap() {
        let args = vec![
            "--exact".into(),
            "runner::tests::helper_process".into(),
            "--nocapture".into(),
        ];
        let mut env = BTreeMap::new();
        let program = std::env::current_exe().unwrap();
        let error = run(
            helper_request(&program, "spam", &args, &mut env, &[]),
            RunLimits {
                max_output_bytes: 128,
                ..RunLimits::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.kind, RunFailureKind::OutputLimit);
    }

    #[test]
    fn reports_a_missing_program_as_a_launch_failure() {
        let args = Vec::new();
        let env = BTreeMap::new();
        let request = RunRequest {
            program: Path::new("definitely-not-a-real-cmdwitness-program"),
            args: &args,
            stdin: None,
            env: &env,
            fixtures: &[],
        };
        let error = run(request, RunLimits::default()).unwrap_err();
        assert_eq!(error.kind, RunFailureKind::Launch);
    }
}
