use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::collections::HashMap;
use serde::Deserialize;

// TUI deps
use anyhow::Result;
use crossterm::{event, execute, terminal};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::{prelude::*, widgets::*};

#[derive(Default, Debug, Clone)]
struct Options {
    tui: bool,
    config_file: Option<String>,
}

fn print_help() {
    println!(
        "ghop [options] <command1> <command2> ... <commandN>\n\nOptions:\n    -h, --help          Print this help message.\n    -v, --version       Print the version.\n    -t, --tui           Run in TUI mode.\n    -f, --file <FILE>   Load commands from YAML file; then specify the set name to run.\n\nYAML format examples:\n    # Simple map of sets\n    build: [\"cargo build\", \"cargo test\"]\n    lint:  [\"cargo clippy\", \"cargo fmt -- --check\"]\n\n    # Or use a top-level 'sets' key\n    sets:\n      dev: [\"npm run dev\", \"cargo watch -x run\"]\n\nUsage with -f:\n    ghop -f ghop.yml build\n"
    );
}

fn is_option(arg: &str) -> bool {
    arg.starts_with('-')
}

#[derive(Deserialize, Debug)]
struct SetsWrapper {
    sets: HashMap<String, Vec<String>>,
}

fn load_commands_from_yaml(path: &str, set_name: &str) -> Result<Vec<String>, String> {
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

#[derive(Clone, Copy, Debug)]
enum StreamKind { Stdout, Stderr }

#[derive(Debug)]
struct LineMsg { idx: usize, kind: StreamKind, text: String }

struct App {
    logs: Vec<Vec<String>>, // one buffer per command
    selected: usize,
}

impl App {
    fn new(n: usize) -> Self { Self { logs: vec![Vec::new(); n], selected: 0 } }
    fn push(&mut self, msg: LineMsg) {
        let buf = &mut self.logs[msg.idx];
        if buf.len() > 10_000 { buf.drain(..5_000); }
        let prefix = match msg.kind { StreamKind::Stdout => "", StreamKind::Stderr => "[err] " };
        buf.push(format!("{}{}", prefix, msg.text));
    }
}

async fn spawn_reader(
    mut child: tokio::process::Child,
    idx: usize,
    tx: tokio::sync::mpsc::Sender<LineMsg>,
    mut cancel_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<i32> {
    use tokio::io::{AsyncBufReadExt, BufReader as AsyncBufReader};

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut out = AsyncBufReader::new(stdout).lines();
    let mut err = AsyncBufReader::new(stderr).lines();

    let tx_out = tx.clone();
    let t_out = tokio::spawn(async move {
        while let Ok(Some(line)) = out.next_line().await {
            let _ = tx_out.send(LineMsg { idx, kind: StreamKind::Stdout, text: line }).await;
        }
    });

    let tx_err = tx.clone();
    let t_err = tokio::spawn(async move {
        while let Ok(Some(line)) = err.next_line().await {
            let _ = tx_err.send(LineMsg { idx, kind: StreamKind::Stderr, text: line }).await;
        }
    });

    // Wait for process to exit or cancellation request
    let status = tokio::select! {
        res = child.wait() => { res? }
        _ = cancel_rx.changed() => {
            // Attempt to kill the child and wait for it to exit
            let _ = child.kill().await;
            child.wait().await?
        }
    };

    let _ = t_out.await; let _ = t_err.await;
    Ok(status.code().unwrap_or(-1))
}

async fn run_tui(commands: Vec<String>) -> Result<i32> {
    use std::io;
    use std::time::Duration;

    // terminal setup
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen, event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // channels
    let (tx, mut rx) = tokio::sync::mpsc::channel::<LineMsg>(1024);
    // cancellation signal for spawned processes
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel::<bool>(false);

    // spawn processes
    let mut join_handles = Vec::new();
    for (idx, cmd) in commands.iter().enumerate() {
        #[cfg(windows)]
        let child = tokio::process::Command::new("cmd").arg("/C").arg(cmd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        #[cfg(not(windows))]
        let child = tokio::process::Command::new("sh").arg("-c").arg(cmd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        let txc = tx.clone();
        let crx = cancel_rx.clone();
        join_handles.push(tokio::spawn(spawn_reader(child, idx, txc, crx)));
    }
    drop(tx);

    let mut app = App::new(commands.len());

    // main loop
    loop {
        // draw
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Min(1),
                    Constraint::Length(1),
                ]).split(f.area());

            let titles: Vec<Line> = commands.iter().enumerate().map(|(i, c)| {
                let title = format!("{}: {}", i+1, c);
                Line::from(Span::styled(title, Style::default().fg(if i==app.selected { Color::Yellow } else { Color::White })))
            }).collect();
            let tabs = Tabs::new(titles).select(app.selected);
            f.render_widget(tabs, chunks[0]);

            let items: Vec<ListItem> = app.logs[app.selected]
                .iter().rev().take(1000).rev()
                .map(|l| ListItem::new(l.as_str()))
                .collect();
            let list = List::new(items).block(Block::default().title("Output").borders(Borders::ALL));
            f.render_widget(list, chunks[1]);

            let help = Paragraph::new("q=quit  ←/→=pane");
            f.render_widget(help, chunks[2]);
        })?;

        // drain new lines with a short timeout
        let mut drained = 0;
        while let Ok(Some(msg)) = tokio::time::timeout(Duration::from_millis(1), rx.recv()).await {
            app.push(msg);
            drained += 1;
            if drained > 10_000 { break; }
        }

        // input
        if event::poll(Duration::from_millis(10))?
            && let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Char('Q') => break,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Left => { app.selected = app.selected.saturating_sub(1); }
                KeyCode::Right => { app.selected = (app.selected + 1).min(commands.len()-1); }
                _ => {}
            }
        }
    }

    // teardown
    terminal::disable_raw_mode()?;
    execute!(std::io::stdout(), terminal::LeaveAlternateScreen, event::DisableMouseCapture)?;

    // request cancellation/kill of all running processes
    let _ = cancel_tx.send(true);

    // gather exit codes
    let mut worst = 0;
    for h in join_handles { if let Ok(Ok(code)) = h.await && code != 0 { worst = code; } }
    Ok(if worst < 0 { 1 } else { worst })
}

