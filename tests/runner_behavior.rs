use predicates::prelude::*;

fn bin() -> assert_cmd::Command { assert_cmd::Command::cargo_bin("ghop").unwrap() }

#[cfg(unix)]
#[test]
fn command_times_out() {
    use std::io::Write;
    let mut tf = tempfile::NamedTempFile::new().unwrap();
    writeln!(tf, "sets:\n  s: [{{ command: 'sleep 2', timeout: 1 }}]").unwrap();

    let mut cmd = bin();
    cmd.args(["--file"]).arg(tf.path()).arg("s");
    cmd.assert()
        .failure()
        .code(predicate::eq(124))
        .stderr(predicate::str::contains("[1][err] command timed out after 1s"));
}

#[cfg(unix)]
#[test]
fn concurrent_labeling_is_per_command() {
    use std::io::Write;
    let mut tf = tempfile::NamedTempFile::new().unwrap();
    // Use echo to ensure newline-delimited output for lines API
    writeln!(tf, "sets:\n  s: ['echo a; sleep 0.1; echo a2', 'echo b; sleep 0.05; echo b2']").unwrap();
    let mut cmd = bin();
    cmd.args(["--file"]).arg(tf.path()).arg("s");
    let out = cmd.assert().success().get_output().stdout.clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("[1] a"), "stdout was: {}", s);
    assert!(s.contains("[1] a2"), "stdout was: {}", s);
    assert!(s.contains("[2] b"), "stdout was: {}", s);
    assert!(s.contains("[2] b2"), "stdout was: {}", s);
}

#[cfg(unix)]
#[test]
fn last_nonzero_exit_code_wins() {
    use std::io::Write;
    let mut tf = tempfile::NamedTempFile::new().unwrap();
    // Command 1 exits 2 after a small delay; Command 2 exits 3 after a slightly longer delay
    writeln!(tf, "sets:\n  s: ['sh -c \"sleep 0.05; exit 2\"', 'sh -c \"sleep 0.10; exit 3\"']").unwrap();
    let mut cmd = bin();
    cmd.args(["--file"]).arg(tf.path()).arg("s");
    cmd.assert().failure().code(predicate::eq(3));
}

#[cfg(unix)]
#[test]
fn concurrent_stderr_is_labeled() {
    use std::io::Write;
    let mut tf = tempfile::NamedTempFile::new().unwrap();
    writeln!(tf, "sets:\n  s: ['sh -c \"echo o1 1>&2\"', 'sh -c \"echo o2 1>&2\"']").unwrap();
    let mut cmd = bin();
    cmd.args(["--file"]).arg(tf.path()).arg("s");
    cmd.assert()
        .success()
        .stderr(predicates::str::contains("[1][err] o1"))
        .stderr(predicates::str::contains("[2][err] o2"));
}

#[cfg(unix)]
#[test]
fn unknown_command_is_nonzero() {
    use std::io::Write;
    let mut tf = tempfile::NamedTempFile::new().unwrap();
    writeln!(tf, "sets:\n  s: ['no_such_executable_12345']").unwrap();
    let mut cmd = bin();
    cmd.args(["--file"]).arg(tf.path()).arg("s");
    cmd.assert().failure(); // exit code platform dependent; just ensure non-zero
}
