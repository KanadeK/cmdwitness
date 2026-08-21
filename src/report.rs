use std::collections::BTreeSet;
use std::fmt::Write;

use crate::compare::{ComparisonReport, ScenarioStatus, Severity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Json,
    Markdown,
    Sarif,
}

impl Format {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            "markdown" => Ok(Self::Markdown),
            "sarif" => Ok(Self::Sarif),
            _ => Err(format!(
                "unknown format {value:?}; expected text, json, markdown, or sarif"
            )),
        }
    }
}

pub fn render(report: &ComparisonReport, format: Format) -> String {
    match format {
        Format::Text => render_text(report),
        Format::Json => {
            serde_json::to_string_pretty(report).expect("serializing the typed report cannot fail")
        }
        Format::Markdown => render_markdown(report),
        Format::Sarif => render_sarif(report),
    }
}

fn render_text(report: &ComparisonReport) -> String {
    let decision = if report.summary.breaking > 0 {
        "INCOMPATIBLE"
    } else if report.summary.additive > 0 {
        "COMPATIBLE WITH ADDITIONS"
    } else {
        "COMPATIBLE"
    };
    let mut output = format!(
        "{decision}\n{} breaking scenario{}, {} additive, {} allowed, {} compatible\n",
        report.summary.breaking,
        plural(report.summary.breaking),
        report.summary.additive,
        report.summary.allowed,
        report.summary.compatible
    );
    for scenario in &report.scenarios {
        writeln!(
            output,
            "\n[{}] {}",
            status_label(scenario.status),
            scenario.id
        )
        .expect("writing to a string cannot fail");
        for finding in &scenario.findings {
            writeln!(
                output,
                "- {} {} {}: {}{}",
                severity_label(finding.severity),
                finding.kind,
                finding.path,
                finding.message,
                evidence(finding.baseline.as_deref(), finding.candidate.as_deref())
            )
            .expect("writing to a string cannot fail");
        }
    }
    output
}

fn render_markdown(report: &ComparisonReport) -> String {
    let mut output = format!(
        "# CmdWitness compatibility report\n\n**Decision:** {}\n\n",
        if report.summary.breaking > 0 {
            "incompatible"
        } else if report.summary.additive > 0 {
            "compatible with additions"
        } else {
            "compatible"
        }
    );
    output.push_str("| Scenarios | Breaking | Additive | Allowed | Compatible | Findings |\n");
    output.push_str("| ---: | ---: | ---: | ---: | ---: | ---: |\n");
    writeln!(
        output,
        "| {} | {} | {} | {} | {} | {} |",
        report.summary.scenarios,
        report.summary.breaking,
        report.summary.additive,
        report.summary.allowed,
        report.summary.compatible,
        report.summary.findings
    )
    .expect("writing to a string cannot fail");

    for scenario in &report.scenarios {
        writeln!(
            output,
            "\n## {} — {}\n",
            markdown_cell(&scenario.id),
            status_label(scenario.status)
        )
        .expect("writing to a string cannot fail");
        if scenario.findings.is_empty() {
            output.push_str("No observed differences.\n");
            continue;
        }
        output.push_str("| Severity | Kind | Path | Evidence |\n");
        output.push_str("| --- | --- | --- | --- |\n");
        for finding in &scenario.findings {
            let evidence = format!(
                "{}{}",
                finding.message,
                markdown_evidence(finding.baseline.as_deref(), finding.candidate.as_deref())
            );
            writeln!(
                output,
                "| {} | {} | {} | {} |",
                severity_label(finding.severity),
                markdown_cell(&finding.kind),
                markdown_cell(&finding.path),
                markdown_cell(&evidence)
            )
            .expect("writing to a string cannot fail");
        }
    }
    output
}

fn render_sarif(report: &ComparisonReport) -> String {
    let rule_ids: BTreeSet<_> = report
        .scenarios
        .iter()
        .flat_map(|scenario| {
            scenario
                .findings
                .iter()
                .map(|finding| finding.kind.as_str())
        })
        .collect();
    let rules: Vec<_> = rule_ids
        .into_iter()
        .map(|id| {
            serde_json::json!({
                "id": id,
                "name": id,
                "shortDescription": {"text": format!("CmdWitness {id} compatibility finding")}
            })
        })
        .collect();
    let results: Vec<_> = report
        .scenarios
        .iter()
        .flat_map(|scenario| {
            scenario.findings.iter().map(|finding| {
                serde_json::json!({
                    "ruleId": finding.kind,
                    "level": match finding.severity {
                        Severity::Breaking => "error",
                        Severity::Additive => "warning",
                        Severity::Allowed => "note",
                    },
                    "message": {"text": format!("{}: {}", finding.path, finding.message)},
                    "properties": {
                        "id": finding.id,
                        "scenario": scenario.id,
                        "severity": severity_label(finding.severity).to_ascii_lowercase(),
                        "path": finding.path,
                        "baseline": finding.baseline,
                        "candidate": finding.candidate
                    }
                })
            })
        })
        .collect();
    let sarif = serde_json::json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {"driver": {
                "name": "CmdWitness",
                "version": report.tool_version,
                "informationUri": "https://github.com/KanadeK/cmdwitness",
                "rules": rules
            }},
            "results": results,
            "properties": {"summary": report.summary}
        }]
    });
    serde_json::to_string_pretty(&sarif).expect("serializing SARIF cannot fail")
}