fn main() {
    let mut args = env::args().skip(1).collect::<Vec<_>>();

    // Parse options
    let mut opts = Options::default();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if !is_option(arg) {
            break;
        }
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return;
            }
            "-v" | "--version" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "-t" | "--tui" => {
                opts.tui = true;
                i += 1;
            }
            "-f" | "--file" => {
                if i + 1 >= args.len() {
                    eprintln!("-f/--file requires a file path");
                    std::process::exit(2);
                }
                opts.config_file = Some(args[i + 1].clone());
                i += 2;
            }
            _ => {
                eprintln!("Unknown option: {arg}");
                print_help();
                std::process::exit(2);
            }
        }
    }

    // Determine commands
    let commands: Vec<String>;
    if let Some(cfg_path) = opts.config_file.clone() {
        // The next non-option arg must be the set name
        if i >= args.len() || is_option(&args[i]) {
            eprintln!("When using -f/--file, you must specify the set name to run.");
            std::process::exit(1);
        }
        let set_name = args[i].clone();
        // Any extra trailing args are ignored for now
        commands = match load_commands_from_yaml(&cfg_path, &set_name) {
            Ok(cmds) => cmds,
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        };
    } else {
        // Remaining args are commands
        commands = args.split_off(i);
        if commands.is_empty() {
            eprintln!("No commands provided. Use -h for help.");
            std::process::exit(1);
        }
    }

    if opts.tui {
        // Run async TUI mode
        let rt = tokio::runtime::Builder::new_multi_thread().enable_io().enable_time().build().expect("tokio runtime");
        match rt.block_on(run_tui(commands)) {
            Ok(code) => {
                if code != 0 { std::process::exit(code); }
                return;
            }
            Err(e) => {
                eprintln!("TUI error: {e}");
                std::process::exit(1);
            }
        }
    }

    // Spawn threads for each command (non-TUI default)
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
        std::process::exit(if worst_code < 0 { 1 } else { worst_code });
    }
}
