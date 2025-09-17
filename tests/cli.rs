use predicates::prelude::*;

fn bin() -> assert_cmd::Command {
    assert_cmd::Command::cargo_bin("ghop").expect("binary built")
}

#[test]
fn help_shows_usage() {
    let mut cmd = bin();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ghop [options]"))
        .stdout(predicate::str::contains("--tui"));
}

#[test]
fn no_commands_errors() {
    let mut cmd = bin();
    cmd.assert()
        .failure()
        .code(predicate::eq(1))
        .stderr(predicate::str::contains("No commands provided"));
}

#[cfg(unix)]
#[test]
fn stdout_labeling_unix() {
    // echo produces a trailing newline; expect labeled output on stdout
    let mut cmd = bin();
    cmd.arg("echo hello");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("[1] hello"));
}

#[cfg(windows)]
#[test]
fn stdout_labeling_windows() {
    // On Windows, use built-in echo via cmd; our app wraps the string in cmd /C
    let mut cmd = bin();
    cmd.arg("echo hello");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("[1] hello"));
}

#[cfg(unix)]
#[test]
fn stderr_labeling_unix() {
    let mut cmd = bin();
    // Redirect to stderr
    cmd.arg("sh -c 'echo oops 1>&2'");
    // But our app itself already wraps with sh -c, so we can simply provide the redirection directly
    let mut cmd = bin();
    cmd.arg("echo oops 1>&2");
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("[1][err] oops"));
}

#[cfg(windows)]
#[test]
fn stderr_labeling_windows() {
    let mut cmd = bin();
    // Redirection works in cmd
    cmd.arg("echo oops 1>&2");
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("[1][err] oops"));
}

#[cfg(unix)]
#[test]
fn ansi_passthrough_unix() {
    // Print red text using ANSI escapes; ensure they are preserved in stdout
    let esc = "\u{001b}"; // ESC
    let arg = format!("printf '{}[31mRED{}[0m\\n'", esc, esc);
    let mut cmd = bin();
    cmd.arg(arg);
    // We expect the raw escape sequences to be present in the output
    cmd.assert()
        .success()
        .stdout(predicate::str::is_match("\\[1\\] \\x1b\\[31mRED\\x1b\\[0m").unwrap());
}

#[cfg(unix)]
#[test]
fn propagates_nonzero_exit_code() {
    let mut cmd = bin();
    cmd.arg("sh -c 'exit 3'");
    // But since our app wraps with sh -c already, we should just pass `exit 3` directly
    let mut cmd = bin();
    cmd.arg("exit 3");
    cmd.assert()
        .failure()
        .code(predicate::eq(3));
}
