use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

fn run_command(label: String, cmd: String, print_lock: Arc<Mutex<()>>) -> i32 {
    // Determine a shell based on a platform
    #[cfg(windows)]
    let mut child = Command::new("cmd")
        .arg("/C")
        .arg(&cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn process");

    #[cfg(not(windows))]
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn process");

    let stdout = child.stdout.take().expect("failed to capture stdout");
    let stderr = child.stderr.take().expect("failed to capture stderr");

    let print_lock_clone = Arc::clone(&print_lock);
    let label_out = label.clone();
    let t_out = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let line = line.unwrap_or_default();
            let _g = print_lock_clone.lock().unwrap();
            println!("[{label_out}] {line}");
        }
    });

    let print_lock_clone = Arc::clone(&print_lock);
    let label_err = label.clone();
    let t_err = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            let line = line.unwrap_or_default();
            let _g = print_lock_clone.lock().unwrap();
            eprintln!("[{label_err}][err] {line}");
        }
    });

    // Wait for output threads
    let _ = t_out.join();
    let _ = t_err.join();

    let status = child.wait().expect("failed to wait on child");
    status.code().unwrap_or(-1)
}

pub fn run_commands(commands: Vec<String>) -> i32 {
    let print_lock = Arc::new(Mutex::new(()));
    let mut handles = Vec::with_capacity(commands.len());
    for (idx, cmd) in commands.into_iter().enumerate() {
        let label = format!("{}", idx + 1);
        let print_lock = Arc::clone(&print_lock);
        handles.push(thread::spawn(move || run_command(label, cmd, print_lock)));
    }

    // Collect exit codes and compute overall status
    let mut worst_code = 0;
    for h in handles {
        match h.join() {
            Ok(code) => {
                if code != 0 {
                    worst_code = code; // last non-zero code wins
                }
            }
            Err(_) => {
                worst_code = -1;
            }
        }
    }

    if worst_code != 0 {
        return if worst_code < 0 { 1 } else { worst_code };
    }
    0
}
