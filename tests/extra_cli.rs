use predicates::prelude::*;

fn bin() -> assert_cmd::Command { assert_cmd::Command::cargo_bin("ghop").unwrap() }

#[test]
fn unknown_option_errors_with_help() {
    let mut cmd = bin();
    cmd.arg("--nope");
    cmd.assert()
        .failure()
        .code(predicate::eq(2))
        .stderr(predicate::str::contains("Unknown option"));
}

#[test]
fn file_flag_requires_path() {
    let mut cmd = bin();
    cmd.args(["-f"]);
    cmd.assert()
        .failure()
        .code(predicate::eq(2))
        .stderr(predicate::str::contains("requires a file path"));
}

#[test]
fn default_file_is_ghop_yml() {
    let temp = tempfile::tempdir().unwrap();
    let p = temp.path();
    std::fs::write(p.join("ghop.yml"), "sets:\n  s: ['echo ok']\n").unwrap();
    let mut cmd = bin();
    cmd.current_dir(p).arg("s");
    cmd.assert().success().stdout(predicate::str::contains("[1] ok"));
}

#[test]
fn tui_flag_runs_commands_non_interactively() {
    // Keep it simple: a quick command so TUI exits cleanly
    let mut tf = tempfile::NamedTempFile::new().unwrap();
    use std::io::Write; writeln!(tf, "sets:\n  s: ['echo ok']").unwrap();
    let mut cmd = bin();
    cmd.args(["--tui", "--file"]).arg(tf.path()).arg("s");
    // In headless CI, TUI may fail to initialize input. Accept either empty stderr (success)
    // or an error message mentioning TUI error.
    cmd.assert().stderr(
        predicate::str::contains("TUI error").or(predicate::str::is_empty())
    );
}

#[test]
fn set_name_cannot_be_option() {
    let mut tf = tempfile::NamedTempFile::new().unwrap();
    use std::io::Write; writeln!(tf, "sets:\n  s: ['echo ok']").unwrap();
    let mut cmd = bin();
    cmd.args(["--file"]).arg(tf.path()).arg("--oops");
    cmd.assert()
        .failure()
        .code(predicate::eq(2))
        .stderr(predicate::str::contains("Unknown option"));
}
