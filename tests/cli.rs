use std::process::Command;

fn cmdwitness() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cmdwitness"))
}

#[test]
fn help_and_invalid_command_use_documented_exit_codes() {
    let help = cmdwitness().arg("help").output().unwrap();
    assert!(help.status.success());
    assert!(
        String::from_utf8(help.stdout)
            .unwrap()
            .contains("cmdwitness compare")
    );

    let invalid = cmdwitness().arg("not-a-command").output().unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    assert!(
        String::from_utf8(invalid.stderr)
            .unwrap()
            .contains("unknown command")
    );
}

#[test]
fn installed_binary_compares_two_real_command_prefixes() {
    let spec = std::env::temp_dir().join(format!(
        "cmdwitness-integration-{}.json",
        std::process::id()
    ));
    std::fs::write(
        &spec,
        r#"{"schemaVersion":1,"scenarios":[{"id":"self-contract","observe":["exitCode","stdout"]}]}"#,
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_cmdwitness");
    let output = cmdwitness()
        .args([
            "compare",
            "--spec",
            spec.to_str().unwrap(),
            "--baseline",
            binary,
            "--baseline-arg",
            "version",
            "--candidate",
            binary,
            "--candidate-arg",
            "help",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    std::fs::remove_file(spec).unwrap();

    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["summary"]["breaking"], 1);
    assert_eq!(report["scenarios"][0]["status"], "breaking");
}
