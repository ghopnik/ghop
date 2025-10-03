use std::collections::HashMap;
use std::fs;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
enum CommandDef {
    Simple(String),
    Detailed { command: String, timeout: Option<u64> },
}

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub command: String,
    pub timeout: Option<u64>,
}

#[derive(Deserialize, Debug)]
struct SetsWrapper {
    sets: HashMap<String, Vec<CommandDef>>,
}

pub fn load_commands_from_yaml(path: &str, set_name: &str) -> Result<Vec<CommandSpec>, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read YAML file '{path}': {e}"))?;

    // Only support wrapper with 'sets' key
    match serde_yaml::from_str::<SetsWrapper>(&text) {
        Ok(w) => {
            if let Some(cmds) = w.sets.get(set_name) {
                if cmds.is_empty() {
                    return Err(format!("Set '{set_name}' in '{path}' is empty"));
                }
                let specs = cmds.iter().map(|d| match d {
                    CommandDef::Simple(s) => CommandSpec { command: s.clone(), timeout: None },
                    CommandDef::Detailed { command, timeout } => CommandSpec { command: command.clone(), timeout: *timeout },
                }).collect();
                Ok(specs)
            } else {
                let mut names: Vec<_> = w.sets.keys().cloned().collect();
                names.sort();
                Err(format!(
                    "Set '{set_name}' not found in '{path}'. Available sets: {}",
                    if names.is_empty() { "<none>".to_string() } else { names.join(", ") }
                ))
            }
        }
        Err(e2) => {
            Err(format!("Failed to parse YAML in '{path}': {e2}"))
        }
    }
}
