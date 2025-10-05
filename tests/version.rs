use predicates::prelude::*;

fn bin() -> assert_cmd::Command { assert_cmd::Command::cargo_bin("ghop").unwrap() }

#[test]
fn version_prints_semver_and_hash() {
    let mut cmd = bin();
    cmd.arg("--version");
    // Expect: "MAJOR.MINOR.PATCH (hash)" with trailing newline
    cmd.assert()
        .success()
        .stdout(predicate::str::is_match(r"\d+\.\d+\.\d+ \([^)]+\)\n").unwrap());
}
