use std::path::PathBuf;
use std::time::Duration;

use crate::compare::{self, CommandTarget};
use crate::model::Spec;
use crate::report::{self, Format};
use crate::runner::RunLimits;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliResult {
    pub exit_code: i32,
    pub stdout: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    pub message: String,
}

pub fn run(args: &[String]) -> Result<CliResult, CliError> {
    match args.first().map(String::as_str) {
        None | Some("help" | "--help" | "-h") => Ok(CliResult {
            exit_code: 0,
            stdout: HELP.into(),
        }),
        Some("version" | "--version" | "-V") => Ok(CliResult {
            exit_code: 0,
            stdout: format!("cmdwitness {}\n", env!("CARGO_PKG_VERSION")),
        }),
        Some("schema") if args.len() == 1 => Ok(CliResult {
            exit_code: 0,
            stdout: scenario_schema(),
        }),
        Some("compare") => run_compare(&args[1..]),
        Some(command) => Err(cli_error(format!(
            "unknown command {command:?}; run `cmdwitness help`"
        ))),
    }
}

const HELP: &str = "CmdWitness — compare observable behavior across two CLI versions\n\n\
Usage:\n  cmdwitness compare --spec <file> --baseline <program> --candidate <program> [options]\n  cmdwitness schema\n  cmdwitness version\n  cmdwitness help\n\n\
Compare options:\n  --baseline-arg <arg>       Fixed argument before each scenario argument (repeatable)\n  --candidate-arg <arg>      Fixed argument before each scenario argument (repeatable)\n  --format <name>            text, json, markdown, or sarif (default: text)\n  --output <file>            Write the report to a file instead of stdout\n  --timeout-ms <number>      Per-command deadline, 100..300000 (default: 10000)\n  --max-output-bytes <n>     Combined stdout/stderr cap, 1024..16777216 (default: 1048576)\n\n\
Exit codes:\n  0  no unallowed breaking differences\n  1  breaking differences found\n  2  invalid input, execution failure, or unknown compatibility\n";

struct CompareOptions {
    spec: PathBuf,
    baseline: CommandTarget,
    candidate: CommandTarget,
    format: Format,
    output: Option<PathBuf>,
    timeout_ms: u64,
    max_output_bytes: usize,
}

fn run_compare(args: &[String]) -> Result<CliResult, CliError> {
    if args == ["--help"] || args == ["-h"] {
        return Ok(CliResult {
            exit_code: 0,
            stdout: HELP.into(),
        });
    }
    let mut options = parse_compare_options(args)?;
    let invocation_dir = std::env::current_dir()
        .map_err(|error| cli_error(format!("could not read current directory: {error}")))?;
    for target in [&mut options.baseline, &mut options.candidate] {
        if target.program.is_relative() && target.program.components().count() > 1 {
            target.program = invocation_dir.join(&target.program);
        }
    }
    let input = std::fs::read_to_string(&options.spec).map_err(|error| {
        cli_error(format!(
            "could not read scenario file {}: {error}",
            options.spec.display()
        ))
    })?;
    let spec = Spec::from_json(&input)
        .map_err(|error| cli_error(format!("invalid scenario file: {error}")))?;
    let report = compare::compare(
        &spec,
        &options.baseline,
        &options.candidate,
        RunLimits {
            timeout: Duration::from_millis(options.timeout_ms),
            max_output_bytes: options.max_output_bytes,
            ..RunLimits::default()
        },
    )
    .map_err(|error| {
        cli_error(format!(
            "scenario {} {} {}: {}",
            error.scenario, error.side, error.kind, error.message
        ))
    })?;
    let exit_code = if report.summary.breaking > 0 { 1 } else { 0 };
    let mut rendered = report::render(&report, options.format);
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    if let Some(path) = options.output {
        std::fs::write(&path, rendered).map_err(|error| {
            cli_error(format!(
                "could not write report {}: {error}",
                path.display()
            ))
        })?;
        rendered = String::new();
    }
    Ok(CliResult {
        exit_code,
        stdout: rendered,
    })
}

fn parse_compare_options(args: &[String]) -> Result<CompareOptions, CliError> {
    let mut spec = None;
    let mut baseline = None;
    let mut candidate = None;
    let mut baseline_args = Vec::new();
    let mut candidate_args = Vec::new();
    let mut format = None;
    let mut output = None;
    let mut timeout_ms = None;
    let mut max_output_bytes = None;
    let mut index = 0;

    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(
            option,
            "--spec"
                | "--baseline"
                | "--candidate"
                | "--baseline-arg"
                | "--candidate-arg"
                | "--format"
                | "--output"
                | "--timeout-ms"
                | "--max-output-bytes"
        ) {
            return Err(cli_error(format!("unknown compare option {option:?}")));
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| cli_error(format!("compare option {option} requires a value")))?;
        match option {
            "--spec" => set_once(&mut spec, PathBuf::from(value), option)?,
            "--baseline" => set_once(&mut baseline, PathBuf::from(value), option)?,
            "--candidate" => set_once(&mut candidate, PathBuf::from(value), option)?,
            "--baseline-arg" => baseline_args.push(value.clone()),
            "--candidate-arg" => candidate_args.push(value.clone()),
            "--format" => set_once(
                &mut format,
                Format::parse(value).map_err(cli_error)?,
                option,
            )?,
            "--output" => set_once(&mut output, PathBuf::from(value), option)?,
            "--timeout-ms" => {
                let value = value
                    .parse::<u64>()
                    .map_err(|_| cli_error("timeout-ms must be an integer"))?;
                if !(100..=300_000).contains(&value) {
                    return Err(cli_error("timeout-ms must be between 100 and 300000"));
                }
                set_once(&mut timeout_ms, value, option)?;
            }
            "--max-output-bytes" => {
                let value = value
                    .parse::<usize>()
                    .map_err(|_| cli_error("max-output-bytes must be an integer"))?;
                if !(1024..=16_777_216).contains(&value) {
                    return Err(cli_error(
                        "max-output-bytes must be between 1024 and 16777216",
                    ));
                }
                set_once(&mut max_output_bytes, value, option)?;
            }
            _ => unreachable!("unknown options are rejected before reading their value"),
        }
        index += 2;
    }

    Ok(CompareOptions {
        spec: spec.ok_or_else(|| cli_error("compare requires --spec"))?,
        baseline: CommandTarget {
            program: baseline.ok_or_else(|| cli_error("compare requires --baseline"))?,
            args: baseline_args,
        },
        candidate: CommandTarget {
            program: candidate.ok_or_else(|| cli_error("compare requires --candidate"))?,
            args: candidate_args,
        },
        format: format.unwrap_or(Format::Text),
        output,
        timeout_ms: timeout_ms.unwrap_or(10_000),
        max_output_bytes: max_output_bytes.unwrap_or(1_048_576),
    })
}

