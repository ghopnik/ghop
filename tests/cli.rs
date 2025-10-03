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
        .stderr(predicate::str::contains("No set specified"));
}

#[cfg(unix)]
#[test]
fn stdout_labeling_unix() {
    // echo produces a trailing newline; expect labeled output on stdout
    use std::io::Write;
    let mut tf = tempfile::NamedTempFile::new().expect("temp file");
    writeln!(tf, "sets:\n  s: ['echo hello']").unwrap();

    let mut cmd = bin();
    cmd.arg("--file").arg(tf.path()).arg("s");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("[1] hello"));
}

#[cfg(windows)]
#[test]
fn stdout_labeling_windows() {
    // On Windows, use built-in echo via cmd; run via YAML set
    use std::io::Write;
    let mut tf = tempfile::NamedTempFile::new().expect("temp file");
    writeln!(tf, "sets:\n  s: ['echo hello']").unwrap();

    let mut cmd = bin();
    cmd.arg("--file").arg(tf.path()).arg("s");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("[1] hello"));
}

#[cfg(unix)]
#[test]
fn stderr_labeling_unix() {
    use std::io::Write;
    let mut tf = tempfile::NamedTempFile::new().expect("temp file");
    writeln!(tf, "sets:\n  s: ['echo oops 1>&2']").unwrap();

    let mut cmd = bin();
    cmd.arg("--file").arg(tf.path()).arg("s");
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("[1][err] oops"));
}

#[cfg(windows)]
#[test]
fn stderr_labeling_windows() {
    use std::io::Write;
    let mut tf = tempfile::NamedTempFile::new().expect("temp file");
    writeln!(tf, "sets:\n  s: ['echo oops 1>&2']").unwrap();

    let mut cmd = bin();
    cmd.arg("--file").arg(tf.path()).arg("s");
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("[1][err] oops"));
}

#[cfg(unix)]
#[test]
fn ansi_passthrough_unix() {
    // Print red text using ANSI escapes; ensure they are preserved in stdout
    use std::io::Write;
    let mut tf = tempfile::NamedTempFile::new().expect("temp file");
    // Use printf with octal escapes to avoid embedding raw control characters in YAML
    writeln!(tf, "sets:\n  s: ['printf \"\\033[31mRED\\033[0m\\n\"']").unwrap();

    let mut cmd = bin();
    cmd.arg("--file").arg(tf.path()).arg("s");
    // We expect the raw escape sequences to be present in the output
    cmd.assert()
        .success()
        .stdout(predicate::str::is_match("\\[1\\] \\x1b\\[31mRED\\x1b\\[0m").unwrap());
}

#[cfg(unix)]
#[test]
fn propagates_nonzero_exit_code() {
    use std::io::Write;
    let mut tf = tempfile::NamedTempFile::new().expect("temp file");
    writeln!(tf, "sets:\n  s: ['exit 3']").unwrap();

    let mut cmd = bin();
    cmd.arg("--file").arg(tf.path()).arg("s");
    cmd.assert()
        .failure()
        .code(predicate::eq(3));
}

// ---- Tests for -f/--file (YAML config) ----

#[test]
fn file_flag_requires_set_name() {
    use std::io::Write;
    let mut tf = tempfile::NamedTempFile::new().expect("temp file");
    // valid YAML but we won't use it in this test
    writeln!(tf, "build: ['echo ok']").unwrap();

    let mut cmd = bin();
    cmd.arg("-f").arg(tf.path());
    cmd.assert()
        .failure()
        .code(predicate::eq(1))
        .stderr(predicate::str::contains("No set specified"));
}


#[test]
fn runs_set_from_wrapper_yaml() {
    use std::io::Write;
    let mut tf = tempfile::NamedTempFile::new().expect("temp file");
    writeln!(tf, "sets:\n  dev: ['echo red', 'echo blue']").unwrap();

    let mut cmd = bin();
    cmd.arg("--file").arg(tf.path()).arg("dev");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("[1] red"))
        .stdout(predicate::str::contains("[2] blue"));
}
