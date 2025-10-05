use ghop::config::load_commands_from_yaml;

#[test]
fn parses_mixed_command_defs() {
    let mut tf = tempfile::NamedTempFile::new().unwrap();
    use std::io::Write;
    writeln!(tf, r#"sets:
  build:
    - "echo a"
    - {{ command: "echo b", timeout: 2 }}
"#).unwrap();

    let cmds = load_commands_from_yaml(tf.path().to_str().unwrap(), "build").unwrap();
    assert_eq!(cmds.len(), 2);
    assert_eq!(cmds[0].command, "echo a");
    assert_eq!(cmds[0].timeout, None);
    assert_eq!(cmds[1].command, "echo b");
    assert_eq!(cmds[1].timeout, Some(2));
}

#[test]
fn error_lists_available_sets_sorted() {
    let mut tf = tempfile::NamedTempFile::new().unwrap();
    use std::io::Write;
    writeln!(tf, "sets:\n  b: ['echo b']\n  a: ['echo a']").unwrap();

    let err = load_commands_from_yaml(tf.path().to_str().unwrap(), "z").unwrap_err();
    assert!(err.contains("Available sets: a, b"), "err was: {err}");
}

#[test]
fn empty_set_is_error() {
    let mut tf = tempfile::NamedTempFile::new().unwrap();
    use std::io::Write;
    writeln!(tf, "sets:\n  empty: []").unwrap();
    let err = load_commands_from_yaml(tf.path().to_str().unwrap(), "empty").unwrap_err();
    assert!(err.contains("is empty"), "err was: {err}");
}

#[test]
fn malformed_yaml_is_error() {
    let mut tf = tempfile::NamedTempFile::new().unwrap();
    use std::io::Write;
    // broken YAML (missing colon)
    writeln!(tf, "sets\n  s: ['echo ok']").unwrap();
    let err = load_commands_from_yaml(tf.path().to_str().unwrap(), "s").unwrap_err();
    assert!(err.starts_with("Failed to parse YAML"), "err was: {err}");
}

#[test]
fn missing_file_is_error() {
    let err = load_commands_from_yaml("/no/such/file/ghop.yml", "x").unwrap_err();
    assert!(err.starts_with("Failed to read YAML file"), "err was: {err}");
}