fn set_once<T>(slot: &mut Option<T>, value: T, option: &str) -> Result<(), CliError> {
    if slot.is_some() {
        return Err(cli_error(format!(
            "compare option {option} may only be used once"
        )));
    }
    *slot = Some(value);
    Ok(())
}

fn cli_error(message: impl Into<String>) -> CliError {
    CliError {
        message: message.into(),
    }
}

fn scenario_schema() -> String {
    include_str!("../schema/cmdwitness-v1.schema.json").into()
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::process;

    use super::*;

    #[test]
    fn baseline_helper() {
        if std::env::var_os("CMDWITNESS_CLI_HELPER").is_none() {
            return;
        }
        print!(
            "Commands:\n  inspect  inspect data\nOptions:\n  --json  JSON output\n  --old  old mode\n"
        );
        std::io::stdout().flush().unwrap();
        process::exit(0);
    }

    #[test]
    fn candidate_helper() {
        if std::env::var_os("CMDWITNESS_CLI_HELPER").is_none() {
            return;
        }
        print!(
            "Commands:\n  inspect  inspect data\nOptions:\n  --json  JSON output\n  --new  new mode\n"
        );
        std::io::stdout().flush().unwrap();
        process::exit(0);
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn exposes_help_version_and_schema() {
        let help = run(&strings(&["help"])).unwrap();
        assert_eq!(help.exit_code, 0);
        assert!(help.stdout.contains("cmdwitness compare"));

        let version = run(&strings(&["version"])).unwrap();
        assert_eq!(
            version.stdout,
            format!("cmdwitness {}\n", env!("CARGO_PKG_VERSION"))
        );

        let schema = run(&strings(&["schema"])).unwrap();
        let value: serde_json::Value = serde_json::from_str(&schema.stdout).unwrap();
        assert_eq!(value["properties"]["schemaVersion"]["const"], 1);
        assert!(
            value["required"]
                .as_array()
                .unwrap()
                .contains(&"scenarios".into())
        );
    }

    #[test]
    fn rejects_unknown_commands_flags_and_bad_limits() {
        assert!(
            run(&strings(&["unknown"]))
                .unwrap_err()
                .message
                .contains("unknown command")
        );
        assert!(
            run(&strings(&["compare", "--wat"]))
                .unwrap_err()
                .message
                .contains("unknown compare option")
        );
        assert!(
            run(&strings(&[
                "compare",
                "--timeout-ms",
                "20",
                "--spec",
                "x",
                "--baseline",
                "a",
                "--candidate",
                "b"
            ]))
            .unwrap_err()
            .message
            .contains("timeout-ms must be between")
        );
    }

    #[test]
    fn runs_two_real_command_prefixes_and_returns_one_for_breakage() {
        let spec_path =
            std::env::temp_dir().join(format!("cmdwitness-cli-test-{}.json", process::id()));
        std::fs::write(
            &spec_path,
            r#"{
              "schemaVersion": 1,
              "scenarios": [{
                "id": "help-surface",
                "env": {"CMDWITNESS_CLI_HELPER": "1"},
                "observe": ["help"]
              }]
            }"#,
        )
        .unwrap();
        let executable = std::env::current_exe().unwrap();
        let args = vec![
            "compare".into(),
            "--spec".into(),
            spec_path.to_string_lossy().into_owned(),
            "--baseline".into(),
            executable.to_string_lossy().into_owned(),
            "--baseline-arg".into(),
            "--exact".into(),
            "--baseline-arg".into(),
            "cli::tests::baseline_helper".into(),
            "--baseline-arg".into(),
            "--nocapture".into(),
            "--candidate".into(),
            executable.to_string_lossy().into_owned(),
            "--candidate-arg".into(),
            "--exact".into(),
            "--candidate-arg".into(),
            "cli::tests::candidate_helper".into(),
            "--candidate-arg".into(),
            "--nocapture".into(),
            "--format".into(),
            "json".into(),
        ];

        let result = run(&args).unwrap();
        std::fs::remove_file(spec_path).unwrap();
        assert_eq!(result.exit_code, 1);
        let report: serde_json::Value = serde_json::from_str(&result.stdout).unwrap();
        assert_eq!(report["summary"]["breaking"], 1);
        assert_eq!(
            report["scenarios"][0]["findings"][0]["kind"],
            "help.flagAdded"
        );
    }
}
