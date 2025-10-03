use anyhow::Result;
use ratatui::{prelude::*, widgets::*};
use crossterm::{event, execute, terminal};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use tokio::io::{AsyncBufReadExt, BufReader as AsyncBufReader};
use tokio::process::Child;
use tokio::sync::{mpsc, watch};

const DEFAULT_EXIT_CODE: i32 = -1;


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

async fn forward_lines<R>(
    mut lines: tokio::io::Lines<tokio::io::BufReader<R>>,
    idx: usize,
    kind: StreamKind,
    tx: mpsc::Sender<LineMsg>,
) where
    R: tokio::io::AsyncRead + Unpin,
{
    while let Ok(Some(text)) = lines.next_line().await {
        // Ignore send errors (receiver might have been dropped)
        let _ = tx.send(make_line_msg(idx, kind, text)).await;
    }
}

#[inline]
fn make_line_msg(idx: usize, kind: StreamKind, text: String) -> LineMsg {
    LineMsg { idx, kind, text }
}

// Reads child's stdout/stderr lines, forwards them via tx, and returns exit code.
async fn spawn_reader(
    mut child: Child,
    idx: usize,
    tx: mpsc::Sender<LineMsg>,
    mut cancel_rx: watch::Receiver<bool>,
) -> Result<i32> {
    // Gracefully handle missing stdio instead of panicking
    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => return Ok(DEFAULT_EXIT_CODE),
    };
    let stderr = match child.stderr.take() {
        Some(s) => s,
        None => return Ok(DEFAULT_EXIT_CODE),
    };

    let out_reader = AsyncBufReader::new(stdout).lines();
    let err_reader = AsyncBufReader::new(stderr).lines();

    // Spawn independent forwarding tasks
    let stdout_task = {
        let tx_out = tx.clone();
        tokio::spawn(async move {
            forward_lines(out_reader, idx, StreamKind::Stdout, tx_out).await;
        })
    };
    let stderr_task = {
        let tx_err = tx.clone();
        tokio::spawn(async move {
            forward_lines(err_reader, idx, StreamKind::Stderr, tx_err).await;
        })
    };

    // Wait for a process to exit or cancellation
    let status = tokio::select! {
        res = child.wait() => res?,
        _ = cancel_rx.changed() => {
            // Best-effort terminate and wait
            let _ = child.kill().await;
            child.wait().await?
        }
    };

    // Ensure forwarding tasks complete
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    Ok(status.code().unwrap_or(DEFAULT_EXIT_CODE))
}

pub async fn run(commands: Vec<String>) -> Result<i32> {
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

            let help = Paragraph::new("q=quit  ←/→=pane  Tab=next  Shift-Tab=prev");
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
                KeyCode::Tab => { if !commands.is_empty() { app.selected = (app.selected + 1) % commands.len(); } }
                KeyCode::BackTab => { if !commands.is_empty() { app.selected = if app.selected == 0 { commands.len()-1 } else { app.selected - 1 }; } }
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
