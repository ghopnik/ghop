use std::collections::HashMap;
use std::fs;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct SetsWrapper {
    sets: HashMap<String, Vec<String>>,
}

pub fn load_commands_from_yaml(path: &str, set_name: &str) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read YAML file '{path}': {e}"))?;

    // Try as a flat map first
    match serde_yaml::from_str::<HashMap<String, Vec<String>>>(&text) {
        Ok(map) => {
            if let Some(cmds) = map.get(set_name) {
                if cmds.is_empty() {
                    return Err(format!("Set '{set_name}' in '{path}' is empty"));
                }
                return Ok(cmds.clone());
            } else {
                let mut names: Vec<_> = map.keys().cloned().collect();
                names.sort();
                return Err(format!(
                    "Set '{set_name}' not found in '{path}'. Available sets: {}",
                    if names.is_empty() { "<none>".to_string() } else { names.join(", ") }
                ));
            }
        }
        Err(_) => {
            // Try wrapper with 'sets' key
            match serde_yaml::from_str::<SetsWrapper>(&text) {
                Ok(w) => {
                    if let Some(cmds) = w.sets.get(set_name) {
                        if cmds.is_empty() {
                            return Err(format!("Set '{set_name}' in '{path}' is empty"));
                        }
                        return Ok(cmds.clone());
                    } else {
                        let mut names: Vec<_> = w.sets.keys().cloned().collect();
                        names.sort();
                        return Err(format!(
                            "Set '{set_name}' not found in '{path}'. Available sets: {}",
                            if names.is_empty() { "<none>".to_string() } else { names.join(", ") }
                        ));
                    }
                }
                Err(e2) => {
                    return Err(format!("Failed to parse YAML in '{path}': {e2}"));
                }
            }
        }
    }
}