fn status_label(status: ScenarioStatus) -> &'static str {
    match status {
        ScenarioStatus::Compatible => "COMPATIBLE",
        ScenarioStatus::Additive => "ADDITIVE",
        ScenarioStatus::Allowed => "ALLOWED",
        ScenarioStatus::Breaking => "BREAKING",
    }
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Breaking => "BREAKING",
        Severity::Additive => "ADDITIVE",
        Severity::Allowed => "ALLOWED",
    }
}

fn evidence(baseline: Option<&str>, candidate: Option<&str>) -> String {
    match (baseline, candidate) {
        (Some(baseline), Some(candidate)) => format!(" ({baseline:?} -> {candidate:?})"),
        (Some(baseline), None) => format!(" (removed {baseline:?})"),
        (None, Some(candidate)) => format!(" (added {candidate:?})"),
        (None, None) => String::new(),
    }
}

fn markdown_evidence(baseline: Option<&str>, candidate: Option<&str>) -> String {
    match (baseline, candidate) {
        (Some(baseline), Some(candidate)) => format!(" (`{baseline}` -> `{candidate}`)"),
        (Some(baseline), None) => format!(" (removed `{baseline}`)"),
        (None, Some(candidate)) => format!(" (added `{candidate}`)"),
        (None, None) => String::new(),
    }
}

fn markdown_cell(value: &str) -> String {
    value
        .replace('|', "\\|")
        .replace("\r\n", "<br>")
        .replace(['\r', '\n'], "<br>")
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
    use crate::compare::{
        ComparisonReport, Finding, PublicObservation, ScenarioReport, ScenarioStatus, Severity,
        Summary,
    };

    use super::*;

    fn sample_report() -> ComparisonReport {
        ComparisonReport {
            schema_version: 1,
            tool_version: "0.1.0".into(),
            summary: Summary {
                scenarios: 1,
                compatible: 0,
                additive: 0,
                allowed: 0,
                breaking: 1,
                findings: 1,
            },
            scenarios: vec![ScenarioReport {
                id: "machine|output".into(),
                status: ScenarioStatus::Breaking,
                normalizers_applied: vec!["ansi".into()],
                findings: vec![Finding {
                    id: "machine-output:json.typeChanged:$.count".into(),
                    severity: Severity::Breaking,
                    kind: "json.typeChanged".into(),
                    path: "$.count".into(),
                    message: "JSON type changed | automation breaks".into(),
                    baseline: Some("number".into()),
                    candidate: Some("string\nvalue".into()),
                }],
                baseline: PublicObservation {
                    exit_code: Some(0),
                    stdout: "{\"count\":1}".into(),
                    stderr: String::new(),
                    files: vec![],
                },
                candidate: PublicObservation {
                    exit_code: Some(0),
                    stdout: "{\"count\":\"1\"}".into(),
                    stderr: String::new(),
                    files: vec![],
                },
            }],
        }
    }

    #[test]
    fn renders_readable_text_with_the_decision_first() {
        let output = render(&sample_report(), Format::Text);
        assert!(output.starts_with("INCOMPATIBLE"));
        assert!(output.contains("json.typeChanged"));
        assert!(output.contains("1 breaking scenario"));
    }

    #[test]
    fn renders_parseable_canonical_report_json() {
        let output = render(&sample_report(), Format::Json);
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["summary"]["findings"], 1);
        assert_eq!(value["scenarios"][0]["findings"][0]["severity"], "breaking");
    }

    #[test]
    fn escapes_markdown_table_cells() {
        let output = render(&sample_report(), Format::Markdown);
        assert!(output.starts_with("# CmdWitness compatibility report"));
        assert!(output.contains("machine\\|output"));
        assert!(output.contains("JSON type changed \\| automation breaks"));
        assert!(output.contains("string<br>value"));
    }

    #[test]
    fn renders_valid_sarif_with_one_result_per_finding() {
        let output = render(&sample_report(), Format::Sarif);
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(value["version"], "2.1.0");
        assert_eq!(value["runs"][0]["tool"]["driver"]["name"], "CmdWitness");
        assert_eq!(value["runs"][0]["results"].as_array().unwrap().len(), 1);
        assert_eq!(value["runs"][0]["results"][0]["level"], "error");
    }

    #[test]
    fn rejects_unknown_format_names() {
        assert!(
            Format::parse("html")
                .unwrap_err()
                .contains("unknown format")
        );
    }
}
