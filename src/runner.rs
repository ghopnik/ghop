use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::thread;

use crate::config::CommandSpec;

fn run_command(label: String, spec: CommandSpec, print_lock: Arc<Mutex<()>>) -> i32 {
    let cmd = spec.command.clone();

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

    // Shared child for watchdog
    let child_arc = Arc::new(Mutex::new(child));
    let timed_out = Arc::new(AtomicBool::new(false));

    // Watchdog thread if timeout specified
    if let Some(secs) = spec.timeout {
        let child_arc_wd = Arc::clone(&child_arc);
        let timed_out_wd = Arc::clone(&timed_out);
        thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(secs));
            // Check if still running and kill
            let mut ch = child_arc_wd.lock().unwrap();
            if let Ok(None) = ch.try_wait() {
                // Still running
                let _ = ch.kill();
                timed_out_wd.store(true, Ordering::SeqCst);
            }
        });
    }

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

    // Wait for a child using non-blocking polling to allow watchdog to acquire the lock
    let code = loop {
        {
            let mut ch = child_arc.lock().unwrap();
            match ch.try_wait() {
                Ok(Some(status)) => break status.code().unwrap_or(-1),
                Ok(None) => { /* still running */ }
                Err(_) => break -1,
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };

    // Wait for output threads (they should exit when pipes close)
    let _ = t_out.join();
    let _ = t_err.join();

    if timed_out.load(Ordering::SeqCst) {
        // Print a timeout message labeled
        let _g = print_lock.lock().unwrap();
        eprintln!("[{label}][err] command timed out after {}s", spec.timeout.unwrap_or(0));
        return 124; // commonly used timeout exit code
    }

    code
}

pub fn run_commands(commands: Vec<CommandSpec>) -> i32 {
    let print_lock = Arc::new(Mutex::new(()));
    let mut handles = Vec::with_capacity(commands.len());
    for (idx, spec) in commands.into_iter().enumerate() {
        let label = format!("{}", idx + 1);
        let print_lock = Arc::clone(&print_lock);
        handles.push(thread::spawn(move || run_command(label, spec, print_lock)));
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
