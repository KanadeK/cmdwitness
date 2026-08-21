use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::model::{NormalizerSpec, Observation, Scenario, Spec};
use crate::normalize;
use crate::runner::{self, CommandObservation, RunFailureKind, RunLimits, RunRequest};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Breaking,
    Additive,
    Allowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ScenarioStatus {
    Compatible,
    Additive,
    Allowed,
    Breaking,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: String,
    pub severity: Severity,
    pub kind: String,
    pub path: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSummary {
    pub path: String,
    pub bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicObservation {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub files: Vec<FileSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioReport {
    pub id: String,
    pub status: ScenarioStatus,
    pub normalizers_applied: Vec<String>,
    pub findings: Vec<Finding>,
    pub baseline: PublicObservation,
    pub candidate: PublicObservation,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub scenarios: usize,
    pub compatible: usize,
    pub additive: usize,
    pub allowed: usize,
    pub breaking: usize,
    pub findings: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonReport {
    pub schema_version: u32,
    pub tool_version: String,
    pub summary: Summary,
    pub scenarios: Vec<ScenarioReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareError {
    pub scenario: String,
    pub side: String,
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandTarget {
    pub program: PathBuf,
    pub args: Vec<String>,
}

pub fn compare(
    spec: &Spec,
    baseline: &CommandTarget,
    candidate: &CommandTarget,
    limits: RunLimits,
) -> Result<ComparisonReport, CompareError> {
    let mut scenarios = Vec::with_capacity(spec.scenarios.len());
    for scenario in &spec.scenarios {
        let mut baseline_args = baseline.args.clone();
        baseline_args.extend_from_slice(&scenario.args);
        let baseline_observation = runner::run(
            RunRequest {
                program: &baseline.program,
                args: &baseline_args,
                stdin: scenario.stdin.as_deref(),
                env: &scenario.env,
                fixtures: &scenario.fixtures,
            },
            limits,
        )
        .map_err(|error| run_error(scenario, "baseline", error.kind, error.message))?;
        let mut candidate_args = candidate.args.clone();
        candidate_args.extend_from_slice(&scenario.args);
        let candidate_observation = runner::run(
            RunRequest {
                program: &candidate.program,
                args: &candidate_args,
                stdin: scenario.stdin.as_deref(),
                env: &scenario.env,
                fixtures: &scenario.fixtures,
            },
            limits,
        )
        .map_err(|error| run_error(scenario, "candidate", error.kind, error.message))?;
        scenarios.push(compare_observations(
            &spec.normalizers,
            scenario,
            &baseline_observation,
            &candidate_observation,
        )?);
    }

    let mut summary = Summary {
        scenarios: scenarios.len(),
        ..Summary::default()
    };
    for scenario in &scenarios {
        summary.findings += scenario.findings.len();
        match scenario.status {
            ScenarioStatus::Compatible => summary.compatible += 1,
            ScenarioStatus::Additive => summary.additive += 1,
            ScenarioStatus::Allowed => summary.allowed += 1,
            ScenarioStatus::Breaking => summary.breaking += 1,
        }
    }

    Ok(ComparisonReport {
        schema_version: 1,
        tool_version: env!("CARGO_PKG_VERSION").into(),
        summary,
        scenarios,
    })
}

fn compare_observations(
    global_normalizers: &[NormalizerSpec],
    scenario: &Scenario,
    baseline: &CommandObservation,
    candidate: &CommandObservation,
) -> Result<ScenarioReport, CompareError> {
    let mut normalizers = global_normalizers.to_vec();
    normalizers.extend_from_slice(&scenario.normalizers);
    let baseline = normalize_observation(baseline, &normalizers);
    let candidate = normalize_observation(candidate, &normalizers);
    let mut applied = baseline.applied.clone();
    applied.extend(candidate.applied.iter().cloned());
    let mut findings = Vec::new();

    for observation in &scenario.observe {
        match observation {
            Observation::ExitCode if baseline.exit_code != candidate.exit_code => add_finding(
                scenario,
                &mut findings,
                Severity::Breaking,
                "exitCode.changed",
                "$exitCode",
                "exit code changed",
                baseline.exit_code.map(|value| value.to_string()),
                candidate.exit_code.map(|value| value.to_string()),
            ),
            Observation::Stdout if baseline.stdout != candidate.stdout => add_finding(
                scenario,
                &mut findings,
                Severity::Breaking,
                "stdout.changed",
                "$stdout",
                "stdout changed after declared normalization",
                Some(preview(&baseline.stdout)),
                Some(preview(&candidate.stdout)),
            ),
            Observation::Stderr if baseline.stderr != candidate.stderr => add_finding(
                scenario,
                &mut findings,
                Severity::Breaking,
                "stderr.changed",
                "$stderr",
                "stderr changed after declared normalization",
                Some(preview(&baseline.stderr)),
                Some(preview(&candidate.stderr)),
            ),
            Observation::JsonStdout => {
                let baseline_json = parse_json(scenario, "baseline", &baseline.stdout)?;
                let candidate_json = parse_json(scenario, "candidate", &candidate.stdout)?;
                compare_json(
                    scenario,
                    "$",
                    &baseline_json,
                    &candidate_json,
                    &mut findings,
                );
            }
            Observation::Help => compare_help(
                scenario,
                &format!("{}\n{}", baseline.stdout, baseline.stderr),
                &format!("{}\n{}", candidate.stdout, candidate.stderr),
                &mut findings,
            ),
            Observation::Files => {
                compare_files(scenario, &baseline.files, &candidate.files, &mut findings)
            }
            _ => {}
        }
    }

    let status = if findings
        .iter()
        .any(|finding| finding.severity == Severity::Breaking)
    {
        ScenarioStatus::Breaking
    } else if findings
        .iter()
        .any(|finding| finding.severity == Severity::Additive)
    {
        ScenarioStatus::Additive
    } else if !findings.is_empty() {
        ScenarioStatus::Allowed
    } else {
        ScenarioStatus::Compatible
    };

    Ok(ScenarioReport {
        id: scenario.id.clone(),
        status,
        normalizers_applied: applied.into_iter().collect(),
        findings,
        baseline: baseline.public(),
        candidate: candidate.public(),
    })
}

struct NormalizedObservation {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    files: BTreeMap<String, Vec<u8>>,
    applied: BTreeSet<String>,
}

impl NormalizedObservation {
    fn public(&self) -> PublicObservation {
        PublicObservation {
            exit_code: self.exit_code,
            stdout: self.stdout.clone(),
            stderr: self.stderr.clone(),
            files: self
                .files
                .iter()
                .map(|(path, content)| FileSummary {
                    path: path.clone(),
                    bytes: content.len(),
                })
                .collect(),
        }
    }
}

fn normalize_observation(
    observation: &CommandObservation,
    normalizers: &[NormalizerSpec],
) -> NormalizedObservation {
    let mut applied = BTreeSet::new();
    let stdout = normalize_value(
        &observation.stdout,
        &observation.workdir,
        normalizers,
        &mut applied,
    );
    let stderr = normalize_value(
        &observation.stderr,
        &observation.workdir,
        normalizers,
        &mut applied,
    );
    let files = observation
        .files
        .iter()
        .map(|(path, content)| {
            let content = match std::str::from_utf8(content) {
                Ok(text) => normalize_value(text, &observation.workdir, normalizers, &mut applied)
                    .into_bytes(),
                Err(_) => content.clone(),
            };
            (path.clone(), content)
        })
        .collect();

    NormalizedObservation {
        exit_code: observation.exit_code,
        stdout,
        stderr,
        files,
        applied,
    }
}

fn normalize_value(
    value: &str,
    workdir: &Path,
    normalizers: &[NormalizerSpec],
    applied: &mut BTreeSet<String>,
) -> String {
    let workdir = workdir.to_string_lossy();
    let mut normalized = value.replace(workdir.as_ref(), "<WORKDIR>");
    let alternate = if workdir.contains('/') {
        workdir.replace('/', "\\")
    } else {
        workdir.replace('\\', "/")
    };
    normalized = normalized.replace(&alternate, "<WORKDIR>");
    if normalized != value {
        applied.insert("workspace".into());
    }
    let result = normalize::apply(&normalized, normalizers);
    applied.extend(result.applied);
    result.text
}

fn parse_json(
    scenario: &Scenario,
    side: &str,
    value: &str,
) -> Result<serde_json::Value, CompareError> {
    serde_json::from_str(value).map_err(|error| CompareError {
        scenario: scenario.id.clone(),
        side: side.into(),
        kind: "invalidJson".into(),
        message: format!("requested JSON stdout is invalid: {error}"),
    })
}

fn compare_json(
    scenario: &Scenario,
    path: &str,
    baseline: &serde_json::Value,
    candidate: &serde_json::Value,
    findings: &mut Vec<Finding>,
) {
    use serde_json::Value;

    match (baseline, candidate) {
        (Value::Object(baseline), Value::Object(candidate)) => {
            let keys: BTreeSet<_> = baseline.keys().chain(candidate.keys()).collect();
            for key in keys {
                let child_path = json_child_path(path, key);
                match (baseline.get(key), candidate.get(key)) {
                    (None, Some(candidate)) => add_finding(
                        scenario,
                        findings,
                        Severity::Additive,
                        "json.added",
                        &child_path,
                        "JSON value was added",
                        None,
                        Some(json_preview(candidate)),
                    ),
                    (Some(baseline), None) => add_finding(
                        scenario,
                        findings,
                        Severity::Breaking,
                        "json.removed",
                        &child_path,
                        "JSON value was removed",
                        Some(json_preview(baseline)),
                        None,
                    ),
                    (Some(baseline), Some(candidate)) => {
                        compare_json(scenario, &child_path, baseline, candidate, findings)
                    }
                    (None, None) => unreachable!("key came from at least one object"),
                }
            }
        }
        (Value::Array(baseline), Value::Array(candidate)) => {
            for index in 0..baseline.len().max(candidate.len()) {
                let child_path = format!("{path}[{index}]");
                match (baseline.get(index), candidate.get(index)) {
                    (None, Some(candidate)) => add_finding(
                        scenario,
                        findings,
                        Severity::Additive,
                        "json.added",
                        &child_path,
                        "JSON array value was added",
                        None,
                        Some(json_preview(candidate)),
                    ),
                    (Some(baseline), None) => add_finding(
                        scenario,
                        findings,
                        Severity::Breaking,
                        "json.removed",
                        &child_path,
                        "JSON array value was removed",
                        Some(json_preview(baseline)),
                        None,
                    ),
                    (Some(baseline), Some(candidate)) => {
                        compare_json(scenario, &child_path, baseline, candidate, findings)
                    }
                    (None, None) => unreachable!("index is within at least one array"),
                }
            }
        }
        _ if json_type(baseline) != json_type(candidate) => add_finding(
            scenario,
            findings,
            Severity::Breaking,
            "json.typeChanged",
            path,
            "JSON value type changed",
            Some(json_type(baseline).into()),
            Some(json_type(candidate).into()),
        ),
        _ if baseline != candidate => add_finding(
            scenario,
            findings,
            Severity::Breaking,
            "json.valueChanged",
            path,
            "JSON scalar value changed",
            Some(json_preview(baseline)),
            Some(json_preview(candidate)),
        ),
        _ => {}
    }
}

fn json_type(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn json_child_path(parent: &str, key: &str) -> String {
    if key
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        format!("{parent}.{key}")
    } else {
        format!(
            "{parent}[{}]",
            serde_json::to_string(key).expect("serializing a string cannot fail")
        )
    }
}

#[derive(Default)]
struct HelpSurface {
    commands: BTreeSet<String>,
    flags: BTreeSet<String>,
}

fn parse_help_surface(text: &str) -> HelpSurface {
    let mut surface = HelpSurface::default();
    let mut in_commands = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("commands:") {
            in_commands = true;
            continue;
        }
        if trimmed.ends_with(':') {
            in_commands = false;
        }
        if in_commands && !trimmed.is_empty() && !trimmed.starts_with('-') {
            if let Some(command) = trimmed.split_whitespace().next() {
                surface.commands.insert(command.to_owned());
            }
        }
        for token in trimmed.split_whitespace() {
            let token = token.trim_matches(|character: char| ",;[]()".contains(character));
            if token.starts_with('-') && token.len() > 1 {
                surface.flags.insert(
                    token
                        .split('=')
                        .next()
                        .expect("split yields one value")
                        .to_owned(),
                );
            }
        }
    }
    surface
}

fn compare_help(scenario: &Scenario, baseline: &str, candidate: &str, findings: &mut Vec<Finding>) {
    let baseline = parse_help_surface(baseline);
    let candidate = parse_help_surface(candidate);
    for command in candidate.commands.difference(&baseline.commands) {
        add_finding(
            scenario,
            findings,
            Severity::Additive,
            "help.commandAdded",
            command,
            "help command was added",
            None,
            Some(command.clone()),
        );
    }
    for command in baseline.commands.difference(&candidate.commands) {
        add_finding(
            scenario,
            findings,
            Severity::Breaking,
            "help.commandRemoved",
            command,
            "help command was removed",
            Some(command.clone()),
            None,
        );
    }
    for flag in candidate.flags.difference(&baseline.flags) {
        add_finding(
            scenario,
            findings,
            Severity::Additive,
            "help.flagAdded",
            flag,
            "help flag was added",
            None,
            Some(flag.clone()),
        );
    }
    for flag in baseline.flags.difference(&candidate.flags) {
        add_finding(
            scenario,
            findings,
            Severity::Breaking,
            "help.flagRemoved",
            flag,
            "help flag was removed",
            Some(flag.clone()),
            None,
        );
    }
}

fn compare_files(
    scenario: &Scenario,
    baseline: &BTreeMap<String, Vec<u8>>,
    candidate: &BTreeMap<String, Vec<u8>>,
    findings: &mut Vec<Finding>,
) {
    let paths: BTreeSet<_> = baseline.keys().chain(candidate.keys()).collect();
    for path in paths {
        match (baseline.get(path), candidate.get(path)) {
            (None, Some(candidate)) => add_finding(
                scenario,
                findings,
                Severity::Additive,
                "files.added",
                path,
                "file was added",
                None,
                Some(file_preview(candidate)),
            ),
            (Some(baseline), None) => add_finding(
                scenario,
                findings,
                Severity::Breaking,
                "files.removed",
                path,
                "file was removed",
                Some(file_preview(baseline)),
                None,
            ),
            (Some(baseline), Some(candidate)) if baseline != candidate => add_finding(
                scenario,
                findings,
                Severity::Breaking,
                "files.changed",
                path,
                "file content changed",
                Some(file_preview(baseline)),
                Some(file_preview(candidate)),
            ),
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn add_finding(
    scenario: &Scenario,
    findings: &mut Vec<Finding>,
    mut severity: Severity,
    kind: &str,
    path: &str,
    message: &str,
    baseline: Option<String>,
    candidate: Option<String>,
) {
    let id = format!("{}:{kind}:{path}", scenario.id);
    if scenario
        .allowances
        .iter()
        .any(|allowance| allowance_matches(allowance, kind, &id))
    {
        severity = Severity::Allowed;
    }
    findings.push(Finding {
        id,
        severity,
        kind: kind.into(),
        path: path.into(),
        message: message.into(),
        baseline,
        candidate,
    });
}

fn allowance_matches(allowance: &str, kind: &str, id: &str) -> bool {
    allowance == kind
        || allowance == id
        || allowance
            .strip_suffix(".*")
            .is_some_and(|prefix| kind.starts_with(&format!("{prefix}.")))
}

fn preview(value: &str) -> String {
    const LIMIT: usize = 160;
    let mut characters = value.chars();
    let head: String = characters.by_ref().take(LIMIT).collect();
    if characters.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

fn json_preview(value: &serde_json::Value) -> String {
    preview(&serde_json::to_string(value).expect("serializing parsed JSON cannot fail"))
}

fn file_preview(value: &[u8]) -> String {
    match std::str::from_utf8(value) {
        Ok(text) => format!("{} bytes: {}", value.len(), preview(text)),
        Err(_) => format!("{} binary bytes", value.len()),
    }
}

fn run_error(
    scenario: &Scenario,
    side: &str,
    kind: RunFailureKind,
    message: String,
) -> CompareError {
    CompareError {
        scenario: scenario.id.clone(),
        side: side.into(),
        kind: match kind {
            RunFailureKind::Launch => "launch",
            RunFailureKind::Timeout => "timeout",
            RunFailureKind::OutputLimit => "outputLimit",
            RunFailureKind::OutputEncoding => "outputEncoding",
            RunFailureKind::Workspace => "workspace",
            RunFailureKind::FileLimit => "fileLimit",
        }
        .into(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::model::NormalizerSpec;

    use super::*;

    fn scenario(observe: Vec<Observation>) -> Scenario {
        Scenario {
            id: "contract".into(),
            args: vec![],
            stdin: None,
            env: BTreeMap::new(),
            fixtures: vec![],
            observe,
            allowances: vec![],
            normalizers: vec![],
        }
    }

    fn observation(exit_code: i32, stdout: &str) -> CommandObservation {
        CommandObservation {
            exit_code: Some(exit_code),
            stdout: stdout.into(),
            stderr: String::new(),
            files: BTreeMap::new(),
            workdir: PathBuf::from("C:/temp/cmdwitness-test"),
        }
    }

    fn kinds(report: &ScenarioReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|finding| finding.kind.as_str())
            .collect()
    }

    #[test]
    fn classifies_exit_and_text_drift_as_breaking() {
        let mut baseline = observation(0, "ok\n");
        baseline.stderr = "note\n".into();
        let mut candidate = observation(2, "changed\n");
        candidate.stderr = "warning\n".into();
        let report = compare_observations(
            &[],
            &scenario(vec![
                Observation::ExitCode,
                Observation::Stdout,
                Observation::Stderr,
            ]),
            &baseline,
            &candidate,
        )
        .unwrap();
        assert_eq!(report.status, ScenarioStatus::Breaking);
        assert_eq!(
            kinds(&report),
            ["exitCode.changed", "stdout.changed", "stderr.changed"]
        );
    }

    #[test]
    fn compares_json_by_path_and_type() {
        let baseline = observation(
            0,
            r#"{"stable":1,"removed":true,"typed":3,"nested":{"value":"old"}}"#,
        );
        let candidate = observation(
            0,
            r#"{"stable":1,"typed":"3","nested":{"value":"new"},"added":9}"#,
        );
        let report = compare_observations(
            &[],
            &scenario(vec![Observation::JsonStdout]),
            &baseline,
            &candidate,
        )
        .unwrap();
        assert_eq!(report.status, ScenarioStatus::Breaking);
        assert_eq!(
            kinds(&report),
            [
                "json.added",
                "json.valueChanged",
                "json.removed",
                "json.typeChanged"
            ]
        );
        assert_eq!(report.findings[0].severity, Severity::Additive);
    }

    #[test]
    fn reports_invalid_requested_json_as_unknown() {
        let error = compare_observations(
            &[],
            &scenario(vec![Observation::JsonStdout]),
            &observation(0, "not-json"),
            &observation(0, "{}"),
        )
        .unwrap_err();
        assert_eq!(error.kind, "invalidJson");
        assert_eq!(error.side, "baseline");
    }

    #[test]
    fn compares_help_flags_and_commands_semantically() {
        let baseline = observation(
            0,
            "Commands:\n  inspect  inspect data\nOptions:\n  -q, --quiet  quiet\n  --old  old mode\n",
        );
        let candidate = observation(
            0,
            "Commands:\n  check  check data\nOptions:\n  -q, --quiet  quiet\n  --new  new mode\n",
        );
        let report = compare_observations(
            &[],
            &scenario(vec![Observation::Help]),
            &baseline,
            &candidate,
        )
        .unwrap();
        assert_eq!(
            kinds(&report),
            [
                "help.commandAdded",
                "help.commandRemoved",
                "help.flagAdded",
                "help.flagRemoved"
            ]
        );
        assert_eq!(report.status, ScenarioStatus::Breaking);
    }

    #[test]
    fn compares_file_additions_removals_and_content() {
        let mut baseline = observation(0, "");
        baseline.files = BTreeMap::from([
            ("changed.txt".into(), b"old".to_vec()),
            ("removed.txt".into(), b"gone".to_vec()),
        ]);
        let mut candidate = observation(0, "");
        candidate.files = BTreeMap::from([
            ("added.txt".into(), b"new".to_vec()),
            ("changed.txt".into(), b"new".to_vec()),
        ]);
        let report = compare_observations(
            &[],
            &scenario(vec![Observation::Files]),
            &baseline,
            &candidate,
        )
        .unwrap();
        assert_eq!(
            kinds(&report),
            ["files.added", "files.changed", "files.removed"]
        );
        assert_eq!(report.findings[0].severity, Severity::Additive);
    }

    #[test]
    fn allowances_keep_evidence_but_do_not_fail_the_scenario() {
        let mut definition = scenario(vec![Observation::Stdout, Observation::Files]);
        definition.allowances = vec!["stdout.changed".into(), "files.*".into()];
        let baseline = observation(0, "old");
        let mut candidate = observation(0, "new");
        candidate.files.insert("new.txt".into(), b"new".to_vec());
        let report = compare_observations(&[], &definition, &baseline, &candidate).unwrap();
        assert_eq!(report.status, ScenarioStatus::Allowed);
        assert!(
            report
                .findings
                .iter()
                .all(|finding| finding.severity == Severity::Allowed)
        );
    }

    #[test]
    fn normalizes_each_synthetic_workspace_before_comparing() {
        let mut baseline = observation(0, "built in C:/temp/baseline\r\n");
        baseline.workdir = PathBuf::from("C:/temp/baseline");
        let mut candidate = observation(0, "built in C:\\temp\\candidate\n");
        candidate.workdir = PathBuf::from("C:/temp/candidate");
        let report = compare_observations(
            &[NormalizerSpec::LineEndings, NormalizerSpec::Slashes],
            &scenario(vec![Observation::Stdout]),
            &baseline,
            &candidate,
        )
        .unwrap();
        assert_eq!(report.status, ScenarioStatus::Compatible);
        assert!(report.findings.is_empty());
        assert!(report.normalizers_applied.contains(&"workspace".into()));
    }
}
