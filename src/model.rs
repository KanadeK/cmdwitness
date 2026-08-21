use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Spec {
    pub schema_version: u32,
    #[serde(default)]
    pub normalizers: Vec<NormalizerSpec>,
    pub scenarios: Vec<Scenario>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scenario {
    pub id: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub stdin: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub fixtures: Vec<FixtureSpec>,
    #[serde(default = "default_observations")]
    pub observe: Vec<Observation>,
    #[serde(default, rename = "allow")]
    pub allowances: Vec<String>,
    #[serde(default)]
    pub normalizers: Vec<NormalizerSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixtureSpec {
    pub path: String,
    pub content: String,
    #[serde(default)]
    pub executable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Observation {
    #[serde(rename = "exitCode")]
    ExitCode,
    #[serde(rename = "stdout")]
    Stdout,
    #[serde(rename = "stderr")]
    Stderr,
    #[serde(rename = "jsonStdout")]
    JsonStdout,
    #[serde(rename = "help")]
    Help,
    #[serde(rename = "files")]
    Files,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum NormalizerSpec {
    Ansi,
    #[serde(rename = "lineEndings")]
    LineEndings,
    Slashes,
    Literal {
        name: String,
        from: String,
        to: String,
    },
}

fn default_observations() -> Vec<Observation> {
    vec![
        Observation::ExitCode,
        Observation::Stdout,
        Observation::Stderr,
    ]
}

impl Spec {
    pub fn from_json(input: &str) -> Result<Self, String> {
        if input.len() > 1_048_576 {
            return Err("scenario file exceeds 1 MiB".into());
        }
        let spec: Self = serde_json::from_str(input).map_err(|error| error.to_string())?;
        spec.validate()?;
        Ok(spec)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("schemaVersion must be 1".into());
        }
        if self.scenarios.is_empty() || self.scenarios.len() > 128 {
            return Err("scenarios must contain between 1 and 128 entries".into());
        }
        validate_normalizers(&self.normalizers)?;

        let mut ids = BTreeSet::new();
        for scenario in &self.scenarios {
            validate_scenario(scenario)?;
            if !ids.insert(scenario.id.as_str()) {
                return Err(format!("duplicate scenario id: {}", scenario.id));
            }
        }
        Ok(())
    }
}

fn validate_scenario(scenario: &Scenario) -> Result<(), String> {
    if scenario.id.is_empty()
        || scenario.id.len() > 80
        || !scenario
            .id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        return Err(format!("invalid scenario id: {}", scenario.id));
    }
    if scenario.args.len() > 64 || scenario.args.iter().any(|arg| arg.len() > 4096) {
        return Err(format!(
            "scenario {} has too many or oversized arguments",
            scenario.id
        ));
    }
    if scenario
        .stdin
        .as_ref()
        .is_some_and(|value| value.len() > 1_048_576)
    {
        return Err(format!("scenario {} stdin exceeds 1 MiB", scenario.id));
    }
    for (name, value) in &scenario.env {
        if name.is_empty() || name.contains(['=', '\0']) || value.contains('\0') {
            return Err(format!(
                "invalid environment name or value in scenario {}",
                scenario.id
            ));
        }
    }

    let mut fixture_paths = BTreeSet::new();
    let mut fixture_bytes = 0usize;
    for fixture in &scenario.fixtures {
        if !is_safe_relative_path(&fixture.path) {
            return Err(format!(
                "fixture path must be a safe relative path in scenario {}: {}",
                scenario.id, fixture.path
            ));
        }
        if !fixture_paths.insert(fixture.path.as_str()) {
            return Err(format!(
                "duplicate fixture path in scenario {}: {}",
                scenario.id, fixture.path
            ));
        }
        fixture_bytes = fixture_bytes.saturating_add(fixture.content.len());
    }
    if scenario.fixtures.len() > 128 || fixture_bytes > 8 * 1_048_576 {
        return Err(format!("scenario {} fixtures exceed limits", scenario.id));
    }

    if scenario.observe.is_empty() {
        return Err(format!(
            "scenario {} must have at least one observation",
            scenario.id
        ));
    }
    let mut observations = BTreeSet::new();
    for observation in &scenario.observe {
        if !observations.insert(*observation) {
            return Err(format!("duplicate observation in scenario {}", scenario.id));
        }
    }
    for allowance in &scenario.allowances {
        if allowance.is_empty() || allowance.len() > 128 {
            return Err(format!("invalid allowance in scenario {}", scenario.id));
        }
    }
    validate_normalizers(&scenario.normalizers)
}

fn validate_normalizers(normalizers: &[NormalizerSpec]) -> Result<(), String> {
    if normalizers.len() > 32 {
        return Err("normalizers may contain at most 32 entries".into());
    }
    let mut names = BTreeSet::new();
    for normalizer in normalizers {
        let name = match normalizer {
            NormalizerSpec::Ansi => "ansi",
            NormalizerSpec::LineEndings => "lineEndings",
            NormalizerSpec::Slashes => "slashes",
            NormalizerSpec::Literal { name, from, to } => {
                if name.is_empty() || name.len() > 80 || from.is_empty() {
                    return Err("literal normalizer needs a non-empty name and from value".into());
                }
                if to.len() > from.len() {
                    return Err("literal normalizer replacement may not expand text".into());
                }
                name
            }
        };
        if !names.insert(name) {
            return Err(format!("duplicate normalizer name: {name}"));
        }
    }
    Ok(())
}

pub fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains(['\\', ':', '\0'])
        && path.chars().all(|character| !character.is_control())
        && path
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"{
      "schemaVersion": 1,
      "normalizers": [{"kind": "ansi"}],
      "scenarios": [{
        "id": "machine-output",
        "args": ["inspect", "--json"],
        "env": {"LANG": "C"},
        "fixtures": [{"path": "input/data.txt", "content": "hello"}],
        "observe": ["exitCode", "jsonStdout", "files"],
        "allow": ["json.added"],
        "normalizers": [{"kind": "literal", "name": "root", "from": "C:/tmp", "to": "<TMP>"}]
      }]
    }"#;

    #[test]
    fn parses_and_validates_a_complete_spec() {
        let spec = Spec::from_json(VALID).unwrap();
        assert_eq!(spec.schema_version, 1);
        assert_eq!(spec.scenarios[0].id, "machine-output");
        assert_eq!(spec.scenarios[0].observe.len(), 3);
    }

    #[test]
    fn applies_default_observations() {
        let spec = Spec::from_json(r#"{"schemaVersion":1,"scenarios":[{"id":"smoke"}]}"#).unwrap();
        assert_eq!(
            spec.scenarios[0].observe,
            vec![
                Observation::ExitCode,
                Observation::Stdout,
                Observation::Stderr
            ]
        );
    }

    #[test]
    fn rejects_unknown_fields() {
        let error = Spec::from_json(
            r#"{"schemaVersion":1,"scenarios":[{"id":"smoke","shell":"echo bad"}]}"#,
        )
        .unwrap_err();
        assert!(error.contains("unknown field"), "{error}");
    }

    #[test]
    fn rejects_unsupported_schema_and_duplicate_scenario_ids() {
        let unsupported =
            Spec::from_json(r#"{"schemaVersion":2,"scenarios":[{"id":"smoke"}]}"#).unwrap_err();
        assert!(
            unsupported.contains("schemaVersion must be 1"),
            "{unsupported}"
        );

        let duplicate =
            Spec::from_json(r#"{"schemaVersion":1,"scenarios":[{"id":"same"},{"id":"same"}]}"#)
                .unwrap_err();
        assert!(duplicate.contains("duplicate scenario id"), "{duplicate}");
    }

    #[test]
    fn rejects_unsafe_and_duplicate_fixture_paths() {
        for path in [
            "../secret",
            "/absolute",
            "C:/windows",
            "a\\b",
            "a//b",
            "./a",
        ] {
            let input = format!(
                r#"{{"schemaVersion":1,"scenarios":[{{"id":"bad","fixtures":[{{"path":"{path}","content":"x"}}]}}]}}"#
            );
            let error = Spec::from_json(&input).unwrap_err();
            assert!(error.contains("safe relative path"), "path={path}: {error}");
        }

        let duplicate = Spec::from_json(
            r#"{"schemaVersion":1,"scenarios":[{"id":"bad","fixtures":[{"path":"a.txt","content":"x"},{"path":"a.txt","content":"y"}]}]}"#,
        )
        .unwrap_err();
        assert!(duplicate.contains("duplicate fixture path"), "{duplicate}");
    }

    #[test]
    fn rejects_invalid_environment_and_normalizers() {
        let bad_env =
            Spec::from_json(r#"{"schemaVersion":1,"scenarios":[{"id":"bad","env":{"A=B":"x"}}]}"#)
                .unwrap_err();
        assert!(bad_env.contains("environment name"), "{bad_env}");

        let bad_literal = Spec::from_json(
            r#"{"schemaVersion":1,"normalizers":[{"kind":"literal","name":"empty","from":"","to":"x"}],"scenarios":[{"id":"bad"}]}"#,
        )
        .unwrap_err();
        assert!(bad_literal.contains("literal normalizer"), "{bad_literal}");

        let expanding = Spec::from_json(
            r#"{"schemaVersion":1,"normalizers":[{"kind":"literal","name":"grow","from":"x","to":"xx"}],"scenarios":[{"id":"bad"}]}"#,
        )
        .unwrap_err();
        assert!(expanding.contains("may not expand"), "{expanding}");

        let normalizers = (0..33)
            .map(|index| {
                serde_json::json!({"kind": "literal", "name": format!("n{index}"), "from": "x", "to": "x"})
            })
            .collect::<Vec<_>>();
        let too_many = Spec::from_json(
            &serde_json::json!({
                "schemaVersion": 1,
                "normalizers": normalizers,
                "scenarios": [{"id": "bad"}]
            })
            .to_string(),
        )
        .unwrap_err();
        assert!(too_many.contains("at most 32"), "{too_many}");
    }

    #[test]
    fn rejects_empty_or_duplicate_observations() {
        let empty =
            Spec::from_json(r#"{"schemaVersion":1,"scenarios":[{"id":"bad","observe":[]}]}"#)
                .unwrap_err();
        assert!(empty.contains("at least one observation"), "{empty}");

        let duplicate = Spec::from_json(
            r#"{"schemaVersion":1,"scenarios":[{"id":"bad","observe":["stdout","stdout"]}]}"#,
        )
        .unwrap_err();
        assert!(duplicate.contains("duplicate observation"), "{duplicate}");
    }
}
